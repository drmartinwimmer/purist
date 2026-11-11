//! Strongly-typed rule identifiers and diagnostic findings for `purist`.
//!
//! Centralizes rule names (`RuleId`), diagnostic variants (`RuleDiagnostic`), default severities,
//! human-readable formatting, and suggested fixes so individual AST rules never construct raw
//! stringly-typed diagnostics directly.

mod fixes;
mod messages;
mod rule_id;

pub(crate) use self::rule_id::RuleId;

use crate::diagnostics::{Diagnostic, Severity, Span};

/// Strongly-typed enum representing every lint diagnostic emitted by built-in `purist` rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RuleDiagnostic {
    UncentralizedCommandNew,
    ClapMissingRunMethod {
        struct_name: String,
    },
    ClapPublicField {
        field_name: String,
        struct_name: String,
    },
    CliRunTakesRefSelf {
        fn_name: String,
        struct_name: String,
    },
    SuppressionMissingReasonAndComment,
    SuppressionMissingReason,
    SuppressionMissingComment,
    ParserStructNaming {
        struct_name: String,
    },
    BareCommandStructSuffix {
        struct_name: String,
    },
    InvalidCommandStructNaming {
        struct_name: String,
    },
    InvalidConfigStructNaming {
        struct_name: String,
        suggested_name: String,
    },
    VagueCheckerName {
        name: String,
    },
    ErrorTypeNaming {
        name: String,
    },
    ManualErrorImpl {
        target_name: String,
    },
    UnstructuredErrorReturn {
        fn_name: String,
        err_desc: String,
    },
    FnReturnsExitCode {
        fn_name: String,
    },
    RawIntProcessExit,
    ProcessExitOutsideMain,
    StatelessNamespaceStruct {
        struct_name: String,
    },
    StandardTestAttr {
        fn_name: String,
    },
    StandardAssertMacro,
    StandardAssertEqMacro {
        mac_name: String,
    },
    ExpectInTest,
    AssertThatBinaryEq,
    AssertThatBinaryNe,
    AssertThatIsOk,
    AssertThatIsErr,
    AssertThatIsSome,
    AssertThatIsNone,
    ManualAstIteration {
        description: String,
        fn_name: String,
        hook_name: String,
    },
    LibRsTooLarge {
        prod_lines: usize,
        max_prod_lines: usize,
    },
    LibRsFnTooLong {
        fn_name: String,
        fn_lines: usize,
        max_fn_lines: usize,
    },
    LibRsPubAdapterMod {
        mod_name: String,
    },
    TestFileTooLong {
        file_name: String,
        total_lines: usize,
        max_total_lines: usize,
    },
    SourceFileTooLong {
        file_name: String,
        prod_lines: usize,
        max_production_lines: usize,
    },
    NestingDepthExceeded {
        depth: usize,
        max_depth: usize,
    },
    BifurcatedTraitDispatch {
        type_a: String,
        type_b: String,
        method_name: String,
        shared_trait: Option<String>,
    },
    BoxedDynErrorReturn {
        fn_name: String,
    },
    NegativeBooleanMethod {
        fn_name: String,
        suggested: String,
    },
    NegativeBooleanPredicate {
        fn_name: String,
        suggested: String,
    },
    NegativeBooleanParameter {
        param_name: String,
    },
    DirectEnvAccess {
        func_name: String,
    },
    HasTraitPrefix {
        trait_name: String,
    },
    InlineModule {
        mod_name: String,
        file_path: String,
    },
    VisitorInteriorMutability {
        struct_name: String,
        field_name: String,
        bad_type: String,
    },
    NegativeBooleanField {
        field_name: String,
        container_desc: String,
        suggested: String,
    },
    NegativeBooleanVariable {
        var_name: String,
        suggested: String,
    },
    VisitorFlagClutter {
        struct_name: String,
        flag_count: usize,
    },
    StructFlagClutter {
        struct_name: String,
        flag_count: usize,
    },
    PrintlnInLibrary {
        macro_name: String,
    },
    RedundantNestedSerializationRoundtrip,
    RedundantVariableSerializationRoundtrip {
        ident: String,
    },
    RedundantWrapperFn {
        fn_name: String,
        target_desc: String,
    },
    RedundantTestPrefix {
        name: String,
    },
    TrivialGetterSetterCombo {
        method_name: String,
        kind: String,
        field_name: String,
        struct_name: String,
        partner_name: String,
    },
    UnsafeFnInTest {
        fn_name: String,
    },
    UnsafeBlockInTest,
    WildcardImport {
        path_str: String,
    },
    IfLetSomeElseFalse,
    IfLetSomeElseNone,
    UnanchoredRelativePath {
        path_val: String,
        file_path: String,
    },
    ManualPairedLifecycleCalls {
        enter_name: String,
        exit_name: String,
        receiver_key: String,
    },
    ManualRemoveDirAllInTest,
    SingleMatchToLetElse,
    RedundantMatcherBorrow {
        method_name: String,
    },
    ShortTestFnName {
        name: String,
    },
    LegacyAssertionMacroInTest {
        macro_name: String,
    },
    UnwrapInTest,
    LongQualifiedPath {
        path_str: String,
        target_ident: String,
    },
}

