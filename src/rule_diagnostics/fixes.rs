//! Suggested fix formatting for [`RuleDiagnostic`].

use super::RuleDiagnostic;

impl RuleDiagnostic {
    /// Formats the actionable suggested fix for this finding.
    pub(crate) fn suggested_fix(&self) -> String {
        match self {
            Self::UncentralizedCommandNew => {
                "Encapsulate command execution and stdout parsing in a dedicated tool builder struct in a 'tools' or 'commands' module.".to_string()
            }
            Self::ClapMissingRunMethod { .. } => {
                "Implement 'pub fn run(self, ...)' for this command struct.".to_string()
            }
            Self::ClapPublicField { .. } => {
                "Make field private and encapsulate logic within struct methods.".to_string()
            }
            Self::CliRunTakesRefSelf { .. } => {
                "Change method receiver from '&self' to 'self'.".to_string()
            }
            Self::SuppressionMissingReasonAndComment => {
                "Add reason = \"...\" to the attribute and document the rationale in a comment directly above it.".to_string()
            }
            Self::SuppressionMissingReason => {
                "Add reason = \"...\" parameter to the suppression attribute.".to_string()
            }
            Self::SuppressionMissingComment => {
                "Add an explanatory '// ...' comment on the line immediately preceding the attribute.".to_string()
            }
            Self::ParserStructNaming { .. } => {
                "Rename the top-level parser struct to 'Cli' or '<Subject>Cli'.".to_string()
            }
            Self::BareCommandStructSuffix { struct_name } => format!(
                "Prefix '{struct_name}' with a descriptive subject (e.g. 'Workspace{struct_name}' or 'BuildCommand')."
            ),
            Self::InvalidCommandStructNaming { .. } => {
                "Rename the struct to end with 'Command', 'Args', or 'Subcommand' (e.g. '<Subject>Command', '<Subject>Args', or '<Subject>Subcommand').".to_string()
            }
            Self::InvalidConfigStructNaming {
                struct_name,
                suggested_name,
            } => format!("Rename configuration struct '{struct_name}' to '{suggested_name}'."),
            Self::VagueCheckerName { .. } => {
                "Rename to include both the subject and the condition/violation being checked.".to_string()
            }
            Self::ErrorTypeNaming { .. } => {
                "Rename the error type to '<Subject>Error' so it remains unambiguous when imported or re-exported.".to_string()
            }
            Self::ManualErrorImpl { .. } => {
                "Replace manual 'impl std::error::Error' and 'Display' boilerplate with '#[derive(Debug, thiserror::Error)]'.".to_string()
            }
            Self::UnstructuredErrorReturn { .. } => {
                "Define a dedicated error enum deriving 'thiserror::Error' or use 'anyhow::Result'.".to_string()
            }
            Self::FnReturnsExitCode { .. } => {
                "Change return type to 'Result<...>' and propagate errors to the caller.".to_string()
            }
            Self::RawIntProcessExit => {
                "Replace integer literal with 'ExitCode::SUCCESS' or 'ExitCode::FAILURE'.".to_string()
            }
            Self::ProcessExitOutsideMain => {
                "Propagate errors via 'Result' and handle exit codes strictly in 'fn main() -> ExitCode'.".to_string()
            }
            Self::StatelessNamespaceStruct { struct_name } => format!(
                "Remove struct '{struct_name}' and export its functions as top-level free functions, or convert it to a unit struct 'struct {struct_name};'."
            ),
            Self::StandardTestAttr { .. } => {
                "Replace '#[test]' with '#[googletest::test]'.".to_string()
            }
            Self::StandardAssertMacro => {
                "Replace with 'assert_that!(actual, matcher)' or 'expect_that!(actual, matcher)'.".to_string()
            }
            Self::StandardAssertEqMacro { .. } => {
                "Replace with 'assert_that!(actual, eq(expected))'.".to_string()
            }
            Self::ExpectInTest => {
                "Return 'Result<(), Box<dyn std::error::Error>>' or 'googletest::Result<()>' and use '?' instead of '.expect(...)'.".to_string()
            }
            Self::AssertThatBinaryEq => {
                "Replace with 'assert_that!(left, eq(right))'.".to_string()
            }
            Self::AssertThatBinaryNe => {
                "Replace with 'assert_that!(left, not(eq(right)))'.".to_string()
            }
            Self::AssertThatIsOk => "Replace with 'assert_that!(target, ok(...))'.".to_string(),
            Self::AssertThatIsErr => "Replace with 'assert_that!(target, err(...))'.".to_string(),
            Self::AssertThatIsSome => "Replace with 'assert_that!(target, some(...))'.".to_string(),
            Self::AssertThatIsNone => "Replace with 'assert_that!(target, none())'.".to_string(),
            Self::ManualAstIteration { hook_name, .. } => format!(
                "Implement the specialized 'fn {hook_name}(...)' visitor hook instead of manual iteration."
            ),
            Self::LibRsTooLarge { .. } => {
                "Move domain logic, command definitions, or helpers into dedicated submodules and re-export them from lib.rs.".to_string()
            }
            Self::LibRsFnTooLong { fn_name, .. } => format!(
                "Move the implementation of '{fn_name}' into a dedicated submodule and re-export it."
            ),
            Self::LibRsPubAdapterMod { mod_name } => format!(
                "Change 'pub mod {mod_name};' to 'mod {mod_name};' or 'pub(crate) mod {mod_name};' and re-export intended public types at the crate root."
            ),
            Self::TestFileTooLong { .. } => {
                "Split tests across multiple integration test files or helper modules.".to_string()
            }
            Self::SourceFileTooLong { .. } => {
                "Extract distinct responsibilities into submodules declared with 'mod <name>;'.".to_string()
            }
            Self::NestingDepthExceeded { .. } => {
                "Consider refactoring deeply nested logic into separate helper functions or using early returns.".to_string()
            }
            Self::BifurcatedTraitDispatch { type_a, type_b, .. } => format!(
                "Consolidate '{type_a}' and '{type_b}' into a single parameterized type, or instantiate a trait object/enum before dispatching."
            ),
            Self::BoxedDynErrorReturn { .. } => {
                "Return a concrete domain error enum deriving 'thiserror::Error' with appropriate '#[from]' conversions.".to_string()
            }
            Self::NegativeBooleanMethod { suggested, .. } => format!(
                "Rename to an affirmative identifier (e.g. '{suggested}')."
            ),
            Self::NegativeBooleanPredicate { suggested, .. } => format!(
                "Rename to an affirmative predicate (e.g. '{suggested}')."
            ),
            Self::NegativeBooleanParameter { .. } => {
                "Rename to an affirmative parameter such as 'enabled' or 'include'.".to_string()
            }
            Self::DirectEnvAccess { .. } => {
                "Parse environment variables in a centralized 'config.rs', 'manifest.rs', 'metadata.rs', 'cli.rs', or 'tool' module and pass explicit parameters.".to_string()
            }
            Self::HasTraitPrefix { .. } => {
                "Rename trait to 'With...' if providing scoped execution/context, or a descriptive noun/adjective.".to_string()
            }
            Self::InlineModule { mod_name, .. } => format!(
                "Move module content to '{mod_name}.rs' or '{mod_name}/mod.rs' and declare 'mod {mod_name};'."
            ),
            Self::VisitorInteriorMutability { .. } => {
                "Replace interior mutability with direct mutable fields or a stack.".to_string()
            }
            Self::NegativeBooleanField {
                field_name,
                suggested,
                ..
            } => format!("Rename '{field_name}' to affirmative '{suggested}'."),
            Self::NegativeBooleanVariable {
                var_name,
                suggested,
            } => format!("Rename '{var_name}' to affirmative '{suggested}'."),
            Self::VisitorFlagClutter { .. } => {
                "Consolidate loose flags into a domain scope with 'With...' traits and derive macros.".to_string()
            }
            Self::StructFlagClutter { .. } => {
                "Encapsulate boolean flags into an enum state machine or configuration type.".to_string()
            }
            Self::PrintlnInLibrary { .. } => {
                "Remove print statement; propagate information via 'Result' or use 'tracing'/'log'.".to_string()
            }
            Self::RedundantNestedSerializationRoundtrip => {
                "Use 'Clone::clone', 'From::from', or 'serde_json::to_value' instead of stringifying and re-parsing.".to_string()
            }
            Self::RedundantVariableSerializationRoundtrip { .. } => {
                "Use 'Clone::clone', 'From::from', or direct mapping instead of roundtrip serialization.".to_string()
            }
            Self::RedundantWrapperFn {
                fn_name,
                target_desc,
            } => format!(
                "Remove wrapper function '{fn_name}' and call '{target_desc}' directly at call sites."
            ),
            Self::RedundantTestPrefix { name } => format!(
                "Rename test '{name}' to remove the 'test_' prefix and follow '<action>_<scenario>_<outcome>'."
            ),
            Self::TrivialGetterSetterCombo {
                method_name,
                kind,
                field_name,
                ..
            } => format!(
                "Remove trivial {kind} '{method_name}' and access field '{field_name}' directly."
            ),
            Self::UnsafeFnInTest { .. } => {
                "Remove 'unsafe' from test code; verify behavior strictly through safe public interfaces.".to_string()
            }
            Self::UnsafeBlockInTest => {
                "Remove 'unsafe' block from test code; verify behavior strictly through safe public interfaces.".to_string()
            }
            Self::WildcardImport { .. } => {
                "Replace wildcard import with explicit item imports.".to_string()
            }
            Self::IfLetSomeElseFalse => "Replace with '.is_some_and(|val| ...)'.".to_string(),
            Self::IfLetSomeElseNone => "Replace with '.and_then(...)' or '.map(...)'. ".to_string(),
            Self::UnanchoredRelativePath { path_val, .. } => format!(
                "Anchor path using 'Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"{path_val}\")' or workspace root."
            ),
            Self::ManualPairedLifecycleCalls { .. } => {
                "Replace manual push/pop with a scoped closure or RAII guard.".to_string()
            }
            Self::ManualRemoveDirAllInTest => {
                "Use an RAII temporary directory guard (implementing 'Drop' or via 'tempfile') instead of manual removal.".to_string()
            }
            Self::SingleMatchToLetElse => {
                "Replace 'match' with 'let Some(...) = expr else { ... };' to reduce indentation.".to_string()
            }
            Self::RedundantMatcherBorrow { method_name } => format!(
                "Replace '.{method_name}()' with a reference borrowing the expression directly ('&<expr>')."
            ),
            Self::ShortTestFnName { .. } => {
                "Rename test to follow '<verb>_<description>_<outcome>' (e.g. 'parse_valid_input_succeeds').".to_string()
            }
            Self::LegacyAssertionMacroInTest { .. } => {
                "Replace with 'assert_that!(actual, eq(expected))' or 'expect_that!(actual, eq(expected))'.".to_string()
            }
            Self::UnwrapInTest => {
                "Return Result<(), Box<dyn std::error::Error>> or googletest::Result<()> and use '?' instead of '.unwrap()'.".to_string()
            }
            Self::LongQualifiedPath {
                path_str,
                target_ident,
            } => format!(
                "Import '{path_str}' via 'use {path_str};' and refer to '{target_ident}' directly."
            ),
        }
    }
}
