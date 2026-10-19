//! TUI additions reuse the CLI's reviewed configuration transaction.
use crate::{
    client,
    control::{Request, Response},
    discovery::{self, Report},
    import::Draft,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::{
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

pub enum Step {
    Path,
    Scanning,
    Projects(usize),
    Manager(usize),
    Scripts(usize),
    Command,
    Preparing,
    Review(Box<Draft>),
    Saving,
}

pub struct Dialog {
    pub step: Step,
    pub input: String,
    pub scroll: u16,
    pub error: Option<String>,
    pub edit_target: Option<String>,
    path: PathBuf,
    kind: String,
    note: Option<String>,
    report: Option<Report>,
    projects: Vec<usize>,
    project: usize,
    manager: String,
    scripts: Vec<String>,
    completion: Option<Completion>,
    completing: bool,
    inspect_after_completion: bool,
}

pub enum Work {
    Complete(String),
    Scan(String),
    LoadEdit(String),
    Preview {
        directory: PathBuf,
        arguments: Vec<String>,
        report: Option<Box<Report>>,
        edit_target: Option<String>,
    },
    Save(Box<Draft>),
}
pub enum Finished {
    Completed {
        input: String,
        result: Result<Completion, String>,
    },
    Scanned(Result<Scan, String>),
    LoadedEdit(Result<(Scan, Box<Draft>), String>),
    Previewed(Result<Box<Draft>, String>),
    Saved {
        draft: Box<Draft>,
        response: Option<Response>,
        error: Option<String>,
        committed: bool,
    },
}
pub struct Scan {
    path: PathBuf,
    kind: String,
    note: Option<String>,
    report: Option<Report>,
}
pub enum Action {
    None,
    Close,
    Work(Work),
}

pub struct Completion {
    options: Vec<String>,
    common: String,
    index: Option<usize>,
}

impl Dialog {
    pub fn new() -> Self {
        Self {
            step: Step::Path,
            input: String::new(),
            scroll: 0,
            error: None,
            edit_target: None,
            path: PathBuf::new(),
            kind: String::new(),
            note: None,
            report: None,
            projects: Vec::new(),
            project: 0,
            manager: String::new(),
            scripts: Vec::new(),
            completion: None,
            completing: false,
            inspect_after_completion: false,
        }
    }
    pub fn new_edit(service_id: String) -> Self {
        Self {
            step: Step::Preparing,
            edit_target: Some(service_id),
            ..Self::new()
        }
    }
    pub fn busy(&self) -> bool {
        matches!(self.step, Step::Scanning | Step::Preparing | Step::Saving)
    }
    pub fn text_input(&self) -> bool {
        matches!(self.step, Step::Path | Step::Command)
    }
    pub fn paste(&mut self, text: &str) {
        if self.text_input() {
            self.edited();
            for c in text.chars().filter(|c| !c.is_control()) {
                if self.input.len() + c.len_utf8() > 4096 {
                    break;
                }
                self.input.push(c);
            }
            self.error = None;
        }
    }
    fn edited(&mut self) {
        self.completion = None;
        self.inspect_after_completion = false;
        self.error = None;
    }

    fn complete_key(&mut self, reverse: bool) -> Action {
        if self.completing {
            return Action::None;
        }
        self.error = None;
        if let Some(completion) = &mut self.completion {
            let count = completion.options.len();
            let index = match completion.index {
                None if reverse => count - 1,
                None => 0,
                Some(index) if reverse => (index + count - 1) % count,
                Some(index) => (index + 1) % count,
            };
            completion.index = Some(index);
            self.input = completion.options[index].clone();
            Action::None
        } else {
            self.completing = true;
            Action::Work(Work::Complete(self.input.clone()))
        }
    }

    pub fn completed(&mut self, input: String, result: Result<Completion, String>) -> Action {
        self.completing = false;
        if matches!(self.step, Step::Path) && self.input == input {
            match result {
                Ok(completion) => {
                    self.input = completion.common.clone();
                    self.completion = (completion.options.len() > 1).then_some(completion);
                    self.error = None;
                }
                Err(error) => self.error = Some(error),
            }
        }
        let inspect = std::mem::take(&mut self.inspect_after_completion);
        if inspect && matches!(self.step, Step::Path) && !self.input.trim().is_empty() {
            self.completion = None;
            self.error = None;
            self.step = Step::Scanning;
            Action::Work(Work::Scan(self.input.clone()))
        } else {
            Action::None
        }
    }
    pub fn key(&mut self, key: KeyEvent) -> Action {
        if key.code == KeyCode::Esc {
            if matches!(self.step, Step::Saving) {
                return Action::None;
            }
            if self.busy() {
                return Action::Close;
            }
            if matches!(self.step, Step::Path) {
                return Action::Close;
            }
            self.input = self.path.to_string_lossy().into_owned();
            self.edited();
            self.step = Step::Path;
            self.error = None;
            self.scroll = 0;
            return Action::None;
        }
        if self.busy() {
            return Action::None;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && key.code == KeyCode::Char('u')
            && self.text_input()
        {
            self.input.clear();
            self.edited();
            return Action::None;
        }
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            return Action::None;
        }
        if self.text_input() {
            match key.code {
                KeyCode::Backspace => {
                    self.input.pop();
                    self.edited();
                }
                KeyCode::Char(c) => self.paste(&c.to_string()),
                KeyCode::Tab | KeyCode::BackTab if matches!(self.step, Step::Path) => {
                    return self.complete_key(
                        key.code == KeyCode::BackTab || key.modifiers.contains(KeyModifiers::SHIFT),
                    );
                }
                KeyCode::Enter
                    if !self.input.trim().is_empty()
                        || (matches!(self.step, Step::Path) && self.completing) =>
                {
                    self.error = None;
                    if matches!(self.step, Step::Path) {
                        if self.completing {
                            self.inspect_after_completion = true;
                            return Action::None;
                        }
                        self.completion = None;
                        self.step = Step::Scanning;
                        return Action::Work(Work::Scan(self.input.clone()));
                    }
                    let args = vec!["/bin/sh".into(), "-c".into(), self.input.clone()];
                    return self.preview(self.path.clone(), args, None);
                }
                _ => {}
            }
            return Action::None;
        }
        if key.code == KeyCode::Char('e')
            && let Step::Review(draft) = &self.step
        {
            self.path = draft.path.clone();
            self.input = match draft.arguments.as_slice() {
                [shell, flag, text] if shell == "/bin/sh" && flag == "-c" => text.clone(),
                _ => draft.command_text(),
            };
            self.note = None;
            self.step = Step::Command;
            self.scroll = 0;
            return Action::None;
        }
        let delta = match key.code {
            KeyCode::Down | KeyCode::Char('j') => 1,
            KeyCode::Up | KeyCode::Char('k') => -1,
            _ => 0,
        };
        match &mut self.step {
            Step::Projects(index) => {
                *index = index.saturating_add_signed(delta).min(self.projects.len())
            }
            Step::Manager(index) => *index = index.saturating_add_signed(delta).min(1),
            Step::Scripts(index) => {
                *index = index.saturating_add_signed(delta).min(self.scripts.len())
            }
            Step::Review(_) => match key.code {
                KeyCode::Down | KeyCode::Char('j') => self.scroll = self.scroll.saturating_add(1),
                KeyCode::Up | KeyCode::Char('k') => self.scroll = self.scroll.saturating_sub(1),
                KeyCode::PageDown => self.scroll = self.scroll.saturating_add(8),
                KeyCode::PageUp => self.scroll = self.scroll.saturating_sub(8),
                _ => {}
            },
            _ => {}
        }
        if key.code != KeyCode::Enter {
            return Action::None;
        }
        self.error = None;
        self.scroll = 0;
        match self.step {
            Step::Projects(index) if index == self.projects.len() => self.custom_command(),
            Step::Projects(index) => return self.choose_project(self.projects[index]),
            Step::Manager(index) => {
                self.manager = ["npm", "pnpm"][index].into();
                return self.choose_scripts();
            }
            Step::Scripts(index) if index == self.scripts.len() => self.custom_command(),
            Step::Scripts(index) => {
                let candidate =
                    &self.report.as_ref().expect("selected report").candidates[self.project];
                let args = vec![
                    self.manager.clone(),
                    "run".into(),
                    self.scripts[index].clone(),
                ];
                return self.preview(candidate.path.clone(), args, self.report.clone());
            }
            Step::Review(_) => {
                if let Step::Review(draft) = std::mem::replace(&mut self.step, Step::Saving) {
                    return Action::Work(Work::Save(draft));
                }
            }
            _ => {}
        }
        Action::None
    }
    fn custom_command(&mut self) {
        if let Some(report) = &self.report
            && !self.projects.is_empty()
            && !matches!(self.step, Step::Projects(_))
        {
            self.path = report.candidates[self.project].path.clone();
        }
        self.input.clear();
        self.step = Step::Command;
    }
    fn preview(
        &mut self,
        directory: PathBuf,
        arguments: Vec<String>,
        report: Option<Report>,
    ) -> Action {
        self.step = Step::Preparing;
        Action::Work(Work::Preview {
            directory,
            arguments,
            report: report.map(Box::new),
            edit_target: self.edit_target.clone(),
        })
    }
    fn choose_project(&mut self, project: usize) -> Action {
        self.project = project;
        let candidate = &self.report.as_ref().expect("project report").candidates[project];
        if let Some(manager) = &candidate.manager {
            self.manager = manager.name.clone();
            self.choose_scripts()
        } else {
            self.step = Step::Manager(0);
            Action::None
        }
    }
    fn choose_scripts(&mut self) -> Action {
        let candidate = &self.report.as_ref().expect("script report").candidates[self.project];
        self.scripts = candidate.scripts.keys().cloned().collect();
        self.scripts
            .sort_by_key(|s| (!candidate.suggestions.contains(s), s.clone()));
        if candidate.suggestions.len() == 1 {
            let args = vec![
                self.manager.clone(),
                "run".into(),
                candidate.suggestions[0].clone(),
            ];
            self.preview(candidate.path.clone(), args, self.report.clone())
        } else {
            self.step = Step::Scripts(0);
            Action::None
        }
    }
    pub fn scanned(&mut self, result: Result<Scan, String>) -> Action {
        let scan = match result {
            Ok(scan) => scan,
            Err(error) => {
                self.error = Some(error);
                self.step = Step::Path;
                return Action::None;
            }
        };
        self.path = scan.path;
        self.kind = scan.kind;
        self.note = scan.note;
        self.report = scan.report;
        self.projects = self
            .report
            .as_ref()
            .map(|r| {
                r.candidates
                    .iter()
                    .enumerate()
                    .filter_map(|(i, c)| (!c.scripts.is_empty()).then_some(i))
                    .collect()
            })
            .unwrap_or_default();
        match self.projects.len() {
            0 => self.custom_command(),
            1 => return self.choose_project(self.projects[0]),
            _ => self.step = Step::Projects(0),
        }
        Action::None
    }
    pub fn loaded_edit(&mut self, result: Result<(Scan, Box<Draft>), String>) {
        match result {
            Ok((scan, draft)) => {
                self.path = scan.path;
                self.kind = scan.kind;
                self.note = scan.note;
                self.report = scan.report;
                self.scroll = 0;
                self.step = Step::Review(draft);
            }
            Err(error) => {
                self.error = Some(error);
                self.step = Step::Path;
            }
        }
    }
    pub fn previewed(&mut self, result: Result<Box<Draft>, String>) {
        match result {
            Ok(draft) => {
                self.scroll = 0;
                self.step = Step::Review(draft);
            }
            Err(error) => {
                self.error = Some(error);
                self.input = self.path.to_string_lossy().into_owned();
                self.step = Step::Path;
            }
        }
    }
    pub fn lines(&self, rows: usize) -> Vec<String> {
        let mut lines = Vec::new();
        match &self.step {
            Step::Path => {
                lines.push("Project directory (paste a path; ~/ is supported):".into());
                if self.completing {
                    lines.push("Completing directory... You can keep typing or cancel.".into());
                }
                if let Some(completion) = &self.completion {
                    lines.push(format!(
                        "{} directories | Tab/Shift-Tab: cycle",
                        completion.options.len()
                    ));
                    let count = rows.saturating_sub(6).clamp(1, 6);
                    let start = completion
                        .index
                        .unwrap_or(0)
                        .saturating_sub(count / 2)
                        .min(completion.options.len().saturating_sub(count));
                    for (index, option) in completion
                        .options
                        .iter()
                        .enumerate()
                        .skip(start)
                        .take(count)
                    {
                        let name = path_text(option)
                            .trim_end_matches('/')
                            .rsplit('/')
                            .next()
                            .unwrap_or(option);
                        lines.push(format!(
                            "{} {name}/",
                            if completion.index == Some(index) {
                                ">"
                            } else {
                                " "
                            }
                        ));
                    }
                }
                lines.push(format!("> {}_", input_tail(&self.input)));
                lines.push("Tab: complete | Enter: inspect | Ctrl-U: clear | Esc: cancel".into());
            }
            Step::Scanning => {
                lines.push("Reading project declarations... No commands are executed.".into())
            }
            Step::Preparing => lines.push("Preparing the configuration preview...".into()),
            Step::Saving => {
                if self.edit_target.is_some() {
                    lines.push("Saving and updating the project in this workspace...".into());
                } else {
                    lines.push("Saving and adding the project to this workspace...".into());
                }
            }
            Step::Command => {
                lines.push(format!("Project type: {}", self.kind));
                lines.push(format!("Directory: {}", self.path.display()));
                if let Some(note) = &self.note {
                    lines.push(note.clone());
                }
                lines.push("Enter the command you normally run (executed with /bin/sh):".into());
                lines.push(format!("> {}_", input_tail(&self.input)));
                lines.push("Enter: review | Ctrl-U: clear | Esc: edit path".into());
            }
            Step::Projects(index) => {
                lines.push("Choose a project (root scripts may also start members):".into());
                let report = self.report.as_ref().expect("project report");
                let mut options: Vec<_> = self
                    .projects
                    .iter()
                    .map(|i| {
                        let c = &report.candidates[*i];
                        c.path
                            .strip_prefix(&report.root)
                            .ok()
                            .filter(|p| !p.as_os_str().is_empty())
                            .map_or_else(
                                || ". (repository root)".into(),
                                |p| p.display().to_string(),
                            )
                    })
                    .collect();
                options.push("Enter a custom command for the supplied directory".into());
                choices(&mut lines, &options, *index, rows);
            }
            Step::Manager(index) => {
                lines.push("Choose the package manager command (no version management):".into());
                choices(&mut lines, &["npm".into(), "pnpm".into()], *index, rows);
            }
            Step::Scripts(index) => {
                lines.push("Choose a declared script:".into());
                let c = &self.report.as_ref().expect("script report").candidates[self.project];
                let mut options: Vec<_> = self
                    .scripts
                    .iter()
                    .map(|s| format!("{s}: {}", c.scripts[s]))
                    .collect();
                options.push("Enter a custom command".into());
                choices(&mut lines, &options, *index, rows);
            }
            Step::Review(draft) => {
                if self.edit_target.is_some() {
                    lines.push("Review project update".into());
                } else {
                    lines.push("Review project addition".into());
                }
                lines.push(format!("Project: {}", draft.id));
                lines.push(format!("Directory: {}", draft.path.display()));
                lines.push(format!("Project type: {}", self.kind));
                lines.push(format!("Command: {}", draft.command_text()));
                lines.push(format!("Configuration: {}", draft.target().display()));
                if draft.changes().is_empty() {
                    if self.edit_target.is_some() {
                        lines.push(
                            "No changes: existing command and directory are unchanged.".into(),
                        );
                    } else {
                        lines.push("Already configured: preserve its existing command and edits; nothing to save.".into());
                    }
                }
                if self.edit_target.is_some() {
                    lines.push(
                        "Save only. The updated project stays stopped. Existing services keep running."
                            .into(),
                    );
                } else {
                    lines.push(
                        "Save only. The new project stays stopped. Existing services keep running."
                            .into(),
                    );
                }
                lines.push(
                    "Enter: save | e: edit command | Esc: edit path | Up/Down: scroll".into(),
                );
            }
        }
        if let Some(error) = &self.error {
            let error = format!("Error: {error}");
            if self.text_input() {
                lines.insert(0, error);
            } else {
                lines.push(error);
            }
        }
        lines
    }
}
fn input_tail(input: &str) -> String {
    let chars: Vec<_> = input.chars().collect();
    if chars.len() > 64 {
        format!(
            "...{}",
            chars[chars.len() - 64..].iter().collect::<String>()
        )
    } else {
        input.to_owned()
    }
}

fn choices(lines: &mut Vec<String>, options: &[String], index: usize, rows: usize) {
    let size = rows.saturating_sub(4).max(1);
    let start = index
        .saturating_sub(size / 2)
        .min(options.len().saturating_sub(size));
    lines.extend(
        options
            .iter()
            .enumerate()
            .skip(start)
            .take(size)
            .map(|(i, s)| format!("{} {s}", if i == index { ">" } else { " " })),
    );
    lines.push(format!(
        "{}/{} | Up/Down: choose | Enter: continue | Esc: edit path",
        index + 1,
        options.len()
    ));
}
fn path_text(input: &str) -> &str {
    let text = input.trim();
    if let Some(quote) = text.chars().next().filter(|c| matches!(c, '\'' | '"')) {
        let rest = &text[1..];
        rest.strip_suffix(quote).unwrap_or(rest)
    } else {
        text
    }
}

fn expand_path(text: &str, home: Option<&Path>) -> Result<PathBuf, String> {
    if text == "~" || text.starts_with("~/") {
        Ok(home
            .ok_or("HOME is not set; enter an absolute directory.")?
            .join(text.strip_prefix("~/").unwrap_or("")))
    } else {
        Ok(PathBuf::from(text))
    }
}

/// Inspect one directory only, off the event loop. Never run a shell for completion.
fn complete(input: &str, home: Option<&Path>) -> Result<Completion, String> {
    let text = path_text(input);
    let quote = input
        .trim()
        .chars()
        .next()
        .filter(|c| matches!(c, '\'' | '"'));
    let render = |path: String| match quote {
        Some(quote) => format!("{quote}{path}{quote}"),
        None => path,
    };
    if matches!(text, "~" | "." | "..") {
        let path = expand_path(text, home)?;
        if !path.is_dir() {
            return Err("Cannot complete path: directory is unavailable.".into());
        }
        let path = render(format!("{text}/"));
        return Ok(Completion {
            common: path.clone(),
            options: vec![path],
            index: None,
        });
    }
    let (base, prefix) = text
        .rfind('/')
        .map_or(("", text), |slash| text.split_at(slash + 1));
    let parent = expand_path(if base.is_empty() { "." } else { base }, home)?;
    let entries =
        std::fs::read_dir(parent).map_err(|error| format!("Cannot complete path: {error}"))?;
    let deadline = Instant::now() + Duration::from_millis(250);
    let mut options = Vec::new();
    for (index, entry) in entries.enumerate() {
        if index >= 8192 || Instant::now() >= deadline {
            return Err(
                "Directory is too large or slow to complete; type a more specific path.".into(),
            );
        }
        let entry = entry.map_err(|error| format!("Cannot complete path: {error}"))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.starts_with(prefix)
            || (name.starts_with('.') && !prefix.starts_with('.'))
            || name.chars().any(char::is_control)
        {
            continue;
        }
        let kind = entry
            .file_type()
            .map_err(|error| format!("Cannot complete path: {error}"))?;
        if !kind.is_dir() && !(kind.is_symlink() && entry.path().is_dir()) {
            continue;
        }
        let option = format!("{base}{name}/");
        if option.len() + quote.map_or(0, |_| 2) > 4096 {
            return Err("Completed path exceeds the input limit; enter a shorter path.".into());
        }
        options.push(option);
        if options.len() > 256 {
            return Err("Too many matching directories; type more of the directory name.".into());
        }
    }
    options.sort();
    let Some(first) = options.first() else {
        return Err("No matching directories. Edit the path and try Tab again.".into());
    };
    let mut common = first.clone();
    for option in options.iter().skip(1) {
        common = common
            .chars()
            .zip(option.chars())
            .take_while(|(a, b)| a == b)
            .map(|(c, _)| c)
            .collect();
    }
    Ok(Completion {
        common: render(common),
        options: options.into_iter().map(render).collect(),
        index: None,
    })
}

fn scan(input: &str) -> Result<Scan, String> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let path = expand_path(path_text(input), home.as_deref())?;
    let path = path
        .canonicalize()
        .map_err(|e| format!("Cannot open directory: {e}"))?;
    if !path.is_dir() {
        return Err("Enter a project directory, not a file.".into());
    }
    let kind = [
        ("package.json", "Node.js"),
        ("pnpm-workspace.yaml", "Node.js workspace"),
        ("pyproject.toml", "Python"),
        ("requirements.txt", "Python"),
        ("Cargo.toml", "Rust"),
        ("go.mod", "Go"),
        ("pom.xml", "Java"),
        ("build.gradle", "Java"),
    ]
    .into_iter()
    .find(|(file, _)| path.join(file).is_file())
    .map_or("Other project", |(_, kind)| kind)
    .to_owned();
    let (report, note) = match discovery::scan(&path) {
        Ok(report) if report.problems.is_empty() => (Some(report), None),
        Ok(report)
            if report.candidates.is_empty()
                && report
                    .problems
                    .first()
                    .is_some_and(|p| p.starts_with("No supported package.json")) =>
        {
            (None, None)
        }
        Ok(report) => (
            None,
            Some(format!(
                "Automatic script selection unavailable: {}. Enter your command below.",
                report
                    .problems
                    .first()
                    .map_or("No declared scripts", String::as_str)
            )),
        ),
        Err(error) => (
            None,
            Some(format!(
                "Automatic script selection unavailable: {error}. Enter your command below."
            )),
        ),
    };
    Ok(Scan {
        path,
        kind,
        note,
        report,
    })
}

pub async fn execute(work: Work, config: PathBuf, workspace: Option<String>) -> Finished {
    match work {
        Work::Complete(input) => {
            let requested = input.clone();
            let result = tokio::task::spawn_blocking(move || {
                let home = std::env::var_os("HOME").map(PathBuf::from);
                complete(&input, home.as_deref())
            })
            .await
            .map_err(|error| error.to_string())
            .and_then(|r| r);
            Finished::Completed {
                input: requested,
                result,
            }
        }
        Work::Scan(input) => Finished::Scanned(
            tokio::task::spawn_blocking(move || scan(&input))
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r),
        ),
        Work::LoadEdit(service_id) => Finished::LoadedEdit(
            tokio::task::spawn_blocking(move || {
                let (path, arguments) =
                    crate::import::inspect_existing(Some(config.clone()), &service_id)?;
                let scanned = scan(&path.to_string_lossy())?;
                let draft = Draft::new_edit(
                    &service_id,
                    &path,
                    Some(config),
                    &arguments,
                    scanned.report.as_ref(),
                )?;
                Ok((scanned, Box::new(draft)))
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r),
        ),
        Work::Preview {
            directory,
            arguments,
            report,
            edit_target,
        } => Finished::Previewed(
            tokio::task::spawn_blocking(move || {
                if let Some(service_id) = edit_target {
                    Draft::new_edit(
                        &service_id,
                        &directory,
                        Some(config),
                        &arguments,
                        report.as_deref(),
                    )
                    .map(Box::new)
                } else {
                    Draft::new(
                        &directory,
                        Some(config),
                        None,
                        &arguments,
                        report.as_deref(),
                    )
                    .map(Box::new)
                }
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r),
        ),
        Work::Save(mut draft) => {
            let create_recovery = workspace.is_none();
            let mut committed = false;
            let result = if draft.changes().is_empty() {
                client::transact(&config, &Request::Status {}, false)
                    .await
                    .map_err(|e| e.to_string())
            } else if let Some(workspace_id) = workspace {
                let proposal = draft.proposal();
                let request = if proposal.replace.is_some() {
                    Request::UpdateProject {
                        workspace_id,
                        proposal,
                    }
                } else {
                    Request::AddProject {
                        workspace_id,
                        proposal,
                    }
                };
                match client::transact(&config, &request, false).await {
                    Ok(response) => {
                        committed = response.error.is_none();
                        Ok(response)
                    }
                    Err(error) => Err(format!("Project save: {error}")),
                }
            } else {
                let saved = tokio::task::spawn_blocking(move || {
                    let result = draft.apply();
                    (draft, result)
                })
                .await;
                let (returned, applied) = match saved {
                    Ok(value) => value,
                    Err(error) => {
                        return Finished::Previewed(Err(format!(
                            "Save outcome unknown: {error}. Inspect the configuration before retrying."
                        )));
                    }
                };
                draft = returned;
                match applied {
                    Ok(changed) => {
                        committed = changed;
                        client::transact(&config, &Request::Status {}, true)
                            .await
                            .map_err(|e| e.to_string())
                    }
                    Err(error) => Err(error),
                }
            }
            .and_then(|response| match &response.error {
                Some(error) => Err(error.clone()),
                None => Ok(response),
            });
            let (response, error) = match result {
                Ok(response) => (Some(response), None),
                Err(error) => {
                    let response = client::transact(&config, &Request::Status {}, create_recovery)
                        .await
                        .ok()
                        .filter(|r| r.error.is_none());
                    (
                        response,
                        Some(if committed {
                            format!(
                                "Configuration saved, but reconnect failed: {error}. Reopen the workspace."
                            )
                        } else {
                            error
                        }),
                    )
                }
            };
            Finished::Saved {
                draft,
                response,
                error,
                committed,
            }
        }
    }
}

#[cfg(test)]
mod completion_tests {
    use super::*;
    use std::{
        fs,
        os::unix::fs::symlink,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "devctl-tab-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
        fn directory(&self, name: &str) {
            fs::create_dir(self.0.join(name)).unwrap();
        }
        fn input(&self, suffix: &str) -> String {
            format!("{}/{suffix}", self.0.display())
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn unique_directory_completes_spaces_and_can_continue_into_children() {
        let f = Fixture::new();
        f.directory("space project");
        fs::create_dir(f.0.join("space project/child")).unwrap();
        fs::write(f.0.join("space-file"), "not a directory").unwrap();
        let input = f.input("sp");
        let mut dialog = Dialog::new();
        dialog.paste(&input);
        assert!(matches!(
            dialog.key(key(KeyCode::Tab)),
            Action::Work(Work::Complete(_))
        ));
        dialog.completed(input.clone(), complete(&input, None));
        assert_eq!(dialog.input, f.input("space project/"));
        assert!(dialog.completion.is_none());
        let input = dialog.input.clone();
        assert!(matches!(
            dialog.key(key(KeyCode::Tab)),
            Action::Work(Work::Complete(_))
        ));
        dialog.completed(input.clone(), complete(&input, None));
        assert_eq!(dialog.input, f.input("space project/child/"));
    }

    #[test]
    fn ambiguous_unicode_directories_extend_the_shared_prefix_and_cycle_in_name_order() {
        let f = Fixture::new();
        f.directory("项目甲");
        f.directory("项目乙");
        let input = f.input("项");
        let completion = complete(&input, None).unwrap();
        assert_eq!(completion.common, f.input("项目"));
        let mut expected = [f.input("项目甲/"), f.input("项目乙/")];
        expected.sort();
        let mut dialog = Dialog::new();
        dialog.paste(&input);
        dialog.completed(input, Ok(completion));
        assert!(
            dialog
                .lines(12)
                .iter()
                .any(|line| line.contains("2 directories"))
        );
        dialog.key(key(KeyCode::Tab));
        assert_eq!(dialog.input, expected[0]);
        dialog.key(key(KeyCode::Tab));
        assert_eq!(dialog.input, expected[1]);
        dialog.key(key(KeyCode::Tab));
        assert_eq!(dialog.input, expected[0]);
        dialog.key(key(KeyCode::BackTab));
        assert_eq!(dialog.input, expected[1]);
        dialog.paste("c");
        assert!(dialog.completion.is_none());
    }

    #[test]
    fn home_relative_and_quoted_paths_share_the_inspection_interpretation() {
        let f = Fixture::new();
        f.directory("space 项目");
        let home = Some(f.0.as_path());
        for (input, expected) in [
            ("~/sp".to_string(), "~/space 项目/".to_string()),
            (f.input("./sp"), f.input("./space 项目/")),
            (
                format!("\"{}\"", f.input("sp")),
                format!("\"{}\"", f.input("space 项目/")),
            ),
            (
                format!("'{}", f.input("sp")),
                format!("'{}'", f.input("space 项目/")),
            ),
        ] {
            let completed = complete(&input, home).unwrap().common;
            assert_eq!(completed, expected);
            assert_eq!(
                expand_path(path_text(&completed), home)
                    .unwrap()
                    .canonicalize()
                    .unwrap(),
                f.0.join("space 项目").canonicalize().unwrap()
            );
        }
        assert_eq!(complete("~", home).unwrap().common, "~/");
        assert_eq!(complete(".", None).unwrap().common, "./");
        assert_eq!(complete("..", None).unwrap().common, "../");
        assert!(complete("~/sp", None).err().unwrap().contains("HOME"));
    }

    #[test]
    fn completion_filters_files_hidden_invalid_names_and_broken_links() {
        let f = Fixture::new();
        f.directory("visible");
        f.directory(".hidden");
        f.directory("control\nname");
        #[cfg(target_os = "linux")]
        {
            use std::os::unix::ffi::OsStringExt;
            fs::create_dir(f.0.join(std::ffi::OsString::from_vec(vec![b'x', 0xff]))).unwrap();
        }
        fs::write(f.0.join("file"), "file").unwrap();
        symlink(f.0.join("visible"), f.0.join("linked")).unwrap();
        symlink(f.0.join("missing"), f.0.join("broken")).unwrap();
        symlink(f.0.join("file"), f.0.join("linked-file")).unwrap();
        assert_eq!(
            complete(&f.input(""), None).unwrap().options,
            [f.input("linked/"), f.input("visible/")]
        );
        assert_eq!(
            complete(&f.input("."), None).unwrap().options,
            [f.input(".hidden/")]
        );
        assert!(
            complete(&f.input("file"), None)
                .err()
                .unwrap()
                .contains("No matching")
        );
    }

    #[test]
    fn failed_or_stale_reads_preserve_input_and_enter_waits_for_completion() {
        let f = Fixture::new();
        f.directory("project");
        let input = f.input("pro");
        let mut dialog = Dialog::new();
        dialog.paste(&input);
        dialog.key(key(KeyCode::Tab));
        assert!(matches!(dialog.key(key(KeyCode::Enter)), Action::None));
        assert!(
            matches!(dialog.completed(input.clone(), complete(&input, None)), Action::Work(Work::Scan(path)) if path == f.input("project/"))
        );
        let full = f.input("project/");
        let mut empty = Dialog::new();
        empty.paste(&full);
        empty.key(key(KeyCode::Tab));
        empty.key(key(KeyCode::Enter));
        assert!(
            matches!(empty.completed(full.clone(), complete(&full, None)), Action::Work(Work::Scan(path)) if path == full)
        );
        assert!(
            empty.error.is_none(),
            "A valid empty directory can still be inspected"
        );
        let mut dialog = Dialog::new();
        dialog.paste(&input);
        dialog.key(key(KeyCode::Tab));
        dialog.paste("x");
        dialog.key(key(KeyCode::Enter));
        assert!(
            matches!(dialog.completed(input.clone(), complete(&input, None)), Action::Work(Work::Scan(path)) if path == format!("{input}x"))
        );
        let mut dialog = Dialog::new();
        dialog.paste(&f.input("missing/child"));
        let input = dialog.input.clone();
        dialog.key(key(KeyCode::Tab));
        dialog.completed(input.clone(), complete(&input, None));
        assert_eq!(dialog.input, input);
        assert!(dialog.error.as_deref().unwrap().contains("Cannot complete"));
        dialog.key(key(KeyCode::Tab));
        assert!(matches!(dialog.key(key(KeyCode::Esc)), Action::Close));
    }

    #[test]
    fn excessive_matches_fail_without_exposing_a_partial_candidate_list() {
        let f = Fixture::new();
        for i in 0..257 {
            f.directory(&format!("dir-{i:03}"));
        }
        assert!(
            complete(&f.input("dir-"), None)
                .err()
                .unwrap()
                .contains("Too many matching")
        );
        assert_eq!(
            complete(&f.input("dir-256"), None).unwrap().common,
            f.input("dir-256/")
        );
    }
}
