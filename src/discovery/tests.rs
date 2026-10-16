use std::{
    fs,
    os::unix::fs::symlink,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

use serde_json::json;

use super::*;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "devctl-project-scan-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join(".git")).unwrap();
        Self(root.canonicalize().unwrap())
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.0.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn package(&self, relative: &str, value: serde_json::Value) {
        self.write(relative, &value.to_string());
    }

    fn report(&self) -> Report {
        scan(&self.0).unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn prioritizes_declared_startup_scripts_without_suggesting_business_or_stop_actions() {
    let fixture = Fixture::new();
    fixture.package(
        "package.json",
        json!({"name":"demo","packageManager":"npm@10.9.0","scripts": {
            "dev":"vite", "dev:weapp":"taro", "start":"node server", "dev:stop":"kill all",
            "build":"build", "db:migrate":"migrate", "dev:seed":"seed", "dev:build":"build", "dev:Stop-api":"stop", "dev:restart":"restart"
        }}),
    );
    let report = fixture.report();
    assert!(report.problems.is_empty(), "{:?}", report.problems);
    let candidate = &report.candidates[0];
    assert_eq!(candidate.suggestions, ["dev", "dev:weapp", "start"]);
    assert_eq!(candidate.scripts.len(), 10);
    assert_eq!(
        candidate.manager.as_ref().unwrap().version.as_deref(),
        Some("10.9.0")
    );
}

#[test]
fn workspace_members_inherit_manager_and_one_installation_root() {
    let fixture = Fixture::new();
    fixture.package(
        "package.json",
        json!({"packageManager":"pnpm@10.3.0", "scripts":{"dev":"turbo dev"}}),
    );
    fixture.write(
        "pnpm-workspace.yaml",
        "packages:\n  - 'apps/*'\n  - 'packages/**'\n  - '!apps/ignored'\n",
    );
    fixture.package("apps/web/package.json", json!({"scripts":{"dev":"vite"}}));
    fixture.package(
        "apps/api/package.json",
        json!({"scripts":{"dev":"node server"}}),
    );
    fixture.package(
        "packages/nested/domain/package.json",
        json!({"name":"domain"}),
    );
    fixture.write("apps/ignored/package.json", "invalid ignored JSON");
    fixture.write("unrelated/package.json", "invalid unrelated JSON");
    let report = fixture.report();
    assert!(report.problems.is_empty(), "{:?}", report.problems);
    assert_eq!(report.candidates.len(), 4);
    assert_eq!(report.warnings.len(), 1);
    for candidate in &report.candidates {
        assert_eq!(candidate.dependency_root, fixture.0);
        assert_eq!(candidate.manager.as_ref().unwrap().name, "pnpm");
    }
    assert!(report.candidates[3].scripts.is_empty());
    let child = scan(&fixture.0.join("apps/web")).unwrap();
    assert_eq!(child.candidates.len(), 1);
    assert_eq!(child.candidates[0].dependency_root, fixture.0);
    assert_eq!(
        child.candidates[0]
            .manager
            .as_ref()
            .unwrap()
            .version
            .as_deref(),
        Some("10.3.0")
    );
}

#[test]
fn independent_projects_do_not_inherit_unrelated_parent_manager() {
    let fixture = Fixture::new();
    fixture.package("package.json", json!({"packageManager":"pnpm@10.3.0"}));
    fixture.package("web/package.json", json!({"scripts":{"dev":"vite"}}));
    fixture.write("web/package-lock.json", "{}");
    let report = fixture.report();
    assert!(report.problems.is_empty());
    let web = &report.candidates[1];
    assert_eq!(web.dependency_root, fixture.0.join("web"));
    assert_eq!(web.manager.as_ref().unwrap().name, "npm");
    assert!(report.warnings.is_empty());
}

#[test]
fn lockfile_inference_does_not_hide_declaration_or_member_conflicts() {
    let fixture = Fixture::new();
    fixture.package(
        "package.json",
        json!({"packageManager":"pnpm@10.3.0", "workspaces":["apps/*"]}),
    );
    fixture.package(
        "apps/web/package.json",
        json!({"packageManager":"npm@10.9.0"}),
    );
    fixture.write("package-lock.json", "{}");
    fixture.write("pnpm-lock.yaml", "lockfileVersion: '9.0'\n");
    let report = fixture.report();
    assert!(
        report
            .problems
            .iter()
            .any(|problem| problem.contains("Conflicting package managers"))
    );
    assert_eq!(report.candidates.len(), 2);
    fs::remove_file(fixture.0.join("package-lock.json")).unwrap();
    fixture.package("package.json", json!({"workspaces":["apps/*"]}));
    fixture.package(
        "apps/web/package.json",
        json!({"packageManager":"pnpm@10.3.0"}),
    );
    let report = fixture.report();
    assert!(report.problems.is_empty(), "{:?}", report.problems);
    assert_eq!(
        report.candidates[1]
            .manager
            .as_ref()
            .unwrap()
            .version
            .as_deref(),
        Some("10.3.0")
    );
}

#[test]
fn shrinkwrap_and_package_lock_are_one_manager_and_source_order_is_stable() {
    let fixture = Fixture::new();
    fixture.package("package.json", json!({"scripts":{"dev":"vite"}}));
    fixture.write("package-lock.json", "{}");
    fixture.write("npm-shrinkwrap.json", "{}");
    let report = fixture.report();
    assert!(report.problems.is_empty());
    assert_eq!(
        report.candidates[0].manager.as_ref().unwrap().source,
        fixture.0.join("npm-shrinkwrap.json")
    );
}

#[test]
fn source_versions_are_evidence_not_a_claim_of_environment_compatibility() {
    let fixture = Fixture::new();
    fixture.package(
        "package.json",
        json!({"engines":{"node":">=22 <25"},"volta":{"node":"24.18.0"}}),
    );
    fixture.write(".nvmrc", "24.18.0\n");
    fixture.write(".tool-versions", "python 3.12\nnodejs 24.18.0\n");
    let report = fixture.report();
    assert!(report.problems.is_empty());
    assert_eq!(report.candidates[0].evidence.len(), 4);
    assert!(
        !report.candidates[0]
            .evidence
            .iter()
            .any(|evidence| evidence.value.contains("python"))
    );
}

#[test]
fn skips_dependency_build_and_symlink_directories_and_resolves_input_aliases() {
    let fixture = Fixture::new();
    fixture.package("package.json", json!({}));
    for name in ["node_modules", "target", ".git", "dist", "build", ".next"] {
        fixture.write(&format!("{name}/hidden/package.json"), "invalid");
    }
    symlink(&fixture.0, fixture.0.join("loop")).unwrap();
    let alias = fixture.0.join("alias");
    symlink(&fixture.0, &alias).unwrap();
    let report = scan(&alias).unwrap();
    assert!(report.problems.is_empty());
    assert_eq!(report.root, fixture.0);
    assert_eq!(report.candidates.len(), 1);
}

#[test]
fn rejects_special_broken_and_escaping_declarations_but_accepts_contained_links() {
    let fixture = Fixture::new();
    fixture.write("declarations/package.json", "{}");
    symlink(
        fixture.0.join("declarations/package.json"),
        fixture.0.join("package.json"),
    )
    .unwrap();
    assert!(fixture.report().problems.is_empty());
    fs::remove_file(fixture.0.join("package.json")).unwrap();
    let outside = Fixture::new();
    outside.package("package.json", json!({}));
    symlink(
        outside.0.join("package.json"),
        fixture.0.join("package.json"),
    )
    .unwrap();
    assert!(
        fixture
            .report()
            .problems
            .iter()
            .any(|problem| problem.contains("escapes repository boundary"))
    );
    fs::remove_file(fixture.0.join("package.json")).unwrap();
    symlink(fixture.0.join("absent"), fixture.0.join("package.json")).unwrap();
    assert!(
        fixture
            .report()
            .problems
            .iter()
            .any(|problem| problem.contains("Cannot resolve declaration"))
    );
    fs::remove_file(fixture.0.join("package.json")).unwrap();
    nix::unistd::mkfifo(
        &fixture.0.join("package.json"),
        nix::sys::stat::Mode::S_IRUSR,
    )
    .unwrap();
    assert!(
        fixture
            .report()
            .problems
            .iter()
            .any(|problem| problem.contains("not a regular file"))
    );
}

#[test]
fn rejects_malformed_and_unsupported_declarations_without_guessing() {
    for (file, text, expected) in [
        ("package.json", "{broken", "Invalid"),
        (
            "package.json",
            r#"{"scripts":{"dev":17}}"#,
            "Invalid script",
        ),
        (
            "package.json",
            r#"{"packageManager":"pnpm@10.3.0+sha224.abc"}"#,
            "Unsupported package manager",
        ),
        (
            "package.json",
            r#"{"packageManager":"yarn@4.0.0"}"#,
            "Unsupported package manager",
        ),
        (
            "package.json",
            r#"{"workspaces":["apps/{web,api}"]}"#,
            "Unsupported workspace pattern",
        ),
        (
            "pnpm-workspace.yaml",
            "packages: !custom ['apps/*']\n",
            "Unsupported YAML",
        ),
        (
            "pnpm-workspace.yaml",
            "packages: &members ['apps/*']\n",
            "Unsupported YAML",
        ),
    ] {
        let fixture = Fixture::new();
        fixture.write(file, text);
        let report = fixture.report();
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.contains(expected)),
            "{file}: {:?}",
            report.problems
        );
    }
}

