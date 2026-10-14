use std::{
    future::{Future, poll_fn},
    pin::Pin,
    task::{Context, Poll, ready},
};

use tokio::{sync::oneshot, task::JoinHandle};

use crate::{
    config::Service,
    logs::Sink,
    process::{self, RunError, RunOutcome},
};

pub struct ServiceTask {
    id: String,
    stop_sender: Option<oneshot::Sender<()>>,
    running: Option<JoinHandle<Result<RunOutcome, RunError>>>,
    started: oneshot::Receiver<()>,
    has_started: bool,
}

impl ServiceTask {
    #[cfg(test)]
    pub fn failed_for_test(id: &str, error: RunError) -> Self {
        Self {
            id: id.into(),
            stop_sender: None,
            running: Some(tokio::spawn(async move { Err(error) })),
            started: oneshot::channel().1,
            has_started: false,
        }
    }

    pub fn spawn(id: String, service: Service, logs: Option<Sink>) -> Self {
        let (stop_sender, stop_request) = oneshot::channel();
        let (started_sender, started) = oneshot::channel();
        let service_id = id.clone();
        let running = tokio::spawn(async move {
            process::run(
                &service_id,
                &service,
                stop_request,
                Some(started_sender),
                logs,
            )
            .await
        });

        Self {
            id,
            stop_sender: Some(stop_sender),
            running: Some(running),
            started,
            has_started: false,
        }
    }

    pub fn has_started(&mut self) -> bool {
        self.has_started |= self.started.try_recv().is_ok();
        self.has_started
    }

    pub fn request_stop(&mut self) {
        if let Some(sender) = self.stop_sender.take() {
            let _ = sender.send(());
        }
    }

    pub fn is_joined(&self) -> bool {
        self.running.is_none()
    }

    pub fn poll_wait(&mut self, cx: &mut Context<'_>) -> Poll<Result<RunOutcome, RunError>> {
        let Some(running) = self.running.as_mut() else {
            return Poll::Ready(Err(RunError::CleanupUnconfirmed(format!(
                "Service '{}' task has already been joined.",
                self.id
            ))));
        };
        let result = ready!(Pin::new(running).poll(cx));
        self.running = None;
        self.stop_sender = None;

        Poll::Ready(match result {
            Ok(result) => result,
            Err(error) => Err(RunError::CleanupUnconfirmed(format!(
                "Service '{}' task failed: {error}",
                self.id
            ))),
        })
    }

