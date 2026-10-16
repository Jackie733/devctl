//! The supervisor owns this worker. Only it mutates archive files; offline readers
//! hold the workspace run lock. Disk I/O never runs on the process-control executor.
use crate::{
    control::Response,
    logs::{self, Batch, Record},
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    os::unix::{
        ffi::OsStrExt,
        fs::{DirBuilderExt, OpenOptionsExt},
    },
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};
use tokio::sync::oneshot;

const SEGMENT_BYTES: u64 = 8 * 1024 * 1024;
const MAX_FRAME: usize = 128 * 1024;
const MAX_FILES: usize = 4096;
const SEARCH_BYTES: u64 = 1024 * 1024;
const DAY_MS: u64 = 86_400_000;

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub enabled: bool,
    pub max_mib: u32,
    pub retain_days: u16,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            max_mib: 256,
            retain_days: 7,
        }
    }
}
impl Settings {
    pub fn validate(&self) -> Result<(), String> {
        if !(8..=4096).contains(&self.max_mib) || !(1..=365).contains(&self.retain_days) {
            return Err("logs.max_mib must be 8..4096 and logs.retain_days must be 1..365.".into());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageInfo {
    pub session: String,
    pub directory: String,
    pub state: String,
    pub warning: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub service: String,
    pub limit: usize,
    pub after: Option<u64>,
    pub before: Option<u64>,
    pub session: Option<String>,
    pub contains: Option<String>,
    pub upper: Option<u64>,
}
impl Query {
    pub fn validate(&self) -> Result<(), String> {
        if !(1..=500).contains(&self.limit) || (self.after.is_some() && self.before.is_some()) {
            return Err("Log limit must be 1..500; select either after or before.".into());
        }
        if self
            .contains
            .as_ref()
            .is_some_and(|s| s.is_empty() || s.len() > 256)
        {
            return Err("Search text must contain 1..256 UTF-8 bytes.".into());
        }
        if self
            .session
            .as_ref()
            .is_some_and(|s| s.len() != 32 || !s.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err("Invalid log session ID.".into());
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub created_ms: u64,
    pub complete: bool,
    #[serde(default)]
    pub incomplete: bool,
    #[serde(default)]
    pub services: BTreeMap<String, u64>,
}
#[derive(Serialize, Deserialize)]
struct Entry {
    version: u32,
    service: String,
    record: Record,
}
#[derive(Clone)]
struct Segment {
    path: PathBuf,
    session: String,
    service_key: String,
    first: u64,
    created_ms: u64,
    size: u64,
}

fn digest(bytes: &[u8]) -> String {
    let mut value = 0xcbf29ce484222325u64;
    for byte in bytes {
        value = (value ^ u64::from(*byte)).wrapping_mul(0x100000001b3);
    }
    format!("{value:016x}")
}
pub fn directory(config: &Path) -> Result<PathBuf, String> {
    let root = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .map(|p| p.join(".local/state"))
        })
        .ok_or("Cannot locate log state directory: set HOME or an absolute XDG_STATE_HOME.")?;
    Ok(root
        .join("devctl/logs")
        .join(digest(config.as_os_str().as_bytes())))
}
fn io_error(error: impl std::fmt::Display) -> String {
    format!("Log storage: {error}")
}
fn private_dir(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() && !meta.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(io::Error::other("log directory is not a real directory")),
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                private_dir(parent)?;
            }
            fs::DirBuilder::new().mode(0o700).create(path)
        }
        Err(e) => Err(e),
    }
}
fn open_private(path: &Path, create: bool) -> io::Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(create)
        .create_new(create)
        .mode(0o600)
        .custom_flags(nix::libc::O_NOFOLLOW | nix::libc::O_NONBLOCK)
        .open(path)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("not a regular log file"));
    }
    Ok(file)
}
fn read_small<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, String> {
    let mut value = String::new();
    open_private(path, false)
        .map_err(io_error)?
        .take((MAX_FRAME + 1) as u64)
        .read_to_string(&mut value)
        .map_err(io_error)?;
    if value.len() > MAX_FRAME {
        return Err("Log metadata exceeds size limit.".into());
    }
    toml::from_str(&value).map_err(io_error)
}
fn save_meta(dir: &Path, session: &Session) -> Result<(), String> {
    let temporary = dir.join(format!("{}.tmp", session.id));
    // Only our own metadata temporary, under the exclusively owned directory.
    if temporary.exists() {
        fs::remove_file(&temporary).map_err(io_error)?;
    }
    let mut file = open_private(&temporary, true).map_err(io_error)?;
    file.write_all(toml::to_string(session).map_err(io_error)?.as_bytes())
        .map_err(io_error)?;
    file.sync_all().map_err(io_error)?;
    fs::rename(temporary, dir.join(format!("{}.toml", session.id))).map_err(io_error)
}
fn segments(dir: &Path, settings: &Settings) -> Result<Vec<Segment>, String> {
    let mut result = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(result),
        Err(e) => return Err(io_error(e)),
    };
    let cutoff = if settings.retain_days == 0 {
        0
    } else {
        logs::now_ms().saturating_sub(u64::from(settings.retain_days) * DAY_MS)
    };
    let mut count = 0;
    for entry in entries {
        let entry = entry.map_err(io_error)?;
        count += 1;
        if count > MAX_FILES * 2 + 4 {
            return Err("Log archive has too many files.".into());
        }
        let name = entry.file_name();
        let Some(name) = name.to_str().and_then(|s| s.strip_suffix(".dlog")) else {
            continue;
        };
        let parts: Vec<_> = name.split('-').collect();
        if parts.len() != 4 || parts[1].len() != 32 || parts[2].len() != 16 {
            continue;
        }
        let (Ok(created_ms), Ok(first)) = (parts[0].parse::<u64>(), parts[3].parse::<u64>()) else {
            continue;
        };
        if created_ms < cutoff {
            continue;
        }
        let meta = entry.path().symlink_metadata().map_err(io_error)?;
        if !meta.is_file()
            || meta.file_type().is_symlink()
            || meta.len() > SEGMENT_BYTES + MAX_FRAME as u64
        {
            return Err("Invalid log segment.".into());
        }
        result.push(Segment {
            path: entry.path(),
            session: parts[1].into(),
            service_key: parts[2].into(),
            first,
            created_ms,
            size: meta.len(),
        });
        if result.len() > MAX_FILES {
            return Err("Log archive has too many segments.".into());
        }
    }
    result.sort_by(|a, b| (a.created_ms, &a.path).cmp(&(b.created_ms, &b.path)));
    Ok(result)
}
fn verify(dir: &Path, config: &Path) -> Result<(), String> {
    let saved: BTreeMap<String, String> = read_small(&dir.join("workspace.toml"))?;
    if saved.get("path")
        != Some(
            &config
                .as_os_str()
                .as_bytes()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
        )
    {
        return Err("Log workspace identity mismatch.".into());
    }
    Ok(())
}
pub fn sessions(dir: &Path, config: &Path, settings: &Settings) -> Result<Vec<Session>, String> {
    if !dir.exists() {
        return Ok(Vec::new());
    }
    verify(dir, config)?;
    let retained = segments(dir, settings)?;
    let mut result = Vec::new();
    for segment in retained.iter().rev() {
        if result.iter().any(|s: &Session| s.id == segment.session) {
            continue;
        }
        result.push(read_small::<Session>(
            &dir.join(format!("{}.toml", segment.session)),
        )?);
        if result.len() == 100 {
            break;
        }
    }
    result.sort_by_key(|s| std::cmp::Reverse(s.created_ms));
    Ok(result)
}

