//! TUI additions reuse the CLI's reviewed configuration transaction.
use crate::{
    client,
    control::{Request, Response},
    discovery::{self, Report},
    import::Draft,
};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::path::PathBuf;

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
    path: PathBuf,
    kind: String,
    note: Option<String>,
    report: Option<Report>,
    projects: Vec<usize>,
    project: usize,
    manager: String,
    scripts: Vec<String>,
}

pub enum Work {
    Scan(String),
    Preview {
        directory: PathBuf,
        arguments: Vec<String>,
        report: Option<Box<Report>>,
    },
    Save(Box<Draft>),
}
pub enum Finished {
    Scanned(Result<Scan, String>),
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

impl Dialog {
    pub fn new() -> Self {
        Self {
            step: Step::Path,
            input: String::new(),
            scroll: 0,
            error: None,
            path: PathBuf::new(),
            kind: String::new(),
            note: None,
            report: None,
            projects: Vec::new(),
            project: 0,
            manager: String::new(),
            scripts: Vec::new(),
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
            for c in text.chars().filter(|c| !c.is_control()) {
                if self.input.len() + c.len_utf8() > 4096 {
                    break;
                }
                self.input.push(c);
            }
            self.error = None;
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
                    self.error = None;
                }
                KeyCode::Char(c) => self.paste(&c.to_string()),
                KeyCode::Enter if !self.input.trim().is_empty() => {
                    self.error = None;
                    if matches!(self.step, Step::Path) {
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
                lines.push(format!("> {}_", input_tail(&self.input)));
                lines.push("Enter: inspect | Ctrl-U: clear | Esc: cancel".into());
            }
            Step::Scanning => {
                lines.push("Reading project declarations... No commands are executed.".into())
            }
            Step::Preparing => lines.push("Preparing the configuration preview...".into()),
            Step::Saving => lines.push("Saving and adding the project to this workspace...".into()),
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
                lines.push("Review project addition".into());
                lines.push(format!("Project: {}", draft.id));
                lines.push(format!("Directory: {}", draft.path.display()));
                lines.push(format!("Project type: {}", self.kind));
                lines.push(format!("Command: {}", draft.command_text()));
                lines.push(format!("Configuration: {}", draft.target().display()));
                if draft.changes().is_empty() {
                    lines.push("Already configured: preserve its existing command and edits; nothing to save.".into());
                }
                lines.push(
                    "Save only. The new project stays stopped. Existing services keep running."
                        .into(),
                );
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
fn scan(input: &str) -> Result<Scan, String> {
    let text = input.trim();
    let text = text
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| text.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(text);
    let path = if text == "~" || text.starts_with("~/") {
        PathBuf::from(
            std::env::var_os("HOME").ok_or("HOME is not set; enter an absolute directory.")?,
        )
        .join(text.strip_prefix("~/").unwrap_or(""))
    } else {
        PathBuf::from(text)
    };
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
        Work::Scan(input) => Finished::Scanned(
            tokio::task::spawn_blocking(move || scan(&input))
                .await
                .map_err(|e| e.to_string())
                .and_then(|r| r),
        ),
        Work::Preview {
            directory,
            arguments,
            report,
        } => Finished::Previewed(
            tokio::task::spawn_blocking(move || {
                Draft::new(
                    &directory,
                    Some(config),
                    None,
                    &arguments,
                    report.as_deref(),
                )
                .map(Box::new)
            })
            .await
            .map_err(|e| e.to_string())
            .and_then(|r| r),
        ),
        Work::Save(mut draft) => {
            let create_recovery = workspace.is_none();
            let mut committed = false;
            let result = if draft.changes().is_empty() {
                client::transact(&config, &Request::Status {}, false).await.map_err(|e| e.to_string())
            } else if let Some(workspace_id) = workspace {
                match client::transact(&config, &Request::AddProject { workspace_id, proposal: draft.proposal() }, false).await {
                    Ok(response) => { committed = response.error.is_none(); Ok(response) }
                    Err(error) => Err(format!("Save completion is unknown: {error}. Inspect configuration and status before retrying.")),
                }
            } else {
                let saved = tokio::task::spawn_blocking(move || {
                    let result = draft.apply();
                    (draft, result)
                }).await;
                let (returned, applied) = match saved {
                    Ok(value) => value,
                    Err(error) => return Finished::Previewed(Err(format!("Save outcome unknown: {error}. Inspect the configuration before retrying."))),
                };
                draft = returned;
                match applied {
                    Ok(changed) => {
                        committed = changed;
                        client::transact(&config, &Request::Status {}, true).await.map_err(|e| e.to_string())
                    }
                    Err(error) => Err(error),
                }
            }.and_then(|response| match &response.error { Some(error) => Err(error.clone()), None => Ok(response) });
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
