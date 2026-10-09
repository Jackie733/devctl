use std::collections::BTreeSet;

use crate::config::Config;

/// Builds a dependency-first plan from a previously validated configuration.
pub fn build(config: &Config, targets: &[String]) -> Result<Vec<String>, String> {
    let roots: Vec<String> = if targets.is_empty() {
        config
            .services
            .iter()
            .filter(|(_, service)| service.autostart)
            .map(|(id, _)| id.clone())
            .collect()
    } else {
        targets.to_vec()
    };

    let mut completed = BTreeSet::new();
    let mut order = Vec::new();

    for id in roots {
        visit(&id, config, &mut completed, &mut order)?;
    }

    Ok(order)
}

fn visit(
    id: &str,
    config: &Config,
    completed: &mut BTreeSet<String>,
    order: &mut Vec<String>,
) -> Result<(), String> {
    if completed.contains(id) {
        return Ok(());
    }

    let service = config
        .services
        .get(id)
        .ok_or_else(|| format!("Unknown service '{id}'."))?;

    for dependency in &service.depends_on {
        visit(&dependency.service, config, completed, order)?;
    }

    completed.insert(id.to_string());
    order.push(id.to_string());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chain() -> Config {
        toml::from_str(include_str!("../examples/dependencies/chain.toml"))
            .expect("The chain example must deserialize.")
    }

    #[test]
    fn default_selection_includes_disabled_dependencies() {
        assert_eq!(build(&chain(), &[]).unwrap(), ["db", "api", "web"]);
    }

    #[test]
    fn explicit_target_excludes_unrelated_services() {
        assert_eq!(
            build(&chain(), &["api".to_string()]).unwrap(),
            ["db", "api"]
        );
    }

    #[test]
    fn shared_dependency_is_included_once() {
        let config = toml::from_str(include_str!("../examples/dependencies/shared.toml"))
            .expect("The shared dependency example must deserialize.");
        assert_eq!(
            build(&config, &["web".to_string()]).unwrap(),
            ["db", "api", "worker", "web"]
        );
    }

    #[test]
    fn repeated_and_overlapping_targets_are_included_once() {
        let targets = vec!["web".to_string(), "api".to_string(), "web".to_string()];
        assert_eq!(build(&chain(), &targets).unwrap(), ["db", "api", "web"]);
    }

    #[test]
    fn unknown_target_returns_an_error() {
        let targets = vec!["web".to_string(), "missing".to_string()];
        assert_eq!(
            build(&chain(), &targets).unwrap_err(),
            "Unknown service 'missing'."
        );
    }

    #[test]
    fn disabled_defaults_produce_an_empty_plan() {
        let mut config = chain();
        for service in config.services.values_mut() {
            service.autostart = false;
        }
        assert!(build(&config, &[]).unwrap().is_empty());
        assert_eq!(
            build(&config, &["web".to_string()]).unwrap(),
            ["db", "api", "web"]
        );
    }

    #[test]
    fn empty_workspace_produces_an_empty_plan() {
        let config = toml::from_str(include_str!("../examples/minimal.toml"))
            .expect("The minimal example must deserialize.");
        assert!(build(&config, &[]).unwrap().is_empty());
    }
}