struct Disk {
    dir: PathBuf,
    settings: Settings,
    session: Session,
    active: BTreeMap<String, (PathBuf, File, u64)>,
    warning: Option<String>,
    segment_bytes: u64,
    last_cleanup: Instant,
    used: u64,
}
impl Disk {
    fn new(
        config: &Path,
        dir: PathBuf,
        settings: Settings,
        session: String,
    ) -> Result<Self, String> {
        private_dir(&dir).map_err(io_error)?;
        if dir.join("workspace.toml").exists() {
            verify(&dir, config)?;
        } else {
            let mut file = open_private(&dir.join("workspace.toml"), true).map_err(io_error)?;
            let value = BTreeMap::from([(
                "path",
                config
                    .as_os_str()
                    .as_bytes()
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<String>(),
            )]);
            file.write_all(toml::to_string(&value).map_err(io_error)?.as_bytes())
                .map_err(io_error)?;
        }
        let mut disk = Self {
            dir,
            settings,
            session: Session {
                id: session,
                created_ms: logs::now_ms(),
                complete: false,
                incomplete: false,
                services: BTreeMap::new(),
            },
            active: BTreeMap::new(),
            warning: None,
            segment_bytes: SEGMENT_BYTES,
            last_cleanup: Instant::now(),
            used: 0,
        };
        disk.cleanup(0)?;
        Ok(disk)
    }
    fn all_segments(&self) -> Result<Vec<Segment>, String> {
        // Include expired files so physical cleanup can remove them.
        segments(
            &self.dir,
            &Settings {
                retain_days: 0,
                ..self.settings.clone()
            },
        )
    }
    fn cleanup(&mut self, reserve: u64) -> Result<(), String> {
        let mut files = self.all_segments()?;
        let mut total: u64 = files.iter().map(|s| s.size).sum();
        let cap = u64::from(self.settings.max_mib) * 1024 * 1024;
        let cutoff = logs::now_ms().saturating_sub(u64::from(self.settings.retain_days) * DAY_MS);
        let mut remaining = files.len();
        for segment in &files {
            if segment.created_ms >= cutoff
                && total.saturating_add(reserve) <= cap
                && remaining < MAX_FILES
            {
                continue;
            }
            self.active.retain(|_, (path, _, _)| *path != segment.path);
            fs::remove_file(&segment.path).map_err(io_error)?;
            total = total.saturating_sub(segment.size);
            remaining -= 1;
        }
        files.retain(|s| s.path.exists());
        // Metadata is bounded by retained data, never by the number of launches.
        for entry in fs::read_dir(&self.dir).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let id = name
                .strip_suffix(".toml")
                .or_else(|| name.strip_suffix(".tmp"));
            if let Some(id) = id
                && id.len() == 32
                && !files.iter().any(|s| s.session == id)
            {
                fs::remove_file(entry.path()).map_err(io_error)?;
            }
        }
        self.used = total;
        self.last_cleanup = Instant::now();
        Ok(())
    }
    fn write(&mut self, service: String, record: Record) -> Result<(), String> {
        let bytes = toml::to_string(&Entry {
            version: 1,
            service: service.clone(),
            record: record.clone(),
        })
        .map_err(io_error)?
        .into_bytes();
        if bytes.len() > MAX_FRAME {
            return Err("Serialized log record exceeds limit.".into());
        }
        let length = bytes.len() as u64 + 8;
        let key = digest(service.as_bytes());
        if self
            .session
            .services
            .keys()
            .any(|id| *id != service && digest(id.as_bytes()) == key)
        {
            return Err("Log service key collision; saving stopped.".into());
        }
        if self
            .active
            .get(&key)
            .is_some_and(|(_, _, size)| size + length > self.segment_bytes)
            && let Some((_, file, _)) = self.active.remove(&key)
        {
            file.sync_data().map_err(io_error)?;
        }
        if self.used.saturating_add(length) > u64::from(self.settings.max_mib) * 1024 * 1024
            || self.last_cleanup.elapsed() >= Duration::from_secs(60)
        {
            self.cleanup(length)?;
        }
        if !self.active.contains_key(&key) {
            self.cleanup(length)?;
            let path = self.dir.join(format!(
                "{:020}-{}-{key}-{:020}.dlog",
                logs::now_ms(),
                self.session.id,
                record.sequence
            ));
            let file = open_private(&path, true).map_err(io_error)?;
            self.active.insert(key.clone(), (path, file, 0));
        }
        self.session.services.insert(service, record.sequence);
        if !self.dir.join(format!("{}.toml", self.session.id)).exists() {
            save_meta(&self.dir, &self.session)?;
        }
        let (_, file, size) = self.active.get_mut(&key).expect("Active segment exists");
        let frame = (bytes.len() as u32).to_be_bytes();
        file.write_all(&frame)
            .and_then(|()| file.write_all(&bytes))
            .and_then(|()| file.write_all(&frame))
            .map_err(io_error)?;
        *size += length;
        self.used += length;
        Ok(())
    }
    fn checkpoint(&mut self, complete: bool, incomplete: bool) -> Result<(), String> {
        self.session.complete = complete;
        self.session.incomplete |= incomplete || self.warning.is_some();
        for (_, file, _) in self.active.values_mut() {
            file.sync_data().map_err(io_error)?;
        }
        if !self.session.services.is_empty()
            && self
                .all_segments()?
                .iter()
                .any(|s| s.session == self.session.id)
        {
            save_meta(&self.dir, &self.session)?;
        }
        if self.last_cleanup.elapsed() >= Duration::from_secs(60) {
            self.cleanup(0)?;
        }
        if complete {
            File::open(&self.dir)
                .and_then(|file| file.sync_all())
                .map_err(io_error)?;
        }
        Ok(())
    }
}