#[test]
fn nested_workspace_and_disagreeing_workspace_declarations_are_explicit_problems() {
    let fixture = Fixture::new();
    fixture.package("package.json", json!({"workspaces":["apps/*"]}));
    fixture.package(
        "apps/web/package.json",
        json!({"workspaces":["packages/*"]}),
    );
    fixture.package("apps/web/packages/lib/package.json", json!({}));
    assert!(
        fixture
            .report()
            .problems
            .iter()
            .any(|problem| problem.contains("Nested workspace"))
    );
    fixture.write("pnpm-workspace.yaml", "packages: ['other/*']\n");
    assert!(
        fixture
            .report()
            .problems
            .iter()
            .any(|problem| problem.contains("Conflicting npm/pnpm workspace"))
    );
}

#[test]
fn every_scan_budget_reports_incompleteness_instead_of_success() {
    for (limits, expected) in [
        (
            Limits {
                depth: 0,
                ..Limits::default()
            },
            "depth limit",
        ),
        (
            Limits {
                entries: 0,
                ..Limits::default()
            },
            "entry limit",
        ),
        (
            Limits {
                manifests: 0,
                ..Limits::default()
            },
            "manifest count limit",
        ),
        (
            Limits {
                declaration_bytes: 1,
                ..Limits::default()
            },
            "input byte limit",
        ),
        (
            Limits {
                total_bytes: 1,
                ..Limits::default()
            },
            "input byte limit",
        ),
        (
            Limits {
                lock_bytes: 1,
                ..Limits::default()
            },
            "input byte limit",
        ),
    ] {
        let fixture = Fixture::new();
        fixture.package("package.json", json!({}));
        fixture.write("package-lock.json", "{}");
        fixture.package("child/package.json", json!({}));
        let report = scan_with_limits(&fixture.0, limits).unwrap();
        assert!(
            report
                .problems
                .iter()
                .any(|problem| problem.contains(expected)),
            "{:?}",
            report.problems
        );
    }
    let fixture = Fixture::new();
    let error = scan_with_limits(
        &fixture.0,
        Limits {
            elapsed: Duration::ZERO,
            ..Limits::default()
        },
    )
    .unwrap_err();
    assert!(error.contains("time limit"));
}

