pub(crate) mod centralized_command_execution;
pub(crate) mod clap_struct_encapsulation;
pub(crate) mod cli_run_consumes_self;
pub(crate) mod clippy_suppression_hygiene;
pub(crate) mod command_struct_naming;
pub(crate) mod common;
pub(crate) mod config_struct_naming;
pub(crate) mod descriptive_checker_names;
pub(crate) mod error_types;
pub(crate) mod exit_code_hygiene;
pub(crate) mod free_functions;
pub(crate) mod googletest_conventions;
pub(crate) mod granular_visitor_hooks;
pub(crate) mod idiomatic_option_bool_mapping;
pub(crate) mod lib_facade_hygiene;
pub(crate) mod max_file_lines;
pub(crate) mod max_nesting_depth;
pub(crate) mod no_bifurcated_trait_dispatch;
pub(crate) mod no_boxed_dyn_error;
pub(crate) mod no_double_negation;
pub(crate) mod no_env_access_outside_config;
pub(crate) mod no_has_trait_prefix;
pub(crate) mod no_inline_mods;
pub(crate) mod no_interior_mutability_in_visitors;
pub(crate) mod no_negative_bool;
pub(crate) mod no_primitive_flag_clutter;
pub(crate) mod no_println_in_libraries;
pub(crate) mod no_redundant_conversions;
pub(crate) mod no_redundant_wrappers;
pub(crate) mod no_test_prefix;
pub(crate) mod no_trivial_getters_setters;
pub(crate) mod no_unsafe_in_tests;
pub(crate) mod no_wildcard_imports;
pub(crate) mod path_resolution;
pub(crate) mod prefer_scoped_closures;
pub(crate) mod raii_temp_directories;
pub(crate) mod single_match_to_let_else;
pub(crate) mod test_matcher_borrow_simplification;
pub(crate) mod test_patterns;
pub(crate) mod use_declarations_over_qualified_paths;

pub use centralized_command_execution::CentralizedCommandExecutionDiagnostic;
pub use clap_struct_encapsulation::ClapStructEncapsulationDiagnostic;
pub use cli_run_consumes_self::CliRunConsumesSelfDiagnostic;
pub use clippy_suppression_hygiene::ClippySuppressionHygieneDiagnostic;
pub use command_struct_naming::CommandStructNamingDiagnostic;
pub use config_struct_naming::ConfigStructNamingDiagnostic;
pub use descriptive_checker_names::DescriptiveCheckerNamesDiagnostic;
pub use error_types::ErrorTypesDiagnostic;
pub use exit_code_hygiene::ExitCodeHygieneDiagnostic;
pub use free_functions::FreeFunctionsDiagnostic;
pub use googletest_conventions::GoogletestConventionsDiagnostic;
pub use granular_visitor_hooks::GranularVisitorHooksDiagnostic;
pub use idiomatic_option_bool_mapping::IdiomaticOptionBoolMappingDiagnostic;
pub use lib_facade_hygiene::LibFacadeHygieneDiagnostic;
pub use max_file_lines::MaxFileLinesDiagnostic;
pub use max_nesting_depth::MaxNestingDepthDiagnostic;
pub use no_bifurcated_trait_dispatch::NoBifurcatedTraitDispatchDiagnostic;
pub use no_boxed_dyn_error::NoBoxedDynErrorDiagnostic;
pub use no_double_negation::NoDoubleNegationDiagnostic;
pub use no_env_access_outside_config::NoEnvAccessOutsideConfigDiagnostic;
pub use no_has_trait_prefix::NoHasTraitPrefixDiagnostic;
pub use no_inline_mods::NoInlineModsDiagnostic;
pub use no_interior_mutability_in_visitors::NoInteriorMutabilityInVisitorsDiagnostic;
pub use no_negative_bool::NoNegativeBoolDiagnostic;
pub use no_primitive_flag_clutter::NoPrimitiveFlagClutterDiagnostic;
pub use no_println_in_libraries::NoPrintlnInLibrariesDiagnostic;
pub use no_redundant_conversions::NoRedundantConversionsDiagnostic;
pub use no_redundant_wrappers::NoRedundantWrappersDiagnostic;
pub use no_test_prefix::NoTestPrefixDiagnostic;
pub use no_trivial_getters_setters::NoTrivialGettersSettersDiagnostic;
pub use no_unsafe_in_tests::NoUnsafeInTestsDiagnostic;
pub use no_wildcard_imports::NoWildcardImportsDiagnostic;
pub use path_resolution::PathResolutionDiagnostic;
pub use prefer_scoped_closures::PreferScopedClosuresDiagnostic;
pub use raii_temp_directories::RaiiTempDirectoriesDiagnostic;
pub use single_match_to_let_else::SingleMatchToLetElseDiagnostic;
pub use test_matcher_borrow_simplification::TestMatcherBorrowSimplificationDiagnostic;
pub use test_patterns::TestPatternsDiagnostic;
pub use use_declarations_over_qualified_paths::UseDeclarationsOverQualifiedPathsDiagnostic;

