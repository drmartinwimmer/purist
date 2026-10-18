//! Rule: `purist::config_struct_naming`
//!
//! # What This Rule Does
//! Enforces that structs representing application or module configuration, manifests, or metadata
//! follow the `<Subject>Config`, `<Subject>Manifest`, or `<Subject>Metadata` naming convention
//! (e.g. `UserConfig`, `ProjectConfig`, `CargoManifest`, `PackageMetadata`),
//! while ensuring non-configuration structs that derive `serde::Serialize` or `serde::Deserialize`
//! (such as runtime state containers, cache entries, session tokens, or API payloads) are not flagged.
//!
//! # Why This Rule Exists
//! Configuration models are foundational boundary types across a workspace. Naming them
//! inconsistently (`*Settings`, `*Options`, `*Parameters`, `Configuration`, `Profile`, or bare
//! `Config`, `Manifest`, `Metadata`) obscures which types represent configuration and degrades discoverability.
//! At the same time, because `Serialize` and `Deserialize` are derived ubiquitously on DTOs,
//! persistence records, and protocol messages, this rule scopes enforcement to configuration
//! modules (`config.rs`, `settings.rs`, `manifest.rs`, `metadata.rs`, `*config*`, `*manifest*`, `*metadata*`),
//! configuration attributes/loaders, and explicit configuration-synonym suffixes (`*Settings`, `*Configuration`).
//!
//! # Non-Compliant Example
//! ```rust,ignore
//! // In src/config.rs:
//! #[derive(serde::Deserialize)]
//! pub struct DatabaseSettings { // Should be 'DatabaseConfig'
//!     pub url: String,
//! }
//!
//! #[derive(serde::Deserialize)]
//! pub struct ServerOptions { // Should be 'ServerConfig'
//!     pub port: u16,
//! }
//! ```
//!
//! # Compliant Example
//! ```rust,ignore
//! // In src/config.rs:
//! #[derive(serde::Deserialize)]
//! pub struct DatabaseConfig {
//!     pub url: String,
//! }
//!
//! // In src/manifest.rs:
//! #[derive(serde::Deserialize)]
//! pub struct PackageManifest {
//!     pub name: String,
//! }
//!
//! // In src/api/models.rs (non-config context):
//! #[derive(serde::Serialize, serde::Deserialize)]
//! pub struct UserSession { // Permitted: non-config Serde model outside config context
//!     pub token: String,
//! }
//! ```

use super::common::{
    TestScope, WithTestScope, derives_any, extract_type_ident, has_suppression_attribute,
};
use crate::checkers::check_call_matches_path;
use crate::diagnostics::{Diagnostic, Severity};
use crate::engine::{LintContext, Rule};
use crate::scopes::{TypeScope, WithTypeScope, derives_clap};
use std::collections::HashSet;
use std::path::Path;
use syn::visit::{self, Visit};

/// Rule enforcing `<Subject>Config`, `<Subject>Manifest`, or `<Subject>Metadata` naming for configuration structs while permitting non-config Serde models.
pub(crate) struct ConfigStructNamingRule;

impl Rule for ConfigStructNamingRule {
    fn name(&self) -> &'static str {
        "purist::config_struct_naming"
    }

    fn check_file(&self, ctx: &LintContext<'_>, file: &syn::File) -> Vec<Diagnostic> {
        if ctx.is_test_file() {
            return Vec::new();
        }

        let in_config_file = is_config_module_path(ctx.file_path());
        let in_tool_file = is_tool_module_path(ctx.file_path());
        let loader_structs = ConfigLoaderCollector::collect_for_file(file, in_tool_file);

        let mut visitor = ConfigStructNamingVisitor {
            ctx,
            diagnostics: Vec::new(),
            test_scope: TestScope::new(ctx.is_test_file()),
            in_config_file,
            config_mod_depth: 0,
            loader_structs,
        };

        visitor.visit_file(file);
        visitor.diagnostics
    }
}

