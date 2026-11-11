//! Canonical identifier enum (`RuleId`) for built-in `purist` lint rules.

macro_rules! define_rule_ids {
    ($( $variant:ident => $name:literal $( | $alias:literal )* ),* $(,)?) => {
        /// Canonical identifier for every built-in `purist` lint rule.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub(crate) enum RuleId {
            $( $variant, )*
        }

        impl RuleId {
            /// Returns the unqualified canonical rule name (e.g. `"error_types"`).
            pub(crate) const fn as_str(self) -> &'static str {
                match self {
                    $( Self::$variant => $name, )*
                }
            }

            /// Returns the `purist::`-prefixed rule identifier (e.g. `"purist::error_types"`).
            pub(crate) const fn qualified_name(self) -> &'static str {
                match self {
                    $( Self::$variant => concat!("purist::", $name), )*
                }
            }

            /// Parses a rule name (with optional `purist::` or `opinionated::` prefix and legacy aliases) into a [`RuleId`].
            pub(crate) fn from_name(rule_name: &str) -> Option<Self> {
                let stripped = rule_name
                    .strip_prefix("purist::")
                    .or_else(|| rule_name.strip_prefix("opinionated::"))
                    .unwrap_or(rule_name);

                match stripped {
                    $( $name $( | $alias )* => Some(Self::$variant), )*
                    _ => None,
                }
            }
        }
    };
}

define_rule_ids! {
    NoInlineMods => "no_inline_mods",
    FreeFunctions => "free_functions",
    PathResolution => "path_resolution",
    ErrorTypes => "error_types",
    ClippySuppressionHygiene => "clippy_suppression_hygiene",
    ConfigStructNaming => "config_struct_naming",
    TestPatterns => "test_patterns",
    NoRedundantConversions => "no_redundant_conversions",
    UseDeclarationsOverQualifiedPaths => "use_declarations_over_qualified_paths",
    NoRedundantWrappers => "no_redundant_wrappers",
    NoBoxedDynError => "no_boxed_dyn_error",
    TestMatcherBorrowSimplification => "test_matcher_borrow_simplification",
    NoTestPrefix => "no_test_prefix",
    NoUnsafeInTests => "no_unsafe_in_tests",
    CentralizedCommandExecution => "centralized_command_execution",
    ClapStructEncapsulation => "clap_struct_encapsulation",
    CommandStructNaming => "command_struct_naming",
    ExitCodeHygiene => "exit_code_hygiene",
    IdiomaticOptionBoolMapping => "idiomatic_option_bool_mapping",
    NoWildcardImports => "no_wildcard_imports",
    NoEnvAccessOutsideConfig => "no_env_access_outside_config",
    SingleMatchToLetElse => "single_match_to_let_else",
    RaiiTempDirectories => "raii_temp_directories",
    NoPrintlnInLibraries => "no_println_in_libraries",
    CliRunConsumesSelf => "cli_run_consumes_self",
    NoDoubleNegation => "no_double_negation",
    NoNegativeBool => "no_negative_bool",
    GoogletestConventions => "googletest_conventions",
    LibFacadeHygiene => "lib_facade_hygiene",
    MaxFileLines => "max_file_lines",
    NoTrivialGettersSetters => "no_trivial_getters_setters" | "no_trivial_getset" | "trivial_getters_setters" | "trivial_getset",
    MaxNestingDepth => "max_nesting_depth",
    NoHasTraitPrefix => "no_has_trait_prefix",
    GranularVisitorHooks => "granular_visitor_hooks",
    NoBifurcatedTraitDispatch => "no_bifurcated_trait_dispatch",
    NoPrimitiveFlagClutter => "no_primitive_flag_clutter",
    NoInteriorMutabilityInVisitors => "no_interior_mutability_in_visitors",
    PreferScopedClosures => "prefer_scoped_closures",
    DescriptiveCheckerNames => "descriptive_checker_names",
}
