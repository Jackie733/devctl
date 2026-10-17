use std::{
    collections::{BTreeMap, BTreeSet},
    future::poll_fn,
    task::Poll,
};

use serde::{Deserialize, Serialize};

use crate::{
    config::Service,
    logs::Sink,
    process::{RunError, RunOutcome},
    service_task::ServiceTask,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ServiceState {
    Stopped,
    Starting,
    Running,
    Stopping,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub state: ServiceState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub cleanup_confirmed: bool,
}

impl Default for ServiceStatus {
    fn default() -> Self {
        Self {
            state: ServiceState::Stopped,
            instance: None,
            error: None,
            cleanup_confirmed: true,
        }
    }
}

#[derive(Default)]
pub struct Manager {
    services: BTreeMap<String, Service>,
    tasks: BTreeMap<String, ServiceTask>,
    statuses: BTreeMap<String, ServiceStatus>,
    stopping_all: BTreeSet<String>,
    logs: BTreeMap<String, Sink>,
    next_instance: u64,
}

impl Manager {
    pub fn new(services: BTreeMap<String, Service>) -> Self {
        let statuses = services
            .keys()
            .map(|id| (id.clone(), ServiceStatus::default()))
            .collect();
        Self {
            services,
            statuses,
            ..Self::default()
        }
    }

    pub fn with_logs(services: BTreeMap<String, Service>, logs: BTreeMap<String, Sink>) -> Self {
        Self {
            logs,
            ..Self::new(services)
        }
    }

    pub fn all_ids(&self) -> Vec<String> {
        self.services.keys().cloned().collect()
    }

    /// Only new declarations may join a live workspace. Never replace task state.
    pub fn add(&mut self, id: String, service: Service, sink: Sink) {
        assert!(!self.services.contains_key(&id), "addition must be new");
        self.services.insert(id.clone(), service);
        self.statuses.insert(id.clone(), ServiceStatus::default());
        self.logs.insert(id, sink);
    }

    pub fn is_pending(&self, id: &str) -> bool {
        self.tasks.get(id).is_some_and(|task| !task.is_joined())
    }

    pub fn has_unconfirmed_cleanup(&self) -> bool {
        self.statuses
            .values()
            .any(|status| status.state == ServiceState::Failed && !status.cleanup_confirmed)
    }

    pub fn has_pending_tasks(&self) -> bool {
        self.tasks.values().any(|task| !task.is_joined())
    }

    pub fn default_targets(&self) -> Vec<String> {
        self.services
            .iter()
            .filter(|(_, service)| service.autostart)
            .map(|(id, _)| id.clone())
            .collect()
    }

    // Validate the entire request before registering any tasks.
    pub fn check_start(&self, targets: &[String]) -> Result<(), String> {
        if !self.stopping_all.is_empty() {
            return Err("All-service stop is still in progress.".into());
        }
        for id in targets {
            let service = self
                .services
                .get(id)
                .ok_or_else(|| format!("Unknown service '{id}'."))?;
            if !service.depends_on.is_empty() {
                return Err(format!(
                    "Service '{id}' has dependencies. Dependency execution is not supported yet."
                ));
            }
            if let Some(status) = self.statuses.get(id) {
                if status.state == ServiceState::Stopping {
                    return Err(format!("Service '{id}' is still stopping."));
                }
                if status.state == ServiceState::Failed && !status.cleanup_confirmed {
                    return Err(status
                        .error
                        .clone()
                        .unwrap_or_else(|| format!("Service '{id}' cleanup is unconfirmed.")));
                }
            }
        }
        Ok(())
    }

    pub fn start_selected(&mut self, targets: &[String]) -> Result<(), String> {
        self.check_start(targets)?;
        for id in targets {
            if !self.tasks.contains_key(id) {
                self.start(id.clone(), self.services[id].clone())?;
            }
        }
        Ok(())
    }

    pub fn start_all(&mut self) -> Result<(), String> {
        self.start_selected(&self.default_targets())
    }

    // Each task owns a one-shot startup report, sampled by state queries.
    pub fn statuses(&mut self) -> &BTreeMap<String, ServiceStatus> {
        self.refresh_started();
        &self.statuses
    }

    fn refresh_started(&mut self) {
        for (id, task) in &mut self.tasks {
            if task.has_started() {
                let status = self
                    .statuses
                    .get_mut(id)
                    .expect("Registered tasks have status.");
                if status.state == ServiceState::Starting {
                    status.state = ServiceState::Running;
                }
            }
        }
    }

    pub fn stop_selected(&mut self, targets: &[String]) -> Result<(), String> {
        for id in targets {
            let status = self
                .statuses
                .get(id)
                .ok_or_else(|| format!("Unknown service '{id}'."))?;
            if status.state == ServiceState::Failed && !status.cleanup_confirmed {
                return Err(status.error.clone().expect("Failed status has an error."));
            }
        }
        for id in targets {
            if self.tasks.contains_key(id) {
                self.request_stop(id)?;
            }
        }
        Ok(())
    }

    // No await: stop every current instance before yielding to completion handling.
    pub fn request_stop_all(&mut self) {
        let ids: Vec<_> = self
            .tasks
            .iter()
            .filter(|(_, task)| !task.is_joined())
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            self.request_stop(&id)
                .expect("Unjoined task can receive a stop request.");
            self.stopping_all.insert(id);
        }
    }

    fn start(&mut self, id: String, service: Service) -> Result<(), String> {
        if self.tasks.contains_key(&id) {
            return Err(format!("Service '{id}' still has a registered task."));
        }
        if !service.depends_on.is_empty() {
            return Err(format!(
                "Service '{id}' has dependencies. Dependency execution is not supported yet."
            ));
        }

        self.next_instance = self
            .next_instance
            .checked_add(1)
            .ok_or_else(|| "Service instance counter exhausted.".to_owned())?;
        let instance = self.next_instance;
        let logs = self.logs.get(&id).map(|sink| sink.with_instance(instance));
        let task = ServiceTask::spawn(id.clone(), service, logs);
        self.statuses.insert(
            id.clone(),
            ServiceStatus {
                state: ServiceState::Starting,
                instance: Some(instance),
                error: None,
                cleanup_confirmed: false,
            },
        );
        self.tasks.insert(id, task);
        Ok(())
    }

    pub fn request_stop(&mut self, id: &str) -> Result<(), String> {
        let task = self
            .tasks
            .get_mut(id)
            .ok_or_else(|| format!("Service '{id}' has no registered task."))?;
        if task.is_joined() {
            return Err(self.statuses[id]
                .error
                .clone()
                .expect("Unconfirmed cleanup has an error."));
        }
        task.request_stop();
        self.statuses
            .get_mut(id)
            .expect("Registered tasks have status.")
            .state = ServiceState::Stopping;
        Ok(())
    }

    pub async fn stop_all(&mut self) -> Result<(), String> {
        let ids: Vec<String> = self.tasks.keys().cloned().collect();

        self.request_stop_all();

        let mut errors = Vec::new();
        for id in ids {
            if let Err(error) = self.wait(&id).await {
                errors.push(format!("Service '{id}': {error}"));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors.join("\n"))
        }
    }

    pub async fn wait(&mut self, id: &str) -> Result<RunOutcome, String> {
        let task = self
            .tasks
            .get_mut(id)
            .ok_or_else(|| format!("Service '{id}' has no registered task."))?;
        if task.is_joined() {
            return Err(self.statuses[id]
                .error
                .clone()
                .expect("Unconfirmed cleanup has an error."));
        }
        let result = task.wait().await;
        self.finish_wait(id, result)
    }

    pub async fn wait_next(&mut self) -> Option<(String, Result<RunOutcome, String>)> {
        self.refresh_started();
        let (id, result) = poll_fn(|cx| {
            let mut waiting = false;
            for (id, task) in &mut self.tasks {
                if task.is_joined() {
                    continue;
                }
                waiting = true;
                if let Poll::Ready(result) = task.poll_wait(cx) {
                    return Poll::Ready(Some((id.clone(), result)));
                }
            }

            if waiting {
                Poll::Pending
            } else {
                Poll::Ready(None)
            }
        })
        .await?;

        let result = self.finish_wait(&id, result);
        Some((id, result))
    }

    fn finish_wait(
        &mut self,
        id: &str,
        result: Result<RunOutcome, RunError>,
    ) -> Result<RunOutcome, String> {
        let cleanup_confirmed = matches!(&result, Ok(_) | Err(RunError::SafeToRetry(_)));
        let error = match &result {
            Ok(RunOutcome::Stopped) => None,
            Ok(RunOutcome::Exited(status)) => {
                Some(format!("Service '{id}' exited unexpectedly ({status})."))
            }
            Err(error) => Some(error.to_string()),
        };
        let instance = self.statuses.get(id).and_then(|status| status.instance);
        self.statuses.insert(
            id.into(),
            ServiceStatus {
                instance,
                state: if error.is_some() {
                    ServiceState::Failed
                } else {
                    ServiceState::Stopped
                },
                error,
                cleanup_confirmed,
            },
        );
        self.stopping_all.remove(id);
        if cleanup_confirmed {
            self.tasks.remove(id);
        }
        result.map_err(|error| error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use tokio::time::{Duration, timeout};

    use super::*;

    fn service(id: &str) -> Service {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/process.toml");
        crate::config::load(&path)
            .unwrap()
            .services
            .remove(id)
            .unwrap()
    }

    fn configured() -> Manager {
        let mut enabled = service("long_running");
        enabled.autostart = true;
        let disabled = service("long_running");
        Manager::new(BTreeMap::from([
            ("first".into(), enabled.clone()),
            ("second".into(), enabled),
            ("manual".into(), disabled),
        ]))
    }

    fn ids(values: &[&str]) -> Vec<String> {
        values.iter().map(|id| (*id).into()).collect()
    }

    async fn running(manager: &mut Manager, targets: &[&str]) {
        timeout(Duration::from_secs(3), async {
            loop {
                if targets
                    .iter()
                    .all(|id| manager.statuses()[*id].state == ServiceState::Running)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn configured_controls_cover_single_all_idempotence_and_restart() {
        let mut manager = configured();
        assert!(
            manager
                .statuses()
                .values()
                .all(|s| s.state == ServiceState::Stopped)
        );
        manager.start_selected(&ids(&["first", "first"])).unwrap();
        assert_eq!(manager.statuses()["first"].state, ServiceState::Starting);
        assert_eq!(manager.statuses()["second"].state, ServiceState::Stopped);
        manager.start_selected(&ids(&["first"])).unwrap();
        assert_eq!(manager.tasks.len(), 1);
        running(&mut manager, &["first"]).await;
        manager.start_all().unwrap();
        running(&mut manager, &["first", "second"]).await;
        assert_eq!(manager.tasks.len(), 2);
        assert_eq!(manager.statuses()["manual"].state, ServiceState::Stopped);
        manager.start_all().unwrap();
        assert_eq!(manager.tasks.len(), 2);

        manager.stop_selected(&ids(&["first", "first"])).unwrap();
        assert_eq!(manager.statuses()["first"].state, ServiceState::Stopping);
        assert!(manager.start_selected(&ids(&["first"])).is_err());
        assert!(matches!(
            timeout(Duration::from_secs(8), manager.wait("first"))
                .await
                .unwrap()
                .unwrap(),
            RunOutcome::Stopped
        ));
        assert_eq!(manager.statuses()["first"].state, ServiceState::Stopped);
        assert!(manager.statuses()["first"].cleanup_confirmed);
        manager.stop_selected(&ids(&["first"])).unwrap();
        assert!(
            timeout(Duration::from_millis(20), manager.wait("second"))
                .await
                .is_err()
        );
        manager.start_selected(&ids(&["first", "manual"])).unwrap();
        running(&mut manager, &["first", "manual"]).await;
        manager.stop_all().await.unwrap();
        assert!(
            manager
                .statuses()
                .values()
                .all(|s| s.state == ServiceState::Stopped && s.cleanup_confirmed)
        );
    }

    #[tokio::test]
    async fn batch_preflight_never_partially_starts_or_stops() {
        let mut manager = configured();
        assert!(manager.start_selected(&ids(&["first", "unknown"])).is_err());
        assert!(manager.tasks.is_empty());
        manager
            .services
            .get_mut("second")
            .unwrap()
            .depends_on
            .push(crate::config::Dependency {
                service: "first".into(),
                condition: crate::config::DependencyCondition::Started,
            });
        assert!(manager.start_all().is_err());
        assert!(manager.tasks.is_empty());
        manager.start_selected(&ids(&["first"])).unwrap();
        assert!(manager.stop_selected(&ids(&["first", "unknown"])).is_err());
        running(&mut manager, &["first"]).await;
        assert!(
            timeout(Duration::from_millis(20), manager.wait("first"))
                .await
                .is_err()
        );
        manager.stop_all().await.unwrap();
    }

    #[tokio::test]
    async fn all_stop_is_nonblocking_covers_starting_tasks_and_fences_new_starts() {
        let mut manager = configured();
        manager.start_all().unwrap();
        manager.request_stop_all();
        assert_eq!(manager.statuses()["first"].state, ServiceState::Stopping);
        assert_eq!(manager.statuses()["second"].state, ServiceState::Stopping);
        assert!(manager.start_selected(&ids(&["manual"])).is_err());
        let (first, result) = timeout(Duration::from_secs(8), manager.wait_next())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(result, Ok(RunOutcome::Stopped)));
        assert!(manager.start_selected(&[first]).is_err());
        let (_, result) = timeout(Duration::from_secs(8), manager.wait_next())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(result, Ok(RunOutcome::Stopped)));
        assert!(manager.wait_next().await.is_none());
        manager.start_all().unwrap();
        manager.stop_all().await.unwrap();
    }

    #[tokio::test]
    async fn failures_remain_queryable_and_clear_only_when_explicitly_restarted() {
        let mut manager = configured();
        manager.services.insert("broken".into(), service("missing"));
        manager
            .statuses
            .insert("broken".into(), ServiceStatus::default());
        manager.start_selected(&ids(&["first", "broken"])).unwrap();
        let (id, result) = timeout(Duration::from_secs(3), manager.wait_next())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(id, "broken");
        let error = result.err().unwrap();
        let status = &manager.statuses()["broken"];
        assert_eq!(status.state, ServiceState::Failed);
        assert_eq!(status.error.as_deref(), Some(error.as_str()));
        assert!(status.cleanup_confirmed);
        manager.stop_selected(&ids(&["broken"])).unwrap();
        assert_eq!(
            manager.statuses()["broken"].error.as_deref(),
            Some(error.as_str())
        );
        assert!(manager.wait("broken").await.is_err());
        assert_eq!(
            manager.statuses()["broken"].error.as_deref(),
            Some(error.as_str())
        );
        manager
            .services
            .insert("broken".into(), service("long_running"));
        manager.start_selected(&ids(&["broken"])).unwrap();
        assert!(manager.statuses()["broken"].error.is_none());
        running(&mut manager, &["first", "broken"]).await;
        manager.stop_all().await.unwrap();
    }

    #[tokio::test]
    async fn unknown_cleanup_preserves_original_error_and_never_releases_the_id() {
        let mut manager = configured();
        manager.tasks.insert(
            "first".into(),
            ServiceTask::failed_for_test(
                "first",
                RunError::CleanupUnconfirmed("injected cleanup failure".into()),
            ),
        );
        manager.start_selected(&ids(&["second"])).unwrap();
        let (id, result) = timeout(Duration::from_secs(3), manager.wait_next())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(id, "first");
        assert_eq!(result.err().unwrap(), "injected cleanup failure");
        assert_eq!(manager.statuses()["first"].state, ServiceState::Failed);
        assert!(!manager.statuses()["first"].cleanup_confirmed);
        assert!(manager.has_unconfirmed_cleanup());
        for _ in 0..2 {
            assert_eq!(
                manager.start_selected(&ids(&["first"])).unwrap_err(),
                "injected cleanup failure"
            );
            assert_eq!(
                manager.stop_selected(&ids(&["first"])).unwrap_err(),
                "injected cleanup failure"
            );
            assert_eq!(
                manager.wait("first").await.err().unwrap(),
                "injected cleanup failure"
            );
        }
        let error = timeout(Duration::from_secs(8), manager.stop_all())
            .await
            .unwrap()
            .unwrap_err();
        assert!(error.contains("injected cleanup failure"));
        assert_eq!(manager.statuses()["second"].state, ServiceState::Stopped);
        assert!(manager.wait_next().await.is_none());
        assert!(manager.tasks.contains_key("first"));
        assert!(manager.start_all().is_err());
        manager.start_selected(&ids(&["second"])).unwrap();
        manager.stop_selected(&ids(&["second"])).unwrap();
        assert!(matches!(
            manager.wait("second").await.unwrap(),
            RunOutcome::Stopped
        ));
    }

    #[tokio::test]
    async fn natural_exit_retains_status_and_empty_default_selection_is_a_noop() {
        let mut manager = Manager::new(BTreeMap::from([("hello".into(), service("hello"))]));
        manager.start_all().unwrap();
        assert!(manager.tasks.is_empty());
        manager.start_selected(&ids(&["hello"])).unwrap();
        manager.wait("hello").await.unwrap();
        let status = &manager.statuses()["hello"];
        assert_eq!(status.state, ServiceState::Failed);
        assert!(status.cleanup_confirmed);
        assert!(status.error.as_ref().unwrap().contains("exit status: 0"));
        manager.start_selected(&ids(&["hello"])).unwrap();
        assert!(manager.statuses()["hello"].error.is_none());
        manager.wait("hello").await.unwrap();
    }

    async fn stopped(manager: &mut Manager, id: &str) {
        manager.request_stop(id).unwrap();
        let outcome = timeout(Duration::from_secs(8), manager.wait(id))
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(outcome, RunOutcome::Stopped));
    }

    #[tokio::test]
    async fn duplicate_and_stopping_ids_are_reserved_until_cleanup_finishes() {
        let mut manager = Manager::default();
        manager
            .start("first".into(), service("long_running"))
            .unwrap();
        manager
            .start("second".into(), service("long_running"))
            .unwrap();
        assert!(
            manager
                .start("first".into(), service("long_running"))
                .is_err()
        );
        assert!(
            timeout(Duration::from_millis(20), manager.wait("first"))
                .await
                .is_err()
        );
        assert!(
            manager
                .start("first".into(), service("long_running"))
                .is_err()
        );
        manager.request_stop("first").unwrap();
        manager.request_stop("first").unwrap();
        assert!(
            manager
                .start("first".into(), service("long_running"))
                .is_err()
        );
        stopped(&mut manager, "first").await;
        assert!(
            timeout(Duration::from_millis(20), manager.wait("second"))
                .await
                .is_err()
        );
        manager
            .start("first".into(), service("long_running"))
            .unwrap();
        stopped(&mut manager, "first").await;
        stopped(&mut manager, "second").await;
        assert!(manager.request_stop("first").is_err());
        assert!(manager.wait("first").await.is_err());
    }

    #[tokio::test]
    async fn a_natural_exit_releases_the_id_only_after_joining() {
        let mut manager = Manager::default();
        for _ in 0..2 {
            manager.start("hello".into(), service("hello")).unwrap();
            assert!(manager.start("hello".into(), service("hello")).is_err());
            let outcome = timeout(Duration::from_secs(3), manager.wait("hello"))
                .await
                .unwrap()
                .unwrap();
            assert!(matches!(outcome, RunOutcome::Exited(status) if status.success()));
        }
    }

    #[tokio::test]
    async fn spawn_errors_release_the_id_and_do_not_stop_other_services() {
        let mut manager = Manager::default();
        assert!(manager.request_stop("unknown").is_err());
        assert!(manager.wait("unknown").await.is_err());

        let mut unsupported = service("long_running");
        unsupported.depends_on.push(crate::config::Dependency {
            service: "other".into(),
            condition: crate::config::DependencyCondition::Started,
        });
        assert!(manager.start("survivor".into(), unsupported).is_err());
        manager
            .start("survivor".into(), service("long_running"))
            .unwrap();
        manager.start("missing".into(), service("missing")).unwrap();
        let error = match timeout(Duration::from_secs(3), manager.wait("missing"))
            .await
            .unwrap()
        {
            Err(error) => error,
            Ok(_) => panic!("A missing executable must fail."),
        };
        assert!(error.contains("Failed to start service 'missing':"));
        manager
            .start("missing".into(), service("long_running"))
            .unwrap();
        stopped(&mut manager, "missing").await;
        assert!(
            timeout(Duration::from_millis(20), manager.wait("survivor"))
                .await
                .is_err()
        );
        stopped(&mut manager, "survivor").await;
    }

    #[tokio::test]
    async fn invalid_commands_and_directories_release_the_id() {
        let mut no_command = service("long_running");
        no_command.cmd.clear();
        let mut no_cwd = service("long_running");
        no_cwd.cwd = None;
        let mut missing_cwd = service("long_running");
        missing_cwd.cwd = Some(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("Cargo.toml")
                .join("not-a-directory"),
        );
        let mut manager = Manager::default();

        for invalid in [no_command, no_cwd, missing_cwd] {
            manager.start("retry".into(), invalid).unwrap();
            assert!(
                timeout(Duration::from_secs(3), manager.wait("retry"))
                    .await
                    .unwrap()
                    .is_err()
            );
            assert!(manager.request_stop("retry").is_err());
            manager
                .start("retry".into(), service("long_running"))
                .unwrap();
            stopped(&mut manager, "retry").await;
        }
    }

    #[tokio::test]
    async fn nonzero_exit_releases_the_id_after_cleanup() {
        let mut manager = Manager::default();
        manager.start("retry".into(), service("failure")).unwrap();
        let outcome = timeout(Duration::from_secs(3), manager.wait("retry"))
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(outcome, RunOutcome::Exited(status) if status.code() == Some(7)));
        manager
            .start("retry".into(), service("long_running"))
            .unwrap();
        stopped(&mut manager, "retry").await;
    }

    #[tokio::test]
    async fn stop_all_handles_empty_and_not_yet_started_tasks() {
        let mut manager = Manager::default();
        manager.stop_all().await.unwrap();

        for _ in 0..2 {
            manager
                .start("first".into(), service("long_running"))
                .unwrap();
            manager
                .start("second".into(), service("long_running"))
                .unwrap();
            timeout(Duration::from_secs(8), manager.stop_all())
                .await
                .unwrap()
                .unwrap();
            assert!(manager.request_stop("first").is_err());
            assert!(manager.request_stop("second").is_err());
        }
        manager.stop_all().await.unwrap();
    }

    #[tokio::test]
    async fn stop_all_reports_each_failure_and_still_joins_other_tasks() {
        let mut manager = Manager::default();
        manager
            .start("a_missing".into(), service("missing"))
            .unwrap();
        manager
            .start("b_missing".into(), service("missing"))
            .unwrap();
        manager
            .start("z_survivor".into(), service("long_running"))
            .unwrap();
        let error = timeout(Duration::from_secs(8), manager.stop_all())
            .await
            .unwrap()
            .unwrap_err();
        assert!(error.contains("Failed to start service 'a_missing':"));
        assert!(error.contains("Failed to start service 'b_missing':"));
        for id in ["a_missing", "b_missing", "z_survivor"] {
            assert!(manager.request_stop(id).is_err());
            manager.start(id.into(), service("long_running")).unwrap();
        }
        timeout(Duration::from_secs(8), manager.stop_all())
            .await
            .unwrap()
            .unwrap();
    }

    struct StopFixture {
        dir: std::path::PathBuf,
        pids: Vec<nix::unistd::Pid>,
    }

    impl StopFixture {
        fn new() -> Self {
            let dir = std::env::temp_dir()
                .join(format!("devctl-stop-all-contract-{}", std::process::id()));
            std::fs::create_dir(&dir).unwrap();
            Self {
                dir,
                pids: Vec::new(),
            }
        }

        fn service(&self, name: &str, script: &str) -> Service {
            let mut value = service("long_running");
            value.cmd = vec![
                "sh".into(),
                "-c".into(),
                script.into(),
                "sh".into(),
                self.dir.join(name).to_str().unwrap().into(),
            ];
            value
        }

        async fn ready(&mut self, name: &str) -> nix::unistd::Pid {
            let path = self.dir.join(name);
            let pid = timeout(Duration::from_secs(3), async {
                loop {
                    if let Ok(value) = std::fs::read_to_string(&path)
                        && let Ok(pid) = value.trim().parse::<i32>()
                    {
                        break nix::unistd::Pid::from_raw(pid);
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            self.pids.push(pid);
            pid
        }
    }

    impl Drop for StopFixture {
        fn drop(&mut self) {
            for pid in &self.pids {
                let _ = nix::sys::signal::killpg(*pid, nix::sys::signal::Signal::SIGKILL);
            }
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[tokio::test]
    async fn stop_all_notifies_every_task_before_waiting_and_survives_cancellation() {
        use nix::{errno::Errno, sys::signal::kill};

        let mut manager = Manager::default();
        let mut fixture = StopFixture::new();
        let slow = fixture.service(
            "slow",
            r#"trap '' TERM; printf '%s\n' "$$" > "$1"; exec sleep 60"#,
        );
        let fast = fixture.service("fast", r#"printf '%s\n' "$$" > "$1"; exec sleep 60"#);
        manager.start("a_slow".into(), slow).unwrap();
        manager.start("z_fast".into(), fast).unwrap();
        let slow_pid = fixture.ready("slow").await;
        let fast_pid = fixture.ready("fast").await;

        assert!(
            timeout(Duration::from_millis(300), manager.stop_all())
                .await
                .is_err()
        );
        timeout(Duration::from_secs(2), async {
            while kill(fast_pid, None) != Err(Errno::ESRCH) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        fixture.pids.retain(|pid| *pid != fast_pid);
        assert_eq!(kill(slow_pid, None), Ok(()));
        assert!(
            manager
                .start("a_slow".into(), service("long_running"))
                .is_err()
        );
        assert!(
            manager
                .start("z_fast".into(), service("long_running"))
                .is_err()
        );

        timeout(Duration::from_secs(8), manager.stop_all())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(kill(slow_pid, None), Err(Errno::ESRCH));
        fixture.pids.clear();
        for id in ["a_slow", "z_fast"] {
            manager.start(id.into(), service("long_running")).unwrap();
        }
        timeout(Duration::from_secs(8), manager.stop_all())
            .await
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn wait_next_observes_a_later_id_without_stopping_an_earlier_running_task() {
        let mut manager = Manager::default();
        assert!(manager.wait_next().await.is_none());
        manager
            .start("a_running".into(), service("long_running"))
            .unwrap();

        for _ in 0..2 {
            manager.start("z_hello".into(), service("hello")).unwrap();
            let (id, result) = timeout(Duration::from_secs(3), manager.wait_next())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(id, "z_hello");
            assert!(matches!(result, Ok(RunOutcome::Exited(status)) if status.success()));
        }

        assert!(
            timeout(Duration::from_millis(20), manager.wait_next())
                .await
                .is_err()
        );
        assert!(
            manager
                .start("a_running".into(), service("long_running"))
                .is_err()
        );
        manager.request_stop("a_running").unwrap();
        let (id, result) = timeout(Duration::from_secs(8), manager.wait_next())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(id, "a_running");
        assert!(matches!(result, Ok(RunOutcome::Stopped)));
        assert!(manager.wait_next().await.is_none());
    }

    #[tokio::test]
    async fn wait_next_delivers_spawn_failure_exit_code_and_signal_once_each() {
        let mut manager = Manager::default();
        manager.start("missing".into(), service("missing")).unwrap();
        manager.start("failure".into(), service("failure")).unwrap();
        manager.start("signal".into(), service("signal")).unwrap();
        let mut seen = std::collections::BTreeSet::new();
        while let Some((id, result)) = timeout(Duration::from_secs(3), manager.wait_next())
            .await
            .unwrap()
        {
            assert!(seen.insert(id.clone()));
            match id.as_str() {
                "missing" => assert!(
                    result
                        .err()
                        .unwrap()
                        .contains("Failed to start service 'missing':")
                ),
                "failure" => assert!(
                    matches!(result, Ok(RunOutcome::Exited(status)) if status.code() == Some(7))
                ),
                "signal" => {
                    use std::os::unix::process::ExitStatusExt;
                    assert!(
                        matches!(result, Ok(RunOutcome::Exited(status)) if status.signal() == Some(15))
                    );
                }
                _ => panic!("Unexpected completion: {id}"),
            }
        }
        assert_eq!(seen.len(), 3);
        for id in seen {
            manager.start(id, service("long_running")).unwrap();
        }
        timeout(Duration::from_secs(8), manager.stop_all())
            .await
            .unwrap()
            .unwrap();
        assert!(manager.wait_next().await.is_none());
    }

    #[tokio::test]
    async fn explicit_wait_and_wait_next_share_the_same_completion_ownership() {
        let mut manager = Manager::default();
        manager.start("first".into(), service("hello")).unwrap();
        manager.start("second".into(), service("hello")).unwrap();
        let result = timeout(Duration::from_secs(3), manager.wait("first"))
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(result, RunOutcome::Exited(status) if status.success()));
        let (id, result) = timeout(Duration::from_secs(3), manager.wait_next())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(id, "second");
        assert!(matches!(result, Ok(RunOutcome::Exited(status)) if status.success()));
        assert!(manager.wait_next().await.is_none());
        manager.start("first".into(), service("hello")).unwrap();
        let (id, result) = timeout(Duration::from_secs(3), manager.wait_next())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(id, "first");
        assert!(result.is_ok());
        assert!(manager.wait_next().await.is_none());
    }
}