// Every frame has a leading and trailing length. A reader can move either way
// without loading a segment or building an unbounded record-offset index.
fn frame(
    file: &mut File,
    position: &mut u64,
    end: u64,
    reverse: bool,
) -> Result<Option<Entry>, String> {
    if (!reverse && *position >= end) || (reverse && *position == 0) {
        return Ok(None);
    }
    if reverse {
        if *position < 8 {
            return Err("Incomplete log segment tail.".into());
        }
        file.seek(SeekFrom::Start(*position - 4))
            .map_err(io_error)?;
    } else {
        file.seek(SeekFrom::Start(*position)).map_err(io_error)?;
    }
    let mut len = [0; 4];
    file.read_exact(&mut len).map_err(io_error)?;
    let count = u32::from_be_bytes(len) as usize;
    if count == 0 || count > MAX_FRAME {
        return Err("Invalid log frame length.".into());
    }
    let start = if reverse {
        position
            .checked_sub(count as u64 + 8)
            .ok_or("Incomplete log segment tail.")?
    } else {
        *position
    };
    if start + count as u64 + 8 > end {
        return Err("Incomplete log segment tail.".into());
    }
    file.seek(SeekFrom::Start(start)).map_err(io_error)?;
    let mut leading = [0; 4];
    file.read_exact(&mut leading).map_err(io_error)?;
    let mut bytes = vec![0; count];
    file.read_exact(&mut bytes).map_err(io_error)?;
    let mut trailing = [0; 4];
    file.read_exact(&mut trailing).map_err(io_error)?;
    if leading != len || trailing != len {
        return Err("Invalid log frame boundary.".into());
    }
    let entry: Entry =
        toml::from_str(std::str::from_utf8(&bytes).map_err(io_error)?).map_err(io_error)?;
    if entry.version != 1
        || entry.record.text.len() > logs::MAX_RECORD_BYTES
        || entry.service.len() > MAX_FRAME / 2
    {
        return Err("Invalid stored log record.".into());
    }
    *position = if reverse {
        start
    } else {
        start + count as u64 + 8
    };
    Ok(Some(entry))
}

/// Recover a complete prefix after an interrupted append. Never repair a file in
/// a reader; intact frames remain available even when the final append tore.
fn segment_end(file: &mut File, size: u64) -> Result<(u64, Option<u64>, bool), String> {
    let mut position = size;
    if let Ok(Some(entry)) = frame(file, &mut position, size, true) {
        return Ok((size, Some(entry.record.sequence), false));
    }
    let mut position = 0;
    let mut latest = None;
    loop {
        let start = position;
        match frame(file, &mut position, size, false) {
            Ok(Some(entry)) => latest = Some(entry.record.sequence),
            Ok(None) => return Ok((position, latest, false)),
            Err(_) => return Ok((start, latest, true)),
        }
    }
}

