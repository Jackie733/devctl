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

#[derive(Default)]
pub struct LogWindow {
    pub records: VecDeque<Record>,
    bytes: usize,
    pub cursor: Option<u64>,
    pub top: Option<u64>,
    pub horizontal: u16,
    pub dropped: u64,
    pub incomplete: bool,
    pub gap: bool,
    pub local_evicted: u64,
    pub anchor_lost: bool,
}

impl LogWindow {
    pub fn append(&mut self, batch: Batch) -> bool {
        let mut changed = self.dropped != batch.dropped
            || self.incomplete != batch.incomplete
            || (!self.gap && batch.gap);
        self.dropped = batch.dropped;
        self.incomplete = batch.incomplete;
        self.gap |= batch.gap;
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
        while self.bytes > MAX_LOG_BYTES || self.records.len() > MAX_LOG_RECORDS {
            if let Some(record) = self.records.pop_front() {
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
        changed
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
        let last = self.records.len().saturating_sub(height);
        let start = self.start(height).saturating_add_signed(amount).min(last);
        self.top = if amount > 0 && start == last {
            None
        } else {
            self.records.get(start).map(|record| record.sequence)
        };
    }

    pub fn oldest(&mut self) {
        self.top = self.records.front().map(|record| record.sequence);
    }

    pub fn follow(&mut self) {
        self.top = None;
        self.anchor_lost = false;
    }
}

pub struct App {
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
}

impl App {
    pub fn new() -> Self {
        Self {
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
        }
    }

    pub fn focus(&mut self, focus: Focus) {
        self.focus = focus;
        self.goto_top = false;
    }

    fn reset_logs(&mut self) {
        self.logs = LogWindow::default();
        self.logs_loaded = false;
        self.logs_error = None;
        self.goto_top = false;
    }

    pub fn apply_logs(&mut self, batch: Batch) -> bool {
        let changed = !self.logs_loaded || self.logs_error.is_some();
        self.logs_loaded = true;
        self.logs_error = None;
        self.logs.append(batch) || changed
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
        self.selected = Some(ids[next].clone());
        self.selection_epoch += 1;
        self.reset_logs();
        true
    }

    pub fn apply_status(&mut self, response: Response) -> bool {
        let reconnected =
            !self.connected || self.workspace_id.as_ref() != Some(&response.workspace_id);
        let changed = reconnected || self.services != response.services;
        if reconnected {
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
        }
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
}