    pub async fn wait(&mut self) -> Result<RunOutcome, RunError> {
        poll_fn(|cx| self.poll_wait(cx)).await
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use std::{fs, path::PathBuf};

    use nix::{
        errno::Errno,
        sys::signal::{Signal, kill, killpg},
        unistd::Pid,
    };
    use tokio::time::{Duration, sleep, timeout};

    use super::*;

    struct Fixture {
        dir: PathBuf,
        pid: Option<Pid>,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "devctl-task-contract-{}-{name}",
                std::process::id()
            ));
            fs::create_dir(&dir).unwrap();
            Self { dir, pid: None }
        }

        fn service(&self) -> Service {
            Service {
                project: "demo".into(),
                cmd: vec![
                    "sh".into(),
                    "-c".into(),
                    "printf '%s\\n' \"$$\" > \"$1\"; exec sleep 60".into(),
                    "sh".into(),
                    self.dir.join("ready").to_str().unwrap().into(),
                ],
                cwd: Some(self.dir.clone()),
                autostart: false,
                depends_on: Vec::new(),
            }
        }

        async fn ready(&mut self) -> Pid {
            let pid = timeout(Duration::from_secs(3), async {
                loop {
                    if let Ok(content) = fs::read_to_string(self.dir.join("ready"))
                        && let Ok(pid) = content.trim().parse::<i32>()
                    {
                        break Pid::from_raw(pid);
                    }
                    sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            self.pid = Some(pid);
            assert_eq!(kill(pid, None), Ok(()));
            pid
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            if let Some(pid) = self.pid {
                let _ = killpg(pid, Signal::SIGKILL);
            }
            let _ = fs::remove_dir_all(&self.dir);
        }
    }

    #[tokio::test]
    async fn stopping_one_task_and_canceling_a_wait_preserves_the_other() {
        let mut first_fixture = Fixture::new("first");
        let mut second_fixture = Fixture::new("second");
        let mut first = ServiceTask::spawn("first".into(), first_fixture.service(), None);
        let mut second = ServiceTask::spawn("second".into(), second_fixture.service(), None);
        let first_pid = first_fixture.ready().await;
        let second_pid = second_fixture.ready().await;

        first.request_stop();
        first.request_stop();
        let outcome = timeout(Duration::from_secs(8), first.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(outcome, RunOutcome::Stopped));
        assert_eq!(kill(first_pid, None), Err(Errno::ESRCH));
        first_fixture.pid = None;
        assert_eq!(kill(second_pid, None), Ok(()));
        assert!(first.wait().await.is_err());

        assert!(
            timeout(Duration::from_millis(20), second.wait())
                .await
                .is_err()
        );
        assert_eq!(kill(second_pid, None), Ok(()));
        second.request_stop();
        let outcome = timeout(Duration::from_secs(8), second.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(outcome, RunOutcome::Stopped));
        assert_eq!(kill(second_pid, None), Err(Errno::ESRCH));
        second_fixture.pid = None;
    }

    #[tokio::test]
    async fn a_spawn_error_does_not_stop_another_task() {
        let mut fixture = Fixture::new("survivor");
        let mut survivor = ServiceTask::spawn("survivor".into(), fixture.service(), None);
        let pid = fixture.ready().await;
        let mut missing_service = fixture.service();
        missing_service.cmd = vec!["devctl-command-that-does-not-exist".into()];
        let mut missing = ServiceTask::spawn("missing".into(), missing_service, None);
        let error = match timeout(Duration::from_secs(3), missing.wait())
            .await
            .unwrap()
        {
            Err(error) => error,
            Ok(_) => panic!("A missing executable must fail."),
        };
        assert!(
            error
                .to_string()
                .contains("Failed to start service 'missing':")
        );
        assert_eq!(kill(pid, None), Ok(()));

        survivor.request_stop();
        let outcome = timeout(Duration::from_secs(8), survivor.wait())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(outcome, RunOutcome::Stopped));
        assert_eq!(kill(pid, None), Err(Errno::ESRCH));
        fixture.pid = None;
    }

    #[tokio::test]
    async fn a_panicked_task_does_not_confirm_cleanup() {
        let mut task = ServiceTask {
            id: "panicked".into(),
            stop_sender: None,
            running: Some(tokio::spawn(async {
                panic!("injected task panic");
            })),
            started: oneshot::channel().1,
            has_started: false,
        };

        let result = task.wait().await;
        assert!(matches!(result, Err(RunError::CleanupUnconfirmed(_))));
        assert!(matches!(
            task.wait().await,
            Err(RunError::CleanupUnconfirmed(_))
        ));
    }

    #[tokio::test]
    async fn canceling_a_wait_allows_a_different_tokio_task_to_join() {
        let mut fixture = Fixture::new("moved-waiter");
        let mut task = ServiceTask::spawn("moved".into(), fixture.service(), None);
        let pid = fixture.ready().await;
        assert!(
            timeout(Duration::from_millis(20), task.wait())
                .await
                .is_err()
        );
        assert!(!task.is_joined());
        let waiter = tokio::spawn(async move {
            task.request_stop();
            let result = task.wait().await;
            assert!(task.is_joined());
            result
        });
        let result = timeout(Duration::from_secs(8), waiter)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(matches!(result, RunOutcome::Stopped));
        assert_eq!(kill(pid, None), Err(Errno::ESRCH));
        fixture.pid = None;
    }
}