#[test]
fn terminal_controls_are_escaped_and_readable_text_is_preserved() {
    assert_eq!(safe("项目\x1b[2J\r\n\t"), "项目\\u{1b}[2J\\r\\n\\t");
}

#[test]
fn advisory_development_tools_do_not_override_explicit_package_manager() {
    let fixture = Fixture::new();
    for policy in ["warn", "ignore"] {
        fixture.package(
            "package.json",
            json!({
                "packageManager":"pnpm@10.3.0",
                "devEngines":{"packageManager":{"name":"npm", "version":"^11", "onFail":policy}}
            }),
        );
        let report = fixture.report();
        assert!(report.problems.is_empty(), "{:?}", report.problems);
        assert_eq!(report.candidates[0].manager.as_ref().unwrap().name, "pnpm");
        assert!(
            report
                .warnings
                .iter()
                .any(|warning| warning.contains("Advisory"))
        );
    }
    fixture.package(
        "package.json",
        json!({"devEngines":{"packageManager":{"name":"npm"}}}),
    );
    let report = fixture.report();
    assert!(report.problems.is_empty());
    assert_eq!(report.candidates[0].manager.as_ref().unwrap().name, "npm");
}

#[test]
fn bounded_parent_inspection_does_not_claim_a_distant_workspace_is_absent() {
    let fixture = Fixture::new();
    let deep = fixture.0.join(vec!["nested"; 17].join("/"));
    fs::create_dir_all(&deep).unwrap();
    let error = scan(&deep).unwrap_err();
    assert!(error.contains("parent inspection limit"));
}

