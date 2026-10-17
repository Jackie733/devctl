use std::{
    collections::{BTreeMap, VecDeque},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

use serde::{Deserialize, Serialize};
use tokio::{
    io::{AsyncRead, AsyncReadExt},
    process::Child,
    sync::mpsc,
    task::JoinSet,
    time::timeout,
};

pub const MAX_RECORD_BYTES: usize = 16 * 1024;
pub const QUEUE_CAPACITY: usize = 256;
const MAX_BUFFER_BYTES: usize = 2 * 1024 * 1024;
const MAX_RECORDS: usize = 4096;
pub const MAX_QUERY_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    #[serde(default)]
    pub captured_ms: u64,
    pub sequence: u64,
    pub instance: u64,
    pub source: Source,
    pub text: String,
    pub truncated: bool,
}

pub struct Event {
    service: Arc<str>,
    record: Record,
}

#[derive(Clone)]
pub struct Sink {
    service: Arc<str>,
    instance: u64,
    sender: mpsc::Sender<Event>,
    dropped: Arc<AtomicU64>,
    incomplete: Arc<AtomicBool>,
}

impl Sink {
    pub fn with_instance(&self, instance: u64) -> Self {
        Self {
            instance,
            ..self.clone()
        }
    }

    fn lost(&self) {
        self.dropped.fetch_add(1, Ordering::Relaxed);
    }

    fn incomplete(&self) {
        self.incomplete.store(true, Ordering::Relaxed);
    }

    fn emit(&self, source: Source, raw: &[u8], split: bool) {
        let (text, clipped) = sanitize(raw);
        let event = Event {
            service: self.service.clone(),
            record: Record {
                captured_ms: now_ms(),
                sequence: 0,
                instance: self.instance,
                source,
                text,
                truncated: split || clipped,
            },
        };
        if self.sender.try_send(event).is_err() {
            self.lost();
        }
    }
}

fn sanitize(raw: &[u8]) -> (String, bool) {
    let mut text = String::with_capacity(raw.len().min(MAX_RECORD_BYTES));
    for character in String::from_utf8_lossy(raw).chars() {
        let mut buffer = [0; 4];
        let escaped;
        let part = if character.is_control() && character != '\t' {
            escaped = character.escape_default().to_string();
            escaped.as_str()
        } else {
            character.encode_utf8(&mut buffer)
        };
        if text.len() + part.len() > MAX_RECORD_BYTES {
            return (text, true);
        }
        text.push_str(part);
    }
    (text, false)
}

async fn capture(
    mut pipe: impl AsyncRead + Unpin,
    sink: Sink,
    source: Source,
) -> std::io::Result<()> {
    let mut buffer = [0; 4096];
    let mut pending = Vec::with_capacity(MAX_RECORD_BYTES);
    let mut continued = false;
    loop {
        let count = pipe.read(&mut buffer).await?;
        if count == 0 {
            if !pending.is_empty() {
                sink.emit(source, &pending, continued);
            }
            return Ok(());
        }
        for byte in &buffer[..count] {
            if *byte == b'\n' {
                if !pending.is_empty() || !continued {
                    sink.emit(source, &pending, continued);
                }
                pending.clear();
                continued = false;
            } else {
                if pending.len() == MAX_RECORD_BYTES {
                    let keep = std::str::from_utf8(&pending)
                        .err()
                        .filter(|error| error.error_len().is_none())
                        .map_or(0, |error| pending.len() - error.valid_up_to());
                    let split = pending.len() - keep;
                    sink.emit(source, &pending[..split], true);
                    pending.copy_within(split.., 0);
                    pending.truncate(keep);
                    continued = true;
                }
                pending.push(*byte);
            }
        }
    }
}

