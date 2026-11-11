//! Naming, expression hygiene, and test convention diagnostics derived with `miette::Diagnostic`.

/// Typed diagnostic variants for naming, expression hygiene, and test convention `purist` rules.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, miette::Diagnostic)]
pub(crate) enum HygieneDiagnostic {
    #[error(
        "Function '{name}' uses a vague check name. Follow the 'check_<subject>_<condition>' naming convention (e.g. 'check_fn_signature_unsafe_in_test')."
    )]
    #[diagnostic(
        code(purist::descriptive_checker_names),
        help("Rename to include both the subject and the condition/violation being checked.")
    )]
    VagueCheckerName { name: String },

    #[error(
        "Test function '{fn_name}' uses standard '#[test]' attribute. Use '#[googletest::test]' instead."
    )]
    #[diagnostic(
        code(purist::googletest_conventions),
        help("Replace '#[test]' with '#[googletest::test]'.")
    )]
    StandardTestAttr { fn_name: String },

    #[error(
        "Usage of 'assert!' in test. Use GoogleTest matchers ('assert_that!' or 'expect_that!') instead."
    )]
    #[diagnostic(
        code(purist::googletest_conventions),
        help("Replace with 'assert_that!(actual, matcher)' or 'expect_that!(actual, matcher)'.")
    )]
    StandardAssertMacro,

    #[error(
        "Usage of '{mac_name}!' in test. Use GoogleTest matchers ('assert_that!' or 'expect_that!') instead."
    )]
    #[diagnostic(
        code(purist::googletest_conventions),
        help("Replace with 'assert_that!(actual, eq(expected))'.")
    )]
    StandardAssertEqMacro { mac_name: String },

    #[error(
        "Avoid calling '.expect(...)' in test bodies. Propagate errors using '?' or assert with GoogleTest matchers."
    )]
    #[diagnostic(
        code(purist::googletest_conventions),
        help(
            "Return 'Result<(), Box<dyn std::error::Error>>' or 'googletest::Result<()>' and use '?' instead of '.expect(...)'."
        )
    )]
    ExpectInTest,

    #[error(
        "Assertion compares equality using '=='. Use GoogleTest matcher 'eq(...)' directly on the left operand."
    )]
    #[diagnostic(
        code(purist::googletest_conventions),
        help("Replace with 'assert_that!(left, eq(right))'.")
    )]
    AssertThatBinaryEq,

    #[error(
        "Assertion compares inequality using '!='. Use GoogleTest matcher 'not(eq(...))' directly on the left operand."
    )]
    #[diagnostic(
        code(purist::googletest_conventions),
        help("Replace with 'assert_that!(left, not(eq(right)))'.")
    )]
    AssertThatBinaryNe,

    #[error(
        "Assertion checks '.is_ok()' using boolean matcher. Use GoogleTest matcher 'ok(...)' directly on the Result."
    )]
    #[diagnostic(
        code(purist::googletest_conventions),
        help("Replace with 'assert_that!(target, ok(...))'.")
    )]
    AssertThatIsOk,

    #[error(
        "Assertion checks '.is_err()' using boolean matcher. Use GoogleTest matcher 'err(...)' directly on the Result."
    )]
    #[diagnostic(
        code(purist::googletest_conventions),
        help("Replace with 'assert_that!(target, err(...))'.")
    )]
    AssertThatIsErr,

    #[error(
        "Assertion checks '.is_some()' using boolean matcher. Use GoogleTest matcher 'some(...)' directly on the Option."
    )]
    #[diagnostic(
        code(purist::googletest_conventions),
        help("Replace with 'assert_that!(target, some(...))'.")
    )]
    AssertThatIsSome,

    #[error(
        "Assertion checks '.is_none()' using boolean matcher. Use GoogleTest matcher 'none()' directly on the Option."
    )]
    #[diagnostic(
        code(purist::googletest_conventions),
        help("Replace with 'assert_that!(target, none())'.")
    )]
    AssertThatIsNone,

    #[error(
        "Method '{fn_name}' uses negative boolean naming causing double negation (e.g. '{fn_name}(false)'). Use affirmative naming instead."
    )]
    #[diagnostic(
        code(purist::no_double_negation),
        help("Rename to an affirmative identifier (e.g. '{suggested}').")
    )]
    NegativeBooleanMethod { fn_name: String, suggested: String },

    #[error(
        "Predicate '{fn_name}' uses negative boolean naming causing double negation when negated (e.g. '!{fn_name}()'). Use affirmative naming instead."
    )]
    #[diagnostic(
        code(purist::no_double_negation),
        help("Rename to an affirmative predicate (e.g. '{suggested}').")
    )]
    NegativeBooleanPredicate { fn_name: String, suggested: String },

    #[error(
        "Parameter '{param_name}' uses negative boolean naming causing double negation when passed 'false'. Use affirmative naming instead (e.g. 'enabled', 'include')."
    )]
    #[diagnostic(
        code(purist::no_double_negation),
        help("Rename to an affirmative parameter such as 'enabled' or 'include'.")
    )]
    NegativeBooleanParameter { param_name: String },

    #[error(
        "Trait '{trait_name}' uses the 'Has...' prefix, which is an OOP-style anti-pattern in Rust. Use 'With...' for contextual capabilities or a descriptive noun/adjective."
    )]
    #[diagnostic(
        code(purist::no_has_trait_prefix),
        help(
            "Rename trait to 'With...' if providing scoped execution/context, or a descriptive noun/adjective."
        )
    )]
    HasTraitPrefix { trait_name: String },

    #[error(
        "Field '{field_name}' in {container_desc} uses negative boolean naming. Use affirmative/positive naming (e.g. '{suggested}') instead of negative booleans."
    )]
    #[diagnostic(
        code(purist::no_negative_bool),
        help("Rename '{field_name}' to affirmative '{suggested}'.")
    )]
    NegativeBooleanField {
        field_name: String,
        container_desc: String,
        suggested: String,
    },

    #[error(
        "Variable '{var_name}' uses negative boolean naming. Use affirmative/positive naming (e.g. '{suggested}') instead of negative booleans."
    )]
    #[diagnostic(
        code(purist::no_negative_bool),
        help("Rename '{var_name}' to affirmative '{suggested}'.")
    )]
    NegativeBooleanVariable { var_name: String, suggested: String },

    #[error("Redundant serialization roundtrip: value serialized and immediately deserialized.")]
    #[diagnostic(
        code(purist::no_redundant_conversions),
        help(
            "Use 'Clone::clone', 'From::from', or 'serde_json::to_value' instead of stringifying and re-parsing."
        )
    )]
    RedundantNestedSerializationRoundtrip,

    #[error(
        "Redundant serialization roundtrip: variable '{ident}' serialized and immediately deserialized."
    )]
    #[diagnostic(
        code(purist::no_redundant_conversions),
        help(
            "Use 'Clone::clone', 'From::from', or direct mapping instead of roundtrip serialization."
        )
    )]
    RedundantVariableSerializationRoundtrip { ident: String },

    #[error(
        "Function '{fn_name}' is a redundant wrapper that merely forwards to '{target_desc}'. Call '{target_desc}' directly."
    )]
    #[diagnostic(
        code(purist::no_redundant_wrappers),
        help(
            "Remove wrapper function '{fn_name}' and call '{target_desc}' directly at call sites."
        )
    )]
    RedundantWrapperFn {
        fn_name: String,
        target_desc: String,
    },

    #[error(
        "Test function '{name}' has a redundant 'test_' prefix. Use '<action>_<scenario>_<outcome>' naming (e.g. 'parse_valid_manifest_succeeds')."
    )]
    #[diagnostic(
        code(purist::no_test_prefix),
        help(
            "Rename test '{name}' to remove the 'test_' prefix and follow '<action>_<scenario>_<outcome>'."
        )
    )]
    RedundantTestPrefix { name: String },

    #[error(
        "Method '{method_name}' is a trivial {kind} for field '{field_name}' on struct '{struct_name}' which forms a trivial getter/setter combo with '{partner_name}'. Trivial getter/setter combos add unnecessary indirection (YAGNI)."
    )]
    #[diagnostic(
        code(purist::no_trivial_getters_setters),
        help("Remove trivial {kind} '{method_name}' and access field '{field_name}' directly.")
    )]
    TrivialGetterSetterCombo {
        method_name: String,
        kind: String,
        field_name: String,
        struct_name: String,
        partner_name: String,
    },

    #[error(
        "Function '{fn_name}' in test context is declared 'unsafe'. Tests must verify code through safe interfaces."
    )]
    #[diagnostic(
        code(purist::no_unsafe_in_tests),
        severity(Error),
        help(
            "Remove 'unsafe' from test code; verify behavior strictly through safe public interfaces."
        )
    )]
    UnsafeFnInTest { fn_name: String },

    #[error(
        "Usage of 'unsafe' block in test context. Tests must exercise safe public abstractions."
    )]
    #[diagnostic(
        code(purist::no_unsafe_in_tests),
        severity(Error),
        help(
            "Remove 'unsafe' block from test code; verify behavior strictly through safe public interfaces."
        )
    )]
    UnsafeBlockInTest,

    #[error(
        "Avoid wildcard import '{path_str}'. Wildcard imports obscure symbol provenance and cause namespace pollution."
    )]
    #[diagnostic(
        code(purist::no_wildcard_imports),
        help("Replace wildcard import with explicit item imports.")
    )]
    WildcardImport { path_str: String },

    #[error(
        "Manual 'if let Some(...) = ... else {{ false }}' construct. Use '.is_some_and(...)' instead."
    )]
    #[diagnostic(
        code(purist::idiomatic_option_bool_mapping),
        help("Replace with '.is_some_and(|val| ...)'.")
    )]
    IfLetSomeElseFalse,

    #[error(
        "Manual 'if let Some(...) = ... else {{ None }}' construct. Use functional combinators like '.and_then(...)' or '.map(...)'."
    )]
    #[diagnostic(
        code(purist::idiomatic_option_bool_mapping),
        help("Replace with '.and_then(...)' or '.map(...)'. ")
    )]
    IfLetSomeElseNone,

    #[error(
        "Manual 'fs::remove_dir_all' in test context. Use an RAII temporary directory guard to guarantee cleanup on assertion failures."
    )]
    #[diagnostic(
        code(purist::raii_temp_directories),
        help(
            "Use an RAII temporary directory guard (implementing 'Drop' or via 'tempfile') instead of manual removal."
        )
    )]
    ManualRemoveDirAllInTest,

    #[error(
        "Match expression can be simplified using idiomatic 'let ... = ... else {{ ... }};' construct."
    )]
    #[diagnostic(
        code(purist::single_match_to_let_else),
        help("Replace 'match' with 'let Some(...) = expr else {{ ... }};' to reduce indentation.")
    )]
    SingleMatchToLetElse,

    #[error(
        "Redundant '.{method_name}()' in GoogleTest assertion. GoogleTest matchers accept borrowed references directly."
    )]
    #[diagnostic(
        code(purist::test_matcher_borrow_simplification),
        help(
            "Replace '.{method_name}()' with a reference borrowing the expression directly ('&<expr>')."
        )
    )]
    RedundantMatcherBorrow { method_name: String },

    #[error(
        "Test function '{name}' does not conform to '<verb>_<description>_<outcome>' naming convention."
    )]
    #[diagnostic(
        code(purist::test_patterns),
        help(
            "Rename test to follow '<verb>_<description>_<outcome>' (e.g. 'parse_valid_input_succeeds')."
        )
    )]
    ShortTestFnName { name: String },

    #[error(
        "Usage of '{macro_name}!' in test. Use GoogleTest matchers ('assert_that!' or 'expect_that!') instead."
    )]
    #[diagnostic(
        code(purist::test_patterns),
        help(
            "Replace with 'assert_that!(actual, eq(expected))' or 'expect_that!(actual, eq(expected))'."
        )
    )]
    LegacyAssertionMacroInTest { macro_name: String },

    #[error(
        "Avoid calling '.unwrap()' in test bodies. Propagate errors using '?' or assert with GoogleTest matchers."
    )]
    #[diagnostic(
        code(purist::test_patterns),
        help(
            "Return Result<(), Box<dyn std::error::Error>> or googletest::Result<()> and use '?' instead of '.unwrap()'."
        )
    )]
    UnwrapInTest,

    #[error(
        "Avoid long qualified path '{path_str}'. Add a 'use' declaration at the top of the module."
    )]
    #[diagnostic(
        code(purist::use_declarations_over_qualified_paths),
        help("Import '{path_str}' via 'use {path_str};' and refer to '{target_ident}' directly.")
    )]
    LongQualifiedPath {
        path_str: String,
        target_ident: String,
    },
}