#[test]
fn unrelated_parent_declarations_do_not_make_a_child_project_unsupported() {
    let fixture = Fixture::new();
    fixture.package("package.json", json!({"packageManager":"yarn@4.0.0"}));
    fixture.package("child/package.json", json!({"packageManager":"npm@10.9.0"}));
    let report = scan(&fixture.0.join("child")).unwrap();
    assert!(report.problems.is_empty(), "{:?}", report.problems);
    assert_eq!(
        report.candidates[0].dependency_root,
        fixture.0.join("child")
    );
}

#[test]
fn declared_patterns_prune_unrelated_trees_before_consuming_scan_budgets() {
    let fixture = Fixture::new();
    fixture.package("package.json", json!({"workspaces":["apps/*"]}));
    fixture.package("apps/web/package.json", json!({"scripts":{"dev":"vite"}}));
    fixture.write("unrelated/deep/a/b/c/d/e/f/g/package.json", "invalid");
    // Matching the package itself does not require scanning its source subtree.
    fixture.write("apps/web/source/deep/a/b/c/d/e/f/package.json", "invalid");
    let report = scan_with_limits(
        &fixture.0,
        Limits {
            entries: 10,
            ..Limits::default()
        },
    )
    .unwrap();
    assert!(report.problems.is_empty(), "{:?}", report.problems);
    assert_eq!(report.candidates.len(), 2);
}

#[test]
fn recursive_and_question_patterns_include_only_members_and_prune_excluded_subtrees() {
    let fixture = Fixture::new();
    fixture.package(
        "package.json",
        json!({"workspaces":["apps/**/web?", "!apps/ignored/**"]}),
    );
    fixture.package("apps/web1/package.json", json!({}));
    fixture.package("apps/group/web2/package.json", json!({}));
    fixture.write("apps/ignored/web3/package.json", "invalid");
    fixture.package("apps/group/web-long/package.json", json!({}));
    let report = fixture.report();
    assert!(report.problems.is_empty(), "{:?}", report.problems);
    assert_eq!(report.candidates.len(), 3);
}

#[test]
fn absent_git_does_not_allow_declaration_links_to_escape_the_project() {
    let fixture = Fixture::new();
    fs::remove_dir(fixture.0.join(".git")).unwrap();
    fixture.write("declarations/package.json", "{}");
    symlink(
        fixture.0.join("declarations/package.json"),
        fixture.0.join("package.json"),
    )
    .unwrap();
    assert!(fixture.report().problems.is_empty());
    fs::remove_file(fixture.0.join("package.json")).unwrap();
    let outside = Fixture::new();
    outside.package("package.json", json!({}));
    symlink(
        outside.0.join("package.json"),
        fixture.0.join("package.json"),
    )
    .unwrap();
    assert!(
        fixture
            .report()
            .problems
            .iter()
            .any(|problem| problem.contains("escapes repository boundary"))
    );
}

#[test]
fn workspace_pattern_count_size_and_component_limits_are_enforced() {
    for patterns in [
        vec!["apps/*".to_owned(); 201],
        vec!["a".repeat(4097)],
        vec![vec!["a"; 33].join("/")],
    ] {
        let fixture = Fixture::new();
        fixture.package("package.json", json!({"workspaces":patterns}));
        let report = fixture.report();
        assert!(!report.problems.is_empty());
        assert_eq!(report.candidates.len(), 1);
    }
}