pub struct Readers {
    tasks: JoinSet<std::io::Result<()>>,
    sink: Sink,
}
impl Readers {
    pub fn start(child: &mut Child, sink: Sink) -> Self {
        let mut tasks = JoinSet::new();
        if let Some(pipe) = child.stdout.take() {
            tasks.spawn(capture(pipe, sink.clone(), Source::Stdout));
        }
        if let Some(pipe) = child.stderr.take() {
            tasks.spawn(capture(pipe, sink.clone(), Source::Stderr));
        }
        Self { tasks, sink }
    }

    pub async fn finish(mut self) {
        if timeout(Duration::from_secs(1), async {
            while let Some(result) = self.tasks.join_next().await {
                if !matches!(result, Ok(Ok(()))) {
                    self.sink.incomplete();
                }
            }
        })
        .await
        .is_err()
        {
            self.sink.incomplete();
            self.tasks.shutdown().await;
        }
    }
}

impl Drop for Readers {
    fn drop(&mut self) {
        if !self.tasks.is_empty() {
            self.sink.incomplete();
        }
    }
}

struct Buffer {
    records: VecDeque<Record>,
    bytes: usize,
    next: u64,
    evicted: u64,
    dropped: Arc<AtomicU64>,
    incomplete: Arc<AtomicBool>,
}

pub struct Store {
    buffers: BTreeMap<String, Buffer>,
    sender: mpsc::Sender<Event>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Batch {
    pub records: Vec<Record>,
    pub dropped: u64,
    pub incomplete: bool,
    pub gap: bool,
    pub latest: u64,
    /// Oldest retained record, independent of the returned page.
    #[serde(default)]
    pub oldest: Option<u64>,
    #[serde(default)]
    pub has_older: bool,
    #[serde(default)]
    pub evicted: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub storage: Option<crate::archive::StorageInfo>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scanned_to: Option<u64>,
    #[serde(default)]
    pub search_complete: bool,
}

pub fn channel(
    ids: impl Iterator<Item = String>,
) -> (BTreeMap<String, Sink>, mpsc::Receiver<Event>, Store) {
    let (sender, receiver) = mpsc::channel(QUEUE_CAPACITY);
    let mut sinks = BTreeMap::new();
    let mut store = Store {
        buffers: BTreeMap::new(),
        sender,
    };
    for id in ids {
        let sink = store.register(&id);
        sinks.insert(id, sink);
    }
    (sinks, receiver, store)
}

impl Store {
    pub fn register(&mut self, id: &str) -> Sink {
        assert!(!self.buffers.contains_key(id), "log stream must be new");
        let dropped = Arc::new(AtomicU64::new(0));
        let incomplete = Arc::new(AtomicBool::new(false));
        self.buffers.insert(
            id.to_owned(),
            Buffer {
                records: VecDeque::new(),
                bytes: 0,
                next: 1,
                evicted: 0,
                dropped: dropped.clone(),
                incomplete: incomplete.clone(),
            },
        );
        Sink {
            service: Arc::from(id),
            instance: 0,
            sender: self.sender.clone(),
            dropped,
            incomplete,
        }
    }

    pub fn push(&mut self, event: Event) -> Option<(String, Record)> {
        let buffer = self.buffers.get_mut(event.service.as_ref())?;
        let mut record = event.record;
        record.sequence = buffer.next;
        buffer.next = buffer.next.saturating_add(1);
        buffer.bytes += record.text.len();
        buffer.records.push_back(record.clone());
        while buffer.bytes > MAX_BUFFER_BYTES || buffer.records.len() > MAX_RECORDS {
            if let Some(record) = buffer.records.pop_front() {
                buffer.bytes -= record.text.len();
                buffer.evicted += 1;
            }
        }
        Some((event.service.to_string(), record))
    }

    pub fn query(
        &self,
        id: &str,
        limit: usize,
        after: Option<u64>,
        before: Option<u64>,
    ) -> Result<Batch, String> {
        self.query_matching(id, limit, after, before, None, None)
    }

