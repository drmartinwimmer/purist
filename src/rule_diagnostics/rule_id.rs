//! Canonical identifier enum (`RuleId`) for built-in `purist` lint rules.

/// Canonical identifier for every built-in `purist` lint rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum RuleId {
    NoInlineMods,
    FreeFunctions,
    PathResolution,
    ErrorTypes,
    ClippySuppressionHygiene,
    ConfigStructNaming,
    TestPatterns,
    NoRedundantConversions,
    UseDeclarationsOverQualifiedPaths,
    NoRedundantWrappers,
    NoBoxedDynError,
    TestMatcherBorrowSimplification,
    NoTestPrefix,
    NoUnsafeInTests,
    CentralizedCommandExecution,
    ClapStructEncapsulation,
    CommandStructNaming,
    ExitCodeHygiene,
    IdiomaticOptionBoolMapping,
    NoWildcardImports,
    NoEnvAccessOutsideConfig,
    SingleMatchToLetElse,
    RaiiTempDirectories,
    NoPrintlnInLibraries,
    CliRunConsumesSelf,
    NoDoubleNegation,
    NoNegativeBool,
    GoogletestConventions,
    LibFacadeHygiene,
    MaxFileLines,
    NoTrivialGettersSetters,
    MaxNestingDepth,
    NoHasTraitPrefix,
    GranularVisitorHooks,
    NoBifurcatedTraitDispatch,
    NoPrimitiveFlagClutter,
    NoInteriorMutabilityInVisitors,
    PreferScopedClosures,
    DescriptiveCheckerNames,
}

