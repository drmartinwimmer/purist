//! Architectural, structural, CLI, and modularity diagnostics derived with `miette::Diagnostic`.

/// Typed diagnostic variants for architectural, structural, CLI, and modularity `purist` rules.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error, miette::Diagnostic)]
pub(crate) enum ArchitectureDiagnostic {
    #[error(
        "Direct invocation of 'Command::new' outside dedicated command/tool module. Encapsulate external process execution in a dedicated tool struct."
    )]
    #[diagnostic(
        code(purist::centralized_command_execution),
        help(
            "Encapsulate command execution and stdout parsing in a dedicated tool builder struct in a 'tools' or 'commands' module."
        )
    )]
    UncentralizedCommandNew,

    #[error(
        "Clap command struct '{struct_name}' lacks an associated 'run' or 'execute' method. Encapsulate execution inside an associated method."
    )]
    #[diagnostic(
        code(purist::clap_struct_encapsulation),
        help("Implement 'pub fn run(self, ...)' for this command struct.")
    )]
    ClapMissingRunMethod { struct_name: String },

    #[error(
        "Field '{field_name}' in Clap struct '{struct_name}' is declared public. Clap argument fields should remain private to encapsulate command execution."
    )]
    #[diagnostic(
        code(purist::clap_struct_encapsulation),
        help("Make field private and encapsulate logic within struct methods.")
    )]
    ClapPublicField {
        field_name: String,
        struct_name: String,
    },

    #[error(
        "CLI execution method '{fn_name}' on command struct '{struct_name}' takes '&self' by reference. CLI commands should consume 'self' by value ('pub fn {fn_name}(self, ...)') to take ownership of parsed arguments and avoid unnecessary cloning."
    )]
    #[diagnostic(
        code(purist::cli_run_consumes_self),
        help("Change method receiver from '&self' to 'self'.")
    )]
    CliRunTakesRefSelf {
        fn_name: String,
        struct_name: String,
    },

    #[error(
        "Lint suppression attribute lacks both a 'reason = \"...\"' parameter and an explanatory code comment on the preceding line."
    )]
    #[diagnostic(
        code(purist::clippy_suppression_hygiene),
        help(
            "Add reason = \"...\" to the attribute and document the rationale in a comment directly above it."
        )
    )]
    SuppressionMissingReasonAndComment,

    #[error(
        "Lint suppression attribute lacks a 'reason = \"...\"' parameter explaining why it is necessary."
    )]
    #[diagnostic(
        code(purist::clippy_suppression_hygiene),
        help("Add reason = \"...\" parameter to the suppression attribute.")
    )]
    SuppressionMissingReason,

    #[error(
        "Lint suppression attribute lacks an accompanying code comment on the preceding line explaining why the lint cannot be resolved."
    )]
    #[diagnostic(
        code(purist::clippy_suppression_hygiene),
        help(
            "Add an explanatory '// ...' comment on the line immediately preceding the attribute."
        )
    )]
    SuppressionMissingComment,

    #[error(
        "Clap parser struct '{struct_name}' does not follow the top-level CLI parser naming convention. Structs deriving 'clap::Parser' must be named 'Cli' or '<Subject>Cli'."
    )]
    #[diagnostic(
        code(purist::command_struct_naming),
        help("Rename the top-level parser struct to 'Cli' or '<Subject>Cli'.")
    )]
    ParserStructNaming { struct_name: String },

    #[error(
        "Clap command struct '{struct_name}' uses an unadorned suffix without a subject prefix. Follow the '<Subject>Command', '<Subject>Args', or '<Subject>Subcommand' naming convention."
    )]
    #[diagnostic(
        code(purist::command_struct_naming),
        help(
            "Prefix '{struct_name}' with a descriptive subject (e.g. 'Workspace{struct_name}' or 'BuildCommand')."
        )
    )]
    BareCommandStructSuffix { struct_name: String },

    #[error(
        "Clap command struct '{struct_name}' does not follow the CLI command naming convention. Command structs must be named '<Subject>Command', '<Subject>Args', or '<Subject>Subcommand'."
    )]
    #[diagnostic(
        code(purist::command_struct_naming),
        help(
            "Rename the struct to end with 'Command', 'Args', or 'Subcommand' (e.g. '<Subject>Command', '<Subject>Args', or '<Subject>Subcommand')."
        )
    )]
    InvalidCommandStructNaming { struct_name: String },

    #[error(
        "Configuration struct '{struct_name}' does not follow the '<Subject>Config', '<Subject>Manifest', or '<Subject>Metadata' naming convention. Rename it to '{suggested_name}'."
    )]
    #[diagnostic(
        code(purist::config_struct_naming),
        help("Rename configuration struct '{struct_name}' to '{suggested_name}'.")
    )]
    InvalidConfigStructNaming {
        struct_name: String,
        suggested_name: String,
    },

    #[error(
        "Error type '{name}' must follow the '<Subject>Error' naming convention (e.g., 'ConfigError', 'ParseError')."
    )]
    #[diagnostic(
        code(purist::error_types),
        help(
            "Rename the error type to '<Subject>Error' so it remains unambiguous when imported or re-exported."
        )
    )]
    ErrorTypeNaming { name: String },

    #[error(
        "Manual 'Error' trait implementation for '{target_name}'. Derive 'thiserror::Error' instead."
    )]
    #[diagnostic(
        code(purist::error_types),
        help(
            "Replace manual 'impl std::error::Error' and 'Display' boilerplate with '#[derive(Debug, thiserror::Error)]'."
        )
    )]
    ManualErrorImpl { target_name: String },

    #[error(
        "Function '{fn_name}' returns unstructured error type '{err_desc}'. Use structured error enums via thiserror or anyhow::Result."
    )]
    #[diagnostic(
        code(purist::error_types),
        help("Define a dedicated error enum deriving 'thiserror::Error' or use 'anyhow::Result'.")
    )]
    UnstructuredErrorReturn { fn_name: String, err_desc: String },

    #[error(
        "Function '{fn_name}' returns 'ExitCode'. Library functions must return 'Result' and let the CLI entrypoint handle exit codes."
    )]
    #[diagnostic(
        code(purist::exit_code_hygiene),
        help("Change return type to 'Result<...>' and propagate errors to the caller.")
    )]
    FnReturnsExitCode { fn_name: String },

    #[error(
        "Raw integer literal passed to 'exit(...)'. Use 'ExitCode::SUCCESS' or 'ExitCode::FAILURE' instead."
    )]
    #[diagnostic(
        code(purist::exit_code_hygiene),
        help("Replace integer literal with 'ExitCode::SUCCESS' or 'ExitCode::FAILURE'.")
    )]
    RawIntProcessExit,

    #[error(
        "Direct invocation of 'process::exit' outside 'main()'. Propagate errors using 'Result' instead."
    )]
    #[diagnostic(
        code(purist::exit_code_hygiene),
        help(
            "Propagate errors via 'Result' and handle exit codes strictly in 'fn main() -> ExitCode'."
        )
    )]
    ProcessExitOutsideMain,

    #[error(
        "Struct '{struct_name}' is stateless and used solely as a namespace for functions. Replace with idiomatic free functions in the module or a unit struct ('struct {struct_name};')."
    )]
    #[diagnostic(
        code(purist::free_functions),
        help(
            "Remove struct '{struct_name}' and export its functions as top-level free functions, or convert it to a unit struct 'struct {struct_name};'."
        )
    )]
    StatelessNamespaceStruct { struct_name: String },

    #[error(
        "Manual iteration over {description} in '{fn_name}'. Implement and delegate to the specialized '{hook_name}' visitor hook instead to preserve recursive traversal and attribute scoping."
    )]
    #[diagnostic(
        code(purist::granular_visitor_hooks),
        help(
            "Implement the specialized 'fn {hook_name}(...)' visitor hook instead of manual iteration."
        )
    )]
    ManualAstIteration {
        description: String,
        fn_name: String,
        hook_name: String,
    },

    #[error(
        "Library root 'lib.rs' is too large ({prod_lines} production lines, limit is {max_prod_lines}). Keep 'lib.rs' as a concise facade index."
    )]
    #[diagnostic(
        code(purist::lib_facade_hygiene),
        help(
            "Move domain logic, command definitions, or helpers into dedicated submodules and re-export them from lib.rs."
        )
    )]
    LibRsTooLarge {
        prod_lines: usize,
        max_prod_lines: usize,
    },

    #[error(
        "Function '{fn_name}' in 'lib.rs' is too long ({fn_lines} lines, limit is {max_fn_lines}). Functions in 'lib.rs' should be brief facade delegates."
    )]
    #[diagnostic(
        code(purist::lib_facade_hygiene),
        help(
            "Move the implementation of '{fn_name}' into a dedicated submodule and re-export it."
        )
    )]
    LibRsFnTooLong {
        fn_name: String,
        fn_lines: usize,
        max_fn_lines: usize,
    },

    #[error(
        "Public module 'pub mod {mod_name};' in 'lib.rs' exposes internal command/tool adapters in the public library API. Use 'mod {mod_name};' or 'pub(crate) mod {mod_name};' and re-export only public facade types via 'pub use'."
    )]
    #[diagnostic(
        code(purist::lib_facade_hygiene),
        help(
            "Change 'pub mod {mod_name};' to 'mod {mod_name};' or 'pub(crate) mod {mod_name};' and re-export intended public types at the crate root."
        )
    )]
    LibRsPubAdapterMod { mod_name: String },

    #[error(
        "Test file '{file_name}' exceeds maximum length ({total_lines} lines, limit is {max_total_lines}). Split large test suites into focused sub-modules or separate integration test files."
    )]
    #[diagnostic(
        code(purist::max_file_lines),
        help("Split tests across multiple integration test files or helper modules.")
    )]
    TestFileTooLong {
        file_name: String,
        total_lines: usize,
        max_total_lines: usize,
    },

    #[error(
        "Source file '{file_name}' exceeds maximum production length ({prod_lines} production lines, limit is {max_production_lines}). Decompose into smaller, cohesive submodules."
    )]
    #[diagnostic(
        code(purist::max_file_lines),
        help("Extract distinct responsibilities into submodules declared with 'mod <name>;'.")
    )]
    SourceFileTooLong {
        file_name: String,
        prod_lines: usize,
        max_production_lines: usize,
    },

    #[error("Control flow nesting depth of {depth} exceeds the maximum limit of {max_depth}.")]
    #[diagnostic(
        code(purist::max_nesting_depth),
        help(
            "Consider refactoring deeply nested logic into separate helper functions or using early returns."
        )
    )]
    NestingDepthExceeded { depth: usize, max_depth: usize },

    #[error(
        "Bifurcated dispatch to sibling types '{type_a}' and '{type_b}' implementing trait '{shared_trait}' on method '{method_name}'. Consolidate into a single parameterized type or unify dispatch outside the branch."
    )]
    #[diagnostic(
        code(purist::no_bifurcated_trait_dispatch),
        help(
            "Consolidate '{type_a}' and '{type_b}' into a single parameterized type, or instantiate a trait object/enum before dispatching."
        )
    )]
    BifurcatedTraitDispatchWithTrait {
        type_a: String,
        type_b: String,
        method_name: String,
        shared_trait: String,
    },

    #[error(
        "Bifurcated dispatch to sibling types '{type_a}' and '{type_b}' on method '{method_name}'. Consolidate into a single parameterized type or unify dispatch outside the branch."
    )]
    #[diagnostic(
        code(purist::no_bifurcated_trait_dispatch),
        help(
            "Consolidate '{type_a}' and '{type_b}' into a single parameterized type, or instantiate a trait object/enum before dispatching."
        )
    )]
    BifurcatedTraitDispatch {
        type_a: String,
        type_b: String,
        method_name: String,
    },

    #[error(
        "Function '{fn_name}' returns 'Box<dyn Error>'. Return a concrete domain error enum deriving 'thiserror::Error' instead."
    )]
    #[diagnostic(
        code(purist::no_boxed_dyn_error),
        severity(Error),
        help(
            "Return a concrete domain error enum deriving 'thiserror::Error' with appropriate '#[from]' conversions."
        )
    )]
    BoxedDynErrorReturn { fn_name: String },

    #[error(
        "Direct invocation of 'std::env::{func_name}' outside configuration/CLI/tool modules. Parse environment parameters in a dedicated configuration layer."
    )]
    #[diagnostic(
        code(purist::no_env_access_outside_config),
        help(
            "Parse environment variables in a centralized 'config.rs', 'manifest.rs', 'metadata.rs', 'cli.rs', or 'tool' module and pass explicit parameters."
        )
    )]
    DirectEnvAccess { func_name: String },

    #[error(
        "Inline module '{mod_name}' in '{file_path}' violates modularity guidelines. Submodules should be placed in dedicated files."
    )]
    #[diagnostic(
        code(purist::no_inline_mods),
        help(
            "Move module content to '{mod_name}.rs' or '{mod_name}/mod.rs' and declare 'mod {mod_name};'."
        )
    )]
    InlineModule { mod_name: String, file_path: String },

    #[error(
        "Visitor struct '{struct_name}' declares field '{field_name}' with interior mutability ('{bad_type}'). AST visitors provide '&mut self' during single-threaded traversal; use direct mutable fields or stack types instead."
    )]
    #[diagnostic(
        code(purist::no_interior_mutability_in_visitors),
        help("Replace interior mutability with direct mutable fields or a stack.")
    )]
    VisitorInteriorMutability {
        struct_name: String,
        field_name: String,
        bad_type: String,
    },

    #[error(
        "Visitor struct '{struct_name}' contains multiple boolean/flag tracking fields ({flag_count}). Consolidate traversal state into cohesive domain scopes (e.g. 'TestScope', 'DepthScope', 'MainScope') instead of loose flags."
    )]
    #[diagnostic(
        code(purist::no_primitive_flag_clutter),
        help(
            "Consolidate loose flags into a domain scope with 'With...' traits and derive macros."
        )
    )]
    VisitorFlagClutter {
        struct_name: String,
        flag_count: usize,
    },

    #[error(
        "Struct '{struct_name}' contains multiple loose boolean fields ({flag_count}). Encapsulate related boolean flags into a domain state enum, configuration struct, or bitflags."
    )]
    #[diagnostic(
        code(purist::no_primitive_flag_clutter),
        help("Encapsulate boolean flags into an enum state machine or configuration type.")
    )]
    StructFlagClutter {
        struct_name: String,
        flag_count: usize,
    },

    #[error(
        "Direct use of '{macro_name}!' in library code. Library code should return structured errors or use logging."
    )]
    #[diagnostic(
        code(purist::no_println_in_libraries),
        help("Remove print statement; propagate information via 'Result' or use 'tracing'/'log'.")
    )]
    PrintlnInLibrary { macro_name: String },

    #[error(
        "Unanchored relative path '{path_val}' in '{file_path}'. Relative paths break when executed outside the crate root."
    )]
    #[diagnostic(
        code(purist::path_resolution),
        help(
            "Anchor path using 'Path::new(env!(\"CARGO_MANIFEST_DIR\")).join(\"{path_val}\")' or workspace root."
        )
    )]
    UnanchoredRelativePath { path_val: String, file_path: String },

    #[error(
        "Manual paired lifecycle calls ('{enter_name}' / '{exit_name}') on '{receiver_key}'. Use a scoped closure ('with_...') or RAII guard to guarantee state restoration on error or early return."
    )]
    #[diagnostic(
        code(purist::prefer_scoped_closures),
        help("Replace manual push/pop with a scoped closure or RAII guard.")
    )]
    ManualPairedLifecycleCalls {
        enter_name: String,
        exit_name: String,
        receiver_key: String,
    },
}