/// Returns true if the file path corresponds to a dedicated configuration, settings, manifest, or metadata module,
/// excluding linter rule implementation files under `rules/`.
fn is_config_module_path(path: &Path) -> bool {
    let components: Vec<String> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_ascii_lowercase())
        .collect();

    if components.iter().any(|c| c == "rules") {
        return false;
    }

    if components.iter().any(|c| {
        matches!(
            c.as_str(),
            "config"
                | "configs"
                | "configuration"
                | "settings"
                | "manifest"
                | "manifests"
                | "metadata"
        )
    }) {
        return true;
    }

    let Some(stem) = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
    else {
        return false;
    };

    matches!(
        stem.as_str(),
        "config" | "configs" | "configuration" | "settings" | "manifest" | "manifests" | "metadata"
    ) || stem.starts_with("config_")
        || stem.ends_with("_config")
        || stem.ends_with("_configs")
        || stem.starts_with("settings_")
        || stem.ends_with("_settings")
        || stem.starts_with("manifest_")
        || stem.ends_with("_manifest")
        || stem.ends_with("_manifests")
        || stem.starts_with("metadata_")
        || stem.ends_with("_metadata")
}

/// Returns true if the file path corresponds to a dedicated tool or command-adapter module,
/// where structs represent external tool wrappers or sessions rather than configuration models.
fn is_tool_module_path(path: &Path) -> bool {
    let components: Vec<String> = path
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_ascii_lowercase())
        .collect();

    if components.iter().any(|c| c == "rules") {
        return false;
    }

    if components
        .iter()
        .any(|c| matches!(c.as_str(), "tool" | "tools"))
    {
        return true;
    }

    let Some(stem) = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_ascii_lowercase())
    else {
        return false;
    };

    matches!(stem.as_str(), "tool" | "tools")
        || stem.starts_with("tool_")
        || stem.ends_with("_tool")
        || stem.ends_with("_tools")
}

/// First-pass collector identifying passive configuration structs whose `impl` blocks define
/// configuration loader methods (e.g. `from_env`, `load_config`) or read environment variables
/// via `std::env::var`, while excluding active behavioral/service structs.
#[derive(Default, WithTypeScope)]
struct ConfigLoaderCollector {
    type_scope: TypeScope,
    loader_structs: HashSet<String>,
    behavioral_structs: HashSet<String>,
}

impl ConfigLoaderCollector {
    /// Collects struct names that act as configuration loaders in non-tool files.
    fn collect_for_file(file: &syn::File, in_tool_file: bool) -> HashSet<String> {
        if in_tool_file {
            return HashSet::new();
        }
        let mut collector = Self::default();
        collector.visit_file(file);
        collector
            .loader_structs
            .into_iter()
            .filter(|name| !collector.behavioral_structs.contains(name))
            .collect()
    }
}

impl<'ast> Visit<'ast> for ConfigLoaderCollector {
    fn visit_item_impl(&mut self, item_impl: &'ast syn::ItemImpl) {
        if item_impl.trait_.is_none()
            && let Some(ident) = extract_type_ident(&item_impl.self_ty)
        {
            self.with_type_impl(ident.to_string(), |this| {
                visit::visit_item_impl(this, item_impl);
            });
        } else {
            visit::visit_item_impl(self, item_impl);
        }
    }

    fn visit_impl_item_fn(&mut self, impl_fn: &'ast syn::ImplItemFn) {
        if let Some(struct_name) = self.type_scope.current_impl_name() {
            let fn_name = impl_fn.sig.ident.to_string();
            if is_config_loader_fn_name(&fn_name) {
                self.loader_structs.insert(struct_name.to_string());
            } else if impl_fn.sig.receiver().is_some() && !is_config_like_struct_name(struct_name) {
                self.behavioral_structs.insert(struct_name.to_string());
            }
        }
        visit::visit_impl_item_fn(self, impl_fn);
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let Some(struct_name) = self.type_scope.current_impl_name() {
            const ENV_TARGETS: &[&[&str]] = &[&["env", "var"], &["env", "var_os"]];
            if check_call_matches_path(call, ENV_TARGETS) {
                self.loader_structs.insert(struct_name.to_string());
            }
        }
        visit::visit_expr_call(self, call);
    }
}