impl RuleDiagnostic {
    /// Returns the canonical [`RuleId`] associated with this diagnostic variant.
    pub(crate) const fn rule_id(&self) -> RuleId {
        match self {
            Self::UncentralizedCommandNew => RuleId::CentralizedCommandExecution,
            Self::ClapMissingRunMethod { .. } | Self::ClapPublicField { .. } => {
                RuleId::ClapStructEncapsulation
            }
            Self::CliRunTakesRefSelf { .. } => RuleId::CliRunConsumesSelf,
            Self::SuppressionMissingReasonAndComment
            | Self::SuppressionMissingReason
            | Self::SuppressionMissingComment => RuleId::ClippySuppressionHygiene,
            Self::ParserStructNaming { .. }
            | Self::BareCommandStructSuffix { .. }
            | Self::InvalidCommandStructNaming { .. } => RuleId::CommandStructNaming,
            Self::InvalidConfigStructNaming { .. } => RuleId::ConfigStructNaming,
            Self::VagueCheckerName { .. } => RuleId::DescriptiveCheckerNames,
            Self::ErrorTypeNaming { .. }
            | Self::ManualErrorImpl { .. }
            | Self::UnstructuredErrorReturn { .. } => RuleId::ErrorTypes,
            Self::FnReturnsExitCode { .. }
            | Self::RawIntProcessExit
            | Self::ProcessExitOutsideMain => RuleId::ExitCodeHygiene,
            Self::StatelessNamespaceStruct { .. } => RuleId::FreeFunctions,
            Self::StandardTestAttr { .. }
            | Self::StandardAssertMacro
            | Self::StandardAssertEqMacro { .. }
            | Self::ExpectInTest
            | Self::AssertThatBinaryEq
            | Self::AssertThatBinaryNe
            | Self::AssertThatIsOk
            | Self::AssertThatIsErr
            | Self::AssertThatIsSome
            | Self::AssertThatIsNone => RuleId::GoogletestConventions,
            Self::ManualAstIteration { .. } => RuleId::GranularVisitorHooks,
            Self::LibRsTooLarge { .. }
            | Self::LibRsFnTooLong { .. }
            | Self::LibRsPubAdapterMod { .. } => RuleId::LibFacadeHygiene,
            Self::TestFileTooLong { .. } | Self::SourceFileTooLong { .. } => RuleId::MaxFileLines,
            Self::NestingDepthExceeded { .. } => RuleId::MaxNestingDepth,
            Self::BifurcatedTraitDispatch { .. } => RuleId::NoBifurcatedTraitDispatch,
            Self::BoxedDynErrorReturn { .. } => RuleId::NoBoxedDynError,
            Self::NegativeBooleanMethod { .. }
            | Self::NegativeBooleanPredicate { .. }
            | Self::NegativeBooleanParameter { .. } => RuleId::NoDoubleNegation,
            Self::DirectEnvAccess { .. } => RuleId::NoEnvAccessOutsideConfig,
            Self::HasTraitPrefix { .. } => RuleId::NoHasTraitPrefix,
            Self::InlineModule { .. } => RuleId::NoInlineMods,
            Self::VisitorInteriorMutability { .. } => RuleId::NoInteriorMutabilityInVisitors,
            Self::NegativeBooleanField { .. } | Self::NegativeBooleanVariable { .. } => {
                RuleId::NoNegativeBool
            }
            Self::VisitorFlagClutter { .. } | Self::StructFlagClutter { .. } => {
                RuleId::NoPrimitiveFlagClutter
            }
            Self::PrintlnInLibrary { .. } => RuleId::NoPrintlnInLibraries,
            Self::RedundantNestedSerializationRoundtrip
            | Self::RedundantVariableSerializationRoundtrip { .. } => {
                RuleId::NoRedundantConversions
            }
            Self::RedundantWrapperFn { .. } => RuleId::NoRedundantWrappers,
            Self::RedundantTestPrefix { .. } => RuleId::NoTestPrefix,
            Self::TrivialGetterSetterCombo { .. } => RuleId::NoTrivialGettersSetters,
            Self::UnsafeFnInTest { .. } | Self::UnsafeBlockInTest => RuleId::NoUnsafeInTests,
            Self::WildcardImport { .. } => RuleId::NoWildcardImports,
            Self::IfLetSomeElseFalse | Self::IfLetSomeElseNone => {
                RuleId::IdiomaticOptionBoolMapping
            }
            Self::UnanchoredRelativePath { .. } => RuleId::PathResolution,
            Self::ManualPairedLifecycleCalls { .. } => RuleId::PreferScopedClosures,
            Self::ManualRemoveDirAllInTest => RuleId::RaiiTempDirectories,
            Self::SingleMatchToLetElse => RuleId::SingleMatchToLetElse,
            Self::RedundantMatcherBorrow { .. } => RuleId::TestMatcherBorrowSimplification,
            Self::ShortTestFnName { .. }
            | Self::LegacyAssertionMacroInTest { .. }
            | Self::UnwrapInTest => RuleId::TestPatterns,
            Self::LongQualifiedPath { .. } => RuleId::UseDeclarationsOverQualifiedPaths,
        }
    }