fn read_page(
    dir: &Path,
    settings: &Settings,
    query: &Query,
    mut memory: Batch,
    mut info: StorageInfo,
) -> Result<Batch, String> {
    query.validate()?;
    let key = digest(query.service.as_bytes());
    let mut files: Vec<_> = segments(dir, settings)?
        .into_iter()
        .filter(|s| s.session == info.session && s.service_key == key)
        .collect();
    files.sort_by_key(|s| s.first);
    let disk_oldest = files.first().map(|s| s.first);
    if let Some(last) = files.last() {
        let mut file = open_private(&last.path, false).map_err(io_error)?;
        let (_, latest, torn) = segment_end(&mut file, last.size)?;
        memory.latest = memory.latest.max(latest.unwrap_or(0));
        memory.incomplete |= torn;
    }
    let reverse = query.after.is_none();
    let upper = query.upper.unwrap_or(memory.latest);
    let eligible = |r: &Record| {
        r.sequence <= upper
            && query.after.is_none_or(|a| r.sequence > a)
            && query.before.is_none_or(|b| r.sequence < b)
    };
    let mut candidates: BTreeMap<u64, Record> = memory
        .records
        .drain(..)
        .filter(&eligible)
        .map(|r| (r.sequence, r))
        .collect();
    // Skip entire segments preceding the cursor. Within a segment the maximum
    // extra scan is 8 MiB; no offset index grows with the number of records.
    let mut ranges: Vec<_> = files
        .iter()
        .enumerate()
        .filter(|(index, s)| {
            query.after.is_none_or(|a| a < upper)
                && !query.before.is_some_and(|b| s.first >= b)
                && s.first <= upper
                && !query.after.is_some_and(|a| {
                    files
                        .get(index + 1)
                        .is_some_and(|next| next.first <= a.saturating_add(1))
                })
        })
        .map(|(_, s)| s.clone())
        .collect();
    if reverse {
        ranges.reverse();
    }
    let mut scanned = 0;
    let mut scan_cursor = None;
    let mut complete = true;
    let mut count = 0;
    let mut text_bytes = 0;
    'segments: for segment in ranges {
        let mut file = open_private(&segment.path, false).map_err(io_error)?;
        let (end, segment_latest, torn) = segment_end(&mut file, segment.size)?;
        memory.incomplete |= torn;
        let mut position = if reverse { end } else { 0 };
        // Live forward polling usually reads near the file tail. Locate its
        // lower cursor backwards when that is nearer than the segment start.
        if !reverse
            && query.contains.is_none()
            && let (Some(after), Some(last)) = (query.after, segment_latest)
            && after
                > segment
                    .first
                    .saturating_add(last.saturating_sub(segment.first) / 2)
        {
            position = end;
            loop {
                let previous = position;
                match frame(&mut file, &mut position, end, true) {
                    Ok(Some(entry)) if entry.record.sequence > after => {}
                    Ok(Some(_)) => {
                        position = previous;
                        break;
                    }
                    Ok(None) => break,
                    Err(_) => {
                        memory.incomplete = true;
                        position = 0;
                        break;
                    }
                }
            }
        }
        loop {
            let previous = position;
            let entry = match frame(&mut file, &mut position, end, reverse) {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(_) => {
                    memory.incomplete = true;
                    break;
                }
            };
            if entry.service != query.service {
                return Err("Log service identity mismatch.".into());
            }
            let record = entry.record;
            if record.sequence < segment.first {
                return Err("Invalid log sequence.".into());
            }
            if !eligible(&record) {
                continue;
            }
            scanned += position.abs_diff(previous);
            let matches = query
                .contains
                .as_ref()
                .is_none_or(|needle| record.text.contains(needle));
            if matches
                && (count == query.limit || text_bytes + record.text.len() > logs::MAX_QUERY_BYTES)
            {
                complete = false;
                break 'segments;
            }
            scan_cursor = Some(record.sequence);
            if matches {
                count += 1;
                text_bytes += record.text.len();
                candidates.entry(record.sequence).or_insert(record);
            }
            if query.contains.is_some() && scanned >= SEARCH_BYTES {
                complete = false;
                break 'segments;
            }
        }
    }
    // A memory hit beyond the unscanned disk range must not jump over an earlier
    // hit. A continuation request resumes strictly beyond the last examined seq.
    if !complete && let Some(cursor) = scan_cursor {
        candidates.retain(|sequence, _| {
            if reverse {
                *sequence >= cursor
            } else {
                *sequence <= cursor
            }
        });
    }
    let mut records: Vec<_> = candidates
        .into_values()
        .filter(|r| {
            query
                .contains
                .as_ref()
                .is_none_or(|needle| r.text.contains(needle))
        })
        .collect();
    if reverse {
        records.reverse();
    }
    let mut bytes = 0;
    let mut page = Vec::new();
    for record in records {
        if page.len() == query.limit || bytes + record.text.len() > logs::MAX_QUERY_BYTES {
            complete = false;
            scan_cursor = page.last().map(|r: &Record| r.sequence);
            break;
        }
        bytes += record.text.len();
        page.push(record);
    }
    if reverse {
        page.reverse();
    }
    memory.oldest = match (disk_oldest, memory.oldest) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    };
    memory.gap = memory.oldest.is_some_and(|oldest| {
        query.after.is_some_and(|a| oldest > a.saturating_add(1))
            || query.before.is_some_and(|b| b <= oldest && oldest > 1)
    });
    memory.gap |= query.contains.is_none()
        && page
            .windows(2)
            .any(|pair| pair[1].sequence != pair[0].sequence.saturating_add(1));
    if disk_oldest.is_some() {
        memory.dropped = memory.dropped.saturating_sub(memory.evicted);
    }
    memory.has_older = page
        .first()
        .is_some_and(|r| memory.oldest.is_some_and(|oldest| oldest < r.sequence));
    memory.scanned_to = if query.contains.is_some() {
        scan_cursor.or_else(|| if reverse { page.first() } else { page.last() }.map(|r| r.sequence))
    } else {
        None
    };
    memory.search_complete = complete;
    memory.records = page;
    if memory.incomplete && info.warning.is_empty() {
        info.warning = "Saved output may be incomplete (capture loss or interrupted write).".into();
    }
    memory.storage = Some(info);
    Ok(memory)
}