/// Returns true if an associated function name indicates a configuration loader entrypoint.
fn is_config_loader_fn_name(name: &str) -> bool {
    matches!(
        name,
        "from_env"
            | "load_config"
            | "from_config_file"
            | "from_manifest"
            | "from_manifest_content"
            | "from_manifest_file"
            | "from_metadata"
            | "from_metadata_file"
    )
}

/// Returns true if a struct name uses a configuration, settings, manifest, metadata, or options/parameters suffix.
fn is_config_like_struct_name(name: &str) -> bool {
    is_explicit_config_synonym_name(name)
        || matches!(name, "Options" | "Parameters" | "Params")
        || name.ends_with("Options")
        || name.ends_with("Parameters")
        || name.ends_with("Params")
        || name.ends_with("Env")
}

/// Visitor that inspects struct definitions and enforces `<Subject>Config`, `<Subject>Manifest`,
/// or `<Subject>Metadata` naming for configuration structs.
#[derive(WithTestScope)]
struct ConfigStructNamingVisitor<'a> {
    ctx: &'a LintContext<'a>,
    diagnostics: Vec<Diagnostic>,
    test_scope: TestScope,
    in_config_file: bool,
    config_mod_depth: usize,
    loader_structs: HashSet<String>,
}

impl<'ast> Visit<'ast> for ConfigStructNamingVisitor<'_> {
    fn visit_item_mod(&mut self, item_mod: &'ast syn::ItemMod) {
        let mod_name = item_mod.ident.to_string().to_ascii_lowercase();
        let is_config_mod = matches!(
            mod_name.as_str(),
            "config"
                | "configs"
                | "configuration"
                | "settings"
                | "manifest"
                | "manifests"
                | "metadata"
        ) || mod_name.ends_with("_config")
            || mod_name.ends_with("_settings")
            || mod_name.ends_with("_manifest")
            || mod_name.ends_with("_metadata");

        if is_config_mod {
            self.config_mod_depth += 1;
        }

        self.with_test_mod(&item_mod.attrs, |this| {
            visit::visit_item_mod(this, item_mod);
        });

        if is_config_mod {
            self.config_mod_depth = self.config_mod_depth.saturating_sub(1);
        }
    }

    fn visit_item_struct(&mut self, item_struct: &'ast syn::ItemStruct) {
        if !self.test_scope.is_in_test()
            && !has_suppression_attribute(&item_struct.attrs, "config_struct_naming")
        {
            self.check_struct_config_naming(item_struct);
        }

        visit::visit_item_struct(self, item_struct);
    }
}

impl ConfigStructNamingVisitor<'_> {
    fn check_struct_config_naming(&mut self, item_struct: &syn::ItemStruct) {
        if derives_clap(&item_struct.attrs) {
            return;
        }

        let has_named_fields =
            matches!(&item_struct.fields, syn::Fields::Named(f) if !f.named.is_empty());
        if !has_named_fields {
            return;
        }

        let struct_name = item_struct.ident.to_string();
        if is_exempt_infrastructure_struct(&struct_name) {
            return;
        }

        let in_config_context = self.in_config_file || self.config_mod_depth > 0;
        let has_config_attrs = has_configuration_attribute(&item_struct.attrs);
        let has_config_loader = self.loader_structs.contains(&struct_name);
        let is_explicit_config_synonym = is_explicit_config_synonym_name(&struct_name)
            && (in_config_context
                || derives_any(&item_struct.attrs, &["Deserialize", "Serialize", "Default"]));

        let represents_config = in_config_context
            || has_config_attrs
            || has_config_loader
            || is_explicit_config_synonym;

        if !represents_config || is_valid_config_struct_name(&struct_name) {
            return;
        }

        let suggested_name = suggest_config_struct_name(&struct_name);
        let span = self.ctx.to_span(item_struct.ident.span());

        self.diagnostics.push(
            Diagnostic::new(
                "purist::config_struct_naming",
                Severity::Warning,
                format!(
                    "Configuration struct '{struct_name}' does not follow the '<Subject>Config', '<Subject>Manifest', or '<Subject>Metadata' naming convention. Rename it to '{suggested_name}'."
                ),
            )
            .with_span(span)
            .with_suggested_fix(format!(
                "Rename configuration struct '{struct_name}' to '{suggested_name}'."
            )),
        );
    }
}

