pub(crate) mod centralized_commands;
pub(crate) mod clap_encapsulation;
pub(crate) mod cli_run_consumes_self;
pub(crate) mod clippy_suppress;
pub(crate) mod command_struct_naming;
pub(crate) mod common;
pub(crate) mod config_struct_naming;
pub(crate) mod descriptive_checker_names;
pub(crate) mod error_types;
pub(crate) mod exit_code_hygiene;
pub(crate) mod free_functions;
pub(crate) mod googletest_conventions;
pub(crate) mod granular_visitor_hooks;
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
pub(crate) mod option_bool_mapping;
pub(crate) mod path_resolution;
pub(crate) mod prefer_scoped_closures;
pub(crate) mod raii_temp_directories;
pub(crate) mod single_match_to_let_else;
pub(crate) mod test_matcher_borrow;
pub(crate) mod test_patterns;
pub(crate) mod use_declarations;

pub(crate) use centralized_commands::CentralizedCommandsRule;
pub(crate) use clap_encapsulation::ClapEncapsulationRule;
pub(crate) use cli_run_consumes_self::CliRunConsumesSelfRule;
pub(crate) use clippy_suppress::ClippySuppressRule;
pub(crate) use command_struct_naming::CommandStructNamingRule;
pub(crate) use config_struct_naming::ConfigStructNamingRule;
pub(crate) use descriptive_checker_names::DescriptiveCheckerNamesRule;
pub(crate) use error_types::ErrorTypesRule;
pub(crate) use exit_code_hygiene::ExitCodeHygieneRule;
pub(crate) use free_functions::FreeFunctionsRule;
pub(crate) use googletest_conventions::GoogletestConventionsRule;
pub(crate) use granular_visitor_hooks::GranularVisitorHooksRule;
pub(crate) use lib_facade_hygiene::LibFacadeHygieneRule;
pub(crate) use max_file_lines::MaxFileLinesRule;
pub(crate) use max_nesting_depth::MaxNestingDepthRule;
pub(crate) use no_bifurcated_trait_dispatch::NoBifurcatedTraitDispatchRule;
pub(crate) use no_boxed_dyn_error::NoBoxedDynErrorRule;
pub(crate) use no_double_negation::NoDoubleNegationRule;
pub(crate) use no_env_access_outside_config::NoEnvAccessOutsideConfigRule;
pub(crate) use no_has_trait_prefix::NoHasTraitPrefixRule;
pub(crate) use no_inline_mods::NoInlineModsRule;
pub(crate) use no_interior_mutability_in_visitors::NoInteriorMutabilityInVisitorsRule;
pub(crate) use no_negative_bool::NoNegativeBoolRule;
pub(crate) use no_primitive_flag_clutter::NoPrimitiveFlagClutterRule;
pub(crate) use no_println_in_libraries::NoPrintlnInLibrariesRule;
pub(crate) use no_redundant_conversions::NoRedundantConversionsRule;
pub(crate) use no_redundant_wrappers::NoRedundantWrappersRule;
pub(crate) use no_test_prefix::NoTestPrefixRule;
pub(crate) use no_trivial_getters_setters::NoTrivialGettersSettersRule;
pub(crate) use no_unsafe_in_tests::NoUnsafeInTestsRule;
pub(crate) use no_wildcard_imports::NoWildcardImportsRule;
pub(crate) use option_bool_mapping::OptionBoolMappingRule;
pub(crate) use path_resolution::PathResolutionRule;
pub(crate) use prefer_scoped_closures::PreferScopedClosuresRule;
pub(crate) use raii_temp_directories::RaiiTempDirectoriesRule;
pub(crate) use single_match_to_let_else::SingleMatchToLetElseRule;
pub(crate) use test_matcher_borrow::TestMatcherBorrowRule;
pub(crate) use test_patterns::TestPatternsRule;
pub(crate) use use_declarations::UseDeclarationsRule;

use crate::engine::Rule;

/// Returns a collection of all standard purist static analysis rules.
pub(crate) fn default_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(NoInlineModsRule),
        Box::new(FreeFunctionsRule),
        Box::new(PathResolutionRule),
        Box::new(ErrorTypesRule),
        Box::new(ClippySuppressRule),
        Box::new(ConfigStructNamingRule),
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
        Box::new(CommandStructNamingRule),
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
        Box::new(GranularVisitorHooksRule),
        Box::new(NoBifurcatedTraitDispatchRule),
        Box::new(NoPrimitiveFlagClutterRule),
        Box::new(NoInteriorMutabilityInVisitorsRule),
        Box::new(PreferScopedClosuresRule),
        Box::new(DescriptiveCheckerNamesRule),
    ]
}