impl RuleId {
    /// Returns the unqualified canonical rule name (e.g. `"error_types"`).
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::NoInlineMods => "no_inline_mods",
            Self::FreeFunctions => "free_functions",
            Self::PathResolution => "path_resolution",
            Self::ErrorTypes => "error_types",
            Self::ClippySuppressionHygiene => "clippy_suppression_hygiene",
            Self::ConfigStructNaming => "config_struct_naming",
            Self::TestPatterns => "test_patterns",
            Self::NoRedundantConversions => "no_redundant_conversions",
            Self::UseDeclarationsOverQualifiedPaths => "use_declarations_over_qualified_paths",
            Self::NoRedundantWrappers => "no_redundant_wrappers",
            Self::NoBoxedDynError => "no_boxed_dyn_error",
            Self::TestMatcherBorrowSimplification => "test_matcher_borrow_simplification",
            Self::NoTestPrefix => "no_test_prefix",
            Self::NoUnsafeInTests => "no_unsafe_in_tests",
            Self::CentralizedCommandExecution => "centralized_command_execution",
            Self::ClapStructEncapsulation => "clap_struct_encapsulation",
            Self::CommandStructNaming => "command_struct_naming",
            Self::ExitCodeHygiene => "exit_code_hygiene",
            Self::IdiomaticOptionBoolMapping => "idiomatic_option_bool_mapping",
            Self::NoWildcardImports => "no_wildcard_imports",
            Self::NoEnvAccessOutsideConfig => "no_env_access_outside_config",
            Self::SingleMatchToLetElse => "single_match_to_let_else",
            Self::RaiiTempDirectories => "raii_temp_directories",
            Self::NoPrintlnInLibraries => "no_println_in_libraries",
            Self::CliRunConsumesSelf => "cli_run_consumes_self",
            Self::NoDoubleNegation => "no_double_negation",
            Self::NoNegativeBool => "no_negative_bool",
            Self::GoogletestConventions => "googletest_conventions",
            Self::LibFacadeHygiene => "lib_facade_hygiene",
            Self::MaxFileLines => "max_file_lines",
            Self::NoTrivialGettersSetters => "no_trivial_getters_setters",
            Self::MaxNestingDepth => "max_nesting_depth",
            Self::NoHasTraitPrefix => "no_has_trait_prefix",
            Self::GranularVisitorHooks => "granular_visitor_hooks",
            Self::NoBifurcatedTraitDispatch => "no_bifurcated_trait_dispatch",
            Self::NoPrimitiveFlagClutter => "no_primitive_flag_clutter",
            Self::NoInteriorMutabilityInVisitors => "no_interior_mutability_in_visitors",
            Self::PreferScopedClosures => "prefer_scoped_closures",
            Self::DescriptiveCheckerNames => "descriptive_checker_names",
        }
    }

    /// Returns the `purist::`-prefixed rule identifier (e.g. `"purist::error_types"`).
    pub(crate) const fn qualified_name(self) -> &'static str {
        match self {
            Self::NoInlineMods => "purist::no_inline_mods",
            Self::FreeFunctions => "purist::free_functions",
            Self::PathResolution => "purist::path_resolution",
            Self::ErrorTypes => "purist::error_types",
            Self::ClippySuppressionHygiene => "purist::clippy_suppression_hygiene",
            Self::ConfigStructNaming => "purist::config_struct_naming",
            Self::TestPatterns => "purist::test_patterns",
            Self::NoRedundantConversions => "purist::no_redundant_conversions",
            Self::UseDeclarationsOverQualifiedPaths => {
                "purist::use_declarations_over_qualified_paths"
            }
            Self::NoRedundantWrappers => "purist::no_redundant_wrappers",
            Self::NoBoxedDynError => "purist::no_boxed_dyn_error",
            Self::TestMatcherBorrowSimplification => "purist::test_matcher_borrow_simplification",
            Self::NoTestPrefix => "purist::no_test_prefix",
            Self::NoUnsafeInTests => "purist::no_unsafe_in_tests",
            Self::CentralizedCommandExecution => "purist::centralized_command_execution",
            Self::ClapStructEncapsulation => "purist::clap_struct_encapsulation",
            Self::CommandStructNaming => "purist::command_struct_naming",
            Self::ExitCodeHygiene => "purist::exit_code_hygiene",
            Self::IdiomaticOptionBoolMapping => "purist::idiomatic_option_bool_mapping",
            Self::NoWildcardImports => "purist::no_wildcard_imports",
            Self::NoEnvAccessOutsideConfig => "purist::no_env_access_outside_config",
            Self::SingleMatchToLetElse => "purist::single_match_to_let_else",
            Self::RaiiTempDirectories => "purist::raii_temp_directories",
            Self::NoPrintlnInLibraries => "purist::no_println_in_libraries",
            Self::CliRunConsumesSelf => "purist::cli_run_consumes_self",
            Self::NoDoubleNegation => "purist::no_double_negation",
            Self::NoNegativeBool => "purist::no_negative_bool",
            Self::GoogletestConventions => "purist::googletest_conventions",
            Self::LibFacadeHygiene => "purist::lib_facade_hygiene",
            Self::MaxFileLines => "purist::max_file_lines",
            Self::NoTrivialGettersSetters => "purist::no_trivial_getters_setters",
            Self::MaxNestingDepth => "purist::max_nesting_depth",
            Self::NoHasTraitPrefix => "purist::no_has_trait_prefix",
            Self::GranularVisitorHooks => "purist::granular_visitor_hooks",
            Self::NoBifurcatedTraitDispatch => "purist::no_bifurcated_trait_dispatch",
            Self::NoPrimitiveFlagClutter => "purist::no_primitive_flag_clutter",
            Self::NoInteriorMutabilityInVisitors => "purist::no_interior_mutability_in_visitors",
            Self::PreferScopedClosures => "purist::prefer_scoped_closures",
            Self::DescriptiveCheckerNames => "purist::descriptive_checker_names",
        }
    }

    /// Parses a rule name (with optional `purist::` or `opinionated::` prefix and legacy aliases) into a [`RuleId`].
    pub(crate) fn from_name(rule_name: &str) -> Option<Self> {
        let stripped = rule_name
            .strip_prefix("purist::")
            .or_else(|| rule_name.strip_prefix("opinionated::"))
            .unwrap_or(rule_name);

        match stripped {
            "no_inline_mods" => Some(Self::NoInlineMods),
            "free_functions" => Some(Self::FreeFunctions),
            "path_resolution" => Some(Self::PathResolution),
            "error_types" => Some(Self::ErrorTypes),
            "clippy_suppression_hygiene" => Some(Self::ClippySuppressionHygiene),
            "config_struct_naming" => Some(Self::ConfigStructNaming),
            "test_patterns" => Some(Self::TestPatterns),
            "no_redundant_conversions" => Some(Self::NoRedundantConversions),
            "use_declarations_over_qualified_paths" => {
                Some(Self::UseDeclarationsOverQualifiedPaths)
            }
            "no_redundant_wrappers" => Some(Self::NoRedundantWrappers),
            "no_boxed_dyn_error" => Some(Self::NoBoxedDynError),
            "test_matcher_borrow_simplification" => Some(Self::TestMatcherBorrowSimplification),
            "no_test_prefix" => Some(Self::NoTestPrefix),
            "no_unsafe_in_tests" => Some(Self::NoUnsafeInTests),
            "centralized_command_execution" => Some(Self::CentralizedCommandExecution),
            "clap_struct_encapsulation" => Some(Self::ClapStructEncapsulation),
            "command_struct_naming" => Some(Self::CommandStructNaming),
            "exit_code_hygiene" => Some(Self::ExitCodeHygiene),
            "idiomatic_option_bool_mapping" => Some(Self::IdiomaticOptionBoolMapping),
            "no_wildcard_imports" => Some(Self::NoWildcardImports),
            "no_env_access_outside_config" => Some(Self::NoEnvAccessOutsideConfig),
            "single_match_to_let_else" => Some(Self::SingleMatchToLetElse),
            "raii_temp_directories" => Some(Self::RaiiTempDirectories),
            "no_println_in_libraries" => Some(Self::NoPrintlnInLibraries),
            "cli_run_consumes_self" => Some(Self::CliRunConsumesSelf),
            "no_double_negation" => Some(Self::NoDoubleNegation),
            "no_negative_bool" => Some(Self::NoNegativeBool),
            "googletest_conventions" => Some(Self::GoogletestConventions),
            "lib_facade_hygiene" => Some(Self::LibFacadeHygiene),
            "max_file_lines" => Some(Self::MaxFileLines),
            "no_trivial_getters_setters"
            | "no_trivial_getset"
            | "trivial_getters_setters"
            | "trivial_getset" => Some(Self::NoTrivialGettersSetters),
            "max_nesting_depth" => Some(Self::MaxNestingDepth),
            "no_has_trait_prefix" => Some(Self::NoHasTraitPrefix),
            "granular_visitor_hooks" => Some(Self::GranularVisitorHooks),
            "no_bifurcated_trait_dispatch" => Some(Self::NoBifurcatedTraitDispatch),
            "no_primitive_flag_clutter" => Some(Self::NoPrimitiveFlagClutter),
            "no_interior_mutability_in_visitors" => Some(Self::NoInteriorMutabilityInVisitors),
            "prefer_scoped_closures" => Some(Self::PreferScopedClosures),
            "descriptive_checker_names" => Some(Self::DescriptiveCheckerNames),
            _ => None,
        }
    }
}