/// Checks whether a struct name satisfies `<Subject>Config`, `<Subject>Manifest`, or `<Subject>Metadata`
/// (with a non-empty `<Subject>` prefix).
fn is_valid_config_struct_name(name: &str) -> bool {
    ["Config", "Manifest", "Metadata"].iter().any(|suffix| {
        name.strip_suffix(suffix)
            .is_some_and(|prefix| !prefix.is_empty())
    })
}

/// Checks whether a struct has explicit configuration framework attributes (`#[config(...)]`, `#[envconfig(...)]`, `#[confique(...)]`).
fn has_configuration_attribute(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path().is_ident("config")
            || attr.path().is_ident("envconfig")
            || attr.path().is_ident("confique")
    }) || derives_any(attrs, &["Envconfig", "Confique", "Config"])
}

/// Returns true if the struct name uses an unambiguous configuration-synonym suffix (`*Settings`, `*Configuration`, `Configuration`, `Settings`, or bare `Config`, `Manifest`, `Metadata`).
fn is_explicit_config_synonym_name(name: &str) -> bool {
    matches!(
        name,
        "Configuration" | "Settings" | "Config" | "Manifest" | "Metadata"
    ) || name.ends_with("Settings")
        || name.ends_with("Configuration")
        || name.ends_with("ConfigOptions")
        || name.ends_with("ConfigParameters")
        || name.ends_with("ConfigParams")
}

/// Returns true if the struct name represents a non-config helper/infrastructure type inside a config module.
fn is_exempt_infrastructure_struct(name: &str) -> bool {
    const EXEMPT_SUFFIXES: &[&str] = &[
        "Error",
        "Visitor",
        "Builder",
        "Guard",
        "Rule",
        "Loader",
        "Context",
        "Scope",
        "Summary",
        "Report",
        "Span",
        "Diagnostic",
    ];
    EXEMPT_SUFFIXES.iter().any(|suffix| name.ends_with(suffix))
}