    /// Returns the default [`Severity`] for this diagnostic variant before `Cargo.toml` overrides.
    pub(crate) const fn default_severity(&self) -> Severity {
        match self {
            Self::BoxedDynErrorReturn { .. }
            | Self::UnsafeFnInTest { .. }
            | Self::UnsafeBlockInTest => Severity::Error,
            _ => Severity::Warning,
        }
    }

    /// Converts this typed diagnostic variant and its source span into a [`Diagnostic`].
    pub(crate) fn into_diagnostic(self, span: Span) -> Diagnostic {
        Diagnostic::new(
            self.rule_id().qualified_name(),
            self.default_severity(),
            self.message(),
        )
        .with_span(span)
        .with_suggested_fix(self.suggested_fix())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use googletest::prelude::*;
    use std::path::Path;

    #[googletest::test]
    fn rule_id_round_trips_unqualified_and_qualified_names() -> googletest::Result<()> {
        let id = RuleId::ErrorTypes;
        verify_that!(id.as_str(), eq("error_types"))?;
        verify_that!(id.qualified_name(), eq("purist::error_types"))?;
        verify_that!(RuleId::from_name("error_types"), some(eq(id)))?;
        verify_that!(RuleId::from_name("purist::error_types"), some(eq(id)))?;
        verify_that!(RuleId::from_name("opinionated::error_types"), some(eq(id)))?;
        Ok(())
    }

    #[googletest::test]
    fn rule_diagnostic_converts_into_populated_diagnostic() -> googletest::Result<()> {
        let span = Span::new(Path::new("src/lib.rs"), 10, 5, 10, 20);
        let diag = RuleDiagnostic::BoxedDynErrorReturn {
            fn_name: "execute".to_string(),
        }
        .into_diagnostic(span);

        verify_that!(diag.rule, eq("purist::no_boxed_dyn_error"))?;
        verify_that!(diag.severity, eq(Severity::Error))?;
        verify_that!(
            diag.message,
            contains_substring("Function 'execute' returns 'Box<dyn Error>'")
        )?;
        verify_that!(
            diag.suggested_fix,
            some(contains_substring("thiserror::Error"))
        )?;
        Ok(())
    }
}