    pub fn query_matching(
        &self,
        id: &str,
        limit: usize,
        after: Option<u64>,
        before: Option<u64>,
        needle: Option<&str>,
        upper: Option<u64>,
    ) -> Result<Batch, String> {
        if !(1..=500).contains(&limit) {
            return Err("Log limit must be between 1 and 500.".into());
        }
        if after.is_some() && before.is_some() {
            return Err("Select either after or before, not both.".into());
        }
        let buffer = self
            .buffers
            .get(id)
            .ok_or_else(|| format!("Unknown service '{id}'."))?;
        let mut records = Vec::new();
        let mut bytes = 0;
        let mut append = |record: &Record| {
            if upper.is_some_and(|upper| record.sequence > upper)
                || needle.is_some_and(|needle| !record.text.contains(needle))
            {
                return true;
            }
            if records.len() == limit || bytes + record.text.len() > MAX_QUERY_BYTES {
                return false;
            }
            bytes += record.text.len();
            records.push(record.clone());
            true
        };
        if let Some(after) = after {
            for record in buffer
                .records
                .iter()
                .filter(|record| record.sequence > after)
            {
                if !append(record) {
                    break;
                }
            }
        } else {
            for record in buffer
                .records
                .iter()
                .rev()
                .filter(|record| before.is_none_or(|before| record.sequence < before))
            {
                if !append(record) {
                    break;
                }
            }
            records.reverse();
        }
        let oldest = buffer.records.front().map(|record| record.sequence);
        let has_older = records
            .first()
            .is_some_and(|first| oldest.is_some_and(|oldest| oldest < first.sequence));
        Ok(Batch {
            records,
            evicted: buffer.evicted,
            dropped: buffer
                .evicted
                .saturating_add(buffer.dropped.load(Ordering::Relaxed)),
            incomplete: buffer.incomplete.load(Ordering::Relaxed),
            gap: oldest.is_some_and(|oldest| {
                after.is_some_and(|after| oldest > after.saturating_add(1))
                    || before.is_some_and(|before| before <= oldest && oldest > 1)
            }),
            latest: buffer.next.saturating_sub(1),
            oldest,
            has_older,
            ..Batch::default()
        })
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::AsyncWriteExt;

    fn setup() -> (Sink, mpsc::Receiver<Event>, Store) {
        let (mut sinks, receiver, store) = channel(["web".to_owned()].into_iter());
        (
            sinks.remove("web").unwrap().with_instance(1),
            receiver,
            store,
        )
    }
    fn drain(receiver: &mut mpsc::Receiver<Event>, store: &mut Store) {
        while let Ok(event) = receiver.try_recv() {
            store.push(event);
        }
    }

    #[test]
    fn adding_a_stream_preserves_existing_sequences_and_loss_counters() {
        let (sink, mut receiver, mut store) = setup();
        sink.with_instance(7).emit(Source::Stdout, b"before", false);
        drain(&mut receiver, &mut store);
        let new = store.register("new");
        sink.with_instance(7).emit(Source::Stdout, b"after", false);
        new.with_instance(8)
            .emit(Source::Stderr, b"new output", false);
        drain(&mut receiver, &mut store);
        let old = store.query("web", 10, None, None).unwrap();
        assert_eq!(
            old.records
                .iter()
                .map(|r| (r.sequence, r.instance))
                .collect::<Vec<_>>(),
            [(1, 7), (2, 7)]
        );
        assert_eq!(old.dropped, 0);
        let added = store.query("new", 10, None, None).unwrap();
        assert_eq!(added.records[0].sequence, 1);
        assert_eq!(added.records[0].instance, 8);
        assert_eq!(added.records[0].text, "new output");
    }

    #[test]
    fn sanitization_removes_terminal_controls_and_bounds_utf8_text() {
        let (text, clipped) = sanitize(b"\x1b[31mred\r\xff");
        assert!(!clipped);
        assert!(!text.contains('\x1b') && !text.contains('\r'));
        assert!(text.contains('�') && text.contains("red"));
        let (text, clipped) = sanitize(&vec![0xff; MAX_RECORD_BYTES]);
        assert!(clipped && text.len() <= MAX_RECORD_BYTES);
    }

    #[tokio::test]
    async fn capture_handles_partial_lines_large_unterminated_output_and_eof() {
        let (sink, mut receiver, mut store) = setup();
        let (mut writer, reader) = tokio::io::duplex(1024);
        let capture = tokio::spawn(capture(reader, sink, Source::Stdout));
        writer.write_all(b"\npart").await.unwrap();
        writer.write_all(b"ial\n").await.unwrap();
        writer
            .write_all(&vec![b'x'; MAX_RECORD_BYTES * 2 + 10])
            .await
            .unwrap();
        drop(writer);
        capture.await.unwrap().unwrap();
        drain(&mut receiver, &mut store);
        let records = store.query("web", 100, None, None).unwrap().records;
        assert_eq!(records.len(), 5);
        assert!(records[0].text.is_empty());
        assert_eq!(records[1].text, "partial");
        assert!(records[2..].iter().all(|record| record.truncated));
        assert!(
            records
                .iter()
                .all(|record| record.text.len() <= MAX_RECORD_BYTES)
        );
    }

    #[tokio::test]
    async fn valid_utf8_is_preserved_across_record_boundaries() {
        let (sink, mut receiver, mut store) = setup();
        let (mut writer, reader) = tokio::io::duplex(1024);
        let task = tokio::spawn(capture(reader, sink, Source::Stdout));
        let original = "界".repeat(MAX_RECORD_BYTES);
        writer.write_all(original.as_bytes()).await.unwrap();
        drop(writer);
        task.await.unwrap().unwrap();
        drain(&mut receiver, &mut store);
        let actual: String = store
            .query("web", 100, Some(0), None)
            .unwrap()
            .records
            .iter()
            .map(|record| record.text.as_str())
            .collect();
        assert_eq!(actual, original);
    }

    #[tokio::test]
    async fn reader_errors_and_drain_timeouts_report_incompleteness_without_fake_drop_counts() {
        for stall in [false, true] {
            let (sink, _receiver, store) = setup();
            let mut tasks = JoinSet::new();
            tasks.spawn(async move {
                if stall {
                    std::future::pending::<()>().await;
                }
                Err(std::io::Error::other("injected pipe failure"))
            });
            Readers { tasks, sink }.finish().await;
            let batch = store.query("web", 1, None, None).unwrap();
            assert!(batch.incomplete);
            assert_eq!(batch.dropped, 0);
        }
    }

    #[test]
    fn full_queue_drops_records_without_waiting_and_reports_loss() {
        let (sink, mut receiver, mut store) = setup();
        for _ in 0..QUEUE_CAPACITY + 10 {
            sink.emit(Source::Stdout, b"line", false);
        }
        assert_eq!(receiver.len(), QUEUE_CAPACITY);
        drain(&mut receiver, &mut store);
        let batch = store.query("web", 500, None, None).unwrap();
        assert_eq!(batch.records.len(), QUEUE_CAPACITY);
        assert_eq!(batch.dropped, 10);
    }

    #[test]
    fn ring_bounds_both_bytes_and_record_count_and_reports_cursor_gaps() {
        let (sink, mut receiver, mut store) = setup();
        for _ in 0..200 {
            sink.emit(Source::Stdout, &vec![b'x'; MAX_RECORD_BYTES], false);
            drain(&mut receiver, &mut store);
        }
        let buffer = &store.buffers["web"];
        assert_eq!(buffer.bytes, MAX_BUFFER_BYTES);
        assert_eq!(buffer.records.len(), MAX_BUFFER_BYTES / MAX_RECORD_BYTES);
        assert_eq!(
            store.query("web", 500, None, None).unwrap().records.len(),
            MAX_QUERY_BYTES / MAX_RECORD_BYTES
        );
        assert!(store.query("web", 1, Some(0), None).unwrap().gap);
        for _ in 0..MAX_RECORDS + 10 {
            sink.emit(Source::Stderr, b"", false);
            drain(&mut receiver, &mut store);
        }
        assert_eq!(store.buffers["web"].records.len(), MAX_RECORDS);
        assert_eq!(store.buffers["web"].bytes, 0);
        assert!(store.query("web", 1, Some(0), None).unwrap().dropped > 0);
    }

    #[test]
    fn tail_and_cursor_queries_preserve_instance_labels_and_validate_limits() {
        let (sink, mut receiver, mut store) = setup();
        for instance in 1..=10 {
            sink.with_instance(instance)
                .emit(Source::Stdout, b"entry", false);
            drain(&mut receiver, &mut store);
        }
        let tail = store.query("web", 3, None, None).unwrap();
        assert_eq!(
            tail.records.iter().map(|r| r.instance).collect::<Vec<_>>(),
            [8, 9, 10]
        );
        let page = store.query("web", 3, Some(4), None).unwrap();
        assert_eq!(
            page.records.iter().map(|r| r.sequence).collect::<Vec<_>>(),
            [5, 6, 7]
        );
        assert_eq!(page.latest, 10);
        for limit in [0, 501] {
            assert!(store.query("web", limit, None, None).is_err());
        }
        assert!(store.query("unknown", 1, None, None).is_err());
    }

    #[test]
    fn backward_pages_are_exclusive_ordered_and_stable_as_new_output_arrives() {
        let (sink, mut receiver, mut store) = setup();
        for _ in 0..250 {
            sink.emit(Source::Stdout, b"entry", false);
            drain(&mut receiver, &mut store);
        }
        let tail = store.query("web", 100, None, None).unwrap();
        assert_eq!(tail.records.first().unwrap().sequence, 151);
        assert_eq!(tail.oldest, Some(1));
        assert!(tail.has_older);
        for _ in 0..20 {
            sink.emit(Source::Stderr, b"new", false);
            drain(&mut receiver, &mut store);
        }
        let page = store.query("web", 100, None, Some(151)).unwrap();
        assert_eq!(page.records.first().unwrap().sequence, 51);
        assert_eq!(page.records.last().unwrap().sequence, 150);
        assert_eq!(page.latest, 270);
        assert!(page.has_older);
        let first = store.query("web", 100, None, Some(51)).unwrap();
        assert_eq!(first.records.len(), 50);
        assert_eq!(first.records.first().unwrap().sequence, 1);
        assert!(!first.has_older && !first.gap);
        for before in [0, 1] {
            let empty = store.query("web", 100, None, Some(before)).unwrap();
            assert!(empty.records.is_empty() && !empty.gap && !empty.has_older);
        }
        assert!(store.query("web", 1, Some(1), Some(2)).is_err());
    }

    #[test]
    fn backward_queries_bound_bytes_and_report_expired_cursors_instead_of_empty_history() {
        let (sink, mut receiver, mut store) = setup();
        for _ in 0..200 {
            sink.emit(Source::Stdout, &vec![b'x'; MAX_RECORD_BYTES], false);
            drain(&mut receiver, &mut store);
        }
        let page = store.query("web", 500, None, Some(150)).unwrap();
        assert_eq!(page.records.len(), MAX_QUERY_BYTES / MAX_RECORD_BYTES);
        assert_eq!(page.records.first().unwrap().sequence, 146);
        assert_eq!(page.records.last().unwrap().sequence, 149);
        assert!(page.has_older);
        let expired = store.query("web", 100, None, Some(50)).unwrap();
        assert!(expired.records.is_empty() && expired.gap);
        assert_eq!(expired.oldest, Some(73));
        let boundary = store.query("web", 100, None, Some(73)).unwrap();
        assert!(boundary.records.is_empty() && boundary.gap);
        let bounds = store.query("web", 1, Some(u64::MAX), None).unwrap();
        assert!(bounds.records.is_empty() && !bounds.gap);
        assert_eq!(bounds.latest, 200);
        let (_, _, empty) = setup();
        let page = empty.query("web", 1, None, Some(10)).unwrap();
        assert!(page.records.is_empty() && page.oldest.is_none() && !page.gap);
    }
}
