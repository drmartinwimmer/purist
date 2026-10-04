# Purist

A fast, opinionated AST-based static analysis tool and linter for enforcing strict Rust code hygiene, idiomatic patterns, and clean architecture.

## Installation

```bash
cargo install purist
```

## Quick Start

Run Purist against your repository or workspace:

```bash
purist --path .
```

To output results as JSON:

```bash
purist --path . --format json
```

## Rules Enforced

Purist enforces opinionated best practices across multiple areas of code quality:

- **Encapsulation & Boundaries**:
  - `opinionated::clap_struct_encapsulation`: CLI command structs should keep fields private and expose a `run(self)` execution method.
  - `opinionated::cli_run_consumes_self`: CLI execution methods consume `self` by value to avoid redundant cloning.
  - `opinionated::centralized_command_execution`: External command invocations must be centralized via dedicated runners.
  - `opinionated::no_inline_mods`: Modules should be placed in dedicated files, not inlined in root source files.
  - `opinionated::free_functions`: Functions should not be namespaced in dummy empty structs.
- **Hygiene & Readability**:
  - `opinionated::use_declarations_over_qualified_paths`: Disallows long inline qualified paths (`crate::a::b::C`) in favor of clear `use` imports.
  - `opinionated::no_wildcard_imports`: Wildcard imports (`use foo::*`) are prohibited outside test contexts and preludes.
  - `opinionated::no_redundant_wrappers`: Functions that merely forward arguments without additional logic are flagged.
  - `opinionated::no_println_in_libraries`: Libraries must use structured returns, diagnostic collectors, or logging facades rather than raw `println!`.
  - `opinionated::clippy_suppress`: Every `#[allow(...)]` or `#[expect(...)]` attribute requires a documented `reason`.
- **Idiomatic Patterns**:
  - `opinionated::single_match_to_let_else`: Recommends `let ... = ... else { ... };` over single-variant `match` with early exits.
  - `opinionated::idiomatic_option_bool_mapping`: Prefers combinators like `.is_some_and(...)` over manual `if let Some(...) = ... else { false }`.
  - `opinionated::no_redundant_conversions`: Flags redundant sequential serialize/deserialize roundtrips.
- **Robustness & Error Handling**:
  - `opinionated::error_types`: Functions must not return raw `String` or `&str` error types.
  - `opinionated::no_boxed_dyn_error`: Production code must use structured error types instead of `Box<dyn Error>`.
  - `opinionated::exit_code_hygiene`: Direct `std::process::exit(...)` is forbidden in library modules.
  - `opinionated::path_resolution`: Unanchored relative path literals are prohibited in filesystem calls.
  - `opinionated::no_env_access_outside_config`: Direct `std::env` reads are restricted to designated config modules.
- **Testing Hygiene**:
  - `opinionated::test_patterns`: Standardizes test naming (`<verb>_<description>_<outcome>`), GoogleTest matchers, and error propagation (`?`) over `.unwrap()`.
  - `opinionated::no_test_prefix`: Forbids redundant `test_` prefixes in test function names.
  - `opinionated::no_unsafe_in_tests`: Forbids raw `unsafe` blocks and functions in test suites without explicit suppression.
  - `opinionated::raii_temp_directories`: Requires RAII temporary directory guards over manual `fs::remove_dir_all`.
  - `opinionated::test_matcher_borrow_simplification`: Disallows redundant `.as_str()` / `.as_slice()` in GoogleTest assertions.

## License

MIT