/// Produces a suggested `<Subject>Config`, `<Subject>Manifest`, or `<Subject>Metadata` replacement identifier for a non-compliant config struct name.
fn suggest_config_struct_name(name: &str) -> String {
    for suffix in &[
        "Configuration",
        "ConfigOptions",
        "ConfigParameters",
        "ConfigParams",
        "Settings",
        "Options",
        "Parameters",
        "Params",
    ] {
        if let Some(prefix) = name.strip_suffix(suffix)
            && !prefix.is_empty()
        {
            return format!("{prefix}Config");
        }
    }

    if name == "Manifest" {
        return "AppManifest".to_string();
    }

    if name == "Metadata" {
        return "AppMetadata".to_string();
    }

    if matches!(
        name,
        "Config" | "Configuration" | "Settings" | "Options" | "Parameters" | "Params"
    ) {
        return "AppConfig".to_string();
    }

    format!("{name}Config")
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn subject_config_structs_in_config_module_are_permitted()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[derive(serde::Deserialize)]
pub struct UserConfig {
    pub username: String,
}

#[derive(serde::Deserialize)]
pub struct ProjectConfig {
    pub root: String,
}

#[derive(serde::Deserialize)]
pub struct DatabaseConfig {
    pub url: String,
}
"#;
        let ctx = LintContext::new(Path::new("src/config.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ConfigStructNamingRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn settings_options_and_parameters_in_config_module_are_flagged()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[derive(serde::Deserialize)]
pub struct DatabaseSettings {
    pub url: String,
}

#[derive(serde::Deserialize)]
pub struct ServerOptions {
    pub port: u16,
}

#[derive(serde::Deserialize)]
pub struct RetryParameters {
    pub max_attempts: u32,
}

#[derive(serde::Deserialize)]
pub struct Profile {
    pub name: String,
}
"#;
        let ctx = LintContext::new(Path::new("src/config.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ConfigStructNamingRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(4));
        let d0 = diags.first().ok_or("expected d0")?;
        let d1 = diags.get(1).ok_or("expected d1")?;
        let d2 = diags.get(2).ok_or("expected d2")?;
        let d3 = diags.get(3).ok_or("expected d3")?;
        assert_that!(&d0.rule, eq("purist::config_struct_naming"));
        assert_that!(
            &d0.message,
            contains_substring("Rename it to 'DatabaseConfig'")
        );
        assert_that!(
            &d1.message,
            contains_substring("Rename it to 'ServerConfig'")
        );
        assert_that!(
            &d2.message,
            contains_substring("Rename it to 'RetryConfig'")
        );
        assert_that!(
            &d3.message,
            contains_substring("Rename it to 'ProfileConfig'")
        );
        Ok(())
    }

    #[googletest::test]
    fn bare_config_and_configuration_names_are_flagged() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[derive(serde::Deserialize)]
pub struct Config {
    pub debug: bool,
}

#[derive(serde::Deserialize)]
pub struct Configuration {
    pub timeout: u64,
}
"#;
        let ctx = LintContext::new(Path::new("src/settings.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ConfigStructNamingRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(2));
        let d0 = diags.first().ok_or("expected d0")?;
        let d1 = diags.get(1).ok_or("expected d1")?;
        assert_that!(&d0.message, contains_substring("Rename it to 'AppConfig'"));
        assert_that!(&d1.message, contains_substring("Rename it to 'AppConfig'"));
        Ok(())
    }

    #[googletest::test]
    fn non_config_serde_models_outside_config_context_are_permitted()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[derive(serde::Serialize, serde::Deserialize)]
pub struct UserSession {
    pub token: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct CacheEntry {
    pub key: String,
    pub ttl: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct ApiResponse {
    pub status: u16,
}
"#;
        let ctx = LintContext::new(Path::new("src/models/session.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ConfigStructNamingRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn struct_with_env_loader_or_config_attr_outside_config_file_is_flagged()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct RuntimeParameters {
    pub endpoint: String,
}

impl RuntimeParameters {
    pub fn from_env() -> Self {
        let endpoint = std::env::var("ENDPOINT").unwrap_or_default();
        Self { endpoint }
    }
}

#[config(prefix = "APP")]
pub struct AppOptions {
    pub workers: usize,
}
"#;
        let ctx = LintContext::new(Path::new("src/runtime.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ConfigStructNamingRule.check_file(&ctx, &ast);

        assert_that!(diags.len(), eq(2));
        let d0 = diags.first().ok_or("expected d0")?;
        let d1 = diags.get(1).ok_or("expected d1")?;
        assert_that!(
            &d0.message,
            contains_substring("Rename it to 'RuntimeConfig'")
        );
        assert_that!(&d1.message, contains_substring("Rename it to 'AppConfig'"));
        Ok(())
    }

    #[googletest::test]
    fn suppressed_config_struct_is_permitted() -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[expect(purist::config_struct_naming, reason = "External schema compatibility")]
#[derive(serde::Deserialize)]
pub struct LegacySettings {
    pub enabled: bool,
}
"#;
        let ctx = LintContext::new(Path::new("src/config.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ConfigStructNamingRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn tool_session_struct_reading_env_or_defining_from_env_is_permitted()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct Herdr {
    session: String,
    tab_id: Option<String>,
}

impl Herdr {
    pub fn from_env() -> Self {
        let session = std::env::var("HERDR_SESSION").unwrap_or_default();
        let tab_id = std::env::var("HERDR_TAB_ID").ok();
        Self { session, tab_id }
    }

    pub fn from_config(session: String) -> Self {
        Self { session, tab_id: None }
    }

    pub fn is_inside_herdr() -> bool {
        std::env::var("HERDR_AGENT").is_ok()
    }

    pub fn create_tab(&self, name: &str) -> Result<(), String> {
        let _ = (&self.session, &self.tab_id, name);
        Ok(())
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/tools/herdr.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ConfigStructNamingRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn behavioral_service_struct_with_from_env_outside_config_is_permitted()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
pub struct SessionClient {
    endpoint: String,
}

impl SessionClient {
    pub fn from_env() -> Self {
        let endpoint = std::env::var("SESSION_ENDPOINT").unwrap_or_default();
        Self { endpoint }
    }

    pub fn execute(&self, command: &str) -> String {
        format!("{}:{}", self.endpoint, command)
    }
}
"#;
        let ctx = LintContext::new(Path::new("src/session.rs"), source);
        let ast = syn::parse_file(source)?;
        let diags = ConfigStructNamingRule.check_file(&ctx, &ast);

        assert_that!(diags.is_empty(), is_true());
        Ok(())
    }

    #[googletest::test]
    fn subject_manifest_and_metadata_structs_in_config_modules_are_permitted()
    -> Result<(), Box<dyn std::error::Error>> {
        let source = r#"
#[derive(serde::Deserialize)]
pub struct PackageManifest {
    pub name: String,
}

#[derive(serde::Deserialize)]
pub struct WorkspaceMetadata {
    pub members: Vec<String>,
}
"#;
        for path in [
            Path::new("src/config.rs"),
            Path::new("src/manifest.rs"),
            Path::new("src/metadata.rs"),
        ] {
            let ctx = LintContext::new(path, source);
            let ast = syn::parse_file(source)?;
            let diags = ConfigStructNamingRule.check_file(&ctx, &ast);
            assert_that!(diags.is_empty(), is_true());
        }
        Ok(())
    }

    #[googletest::test]
    fn bare_manifest_and_metadata_or_unprefixed_structs_in_manifest_and_metadata_modules_are_flagged()
    -> Result<(), Box<dyn std::error::Error>> {
        let manifest_source = r#"
#[derive(serde::Deserialize)]
pub struct Manifest {
    pub name: String,
}

#[derive(serde::Deserialize)]
pub struct Dependencies {
    pub items: Vec<String>,
}
"#;
        let manifest_ctx = LintContext::new(Path::new("src/manifest.rs"), manifest_source);
        let manifest_ast = syn::parse_file(manifest_source)?;
        let manifest_diags = ConfigStructNamingRule.check_file(&manifest_ctx, &manifest_ast);

        assert_that!(manifest_diags.len(), eq(2));
        let m0 = manifest_diags.first().ok_or("expected m0")?;
        let m1 = manifest_diags.get(1).ok_or("expected m1")?;
        assert_that!(
            &m0.message,
            contains_substring("Rename it to 'AppManifest'")
        );
        assert_that!(
            &m1.message,
            contains_substring("Rename it to 'DependenciesConfig'")
        );

        let metadata_source = r#"
#[derive(serde::Deserialize)]
pub struct Metadata {
    pub version: String,
}
"#;
        let metadata_ctx = LintContext::new(Path::new("src/metadata.rs"), metadata_source);
        let metadata_ast = syn::parse_file(metadata_source)?;
        let metadata_diags = ConfigStructNamingRule.check_file(&metadata_ctx, &metadata_ast);

        assert_that!(metadata_diags.len(), eq(1));
        let md0 = metadata_diags.first().ok_or("expected md0")?;
        assert_that!(
            &md0.message,
            contains_substring("Rename it to 'AppMetadata'")
        );
        Ok(())
    }
}
