use std::collections::{BTreeMap, VecDeque};

use crate::{
    control::{Request, Response},
    logs::{Batch, Record},
    manager::ServiceStatus,
};

const MAX_LOG_BYTES: usize = 2 * 1024 * 1024;
const MAX_LOG_RECORDS: usize = 4096;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Focus {
    #[default]
    Projects,
    Logs,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    Start(String),
    Stop(String),
    StartAll,
    StopAll,
    Shutdown,
}

impl Operation {
    pub fn request(&self) -> Request {
        match self {
            Self::Start(id) => Request::Start {
                targets: vec![id.clone()],
                all: false,
            },
            Self::Stop(id) => Request::Stop {
                targets: vec![id.clone()],
                all: false,
            },
            Self::StartAll => Request::Start {
                targets: Vec::new(),
                all: true,
            },
            Self::StopAll => Request::Stop {
                targets: Vec::new(),
                all: true,
            },
            Self::Shutdown => Request::Shutdown {},
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Start(id) => format!("Start {id}"),
            Self::Stop(id) => format!("Stop {id}"),
            Self::StartAll => "Start default services".into(),
            Self::StopAll => "Stop all services".into(),
            Self::Shutdown => "End workspace".into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LineSelection {
    pub anchor: u64,
    pub head: u64,
}

impl LineSelection {
    pub fn contains(self, sequence: u64) -> bool {
        (self.anchor.min(self.head)..=self.anchor.max(self.head)).contains(&sequence)
    }
}

#[derive(Default)]
pub struct LogWindow {
    pub records: VecDeque<Record>,
    bytes: usize,
    pub cursor: Option<u64>,
    pub top: Option<u64>,
    pub reading: Option<u64>,
    pub horizontal: u16,
    pub dropped: u64,
    pub incomplete: bool,
    pub gap: bool,
    pub local_evicted: u64,
    pub anchor_lost: bool,
    pub selection: Option<LineSelection>,
    pub selection_lost: bool,
    pub paused: bool,
    pub latest: u64,
    pub oldest_retained: Option<u64>,
    pub pause_at: u64,
    pub requested: Option<LogQuery>,
    pub storage: Option<crate::archive::StorageInfo>,
    pub search: Option<Search>,
}

pub struct Search {
    pub text: String,
    pub upper: u64,
    pub reverse: bool,
    pub running: bool,
    pub matched: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogQuery {
    Tail,
    After(u64),
    Before(u64),
    Restore(u64),
    Bounds,
    Search(u64),
    Context(u64),
    ContextAfter(u64),
}

impl LogQuery {
    pub fn request(self, service: String, search: Option<&Search>) -> Request {
        if let Self::Search(cursor) = self {
            let search = search.expect("Search request has a search state");
            return Request::ReadLogs {
                query: crate::archive::Query {
                    service,
                    limit: 1,
                    after: (!search.reverse).then_some(cursor),
                    before: search.reverse.then_some(cursor),
                    session: None,
                    contains: Some(search.text.clone()),
                    upper: Some(search.upper),
                },
            };
        }
        let (after, before) = match self {
            Self::Tail => (None, None),
            Self::Context(target) => (None, Some(target.saturating_add(1))),
            Self::ContextAfter(target) => (Some(target), None),
            Self::Search(_) => unreachable!(),
            Self::After(sequence) => (Some(sequence), None),
            Self::Before(sequence) => (None, Some(sequence)),
            Self::Restore(sequence) => (Some(sequence.saturating_sub(1)), None),
            // Poll metadata without transferring or accumulating unread output.
            Self::Bounds => (Some(u64::MAX), None),
        };
        Request::Logs {
            service,
            limit: 100,
            after,
            before,
        }
    }
}

#[derive(Clone, Copy)]
struct Bookmark {
    top: u64,
    reading: Option<u64>,
    horizontal: u16,
    pause_at: u64,
}

impl LogWindow {
    pub fn query(&self) -> LogQuery {
        self.requested.unwrap_or(match self.cursor {
            None => LogQuery::Tail,
            Some(_) if self.paused => LogQuery::Bounds,
            Some(cursor) => LogQuery::After(cursor),
        })
    }

    pub fn pause(&mut self, height: usize) {
        if !self.paused {
            self.pause_at = self.latest.max(self.cursor.unwrap_or(0));
            self.paused = true;
            if self.cursor.is_some() && self.requested == Some(LogQuery::Tail) {
                self.requested = None;
            }
        }
        if self.top.is_none() {
            self.top = self
                .records
                .get(self.start(height))
                .map(|record| record.sequence);
        }
    }

    pub fn new_records(&self) -> u64 {
        self.latest.saturating_sub(self.pause_at)
    }

    fn request_edge(&mut self, earlier: bool) {
        if earlier {
            if let Some(first) = self.records.front()
                && self
                    .oldest_retained
                    .is_some_and(|oldest| oldest < first.sequence)
            {
                self.requested = Some(LogQuery::Before(first.sequence));
            }
        } else if let Some(last) = self.records.back()
            && self.oldest_retained.is_some()
            && last.sequence < self.latest
        {
            self.requested = Some(LogQuery::After(last.sequence));
        }
    }

    fn metadata(&mut self, batch: &Batch) -> bool {
        let changed = self.storage != batch.storage
            || self.latest != batch.latest
            || self.oldest_retained != batch.oldest
            || self.dropped != batch.dropped
            || self.incomplete != batch.incomplete
            || (!self.gap && batch.gap);
        self.storage = batch.storage.clone();
        self.latest = batch.latest;
        self.oldest_retained = batch.oldest;
        self.dropped = batch.dropped;
        self.incomplete = batch.incomplete;
        self.gap |= batch.gap;
        changed
    }

    pub fn apply_page(&mut self, query: LogQuery, batch: Batch) -> bool {
        if self.requested == Some(query) {
            self.requested = None;
        }
        match query {
            LogQuery::Bounds => self.metadata(&batch),
            LogQuery::Before(_) => {
                self.metadata(&batch);
                let first = self.records.front().map(|record| record.sequence);
                for record in batch.records.into_iter().rev() {
                    if first.is_some_and(|first| record.sequence >= first) {
                        continue;
                    }
                    self.bytes += record.text.len();
                    self.records.push_front(record);
                }
                self.trim(true);
                // The tail may have been removed to make room for older records.
                self.cursor = self.records.back().map(|record| record.sequence);
                true
            }
            LogQuery::Tail | LogQuery::Restore(_) | LogQuery::Context(_) => {
                let (top, reading) = (self.top, self.reading);
                // A saved cursor may lie beyond this byte-limited page. Its absence
                // from the first page is not evidence that the backend evicted it.
                self.reading = None;
                self.top = match query {
                    LogQuery::Restore(target) => Some(target),
                    _ => None,
                };
                self.records.clear();
                self.bytes = 0;
                self.cursor = None;
                self.append(batch);
                if let LogQuery::Restore(target) = query {
                    self.anchor_lost |= self.records.front().is_none_or(|r| r.sequence != target);
                    self.top = self.records.front().map(|r| r.sequence);
                    self.reading = reading
                        .filter(|sequence| self.records.iter().any(|r| r.sequence == *sequence));
                } else if let LogQuery::Context(target) = query {
                    self.reading = self
                        .records
                        .iter()
                        .find(|r| r.sequence == target)
                        .map(|r| r.sequence);
                    if self.reading.is_some() && target < self.latest {
                        self.requested = Some(LogQuery::ContextAfter(target));
                    }
                    self.top = self
                        .records
                        .iter()
                        .rev()
                        .take(6)
                        .next_back()
                        .map(|r| r.sequence);
                } else if self.paused {
                    self.top = top.or_else(|| self.records.front().map(|r| r.sequence));
                }
                true
            }
            LogQuery::After(_) | LogQuery::ContextAfter(_) => self.append(batch),
            LogQuery::Search(_) => false,
        }
    }

    pub fn append(&mut self, batch: Batch) -> bool {
        let mut changed = self.metadata(&batch);
        for record in batch.records {
            if self.cursor.is_some_and(|cursor| record.sequence <= cursor) {
                continue;
            }
            // The highest available sequence may include pages we have not received.
            self.cursor = Some(record.sequence);
            self.bytes += record.text.len();
            self.records.push_back(record);
            changed = true;
        }
        self.trim(false);
        changed
    }

    fn trim(&mut self, from_back: bool) {
        while self.bytes > MAX_LOG_BYTES || self.records.len() > MAX_LOG_RECORDS {
            let record = if from_back {
                self.records.pop_back()
            } else {
                self.records.pop_front()
            };
            if let Some(record) = record {
                self.bytes -= record.text.len();
                self.local_evicted = self.local_evicted.saturating_add(1);
            }
        }
        if let Some(top) = self.top
            && !self.records.iter().any(|record| record.sequence == top)
        {
            self.top = self.records.front().map(|record| record.sequence);
            self.anchor_lost = true;
        }
        if self.reading.is_some_and(|sequence| {
            !self
                .records
                .iter()
                .any(|record| record.sequence == sequence)
        }) {
            self.reading = None;
            self.anchor_lost = true;
        }
        if self.selection.is_some_and(|selection| {
            !self
                .records
                .iter()
                .any(|record| record.sequence == selection.anchor)
                || !self
                    .records
                    .iter()
                    .any(|record| record.sequence == selection.head)
        }) {
            self.selection = None;
            self.selection_lost = true;
        }
    }

    pub fn begin_selection(&mut self, height: usize) -> bool {
        let start = self.start(height);
        let Some(record) = self
            .reading
            .and_then(|sequence| {
                self.records
                    .iter()
                    .find(|record| record.sequence == sequence)
            })
            .or_else(|| self.records.get(start))
        else {
            return false;
        };
        let sequence = record.sequence;
        self.pause(height);
        self.top = Some(self.records[start].sequence);
        self.reading = Some(sequence);
        self.selection = Some(LineSelection {
            anchor: sequence,
            head: sequence,
        });
        self.selection_lost = false;
        true
    }

    pub fn move_selection(&mut self, amount: isize, height: usize) {
        let Some(selection) = self.selection.as_mut() else {
            return;
        };
        let Some(current) = self
            .records
            .iter()
            .position(|record| record.sequence == selection.head)
        else {
            return;
        };
        let next = current
            .saturating_add_signed(amount)
            .min(self.records.len().saturating_sub(1));
        selection.head = self.records[next].sequence;
        self.reading = Some(selection.head);
        self.keep_visible(height);
        if next == 0 && amount < 0 {
            self.request_edge(true);
        }
        if next + 1 == self.records.len() && amount > 0 {
            self.request_edge(false);
        }
    }

    pub fn move_reading(&mut self, amount: isize, height: usize) {
        if self.records.is_empty() {
            return;
        }
        self.pause(height);
        let start = self.start(height);
        let current = self
            .reading
            .and_then(|sequence| {
                self.records
                    .iter()
                    .position(|record| record.sequence == sequence)
            })
            .unwrap_or(start);
        let next = current
            .saturating_add_signed(amount)
            .min(self.records.len() - 1);
        self.reading = Some(self.records[next].sequence);
        self.top = Some(self.records[start].sequence);
        self.keep_visible(height);
        if next == 0 && amount < 0 {
            self.request_edge(true);
        }
        if next + 1 == self.records.len() && amount > 0 {
            self.request_edge(false);
        }
    }

    pub fn keep_visible(&mut self, height: usize) {
        let Some(sequence) = self
            .selection
            .map(|selection| selection.head)
            .or(self.reading)
        else {
            return;
        };
        let Some(next) = self
            .records
            .iter()
            .position(|record| record.sequence == sequence)
        else {
            return;
        };
        let start = self.start(height);
        if next < start {
            self.top = Some(self.records[next].sequence);
        } else if next >= start.saturating_add(height) {
            self.top = Some(self.records[next.saturating_sub(height.saturating_sub(1))].sequence);
        }
    }

    pub fn selected_text(&self) -> Result<(String, usize), &'static str> {
        let selection = self.selection.ok_or("No log lines selected.")?;
        if !self
            .records
            .iter()
            .any(|record| record.sequence == selection.anchor)
            || !self
                .records
                .iter()
                .any(|record| record.sequence == selection.head)
        {
            return Err("Selected logs were evicted; select again.");
        }
        let mut text = String::new();
        let mut count = 0;
        for record in self
            .records
            .iter()
            .filter(|record| selection.contains(record.sequence))
        {
            let marker = if record.truncated { " [truncated]" } else { "" };
            if text.len() + record.text.len() + marker.len() + 1 > 64 * 1024 {
                return Err("Selection exceeds 64 KiB; select fewer log lines.");
            }
            text.push_str(&record.text);
            text.push_str(marker);
            text.push('\n');
            count += 1;
        }
        Ok((text, count))
    }

    pub fn start(&self, height: usize) -> usize {
        self.top
            .and_then(|top| {
                self.records
                    .iter()
                    .position(|record| record.sequence == top)
            })
            .unwrap_or_else(|| self.records.len().saturating_sub(height))
    }

    pub fn scroll(&mut self, amount: isize, height: usize) {
        self.pause(height);
        self.reading = None;
        let last = self.records.len().saturating_sub(height);
        let start = self.start(height).saturating_add_signed(amount).min(last);
        self.top = self.records.get(start).map(|record| record.sequence);
        if start == 0 && amount < 0 {
            self.request_edge(true);
        }
        if start == last && amount > 0 {
            self.request_edge(false);
            if self.requested.is_none() {
                self.follow();
            }
        }
    }

    pub fn oldest(&mut self) {
        self.pause(1);
        self.reading = None;
        self.top = self.records.front().map(|record| record.sequence);
        if let Some(oldest) = self.oldest_retained
            && self.top.is_none_or(|top| oldest < top)
        {
            self.requested = Some(LogQuery::Restore(oldest));
        }
    }

    pub fn start_search(&mut self, text: String, reverse: bool, fresh: bool, height: usize) {
        self.pause(height);
        let upper = if fresh {
            self.latest
        } else {
            self.search.as_ref().map_or(self.latest, |s| s.upper)
        };
        let cursor = if reverse {
            self.reading.unwrap_or(upper.saturating_add(1))
        } else {
            self.reading.unwrap_or(0)
        };
        self.selection = None;
        self.search = Some(Search {
            text,
            upper,
            reverse,
            running: true,
            matched: None,
        });
        self.requested = Some(LogQuery::Search(cursor));
    }

    pub fn cancel_search(&mut self) {
        if let Some(search) = &mut self.search {
            search.running = false;
        }
        if matches!(
            self.requested,
            Some(LogQuery::Search(_) | LogQuery::Context(_) | LogQuery::ContextAfter(_))
        ) {
            self.requested = None;
        }
    }

    pub fn clear_search(&mut self) {
        self.cancel_search();
        self.search = None;
    }

    pub fn reveal_match(&mut self, height: usize) {
        let Some(target) = self.search.as_ref().and_then(|s| s.matched) else {
            return;
        };
        if let Some(index) = self.records.iter().position(|r| r.sequence == target) {
            let start = index
                .saturating_sub(height / 2)
                .min(self.records.len().saturating_sub(height));
            self.top = self.records.get(start).map(|r| r.sequence);
            self.reading = Some(target);
            if self.requested.is_none()
                && (self.records.len() < height
                    || self.records.len().saturating_sub(index + 1) < height / 2)
                && self.records.len() + 100 <= MAX_LOG_RECORDS
                && self.bytes + 64 * 1024 <= MAX_LOG_BYTES
                && let Some(last) = self.records.back()
                && last.sequence < self.latest
            {
                self.requested = Some(LogQuery::ContextAfter(last.sequence));
            }
        }
    }

    pub fn follow(&mut self) {
        self.cancel_search();
        self.selection = None;
        self.reading = None;
        self.top = None;
        self.anchor_lost = false;
        self.paused = false;
        self.requested = Some(LogQuery::Tail);
    }
}

pub struct App {
    bookmarks: BTreeMap<String, Bookmark>,
    pub focus: Focus,
    pub goto_top: bool,
    pub help_scroll: u16,
    pub logs_loaded: bool,
    pub logs_error: Option<String>,
    pub services: BTreeMap<String, ServiceStatus>,
    pub selected: Option<String>,
    pub workspace_id: Option<String>,
    pub connected: bool,
    pub epoch: u64,
    pub selection_epoch: u64,
    pub logs: LogWindow,
    pub pending: Option<Operation>,
    pub feedback: String,
    pub connection_error: Option<String>,
    pub confirm_shutdown: bool,
    pub help: bool,
    pub search_input: Option<String>,
}

impl App {
    pub fn new() -> Self {
        Self {
            bookmarks: BTreeMap::new(),
            focus: Focus::Projects,
            goto_top: false,
            help_scroll: 0,
            logs_loaded: false,
            logs_error: None,
            services: BTreeMap::new(),
            selected: None,
            workspace_id: None,
            connected: false,
            epoch: 0,
            selection_epoch: 0,
            logs: LogWindow::default(),
            pending: None,
            feedback: "Connecting; services will not start automatically.".into(),
            connection_error: None,
            confirm_shutdown: false,
            help: false,
            search_input: None,
        }
    }

    pub fn focus(&mut self, focus: Focus) {
        self.focus = focus;
        self.goto_top = false;
        if focus == Focus::Projects {
            self.logs.selection = None;
        }
    }

    fn reset_logs(&mut self) {
        self.search_input = None;
        self.logs = LogWindow::default();
        if let Some(bookmark) = self.selected.as_ref().and_then(|id| self.bookmarks.get(id)) {
            self.logs.paused = true;
            self.logs.top = Some(bookmark.top);
            self.logs.reading = bookmark.reading;
            self.logs.horizontal = bookmark.horizontal;
            self.logs.pause_at = bookmark.pause_at;
            self.logs.requested = Some(LogQuery::Restore(bookmark.top));
        }
        self.logs_loaded = false;
        self.logs_error = None;
        self.goto_top = false;
    }

    #[cfg(test)]
    pub fn apply_logs(&mut self, batch: Batch) -> bool {
        self.apply_log_page(LogQuery::After(self.logs.cursor.unwrap_or(0)), batch)
    }

    pub fn apply_log_page(&mut self, query: LogQuery, batch: Batch) -> bool {
        if let LogQuery::Search(previous) = query {
            self.logs.storage = batch.storage.clone();
            if let Some(record) = batch.records.first() {
                self.logs.cancel_search();
                if let Some(search) = &mut self.logs.search {
                    search.matched = Some(record.sequence);
                }
                if self
                    .logs
                    .records
                    .iter()
                    .any(|r| r.sequence == record.sequence)
                {
                    self.logs.reading = Some(record.sequence);
                    if self.logs.records.back().is_some_and(|r| {
                        r.sequence == record.sequence && r.sequence < self.logs.latest
                    }) {
                        self.logs.requested = Some(LogQuery::ContextAfter(record.sequence));
                    }
                } else {
                    self.logs.requested = Some(LogQuery::Context(record.sequence));
                }
                self.feedback = format!(
                    "Match seq={}; n next / N previous; fixed upper={}.",
                    record.sequence,
                    self.logs.search.as_ref().map_or(0, |s| s.upper)
                );
            } else if !batch.search_complete
                && batch.scanned_to.is_some_and(|cursor| cursor != previous)
            {
                self.logs.requested = batch.scanned_to.map(LogQuery::Search);
                self.feedback = format!(
                    "Searching retained history... scanned to {}; Esc cancels.",
                    batch.scanned_to.unwrap_or(0)
                );
            } else {
                self.logs.cancel_search();
                self.feedback = if batch.gap
                    || batch.incomplete
                    || batch
                        .storage
                        .as_ref()
                        .is_some_and(|s| s.state == "degraded")
                {
                    "No further match in available output; history has gaps."
                } else {
                    "No further match in this direction (fixed snapshot)."
                }
                .into();
            }
            return true;
        }
        let changed = !self.logs_loaded || self.logs_error.is_some();
        self.logs_loaded = true;
        self.logs_error = None;
        let updated = self.logs.apply_page(query, batch);
        if matches!(query, LogQuery::Context(_)) && self.logs.reading.is_none() {
            self.logs.anchor_lost = true;
            self.feedback = "Matched log is no longer retained; showing available history.".into();
            if self.logs.records.is_empty()
                && let Some(oldest) = self.logs.oldest_retained
            {
                self.logs.requested = Some(LogQuery::Restore(oldest));
            }
        }
        updated || changed
    }

    fn remember_position(&mut self) {
        let Some(id) = self.selected.as_ref() else {
            return;
        };
        if !self.logs_loaded {
            return;
        }
        if self.logs.paused {
            if let Some(top) = self.logs.top {
                self.bookmarks.insert(
                    id.clone(),
                    Bookmark {
                        top,
                        reading: self.logs.reading,
                        horizontal: self.logs.horizontal,
                        pause_at: self.logs.pause_at,
                    },
                );
            }
        } else {
            self.bookmarks.remove(id);
        }
    }

    pub fn select(&mut self, direction: isize) -> bool {
        let ids: Vec<_> = self.services.keys().collect();
        if ids.is_empty() {
            return false;
        }
        let current = ids
            .iter()
            .position(|id| Some(id.as_str()) == self.selected.as_deref())
            .unwrap_or(0);
        let next = current.saturating_add_signed(direction).min(ids.len() - 1);
        if self.selected.as_ref() == Some(ids[next]) {
            return false;
        }
        let next = ids[next].clone();
        self.remember_position();
        self.selected = Some(next);
        self.selection_epoch += 1;
        self.reset_logs();
        true
    }

    pub fn apply_status(&mut self, response: Response) -> bool {
        let reconnected =
            !self.connected || self.workspace_id.as_ref() != Some(&response.workspace_id);
        let changed = reconnected || self.services != response.services;
        if reconnected {
            if self
                .workspace_id
                .as_ref()
                .is_some_and(|id| id != &response.workspace_id)
            {
                self.bookmarks.clear();
                self.feedback = "Workspace changed; old log positions are no longer valid.".into();
            } else {
                self.remember_position();
            }
            self.epoch += 1;
            self.selection_epoch += 1;
            self.reset_logs();
            self.confirm_shutdown = false;
            if self.workspace_id.is_none() {
                self.feedback = "Connected. Services were not started automatically. s: start selected; a: start default.".into();
            }
        }
        self.connected = true;
        self.connection_error = None;
        self.workspace_id = Some(response.workspace_id);
        self.services = response.services;
        self.bookmarks
            .retain(|id, _| self.services.contains_key(id));
        if !self
            .selected
            .as_ref()
            .is_some_and(|id| self.services.contains_key(id))
        {
            self.selected = self.services.keys().next().cloned();
            self.selection_epoch += 1;
            self.reset_logs();
        }
        changed
    }

    pub fn disconnect(&mut self, message: String) {
        self.remember_position();
        self.connected = false;
        self.epoch += 1;
        self.selection_epoch += 1;
        self.reset_logs();
        self.confirm_shutdown = false;
        self.connection_error = Some(message);
    }

    pub fn accepts_logs(&self, epoch: u64, selection: u64, service: &str) -> bool {
        self.connected
            && self.epoch == epoch
            && self.selection_epoch == selection
            && self.selected.as_deref() == Some(service)
    }

    pub fn begin(&mut self, operation: Operation) -> bool {
        if !self.connected {
            self.feedback =
                "Disconnected; state unknown. Reconnect before controlling services.".into();
            return false;
        }
        if let Some(pending) = &self.pending {
            self.feedback = format!(
                "Waiting for {}; {} was not sent. Browsing and q leave remain available.",
                pending.label(),
                operation.label()
            );
            return false;
        }
        self.feedback = format!(
            "{}: waiting for the workspace; browse or q leave.",
            operation.label()
        );
        self.pending = Some(operation);
        true
    }

    /// Control responses never overwrite snapshots from the status query stream.
    pub fn control_response(
        &mut self,
        operation: &Operation,
        expected: &str,
        response: Response,
    ) -> bool {
        self.pending = None;
        if response.workspace_id != expected || self.workspace_id.as_deref() != Some(expected) {
            self.feedback = format!(
                "{} (workspace changed): {}. Refreshing state.",
                operation.label(),
                response
                    .error
                    .as_deref()
                    .unwrap_or("request completed on another workspace instance")
            );
            self.disconnect("Workspace changed; refreshing state and recent logs.".into());
            return false;
        }
        // Any in-flight status query predates this completion and cannot confirm it.
        self.epoch += 1;
        if let Some(error) = response.error {
            self.feedback = format!("{}: {error}", operation.label());
            return false;
        }
        self.feedback = match operation {
            Operation::Start(_) | Operation::StartAll => format!(
                "{}: Start request accepted; waiting for process state.",
                operation.label()
            ),
            Operation::Stop(id) => format!("{id}: stopped; cleanup confirmed."),
            Operation::StopAll => "All services stopped; background workspace retained.".into(),
            Operation::Shutdown => "Workspace ended.".into(),
        };
        matches!(operation, Operation::Shutdown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{logs::Source, manager::ServiceState};

    fn status(workspace: &str) -> Response {
        Response {
            workspace_id: workspace.into(),
            services: ["a", "b"]
                .into_iter()
                .map(|id| (id.into(), ServiceStatus::default()))
                .collect(),
            ..Response::default()
        }
    }

    fn batch(sequences: std::ops::RangeInclusive<u64>, size: usize) -> Batch {
        Batch {
            records: sequences
                .map(|sequence| Record {
                    captured_ms: 0,
                    sequence,
                    instance: 1,
                    source: Source::Stdout,
                    text: "x".repeat(size),
                    truncated: false,
                })
                .collect(),
            dropped: 0,
            incomplete: false,
            gap: false,
            latest: 100_000,
            ..Batch::default()
        }
    }

    fn page(sequences: std::ops::RangeInclusive<u64>, oldest: u64, latest: u64) -> Batch {
        Batch {
            oldest: Some(oldest),
            latest,
            ..batch(sequences, 1)
        }
    }

    #[test]
    fn paused_bounds_polling_keeps_records_selection_and_position_stable() {
        let mut logs = LogWindow::default();
        logs.apply_page(LogQuery::Tail, page(201..=300, 1, 300));
        logs.pause(10);
        logs.begin_selection(10);
        let selection = logs.selection;
        let top = logs.top;
        assert_eq!(logs.query(), LogQuery::Bounds);
        let mut bounds = Batch {
            oldest: Some(401),
            latest: 4500,
            ..Batch::default()
        };
        bounds.dropped = 400;
        logs.apply_page(LogQuery::Bounds, bounds);
        assert_eq!(logs.records.len(), 100);
        assert_eq!(logs.top, top);
        assert_eq!(logs.selection, selection);
        assert_eq!(logs.new_records(), 4200);
        assert!(!logs.anchor_lost);
        logs.follow();
        assert_eq!(logs.query(), LogQuery::Tail);
        logs.apply_page(LogQuery::Tail, page(4401..=4500, 401, 4500));
        assert!(!logs.paused && logs.top.is_none());
        assert_eq!(logs.records.front().unwrap().sequence, 4401);
        assert_eq!(logs.records.len(), 100);
    }

    #[test]
    fn history_prepend_deduplicates_and_keeps_reading_and_copy_anchors() {
        let mut logs = LogWindow::default();
        logs.apply_page(LogQuery::Tail, page(201..=300, 1, 300));
        logs.move_reading(isize::MIN, 10);
        logs.begin_selection(10);
        assert_eq!(logs.query(), LogQuery::Before(201));
        let selection = logs.selection;
        logs.apply_page(LogQuery::Before(201), page(101..=210, 1, 350));
        assert_eq!(logs.records.len(), 200);
        assert_eq!(logs.top, Some(201));
        assert_eq!(logs.reading, Some(201));
        assert_eq!(logs.selection, selection);
        assert_eq!(logs.selected_text().unwrap(), ("x\n".into(), 1));
        logs.move_selection(-1, 10);
        assert_eq!(logs.selection.unwrap().head, 200);
        assert_eq!(logs.query(), LogQuery::Bounds);
    }

    #[test]
    fn prepending_respects_memory_limits_and_forward_paging_does_not_skip_trimmed_tail() {
        let mut logs = LogWindow::default();
        logs.apply_page(LogQuery::Tail, page(101..=4196, 1, 4196));
        logs.oldest();
        logs.requested = Some(LogQuery::Before(101));
        logs.apply_page(LogQuery::Before(101), page(1..=100, 1, 4196));
        assert_eq!(logs.records.len(), MAX_LOG_RECORDS);
        assert_eq!(logs.cursor, Some(4096));
        assert_eq!(logs.records.front().unwrap().sequence, 1);
        logs.scroll(isize::MAX, 10);
        assert!(logs.paused);
        assert_eq!(logs.query(), LogQuery::After(4096));
        logs.apply_page(LogQuery::After(4096), page(4097..=4196, 1, 4196));
        assert_eq!(logs.records.back().unwrap().sequence, 4196);

        let mut logs = LogWindow::default();
        logs.append(batch(10..=137, 16 * 1024));
        logs.apply_page(LogQuery::Before(10), batch(1..=9, 16 * 1024));
        assert_eq!(logs.bytes, MAX_LOG_BYTES);
        assert_eq!(logs.records.len(), 128);
        assert_eq!(logs.cursor, Some(128));
    }

    #[test]
    fn bookmarks_restore_across_project_switch_and_reconnect_but_not_workspace_replacement() {
        let mut app = App::new();
        app.apply_status(status("one"));
        app.apply_log_page(LogQuery::Tail, page(201..=300, 1, 300));
        app.logs.move_reading(-2, 10);
        app.logs.horizontal = 16;
        let (top, reading) = (app.logs.top, app.logs.reading);
        app.select(1);
        app.apply_log_page(LogQuery::Tail, page(1..=5, 1, 5));
        app.select(-1);
        assert_eq!(app.logs.query(), LogQuery::Restore(top.unwrap()));
        app.apply_log_page(app.logs.query(), page(top.unwrap()..=300, 1, 320));
        assert_eq!((app.logs.top, app.logs.reading), (top, reading));
        assert_eq!(app.logs.horizontal, 16);
        app.disconnect("lost".into());
        app.apply_status(status("one"));
        assert_eq!(app.logs.query(), LogQuery::Restore(top.unwrap()));
        app.apply_log_page(app.logs.query(), page(top.unwrap()..=300, 1, 320));
        app.apply_status(status("two"));
        assert_eq!(app.logs.query(), LogQuery::Tail);
        assert!(!app.logs.paused);
        assert!(app.bookmarks.is_empty());
        assert!(app.feedback.contains("no longer valid"));
    }

    #[test]
    fn expired_bookmarks_land_at_retained_boundary_with_a_warning() {
        let mut logs = LogWindow {
            paused: true,
            top: Some(10),
            reading: Some(15),
            ..LogWindow::default()
        };
        logs.apply_page(LogQuery::Restore(10), page(500..=599, 500, 700));
        assert_eq!(logs.top, Some(500));
        assert!(logs.reading.is_none() && logs.anchor_lost);
        logs.oldest();
        assert_eq!(logs.query(), LogQuery::Bounds);
        logs.apply_page(
            LogQuery::Bounds,
            Batch {
                oldest: Some(600),
                latest: 800,
                ..Batch::default()
            },
        );
        logs.move_reading(isize::MIN, 10);
        assert_eq!(logs.query(), LogQuery::Bounds);
    }

    #[test]
    fn jumping_to_retained_start_and_short_restore_pages_do_not_report_false_eviction() {
        let mut logs = LogWindow::default();
        logs.apply_page(LogQuery::Tail, page(201..=300, 1, 300));
        logs.oldest();
        assert_eq!(logs.query(), LogQuery::Restore(1));
        logs.apply_page(LogQuery::Restore(1), page(1..=100, 1, 300));
        assert_eq!(logs.top, Some(1));
        assert!(!logs.anchor_lost);
        logs.reading = Some(10);
        logs.apply_page(LogQuery::Restore(1), page(1..=4, 1, 300));
        assert!(logs.reading.is_none());
        assert_eq!(logs.top, Some(1));
        assert!(!logs.anchor_lost);
        logs.follow();
        logs.pause(10);
        assert_eq!(logs.query(), LogQuery::Bounds);
    }

    #[test]
    fn reading_cursor_starts_selection_in_the_middle_and_survives_output_and_resize() {
        let mut logs = LogWindow::default();
        logs.append(batch(1..=20, 1));
        logs.move_reading(2, 5);
        assert_eq!(logs.reading, Some(18));
        assert_eq!(logs.top, Some(16));
        logs.begin_selection(5);
        assert_eq!(logs.selection.unwrap().anchor, 18);
        assert_eq!(logs.top, Some(16));
        logs.move_selection(1, 5);
        logs.append(batch(21..=25, 1));
        assert_eq!(logs.selection.unwrap().head, 19);
        assert_eq!(logs.reading, Some(19));
        assert_eq!(logs.top, Some(16));
        logs.selection = None;
        logs.keep_visible(1);
        assert_eq!(logs.top, Some(19));
        logs.begin_selection(1);
        assert_eq!(logs.selection.unwrap().anchor, 19);
        logs.follow();
        assert_eq!(logs.reading, None);
        assert_eq!(logs.selection, None);
        assert_eq!(logs.top, None);
    }

    #[test]
    fn evicted_reading_cursor_is_cleared_and_paging_repositions_selection_start() {
        let mut logs = LogWindow::default();
        logs.append(batch(1..=20, 1));
        logs.oldest();
        logs.move_reading(2, 5);
        logs.scroll(5, 5);
        assert_eq!(logs.reading, None);
        logs.begin_selection(5);
        assert_eq!(logs.selection.unwrap().anchor, 6);
        logs.selection = None;
        logs.append(batch(21..=4102, 1));
        assert_eq!(logs.reading, None);
        assert!(logs.anchor_lost);
        logs.begin_selection(5);
        assert_eq!(logs.selection.unwrap().anchor, 7);
    }

    #[test]
    fn line_selection_extends_both_ways_without_following_new_output() {
        let mut logs = LogWindow::default();
        let mut page = batch(1..=20, 1);
        for record in &mut page.records {
            record.text = record.sequence.to_string();
        }
        logs.append(page);
        assert!(logs.begin_selection(5));
        assert_eq!(logs.top, Some(16));
        logs.move_selection(2, 5);
        assert_eq!(logs.selected_text().unwrap(), ("16\n17\n18\n".into(), 3));
        logs.append(batch(21..=25, 1));
        assert_eq!(logs.top, Some(16));
        assert_eq!(logs.selection.unwrap().head, 18);
        logs.move_selection(-4, 5);
        assert_eq!(logs.selection.unwrap().head, 14);
        assert_eq!(logs.top, Some(14));
        assert_eq!(logs.selected_text().unwrap(), ("14\n15\n16\n".into(), 3));
        logs.move_selection(isize::MIN, 5);
        assert_eq!(logs.selection.unwrap().head, 1);
        logs.move_selection(isize::MAX, 5);
        assert_eq!(logs.selection.unwrap().head, 25);
        assert_eq!(logs.top, Some(21));
    }

    #[test]
    fn copy_uses_full_cached_body_including_unicode_and_marks_truncation() {
        let mut logs = LogWindow::default();
        let mut page = batch(1..=3, 1);
        page.records[0].text = "中文\tlog".into();
        page.records[1].text = "second line".into();
        page.records[1].truncated = true;
        logs.append(page);
        logs.horizontal = 100;
        logs.begin_selection(5);
        logs.move_selection(1, 5);
        assert_eq!(
            logs.selected_text().unwrap(),
            ("中文\tlog\nsecond line [truncated]\n".into(), 2)
        );
        assert!(!logs.selected_text().unwrap().0.contains("seq="));
    }

    #[test]
    fn evicted_selection_is_canceled_and_oversize_copy_is_rejected() {
        let mut logs = LogWindow::default();
        assert!(!logs.begin_selection(5));
        assert!(logs.selected_text().is_err());
        logs.append(batch(1..=5, 1));
        logs.begin_selection(5);
        logs.append(batch(6..=4100, 1));
        assert!(logs.selection.is_none() && logs.selection_lost);
        assert!(logs.selected_text().is_err());
        logs.begin_selection(5);
        assert!(!logs.selection_lost);
        let mut logs = LogWindow::default();
        logs.append(batch(1..=5, 16 * 1024));
        logs.begin_selection(5);
        logs.move_selection(isize::MAX, 5);
        assert!(logs.selected_text().unwrap_err().contains("64 KiB"));
        assert!(logs.selection.is_some());
        let mut logs = LogWindow::default();
        logs.append(batch(1..=4, 16 * 1024 - 1));
        logs.begin_selection(4);
        logs.move_selection(isize::MAX, 4);
        let (text, count) = logs.selected_text().unwrap();
        assert_eq!((text.len(), count), (64 * 1024, 4));
    }

    #[test]
    fn log_loading_and_errors_follow_selection_identity() {
        let mut app = App::new();
        app.apply_status(status("one"));
        assert!(!app.logs_loaded);
        app.logs_error = Some("temporary failure".into());
        assert!(app.apply_logs(Batch::default()));
        assert!(app.logs_loaded && app.logs_error.is_none());
        assert!(!app.apply_logs(Batch::default()));
        app.goto_top = true;
        app.select(1);
        assert!(!app.logs_loaded && !app.goto_top);
        app.apply_logs(batch(1..=10, 1));
        app.focus(Focus::Logs);
        assert!(app.logs_loaded);
        app.disconnect("lost".into());
        assert!(!app.logs_loaded);
    }

    #[test]
    fn operation_feedback_keeps_the_requested_target_after_selection_changes() {
        let mut app = App::new();
        app.apply_status(status("one"));
        assert!(app.begin(Operation::Stop("a".into())));
        app.select(1);
        assert!(!app.begin(Operation::Start("b".into())));
        assert!(app.feedback.contains("Stop a"));
        assert!(app.feedback.contains("Start b was not sent"));
        app.control_response(&Operation::Stop("a".into()), "one", status("one"));
        assert_eq!(app.feedback, "a: stopped; cleanup confirmed.");
    }

    #[test]
    fn selection_uses_identity_and_rejects_a_b_a_log_results() {
        let mut app = App::new();
        app.apply_status(status("one"));
        let (epoch, selection) = (app.epoch, app.selection_epoch);
        assert_eq!(app.selected.as_deref(), Some("a"));
        assert!(!app.select(-1));
        assert!(app.select(1));
        assert!(app.select(-1));
        assert!(!app.accepts_logs(epoch, selection, "a"));
        app.select(1);
        app.apply_status(status("one"));
        assert_eq!(app.selected.as_deref(), Some("b"));
    }

    #[test]
    fn reconnect_and_workspace_replacement_invalidate_caches_and_queries() {
        let mut app = App::new();
        app.apply_status(status("one"));
        app.logs.append(batch(1..=3, 1));
        let epoch = app.epoch;
        app.disconnect("unknown".into());
        assert!(!app.connected);
        assert!(!app.begin(Operation::StartAll));
        assert!(app.logs.records.is_empty());
        assert!(!app.accepts_logs(epoch, app.selection_epoch, "a"));
        app.apply_status(status("one"));
        assert_eq!(app.logs.cursor, None);
        app.logs.append(batch(1..=3, 1));
        app.confirm_shutdown = true;
        app.apply_status(status("two"));
        assert!(app.logs.records.is_empty());
        assert_eq!(app.logs.cursor, None);
        assert!(!app.confirm_shutdown);
    }

    #[test]
    fn controls_are_bounded_and_never_replace_snapshots_optimistically() {
        let mut app = App::new();
        app.apply_status(status("one"));
        let previous_query = app.epoch;
        assert!(app.begin(Operation::Start("a".into())));
        assert!(!app.begin(Operation::StopAll));
        assert!(app.select(1));
        let mut response = status("one");
        response.services.get_mut("a").unwrap().state = ServiceState::Running;
        assert!(!app.control_response(&Operation::Start("a".into()), "one", response));
        assert!(app.pending.is_none());
        assert_eq!(app.services["a"].state, ServiceState::Stopped);
        assert_ne!(previous_query, app.epoch);
        assert!(app.feedback.contains("accepted"));
    }

    #[test]
    fn server_errors_and_cross_workspace_shutdown_do_not_claim_success() {
        let mut app = App::new();
        app.apply_status(status("one"));
        let mut response = status("one");
        response.error = Some("completion timed out; query status".into());
        assert!(!app.control_response(&Operation::Shutdown, "one", response));
        assert!(app.feedback.contains("completion timed out"));
        app.apply_status(status("two"));
        assert!(!app.control_response(&Operation::Shutdown, "one", status("one")));
        assert!(!app.connected);
        assert!(app.feedback.contains("workspace changed"));
        app.apply_status(status("two"));
        assert!(app.control_response(&Operation::Shutdown, "two", status("two")));
    }

    #[test]
    fn reconnect_does_not_erase_an_unknown_control_outcome() {
        let mut app = App::new();
        app.apply_status(status("one"));
        app.feedback = "Stop a: completion unknown; query state before retrying.".into();
        app.disconnect("Connection lost.".into());
        assert!(app.connection_error.is_some());
        app.apply_status(status("one"));
        assert!(app.connection_error.is_none());
        assert!(app.feedback.contains("completion unknown"));
    }

    #[test]
    fn cursor_tracks_received_records_and_ignores_duplicates_and_empty_pages() {
        let mut logs = LogWindow::default();
        assert!(logs.append(batch(1..=3, 1)));
        assert_eq!(logs.cursor, Some(3));
        assert!(!logs.append(batch(1..=3, 1)));
        let mut empty = batch(1..=1, 1);
        empty.records.clear();
        assert!(!logs.append(empty));
        assert_eq!(logs.cursor, Some(3));
        assert_eq!(logs.records.len(), 3);
    }

    #[test]
    fn scrolling_keeps_its_sequence_anchor_as_output_arrives() {
        let mut logs = LogWindow::default();
        logs.append(batch(1..=20, 1));
        assert_eq!(logs.start(5), 15);
        logs.scroll(-4, 5);
        let anchor = logs.top;
        logs.append(batch(21..=25, 1));
        assert_eq!(logs.top, anchor);
        assert_eq!(logs.start(5), 11);
        logs.oldest();
        assert_eq!(logs.start(5), 0);
        logs.scroll(100, 5);
        assert_eq!(logs.top, None);
        assert_eq!(logs.start(5), 20);
    }

    #[test]
    fn record_and_byte_limits_evict_old_data_and_report_lost_read_position() {
        let mut logs = LogWindow::default();
        logs.append(batch(1..=5, 1));
        logs.oldest();
        logs.append(batch(6..=4100, 1));
        assert_eq!(logs.records.len(), 4096);
        assert_eq!(logs.local_evicted, 4);
        assert!(logs.anchor_lost);
        assert_eq!(logs.top, Some(5));
        let mut logs = LogWindow::default();
        logs.append(batch(1..=129, 16 * 1024));
        assert_eq!(logs.records.len(), 128);
        assert_eq!(logs.bytes, 2 * 1024 * 1024);
        assert_eq!(logs.local_evicted, 1);
    }

    #[test]
    fn capture_warnings_survive_empty_pages() {
        let mut logs = LogWindow::default();
        let mut page = batch(1..=1, 1);
        page.dropped = 4;
        page.gap = true;
        page.incomplete = true;
        logs.append(page);
        let mut empty = batch(1..=1, 1);
        empty.records.clear();
        empty.dropped = 4;
        empty.incomplete = true;
        assert!(!logs.append(empty));
        assert!(logs.gap && logs.incomplete);
        assert_eq!(logs.dropped, 4);
    }
    #[test]
    fn search_continues_empty_batches_and_opens_copyable_context_with_fixed_upper() {
        let mut app = App::new();
        app.logs.latest = 1000;
        app.logs.start_search("needle".into(), false, true, 10);
        app.apply_log_page(
            LogQuery::Search(0),
            Batch {
                scanned_to: Some(300),
                search_complete: false,
                ..Batch::default()
            },
        );
        assert_eq!(app.logs.requested, Some(LogQuery::Search(300)));
        app.apply_log_page(LogQuery::Search(300), page(450..=450, 1, 1200));
        assert_eq!(app.logs.requested, Some(LogQuery::Context(450)));
        app.apply_log_page(LogQuery::Context(450), page(351..=450, 1, 1200));
        assert_eq!(app.logs.reading, Some(450));
        assert_eq!(app.logs.requested, Some(LogQuery::ContextAfter(450)));
        app.logs.keep_visible(2);
        assert!(app.logs.start(2) + 2 >= app.logs.records.len());
        app.logs.start_search("needle".into(), true, false, 2);
        assert_eq!(app.logs.search.as_ref().unwrap().upper, 1000);
        assert_eq!(app.logs.requested, Some(LogQuery::Search(450)));
        app.logs.cancel_search();
        assert_eq!(app.logs.query(), LogQuery::Bounds);
        assert!(!app.logs.search.as_ref().unwrap().running);
        app.logs.follow();
        assert_eq!(app.logs.query(), LogQuery::Tail);
    }

    #[test]
    fn search_reports_no_match_only_when_scan_completes_and_keeps_the_reading_snapshot() {
        let mut app = App::new();
        app.logs.apply_page(LogQuery::Tail, page(1..=100, 1, 100));
        app.logs.start_search("absent".into(), false, true, 10);
        let top = app.logs.top;
        app.apply_log_page(
            LogQuery::Search(0),
            Batch {
                search_complete: true,
                ..Batch::default()
            },
        );
        assert!(app.feedback.contains("No further match"));
        assert_eq!(app.logs.top, top);
        assert_eq!(app.logs.records.len(), 100);
        assert!(app.logs.paused);
    }
    #[test]
    fn cached_search_hits_keep_all_surrounding_logs_and_clear_without_moving() {
        let mut app = App::new();
        app.apply_log_page(LogQuery::Tail, page(1..=200, 1, 200));
        let original = app
            .logs
            .records
            .iter()
            .map(|r| r.sequence)
            .collect::<Vec<_>>();
        app.logs.start_search("x".into(), false, true, 20);
        app.apply_log_page(LogQuery::Search(0), page(80..=80, 1, 200));
        app.logs.reveal_match(20);
        assert_eq!(
            app.logs
                .records
                .iter()
                .map(|r| r.sequence)
                .collect::<Vec<_>>(),
            original
        );
        assert_eq!(app.logs.reading, Some(80));
        assert!(app.logs.requested.is_none());
        let top = app.logs.top;
        app.logs.clear_search();
        assert_eq!(app.logs.top, top);
        assert_eq!(app.logs.reading, Some(80));
        assert_eq!(app.logs.records.len(), 200);
        assert!(app.logs.search.is_none());
    }

    #[test]
    fn distant_search_hits_load_both_sides_and_fill_byte_limited_context() {
        let mut app = App::new();
        app.apply_log_page(LogQuery::Tail, page(901..=1000, 1, 1000));
        app.logs.start_search("x".into(), false, true, 24);
        app.apply_log_page(LogQuery::Search(0), page(450..=450, 1, 1000));
        app.apply_log_page(
            LogQuery::Context(450),
            Batch {
                oldest: Some(1),
                latest: 1000,
                ..batch(447..=450, 16 * 1024)
            },
        );
        assert_eq!(app.logs.requested, Some(LogQuery::ContextAfter(450)));
        for end in [454, 458, 462, 466, 470, 474] {
            app.apply_log_page(
                LogQuery::ContextAfter(end - 4),
                Batch {
                    oldest: Some(1),
                    latest: 1000,
                    ..batch(end - 3..=end, 16 * 1024)
                },
            );
            app.logs.reveal_match(24);
        }
        assert_eq!(app.logs.records.front().unwrap().sequence, 447);
        assert_eq!(app.logs.records.back().unwrap().sequence, 474);
        assert_eq!(app.logs.reading, Some(450));
        assert_eq!(app.logs.records.len(), 28);
        assert!(app.logs.requested.is_none() && !app.logs.anchor_lost);
    }

    #[test]
    fn an_expired_search_hit_reports_loss_and_falls_back_to_retained_history() {
        let mut app = App::new();
        app.logs.start_search("x".into(), false, true, 10);
        app.apply_log_page(LogQuery::Search(0), page(10..=10, 1, 100));
        app.apply_log_page(
            LogQuery::Context(10),
            Batch {
                oldest: Some(50),
                latest: 100,
                gap: true,
                ..Batch::default()
            },
        );
        assert!(app.logs.anchor_lost);
        assert_eq!(app.logs.requested, Some(LogQuery::Restore(50)));
        assert!(app.feedback.contains("no longer retained"));
    }
}
