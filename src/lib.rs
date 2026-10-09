extern crate self as purist;

pub mod allow;
pub mod cargo;
pub mod checkers;
pub mod command;
pub mod diagnostics;
pub mod discovery;
pub mod engine;
pub mod reporter;
pub mod rule_config;
pub mod rules;
pub mod scopes;

pub use allow::{allow_project, canonical_rule_name, disable_rules_in_manifest};
pub use cargo::{CargoManifest, LintConfig, PuristLintsConfig, RuleLevel};
pub use checkers::{
    ReceiverKind, check_call_matches_path, check_expr_matches_path, check_fn_receiver,
    check_ident_has_negative_name, check_ident_has_prefix, check_ident_has_test_prefix,
    check_macro_matches, check_method_call_matches_name, check_path_matches,
    extract_fn_return_type, extract_result_error_type,
};
pub use command::{PuristCommand, PuristError};
pub use diagnostics::{Diagnostic, DiagnosticReport, ReportSummary, Severity, Span};
pub use discovery::{
    discover_project_files, discover_project_files_from_manifest, discover_rust_files,
    find_cargo_toml, find_workspace_cargo_toml,
};
pub use engine::{LintContext, PuristEngine, Rule};
pub use reporter::{OutputFormat, render_report, render_report_with_options};
pub use rules::default_rules;
pub use scopes::{
    BlockScope, ClapScope, DepthScope, FlagScope, MainScope, SuppressionScope, TestScope,
    TypeScope, WithBlockScope, WithClapScope, WithDepthScope, WithMainScope, WithSuppressionScope,
    WithTestScope, WithTypeScope,
};

#[cfg(test)]
mod tests {
    use googletest::prelude::*;
    use std::fs;
    use std::path::Path;

    #[googletest::test]
    fn verify_msrv_badge_consistency_succeeds() -> Result<(), Box<dyn std::error::Error>> {
        let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
        let cargo_toml_content = fs::read_to_string(manifest_dir.join("Cargo.toml"))?;
        let rust_version_line = cargo_toml_content
            .lines()
            .find(|line| line.trim().starts_with("rust-version"))
            .ok_or("rust-version not found in Cargo.toml")?;
        let rust_version = rust_version_line
            .split('=')
            .nth(1)
            .ok_or("Invalid rust-version entry")?
            .trim()
            .trim_matches('"');

        let mut parts = rust_version.split('.');
        let major = parts.next().ok_or("Missing major version component")?;
        let minor = parts.next().ok_or("Missing minor version component")?;
        let badge_target = format!("https://img.shields.io/badge/MSRV-{major}.{minor}%2B-");

        let readme_content = fs::read_to_string(manifest_dir.join("README.md"))?;
        assert_that!(readme_content.contains(&badge_target), is_true());

        Ok(())
    }
}
