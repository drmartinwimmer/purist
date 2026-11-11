//! Human-readable diagnostic message formatting for [`RuleDiagnostic`].

use super::RuleDiagnostic;

impl RuleDiagnostic {
    /// Formats the primary human-readable diagnostic message for this finding.
    pub(crate) fn message(&self) -> String {
        match self {
            Self::UncentralizedCommandNew => {
                "Direct invocation of 'Command::new' outside dedicated command/tool module. Encapsulate external process execution in a dedicated tool struct.".to_string()
            }
            Self::ClapMissingRunMethod { struct_name } => format!(
                "Clap command struct '{struct_name}' lacks an associated 'run' or 'execute' method. Encapsulate execution inside an associated method."
            ),
            Self::ClapPublicField {
                field_name,
                struct_name,
            } => format!(
                "Field '{field_name}' in Clap struct '{struct_name}' is declared public. Clap argument fields should remain private to encapsulate command execution."
            ),
            Self::CliRunTakesRefSelf {
                fn_name,
                struct_name,
            } => format!(
                "CLI execution method '{fn_name}' on command struct '{struct_name}' takes '&self' by reference. CLI commands should consume 'self' by value ('pub fn {fn_name}(self, ...)') to take ownership of parsed arguments and avoid unnecessary cloning."
            ),
            Self::SuppressionMissingReasonAndComment => {
                "Lint suppression attribute lacks both a 'reason = \"...\"' parameter and an explanatory code comment on the preceding line.".to_string()
            }
            Self::SuppressionMissingReason => {
                "Lint suppression attribute lacks a 'reason = \"...\"' parameter explaining why it is necessary.".to_string()
            }
            Self::SuppressionMissingComment => {
                "Lint suppression attribute lacks an accompanying code comment on the preceding line explaining why the lint cannot be resolved.".to_string()
            }
            Self::ParserStructNaming { struct_name } => format!(
                "Clap parser struct '{struct_name}' does not follow the top-level CLI parser naming convention. Structs deriving 'clap::Parser' must be named 'Cli' or '<Subject>Cli'."
            ),
            Self::BareCommandStructSuffix { struct_name } => format!(
                "Clap command struct '{struct_name}' uses an unadorned suffix without a subject prefix. Follow the '<Subject>Command', '<Subject>Args', or '<Subject>Subcommand' naming convention."
            ),
            Self::InvalidCommandStructNaming { struct_name } => format!(
                "Clap command struct '{struct_name}' does not follow the CLI command naming convention. Command structs must be named '<Subject>Command', '<Subject>Args', or '<Subject>Subcommand'."
            ),
            Self::InvalidConfigStructNaming {
                struct_name,
                suggested_name,
            } => format!(
                "Configuration struct '{struct_name}' does not follow the '<Subject>Config', '<Subject>Manifest', or '<Subject>Metadata' naming convention. Rename it to '{suggested_name}'."
            ),
            Self::VagueCheckerName { name } => format!(
                "Function '{name}' uses a vague check name. Follow the 'check_<subject>_<condition>' naming convention (e.g. 'check_fn_signature_unsafe_in_test')."
            ),
            Self::ErrorTypeNaming { name } => format!(
                "Error type '{name}' must follow the '<Subject>Error' naming convention (e.g., 'ConfigError', 'ParseError')."
            ),
            Self::ManualErrorImpl { target_name } => format!(
                "Manual 'Error' trait implementation for '{target_name}'. Derive 'thiserror::Error' instead."
            ),
            Self::UnstructuredErrorReturn { fn_name, err_desc } => format!(
                "Function '{fn_name}' returns unstructured error type '{err_desc}'. Use structured error enums via thiserror or anyhow::Result."
            ),
            Self::FnReturnsExitCode { fn_name } => format!(
                "Function '{fn_name}' returns 'ExitCode'. Library functions must return 'Result' and let the CLI entrypoint handle exit codes."
            ),
            Self::RawIntProcessExit => {
                "Raw integer literal passed to 'exit(...)'. Use 'ExitCode::SUCCESS' or 'ExitCode::FAILURE' instead.".to_string()
            }
            Self::ProcessExitOutsideMain => {
                "Direct invocation of 'process::exit' outside 'main()'. Propagate errors using 'Result' instead.".to_string()
            }
            Self::StatelessNamespaceStruct { struct_name } => format!(
                "Struct '{struct_name}' is stateless and used solely as a namespace for functions. Replace with idiomatic free functions in the module or a unit struct ('struct {struct_name};')."
            ),
            Self::StandardTestAttr { fn_name } => format!(
                "Test function '{fn_name}' uses standard '#[test]' attribute. Use '#[googletest::test]' instead."
            ),
            Self::StandardAssertMacro => {
                "Usage of 'assert!' in test. Use GoogleTest matchers ('assert_that!' or 'expect_that!') instead.".to_string()
            }
            Self::StandardAssertEqMacro { mac_name } => format!(
                "Usage of '{mac_name}!' in test. Use GoogleTest matchers ('assert_that!' or 'expect_that!') instead."
            ),
            Self::ExpectInTest => {
                "Avoid calling '.expect(...)' in test bodies. Propagate errors using '?' or assert with GoogleTest matchers.".to_string()
            }
            Self::AssertThatBinaryEq => {
                "Assertion compares equality using '=='. Use GoogleTest matcher 'eq(...)' directly on the left operand.".to_string()
            }
            Self::AssertThatBinaryNe => {
                "Assertion compares inequality using '!='. Use GoogleTest matcher 'not(eq(...))' directly on the left operand.".to_string()
            }
            Self::AssertThatIsOk => {
                "Assertion checks '.is_ok()' using boolean matcher. Use GoogleTest matcher 'ok(...)' directly on the Result.".to_string()
            }
            Self::AssertThatIsErr => {
                "Assertion checks '.is_err()' using boolean matcher. Use GoogleTest matcher 'err(...)' directly on the Result.".to_string()
            }
            Self::AssertThatIsSome => {
                "Assertion checks '.is_some()' using boolean matcher. Use GoogleTest matcher 'some(...)' directly on the Option.".to_string()
            }
            Self::AssertThatIsNone => {
                "Assertion checks '.is_none()' using boolean matcher. Use GoogleTest matcher 'none()' directly on the Option.".to_string()
            }
            Self::ManualAstIteration {
                description,
                fn_name,
                hook_name,
            } => format!(
                "Manual iteration over {description} in '{fn_name}'. Implement and delegate to the specialized '{hook_name}' visitor hook instead to preserve recursive traversal and attribute scoping."
            ),
            Self::LibRsTooLarge {
                prod_lines,
                max_prod_lines,
            } => format!(
                "Library root 'lib.rs' is too large ({prod_lines} production lines, limit is {max_prod_lines}). Keep 'lib.rs' as a concise facade index."
            ),
            Self::LibRsFnTooLong {
                fn_name,
                fn_lines,
                max_fn_lines,
            } => format!(
                "Function '{fn_name}' in 'lib.rs' is too long ({fn_lines} lines, limit is {max_fn_lines}). Functions in 'lib.rs' should be brief facade delegates."
            ),
            Self::LibRsPubAdapterMod { mod_name } => format!(
                "Public module 'pub mod {mod_name};' in 'lib.rs' exposes internal command/tool adapters in the public library API. Use 'mod {mod_name};' or 'pub(crate) mod {mod_name};' and re-export only public facade types via 'pub use'."
            ),
            Self::TestFileTooLong {
                file_name,
                total_lines,
                max_total_lines,
            } => format!(
                "Test file '{file_name}' exceeds maximum length ({total_lines} lines, limit is {max_total_lines}). Split large test suites into focused sub-modules or separate integration test files."
            ),
            Self::SourceFileTooLong {
                file_name,
                prod_lines,
                max_production_lines,
            } => format!(
                "Source file '{file_name}' exceeds maximum production length ({prod_lines} production lines, limit is {max_production_lines}). Decompose into smaller, cohesive submodules."
            ),
            Self::NestingDepthExceeded { depth, max_depth } => format!(
                "Control flow nesting depth of {depth} exceeds the maximum limit of {max_depth}."
            ),
            Self::BifurcatedTraitDispatch {
                type_a,
                type_b,
                method_name,
                shared_trait,
            } => {
                if let Some(trait_name) = shared_trait {
                    format!(
                        "Bifurcated dispatch to sibling types '{type_a}' and '{type_b}' implementing trait '{trait_name}' on method '{method_name}'. Consolidate into a single parameterized type or unify dispatch outside the branch."
                    )
                } else {
                    format!(
                        "Bifurcated dispatch to sibling types '{type_a}' and '{type_b}' on method '{method_name}'. Consolidate into a single parameterized type or unify dispatch outside the branch."
                    )
                }
            }
            Self::BoxedDynErrorReturn { fn_name } => format!(
                "Function '{fn_name}' returns 'Box<dyn Error>'. Return a concrete domain error enum deriving 'thiserror::Error' instead."
            ),
            Self::NegativeBooleanMethod { fn_name, .. } => format!(
                "Method '{fn_name}' uses negative boolean naming causing double negation (e.g. '{fn_name}(false)'). Use affirmative naming instead."
            ),
            Self::NegativeBooleanPredicate { fn_name, .. } => format!(
                "Predicate '{fn_name}' uses negative boolean naming causing double negation when negated (e.g. '!{fn_name}()'). Use affirmative naming instead."
            ),
            Self::NegativeBooleanParameter { param_name } => format!(
                "Parameter '{param_name}' uses negative boolean naming causing double negation when passed 'false'. Use affirmative naming instead (e.g. 'enabled', 'include')."
            ),
            Self::DirectEnvAccess { func_name } => format!(
                "Direct invocation of 'std::env::{func_name}' outside configuration/CLI/tool modules. Parse environment parameters in a dedicated configuration layer."
            ),
            Self::HasTraitPrefix { trait_name } => format!(
                "Trait '{trait_name}' uses the 'Has...' prefix, which is an OOP-style anti-pattern in Rust. Use 'With...' for contextual capabilities or a descriptive noun/adjective."
            ),
            Self::InlineModule {
                mod_name,
                file_path,
            } => format!(
                "Inline module '{mod_name}' in '{file_path}' violates modularity guidelines. Submodules should be placed in dedicated files."
            ),
            Self::VisitorInteriorMutability {
                struct_name,
                field_name,
                bad_type,
            } => format!(
                "Visitor struct '{struct_name}' declares field '{field_name}' with interior mutability ('{bad_type}'). AST visitors provide '&mut self' during single-threaded traversal; use direct mutable fields or stack types instead."
            ),
            Self::NegativeBooleanField {
                field_name,
                container_desc,
                suggested,
            } => format!(
                "Field '{field_name}' in {container_desc} uses negative boolean naming. Use affirmative/positive naming (e.g. '{suggested}') instead of negative booleans."
            ),
            Self::NegativeBooleanVariable {
                var_name,
                suggested,
            } => format!(
                "Variable '{var_name}' uses negative boolean naming. Use affirmative/positive naming (e.g. '{suggested}') instead of negative booleans."
            ),
            Self::VisitorFlagClutter {
                struct_name,
                flag_count,
            } => format!(
                "Visitor struct '{struct_name}' contains multiple boolean/flag tracking fields ({flag_count}). Consolidate traversal state into cohesive domain scopes (e.g. 'TestScope', 'DepthScope', 'MainScope') instead of loose flags."
            ),
            Self::StructFlagClutter {
                struct_name,
                flag_count,
            } => format!(
                "Struct '{struct_name}' contains multiple loose boolean fields ({flag_count}). Encapsulate related boolean flags into a domain state enum, configuration struct, or bitflags."
            ),
            Self::PrintlnInLibrary { macro_name } => format!(
                "Direct use of '{macro_name}!' in library code. Library code should return structured errors or use logging."
            ),
            Self::RedundantNestedSerializationRoundtrip => {
                "Redundant serialization roundtrip: value serialized and immediately deserialized.".to_string()
            }
            Self::RedundantVariableSerializationRoundtrip { ident } => format!(
                "Redundant serialization roundtrip: variable '{ident}' serialized and immediately deserialized."
            ),
            Self::RedundantWrapperFn {
                fn_name,
                target_desc,
            } => format!(
                "Function '{fn_name}' is a redundant wrapper that merely forwards to '{target_desc}'. Call '{target_desc}' directly."
            ),
            Self::RedundantTestPrefix { name } => format!(
                "Test function '{name}' has a redundant 'test_' prefix. Use '<action>_<scenario>_<outcome>' naming (e.g. 'parse_valid_manifest_succeeds')."
            ),
            Self::TrivialGetterSetterCombo {
                method_name,
                kind,
                field_name,
                struct_name,
                partner_name,
            } => format!(
                "Method '{method_name}' is a trivial {kind} for field '{field_name}' on struct '{struct_name}' which forms a trivial getter/setter combo with '{partner_name}'. Trivial getter/setter combos add unnecessary indirection (YAGNI)."
            ),
            Self::UnsafeFnInTest { fn_name } => format!(
                "Function '{fn_name}' in test context is declared 'unsafe'. Tests must verify code through safe interfaces."
            ),
            Self::UnsafeBlockInTest => {
                "Usage of 'unsafe' block in test context. Tests must exercise safe public abstractions.".to_string()
            }
            Self::WildcardImport { path_str } => format!(
                "Avoid wildcard import '{path_str}'. Wildcard imports obscure symbol provenance and cause namespace pollution."
            ),
            Self::IfLetSomeElseFalse => {
                "Manual 'if let Some(...) = ... else { false }' construct. Use '.is_some_and(...)' instead.".to_string()
            }
            Self::IfLetSomeElseNone => {
                "Manual 'if let Some(...) = ... else { None }' construct. Use functional combinators like '.and_then(...)' or '.map(...)'.".to_string()
            }
            Self::UnanchoredRelativePath {
                path_val,
                file_path,
            } => format!(
                "Unanchored relative path '{path_val}' in '{file_path}'. Relative paths break when executed outside the crate root."
            ),
            Self::ManualPairedLifecycleCalls {
                enter_name,
                exit_name,
                receiver_key,
            } => format!(
                "Manual paired lifecycle calls ('{enter_name}' / '{exit_name}') on '{receiver_key}'. Use a scoped closure ('with_...') or RAII guard to guarantee state restoration on error or early return."
            ),
            Self::ManualRemoveDirAllInTest => {
                "Manual 'fs::remove_dir_all' in test context. Use an RAII temporary directory guard to guarantee cleanup on assertion failures.".to_string()
            }
            Self::SingleMatchToLetElse => {
                "Match expression can be simplified using idiomatic 'let ... = ... else { ... };' construct.".to_string()
            }
            Self::RedundantMatcherBorrow { method_name } => format!(
                "Redundant '.{method_name}()' in GoogleTest assertion. GoogleTest matchers accept borrowed references directly."
            ),
            Self::ShortTestFnName { name } => format!(
                "Test function '{name}' does not conform to '<verb>_<description>_<outcome>' naming convention."
            ),
            Self::LegacyAssertionMacroInTest { macro_name } => format!(
                "Usage of '{macro_name}!' in test. Use GoogleTest matchers ('assert_that!' or 'expect_that!') instead."
            ),
            Self::UnwrapInTest => {
                "Avoid calling '.unwrap()' in test bodies. Propagate errors using '?' or assert with GoogleTest matchers.".to_string()
            }
            Self::LongQualifiedPath { path_str, .. } => format!(
                "Avoid long qualified path '{path_str}'. Add a 'use' declaration at the top of the module."
            ),
        }
    }
}
