pub mod centralized_commands;
pub mod clap_encapsulation;
pub mod cli_run_consumes_self;
pub mod clippy_suppress;
pub mod common;
pub mod error_types;
pub mod exit_code_hygiene;
pub mod free_functions;
pub mod googletest_conventions;
pub mod lib_facade_hygiene;
pub mod max_file_lines;
pub mod max_nesting_depth;
pub mod no_boxed_dyn_error;
pub mod no_double_negation;
pub mod no_env_access_outside_config;
pub mod no_has_trait_prefix;
pub mod no_inline_mods;
pub mod no_negative_bool;
pub mod no_println_in_libraries;
pub mod no_redundant_conversions;
pub mod no_redundant_wrappers;
pub mod no_test_prefix;
pub mod no_trivial_getters_setters;
pub mod no_unsafe_in_tests;
pub mod no_wildcard_imports;
pub mod option_bool_mapping;
pub mod path_resolution;
pub mod raii_temp_directories;
pub mod single_match_to_let_else;
pub mod test_matcher_borrow;
pub mod test_patterns;
pub mod use_declarations;

pub use centralized_commands::CentralizedCommandsRule;
pub use clap_encapsulation::ClapEncapsulationRule;
pub use cli_run_consumes_self::CliRunConsumesSelfRule;
pub use clippy_suppress::ClippySuppressRule;
pub use error_types::ErrorTypesRule;
pub use exit_code_hygiene::ExitCodeHygieneRule;
pub use free_functions::FreeFunctionsRule;
pub use googletest_conventions::GoogletestConventionsRule;
pub use lib_facade_hygiene::LibFacadeHygieneRule;
pub use max_file_lines::MaxFileLinesRule;
pub use max_nesting_depth::MaxNestingDepthRule;
pub use no_boxed_dyn_error::NoBoxedDynErrorRule;
pub use no_double_negation::NoDoubleNegationRule;
pub use no_env_access_outside_config::NoEnvAccessOutsideConfigRule;
pub use no_has_trait_prefix::NoHasTraitPrefixRule;
pub use no_inline_mods::NoInlineModsRule;
pub use no_negative_bool::NoNegativeBoolRule;
pub use no_println_in_libraries::NoPrintlnInLibrariesRule;
pub use no_redundant_conversions::NoRedundantConversionsRule;
pub use no_redundant_wrappers::NoRedundantWrappersRule;
pub use no_test_prefix::NoTestPrefixRule;
pub use no_trivial_getters_setters::NoTrivialGettersSettersRule;
pub use no_unsafe_in_tests::NoUnsafeInTestsRule;
pub use no_wildcard_imports::NoWildcardImportsRule;
pub use option_bool_mapping::OptionBoolMappingRule;
pub use path_resolution::PathResolutionRule;
pub use raii_temp_directories::RaiiTempDirectoriesRule;
pub use single_match_to_let_else::SingleMatchToLetElseRule;
pub use test_matcher_borrow::TestMatcherBorrowRule;
pub use test_patterns::TestPatternsRule;
pub use use_declarations::UseDeclarationsRule;

use crate::engine::Rule;

/// Returns a collection of all standard purist static analysis rules.
pub fn default_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(NoInlineModsRule),
        Box::new(FreeFunctionsRule),
        Box::new(PathResolutionRule),
        Box::new(ErrorTypesRule),
        Box::new(ClippySuppressRule),
        Box::new(TestPatternsRule),
        Box::new(NoRedundantConversionsRule),
        Box::new(UseDeclarationsRule),
        Box::new(NoRedundantWrappersRule),
        Box::new(NoBoxedDynErrorRule),
        Box::new(TestMatcherBorrowRule),
        Box::new(NoTestPrefixRule),
        Box::new(NoUnsafeInTestsRule),
        Box::new(CentralizedCommandsRule),
        Box::new(ClapEncapsulationRule),
        Box::new(ExitCodeHygieneRule),
        Box::new(OptionBoolMappingRule),
        Box::new(NoWildcardImportsRule),
        Box::new(NoEnvAccessOutsideConfigRule),
        Box::new(SingleMatchToLetElseRule),
        Box::new(RaiiTempDirectoriesRule),
        Box::new(NoPrintlnInLibrariesRule),
        Box::new(CliRunConsumesSelfRule),
        Box::new(NoDoubleNegationRule),
        Box::new(NoNegativeBoolRule),
        Box::new(GoogletestConventionsRule),
        Box::new(LibFacadeHygieneRule),
        Box::new(MaxFileLinesRule),
        Box::new(NoTrivialGettersSettersRule),
        Box::new(MaxNestingDepthRule),
        Box::new(NoHasTraitPrefixRule),
    ]
}
