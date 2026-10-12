use std::collections::BTreeMap;

use crate::{config::Service, process::RunOutcome, service_task::ServiceTask};

#[derive(Default)]
pub struct Manager {
    tasks: BTreeMap<String, ServiceTask>,
}

impl Manager {
    pub fn start(&mut self, id: String, service: Service) -> Result<(), String> {
        if self.tasks.contains_key(&id) {
            return Err(format!("Service '{id}' still has a registered task."));
        }
        if !service.depends_on.is_empty() {
            return Err(format!(
                "Service '{id}' has dependencies. Dependency execution is not supported yet."
            ));
        }

        let task = ServiceTask::spawn(id.clone(), service);
        self.tasks.insert(id, task);
        Ok(())
    }

    pub fn request_stop(&mut self, id: &str) -> Result<(), String> {
        let task = self
            .tasks
            .get_mut(id)
            .ok_or_else(|| format!("Service '{id}' has no registered task."))?;
        task.request_stop();
        Ok(())
    }

    pub async fn wait(&mut self, id: &str) -> Result<RunOutcome, String> {
        let task = self
            .tasks
            .get_mut(id)
            .ok_or_else(|| format!("Service '{id}' has no registered task."))?;
        let result = task.wait().await;

        // An error does not prove that the process group was cleaned up.
        if result.is_ok() {
            self.tasks.remove(id);
        }

        result
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
    async fn errors_keep_the_id_reserved_and_do_not_stop_other_services() {
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
        assert!(
            manager
                .start("missing".into(), service("long_running"))
                .is_err()
        );
        assert!(
            timeout(Duration::from_millis(20), manager.wait("survivor"))
                .await
                .is_err()
        );
        stopped(&mut manager, "survivor").await;
    }
}
