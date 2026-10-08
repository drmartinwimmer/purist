# Purist

> **A fast, purist AST-based static analysis tool and linter for enforcing strict Rust code hygiene, idiomatic patterns, and clean architecture.**

[![CI](https://github.com/drmartinwimmer/purist/actions/workflows/ci.yml/badge.svg)](https://github.com/drmartinwimmer/purist/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/purist.svg)](https://crates.io/crates/purist)
[![Docs.rs](https://docs.rs/purist/badge.svg)](https://docs.rs/purist)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://github.com/drmartinwimmer/purist/blob/main/LICENSE)
[![Built with Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org/)
[![MSRV: 1.88+](https://img.shields.io/badge/MSRV-1.88%2B-blue.svg?logo=rust)](https://www.rust-lang.org/)

## Installation

```bash
cargo install purist
```

## Quick Start

Run Purist against your repository or workspace:

```bash
purist
```

By default, Purist analyzes the Cargo project in the current working directory (`.`), which must contain a `Cargo.toml`. Discovery is strictly focused on targets defined by the Cargo manifest (including all workspace members and child crates) with no directory-crawling fallbacks.

To analyze a specific file within the workspace, use the `--path` option:

```bash
purist --path src/lib.rs
```

To onboard an existing project without a massive upfront refactoring, use `--allow`. This runs all checks and automatically disables any triggered rules under `[lints.purist]` in `Cargo.toml` by setting them to `"allow"`, allowing you to adopt Purist immediately and re-enable/resolve rules incrementally:

```bash
purist --allow
```

To output results as JSON:

```bash
purist --format json
```

## Rules Enforced

Purist enforces strict best practices across multiple areas of code quality:

- **Encapsulation & Boundaries**:
  - `purist::clap_struct_encapsulation`: CLI command structs should keep fields private and expose a `run(self)` execution method.
  - `purist::cli_run_consumes_self`: CLI execution methods consume `self` by value to avoid redundant cloning.
  - `purist::centralized_command_execution`: External command invocations must be centralized via dedicated runners.
  - `purist::no_inline_mods`: Modules should be placed in dedicated files, not inlined in root source files.
  - `purist::free_functions`: Functions should not be namespaced in dummy empty structs.
  - `purist::no_trivial_getters_setters`: Flags trivial getter/setter combos where exposing or accessing the field directly would suffice (YAGNI).
- **Hygiene & Readability**:
  - `purist::use_declarations_over_qualified_paths`: Disallows long inline qualified paths (`crate::a::b::C`) in favor of clear `use` imports.
  - `purist::no_wildcard_imports`: Wildcard imports (`use foo::*`) are prohibited outside test contexts and preludes.
  - `purist::no_redundant_wrappers`: Functions that merely forward arguments without additional logic are flagged.
  - `purist::no_println_in_libraries`: Libraries must use structured returns, diagnostic collectors, or logging facades rather than raw `println!`.
  - `purist::no_double_negation`: Prohibits negative boolean naming (such as `with_skip_*`, `is_skip_*`, `skip: bool`) to prevent double negations.
  - `purist::clippy_suppression_hygiene`: Every `#[allow(...)]` or `#[expect(...)]` attribute requires a documented `reason`.
- **Idiomatic Patterns**:
  - `purist::single_match_to_let_else`: Recommends `let ... = ... else { ... };` over single-variant `match` with early exits.
  - `purist::idiomatic_option_bool_mapping`: Prefers combinators like `.is_some_and(...)` over manual `if let Some(...) = ... else { false }`.
  - `purist::no_redundant_conversions`: Flags redundant sequential serialize/deserialize roundtrips.
- **Robustness & Error Handling**:
  - `purist::error_types`: Functions must not return raw `String` or `&str` error types.
  - `purist::no_boxed_dyn_error`: Production code must use structured error types instead of `Box<dyn Error>`.
  - `purist::exit_code_hygiene`: Direct `std::process::exit(...)` is forbidden in library modules.
  - `purist::path_resolution`: Unanchored relative path literals are prohibited in filesystem calls.
  - `purist::no_env_access_outside_config`: Direct `std::env` reads are restricted to designated config modules.
- **Testing Hygiene**:
  - `purist::test_patterns`: Standardizes test naming (`<verb>_<description>_<outcome>`), GoogleTest matchers, and error propagation (`?`) over `.unwrap()`.
  - `purist::no_test_prefix`: Forbids redundant `test_` prefixes in test function names.
  - `purist::no_unsafe_in_tests`: Forbids raw `unsafe` blocks and functions in test suites without explicit suppression.
  - `purist::raii_temp_directories`: Requires RAII temporary directory guards over manual `fs::remove_dir_all`.
  - `purist::test_matcher_borrow_simplification`: Disallows redundant `.as_str()` / `.as_slice()` in GoogleTest assertions.
  - `purist::googletest_conventions`: Enforces idiomatic GoogleTest conventions (`#[googletest::test]`, `assert_that!`, `?` over `.expect()`, and direct expressive matchers).

## License

MIT