/// Top-level `miette::Diagnostic` enum flattening all built-in `purist` rule diagnostics.
///
/// Each variant delegates `Display`, `Error`, `code()`, `severity()`, and `help()` transparently
/// to its per-rule `<RuleName>Diagnostic` type, and `#[derive(RuleRegistry)]` auto-generates
/// `RuleId`, `PuristLintsConfig`, and `default_rules()` from this single definition.
#[derive(
    Debug, Clone, PartialEq, Eq, thiserror::Error, miette::Diagnostic, purist_derive::RuleRegistry,
)]
pub enum RuleDiagnostic {
    #[error(transparent)]
    #[diagnostic(transparent)]
    CentralizedCommandExecution(#[from] CentralizedCommandExecutionDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    ClapStructEncapsulation(#[from] ClapStructEncapsulationDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    CliRunConsumesSelf(#[from] CliRunConsumesSelfDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    ClippySuppressionHygiene(#[from] ClippySuppressionHygieneDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    CommandStructNaming(#[from] CommandStructNamingDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    ConfigStructNaming(#[from] ConfigStructNamingDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    DescriptiveCheckerNames(#[from] DescriptiveCheckerNamesDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    ErrorTypes(#[from] ErrorTypesDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    ExitCodeHygiene(#[from] ExitCodeHygieneDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    FreeFunctions(#[from] FreeFunctionsDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    GoogletestConventions(#[from] GoogletestConventionsDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    GranularVisitorHooks(#[from] GranularVisitorHooksDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    IdiomaticOptionBoolMapping(#[from] IdiomaticOptionBoolMappingDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    LibFacadeHygiene(#[from] LibFacadeHygieneDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    MaxFileLines(#[from] MaxFileLinesDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    MaxNestingDepth(#[from] MaxNestingDepthDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    NoBifurcatedTraitDispatch(#[from] NoBifurcatedTraitDispatchDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    NoBoxedDynError(#[from] NoBoxedDynErrorDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    NoDoubleNegation(#[from] NoDoubleNegationDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    NoEnvAccessOutsideConfig(#[from] NoEnvAccessOutsideConfigDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    NoHasTraitPrefix(#[from] NoHasTraitPrefixDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    NoInlineMods(#[from] NoInlineModsDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    NoInteriorMutabilityInVisitors(#[from] NoInteriorMutabilityInVisitorsDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    NoNegativeBool(#[from] NoNegativeBoolDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    NoPrimitiveFlagClutter(#[from] NoPrimitiveFlagClutterDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    NoPrintlnInLibraries(#[from] NoPrintlnInLibrariesDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    NoRedundantConversions(#[from] NoRedundantConversionsDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    NoRedundantWrappers(#[from] NoRedundantWrappersDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    NoTestPrefix(#[from] NoTestPrefixDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    #[rule_alias("no_trivial_getset")]
    #[rule_alias("trivial_getters_setters")]
    #[rule_alias("trivial_getset")]
    NoTrivialGettersSetters(#[from] NoTrivialGettersSettersDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    NoUnsafeInTests(#[from] NoUnsafeInTestsDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    NoWildcardImports(#[from] NoWildcardImportsDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    PathResolution(#[from] PathResolutionDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    PreferScopedClosures(#[from] PreferScopedClosuresDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    RaiiTempDirectories(#[from] RaiiTempDirectoriesDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    SingleMatchToLetElse(#[from] SingleMatchToLetElseDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    TestMatcherBorrowSimplification(#[from] TestMatcherBorrowSimplificationDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    TestPatterns(#[from] TestPatternsDiagnostic),

    #[error(transparent)]
    #[diagnostic(transparent)]
    UseDeclarationsOverQualifiedPaths(#[from] UseDeclarationsOverQualifiedPathsDiagnostic),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::{PuristDiagnostic, Severity, Span};
    use googletest::prelude::*;
    use miette::Diagnostic as MietteDiagnostic;
    use std::path::Path;

    #[googletest::test]
    fn rule_registry_maintains_one_to_one_consistency_across_all_rules() -> googletest::Result<()> {
        let rules = default_rules();
        verify_that!(rules.len(), eq(RuleId::ALL.len()))?;

        for (rule, &rule_id) in rules.iter().zip(RuleId::ALL.iter()) {
            verify_that!(rule.name(), eq(rule_id.qualified_name()))?;
            verify_that!(RuleId::from_name(rule_id.as_str()), some(eq(rule_id)))?;
            verify_that!(
                RuleId::from_name(rule_id.qualified_name()),
                some(eq(rule_id))
            )?;
        }
        Ok(())
    }

    #[googletest::test]
    fn flattened_rule_diagnostic_delegates_miette_metadata() -> googletest::Result<()> {
        let span = Span::new(Path::new("src/lib.rs"), 10, 5, 10, 20);
        let inner = NoBoxedDynErrorDiagnostic {
            fn_name: "execute".to_string(),
            span: span.clone(),
        };
        verify_that!(inner.rule_id(), eq(RuleId::NoBoxedDynError))?;

        let top_level = RuleDiagnostic::from(inner);
        verify_that!(top_level.rule_id(), eq(RuleId::NoBoxedDynError))?;
        verify_that!(
            top_level.code().map(|c| c.to_string()),
            some(eq("purist::no_boxed_dyn_error"))
        )?;

        let report_diag = PuristDiagnostic::from(top_level);
        verify_that!(
            report_diag.code().map(|c| c.to_string()),
            some(eq("purist::no_boxed_dyn_error"))
        )?;
        verify_that!(report_diag.severity(), some(eq(Severity::Error)))?;
        verify_that!(report_diag.span(), eq(&span))?;
        verify_that!(
            report_diag.to_string(),
            contains_substring("Function 'execute' returns 'Box<dyn Error>'")
        )?;
        verify_that!(
            report_diag.help().map(|h| h.to_string()),
            some(contains_substring("thiserror::Error"))
        )?;
        Ok(())
    }
}