pub fn offline(config: &Path, settings: &Settings, query: &Query) -> Result<Batch, String> {
    query.validate()?;
    let dir = directory(config)?;
    verify(&dir, config)?;
    let retained = segments(&dir, settings)?;
    let selected_segment = retained
        .iter()
        .rev()
        .find(|s| {
            s.service_key == digest(query.service.as_bytes())
                && query.session.as_ref().is_none_or(|id| s.session == *id)
        })
        .ok_or("No retained log session for this service (history may have expired).")?;
    let selected: Session = read_small(&dir.join(format!("{}.toml", selected_segment.session)))?;
    let latest = selected.services.get(&query.service).copied().unwrap_or(0);
    read_page(
        &dir,
        settings,
        query,
        Batch {
            latest,
            incomplete: !selected.complete || selected.incomplete,
            ..Batch::default()
        },
        StorageInfo {
            session: selected.id.clone(),
            directory: dir.display().to_string(),
            state: "saved".into(),
            warning: String::new(),
        },
    )
}

enum Work {
    Record(String, Record),
    Query(Query, Box<Batch>, oneshot::Sender<Response>),
    Sessions(oneshot::Sender<Response>),
    Finish(bool, oneshot::Sender<Result<(), String>>),
}
pub struct Archive {
    sender: mpsc::SyncSender<Work>,
    lost: Arc<AtomicU64>,
    enabled: bool,
}
impl Archive {
    pub fn start(
        config: &Path,
        settings: Settings,
        session: String,
        owner: Arc<crate::run_lock::RunLock>,
    ) -> Result<Self, String> {
        let enabled = settings.enabled;
        let (sender, receiver) = mpsc::sync_channel(256);
        let lost = Arc::new(AtomicU64::new(0));
        let mut info = StorageInfo {
            session: session.clone(),
            directory: directory(config)
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
            state: if settings.enabled {
                "saving"
            } else {
                "disabled"
            }
            .into(),
            warning: String::new(),
        };
        let config = config.to_owned();
        let worker_lost = lost.clone();
        std::thread::Builder::new()
            .name("devctl-logs".into())
            .spawn(move || {
                let _owner = owner;
                let mut disk = if settings.enabled {
                    directory(&config)
                        .and_then(|dir| Disk::new(&config, dir, settings.clone(), session))
                        .map(Some)
                } else {
                    Ok(None)
                };
                if let Err(error) = &disk {
                    info.state = "degraded".into();
                    info.warning = error.clone();
                }
                let mut last_sync = Instant::now();
                loop {
                    let work = match receiver.recv_timeout(Duration::from_secs(1)) {
                        Ok(work) => Some(work),
                        Err(mpsc::RecvTimeoutError::Timeout) => None,
                        Err(_) => break,
                    };
                    let mut failure = None;
                    match work {
                        Some(Work::Record(service, record)) => {
                            if let Ok(Some(disk)) = &mut disk
                                && disk.warning.is_none()
                            {
                                failure = disk.write(service, record).err();
                            }
                        }
                        Some(Work::Query(query, memory, reply)) => {
                            let memory = *memory;
                            if reply.is_closed() {
                                continue;
                            }
                            let mut query_info = info.clone();
                            if worker_lost.load(Ordering::Relaxed) > 0 {
                                query_info.state = "degraded".into();
                                query_info.warning = format!(
                                    "{} records not saved: storage queue overloaded.",
                                    worker_lost.load(Ordering::Relaxed)
                                );
                            }
                            let result = match &disk {
                                _ if query.session.as_ref().is_some_and(|s| *s != info.session) => {
                                    offline(&config,&settings,&query)
                                }
                                Ok(Some(disk)) => read_page(
                                    &disk.dir,
                                    &settings,
                                    &query,
                                    memory.clone(),
                                    query_info.clone(),
                                )
                                .or_else(|error| {
                                    let mut batch = memory;
                                    query_info.state = "degraded".into();
                                    query_info.warning = error;
                                    batch.storage = Some(query_info);
                                    batch.incomplete = true;
                                    Ok(batch)
                                }),
                                _ => {
                                    let mut batch = memory;
                                    batch.storage = Some(query_info);
                                    batch.scanned_to =
                                        if query.before.is_some() || query.after.is_none() {
                                            batch.records.first()
                                        } else {
                                            batch.records.last()
                                        }
                                        .map(|r| r.sequence);
                                    batch.search_complete = batch.records.len() < query.limit;
                                    if query.contains.is_some() {
                                        batch.records.retain(|r| {
                                            query
                                                .contains
                                                .as_ref()
                                                .is_none_or(|n| r.text.contains(n))
                                        });
                                    }
                                    Ok(batch)
                                }
                            };
                            let _ = reply.send(match result {
                                Ok(logs) => Response {
                                    logs: Some(logs),
                                    ..Response::default()
                                },
                                Err(e) => Response::failed(e),
                            });
                        }
                        Some(Work::Sessions(reply)) => {
                            if let Ok(Some(disk)) = &mut disk {
                                failure = disk
                                    .checkpoint(false, worker_lost.load(Ordering::Relaxed) > 0)
                                    .err();
                            }
                            let result = directory(&config)
                                .and_then(|dir| sessions(&dir, &config, &settings));
                            let _ = reply.send(match result {
                                Ok(sessions) => Response {
                                    sessions,
                                    ..Response::default()
                                },
                                Err(e) => Response::failed(e),
                            });
                        }
                        Some(Work::Finish(incomplete, reply)) => {
                            let result = match &mut disk {
                                Ok(Some(disk)) => disk
                                    .checkpoint(
                                        true,
                                        incomplete || worker_lost.load(Ordering::Relaxed) > 0,
                                    )
                                    .and_then(|()| disk.warning.clone().map_or(Ok(()), Err))
                                    .and_then(|()| if worker_lost.load(Ordering::Relaxed) > 0 {Err(format!("{} log records were not saved: storage queue overloaded.",worker_lost.load(Ordering::Relaxed)))} else {Ok(())}),
                                Ok(None) => Ok(()),
                                Err(e) => Err(e.clone()),
                            };
                            let _ = reply.send(result);
                            break;
                        }
                        None => {}
                    }
                    if last_sync.elapsed() >= Duration::from_secs(1) {
                        if let Ok(Some(disk)) = &mut disk {
                            failure = failure.or_else(|| {
                                disk.checkpoint(false, worker_lost.load(Ordering::Relaxed) > 0)
                                    .err()
                            });
                        }
                        last_sync = Instant::now();
                    }
                    if let Some(error) = failure {
                        if let Ok(Some(disk)) = &mut disk {
                            disk.warning = Some(error.clone());
                            disk.session.incomplete = true;
                        }
                        info.state = "degraded".into();
                        info.warning = error;
                    }
                }
            })
            .map_err(|e| format!("Cannot start log storage worker: {e}"))?;
        Ok(Self {
            sender,
            lost,
            enabled,
        })
    }
    pub fn push(&self, service: String, record: Record) {
        if !self.enabled {
            return;
        }
        if self.sender.try_send(Work::Record(service, record)).is_err() {
            self.lost.fetch_add(1, Ordering::Relaxed);
        }
    }
    pub fn query(&self, query: Query, memory: Batch, reply: oneshot::Sender<Response>) {
        if let Err(error) = query.validate() {
            let _ = reply.send(Response::failed(error));
            return;
        }
        if let Err(
            mpsc::TrySendError::Full(Work::Query(_, _, reply))
            | mpsc::TrySendError::Disconnected(Work::Query(_, _, reply)),
        ) = self
            .sender
            .try_send(Work::Query(query, Box::new(memory), reply))
        {
            let _ = reply.send(Response::failed(
                "Log storage busy or unavailable; retry query.",
            ));
        }
    }
    pub fn list(&self, reply: oneshot::Sender<Response>) {
        if let Err(
            mpsc::TrySendError::Full(Work::Sessions(reply))
            | mpsc::TrySendError::Disconnected(Work::Sessions(reply)),
        ) = self.sender.try_send(Work::Sessions(reply))
        {
            let _ = reply.send(Response::failed(
                "Log storage busy or unavailable; retry session listing.",
            ));
        }
    }
    pub async fn finish(self, incomplete: bool) -> Result<(), String> {
        let (send, recv) = oneshot::channel();
        let mut work = Work::Finish(incomplete, send);
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                match self.sender.try_send(work) {
                    Ok(()) => break,
                    Err(mpsc::TrySendError::Full(pending)) => {
                        work = pending;
                        tokio::time::sleep(Duration::from_millis(10)).await;
                    }
                    Err(mpsc::TrySendError::Disconnected(_)) => {
                        return Err("Log storage worker stopped.".into());
                    }
                }
            }
            recv.await.map_err(io_error)?
        })
        .await
        .map_err(|_| "Log storage shutdown timed out; saved tail unconfirmed.".to_string())?
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture {
        root: PathBuf,
        config: PathBuf,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "devctl-archive-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            let config = root.join("devctl.toml");
            fs::write(&config, "version=1").unwrap();
            Self { root, config }
        }
        fn disk(&self, session: u8) -> Disk {
            Disk::new(
                &self.config,
                self.root.join("archive"),
                Settings::default(),
                format!("{session:032x}"),
            )
            .unwrap()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    fn record(sequence: u64, text: &str) -> Record {
        Record {
            captured_ms: logs::now_ms(),
            sequence,
            instance: 2,
            source: logs::Source::Stderr,
            text: text.into(),
            truncated: false,
        }
    }
    fn query() -> Query {
        Query {
            service: "web".into(),
            limit: 100,
            after: None,
            before: None,
            session: None,
            contains: None,
            upper: None,
        }
    }
    fn page(disk: &Disk, query: &Query, memory: Batch) -> Batch {
        read_page(
            &disk.dir,
            &disk.settings,
            query,
            memory,
            StorageInfo {
                session: disk.session.id.clone(),
                ..StorageInfo::default()
            },
        )
        .unwrap()
    }
    fn sequences(batch: &Batch) -> Vec<u64> {
        batch.records.iter().map(|r| r.sequence).collect()
    }

    #[test]
    fn disk_history_pages_merge_memory_without_duplicate_or_false_eviction_loss() {
        let fixture = Fixture::new();
        let mut disk = fixture.disk(1);
        disk.segment_bytes = 1024;
        for seq in 1..=35 {
            disk.write("web".into(), record(seq, "日志 error")).unwrap();
        }
        disk.write("weapp".into(), record(1, "other service"))
            .unwrap();
        disk.checkpoint(true, false).unwrap();
        let memory = Batch {
            records: (31..=40).map(|seq| record(seq, "日志 error")).collect(),
            oldest: Some(31),
            latest: 40,
            evicted: 30,
            dropped: 32,
            ..Batch::default()
        };
        let tail = page(
            &disk,
            &Query {
                limit: 15,
                ..query()
            },
            memory.clone(),
        );
        assert_eq!(sequences(&tail), (26..=40).collect::<Vec<_>>());
        assert_eq!(tail.dropped, 2);
        assert_eq!(tail.oldest, Some(1));
        let before = page(
            &disk,
            &Query {
                before: Some(26),
                limit: 7,
                ..query()
            },
            memory.clone(),
        );
        assert_eq!(sequences(&before), (19..=25).collect::<Vec<_>>());
        let after = page(
            &disk,
            &Query {
                after: Some(25),
                limit: 7,
                ..query()
            },
            memory,
        );
        assert_eq!(sequences(&after), (26..=32).collect::<Vec<_>>());
        assert!(after.records.iter().all(|r| r.instance == 2
            && matches!(r.source, logs::Source::Stderr)
            && r.captured_ms > 0));
        let files = segments(&disk.dir, &disk.settings).unwrap();
        assert!(files.len() > 2);
        assert_eq!(
            fs::metadata(&disk.dir).unwrap().permissions().mode() & 0o777,
            0o700
        );
        assert_eq!(
            fs::metadata(&files[0].path).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }

    #[test]
    fn sessions_do_not_mix_even_when_service_sequences_restart() {
        let fixture = Fixture::new();
        let mut first = fixture.disk(1);
        first
            .write("web".into(), record(1, "first session"))
            .unwrap();
        first.checkpoint(true, false).unwrap();
        let mut second = fixture.disk(2);
        second
            .write("web".into(), record(1, "second session"))
            .unwrap();
        second.checkpoint(false, true).unwrap();
        let sessions = sessions(&second.dir, &fixture.config, &second.settings).unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(
            page(&second, &query(), Batch::default()).records[0].text,
            "second session"
        );
        assert_eq!(
            page(&first, &query(), Batch::default()).records[0].text,
            "first session"
        );
        assert!(sessions.iter().any(|s| !s.complete && s.incomplete));
        assert!(verify(&first.dir, &fixture.root.join("other.toml")).is_err());
    }

    #[test]
    fn capacity_and_age_cleanup_bound_segments_and_remove_orphan_metadata() {
        let fixture = Fixture::new();
        let mut disk = fixture.disk(1);
        disk.settings.max_mib = 1;
        disk.segment_bytes = 128 * 1024;
        for seq in 1..=100 {
            disk.write("web".into(), record(seq, &"x".repeat(16 * 1024)))
                .unwrap();
        }
        let retained = segments(&disk.dir, &disk.settings).unwrap();
        assert!(retained.iter().map(|s| s.size).sum::<u64>() <= 1024 * 1024);
        let expired = page(
            &disk,
            &Query {
                after: Some(0),
                ..query()
            },
            Batch::default(),
        );
        assert!(expired.oldest.unwrap() > 1 && expired.gap);
        disk.active.clear();
        for segment in retained {
            let name = format!(
                "{:020}-{}-{}-{:020}.dlog",
                logs::now_ms() - 8 * DAY_MS,
                segment.session,
                segment.service_key,
                segment.first
            );
            fs::rename(segment.path, disk.dir.join(name)).unwrap();
        }
        assert!(segments(&disk.dir, &disk.settings).unwrap().is_empty());
        disk.cleanup(0).unwrap();
        assert_eq!(disk.used, 0);
        assert!(!disk.dir.join(format!("{}.toml", disk.session.id)).exists());
    }

    #[test]
    fn interrupted_tail_preserves_complete_prefix_and_reports_incompleteness() {
        let fixture = Fixture::new();
        let mut disk = fixture.disk(1);
        for seq in 1..=4 {
            disk.write("web".into(), record(seq, "retained")).unwrap();
        }
        disk.checkpoint(false, false).unwrap();
        disk.active.clear();
        let path = &segments(&disk.dir, &disk.settings).unwrap()[0].path;
        OpenOptions::new()
            .append(true)
            .open(path)
            .unwrap()
            .write_all(&[0, 0, 1, 0, b'x'])
            .unwrap();
        let tail = page(&disk, &query(), Batch::default());
        assert_eq!(sequences(&tail), vec![1, 2, 3, 4]);
        assert!(tail.incomplete);
        let after = page(
            &disk,
            &Query {
                after: Some(1),
                ..query()
            },
            Batch::default(),
        );
        assert_eq!(sequences(&after), vec![2, 3, 4]);
        assert!(after.incomplete);
    }

    #[test]
    fn literal_search_scans_beyond_memory_in_bounded_continuations_and_fixes_upper() {
        let fixture = Fixture::new();
        let mut disk = fixture.disk(1);
        disk.segment_bytes = 512 * 1024;
        for seq in 1..=160 {
            disk.write(
                "web".into(),
                record(
                    seq,
                    &format!(
                        "{}{}",
                        if seq == 3 || seq == 150 {
                            "ERROR \\b"
                        } else {
                            "ordinary "
                        },
                        "x".repeat(12 * 1024)
                    ),
                ),
            )
            .unwrap();
        }
        let search = Query {
            after: Some(3),
            contains: Some("ERROR \\b".into()),
            upper: Some(151),
            limit: 1,
            ..query()
        };
        let first = page(&disk, &search, Batch::default());
        assert!(first.records.is_empty() && !first.search_complete);
        assert!(first.scanned_to.unwrap() > 3);
        let second = page(
            &disk,
            &Query {
                after: first.scanned_to,
                ..search.clone()
            },
            Batch::default(),
        );
        assert_eq!(sequences(&second), vec![150]);
        let previous = page(
            &disk,
            &Query {
                after: None,
                before: Some(150),
                ..search.clone()
            },
            Batch::default(),
        );
        assert!(previous.records.is_empty() && !previous.search_complete);
        let previous = page(
            &disk,
            &Query {
                after: None,
                before: previous.scanned_to,
                ..search.clone()
            },
            Batch::default(),
        );
        assert_eq!(sequences(&previous), vec![3]);
        let bounded = page(
            &disk,
            &Query {
                after: Some(3),
                upper: Some(100),
                ..search
            },
            Batch::default(),
        );
        assert!(bounded.records.is_empty());
    }

    #[test]
    fn unsafe_file_and_directory_paths_are_rejected_without_touching_targets() {
        let fixture = Fixture::new();
        let disk = fixture.disk(1);
        let target = fixture.root.join("target");
        fs::write(&target, "keep").unwrap();
        let link = disk.dir.join("link");
        symlink(&target, &link).unwrap();
        assert!(open_private(&link, false).is_err());
        assert!(open_private(&link, true).is_err());
        assert!(private_dir(&link).is_err());
        assert_eq!(fs::read_to_string(target).unwrap(), "keep");
        assert!(
            Query {
                session: Some("../../escape".into()),
                ..query()
            }
            .validate()
            .is_err()
        );
        assert!(
            Query {
                contains: Some(String::new()),
                ..query()
            }
            .validate()
            .is_err()
        );
        assert!(
            Settings {
                max_mib: 0,
                ..Settings::default()
            }
            .validate()
            .is_err()
        );
    }
    #[tokio::test]
    async fn full_storage_queue_drops_without_waiting_and_rejects_queries_explicitly() {
        let (sender, _receiver) = mpsc::sync_channel(1);
        let archive = Archive {
            sender,
            lost: Arc::new(AtomicU64::new(0)),
            enabled: true,
        };
        archive.push("web".into(), record(1, "one"));
        archive.push("web".into(), record(2, "two"));
        assert_eq!(archive.lost.load(Ordering::Relaxed), 1);
        let (send, recv) = oneshot::channel();
        archive.query(query(), Batch::default(), send);
        assert!(recv.await.unwrap().error.unwrap().contains("busy"));
        let disabled = Archive {
            enabled: false,
            ..archive
        };
        disabled.push("web".into(), record(3, "disabled"));
        assert_eq!(disabled.lost.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn metadata_polling_and_near_tail_forward_pages_preserve_bounds_and_order() {
        let fixture = Fixture::new();
        let mut disk = fixture.disk(1);
        for seq in 1..=150 {
            disk.write("web".into(), record(seq, "line")).unwrap();
        }
        let bounds = page(
            &disk,
            &Query {
                after: Some(u64::MAX),
                ..query()
            },
            Batch::default(),
        );
        assert!(bounds.records.is_empty());
        assert_eq!(bounds.latest, 150);
        assert_eq!(bounds.oldest, Some(1));
        assert!(!bounds.gap);
        let near = page(
            &disk,
            &Query {
                after: Some(140),
                limit: 3,
                ..query()
            },
            Batch::default(),
        );
        assert_eq!(sequences(&near), vec![141, 142, 143]);
        let empty = page(
            &disk,
            &Query {
                after: Some(150),
                ..query()
            },
            Batch::default(),
        );
        assert!(empty.records.is_empty());
    }
}
