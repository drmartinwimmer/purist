# Changelog

## [0.3.0](https://github.com/drmartinwimmer/review/compare/purist-v0.2.0...purist-v0.3.0) (2026-10-06)


### Features

* **check:** reflect purist rename, add markdown/toml/json checks, and use affirmative boolean naming ([11e3dc7](https://github.com/drmartinwimmer/review/commit/11e3dc7fda10be7a7a339d37a9f928f1fc888a3f))
* Milestone 3 — linter & formatter runner aggregator (code-review check) ([cf4c469](https://github.com/drmartinwimmer/review/commit/cf4c469f2b6291440f20562ef3b0eb28906759bf))
* **purist:** add googletest_conventions lint rule ([0045346](https://github.com/drmartinwimmer/review/commit/004534687742a087ff9bcbfe9fc2a0ebc6ef4fa5))
* **purist:** add googletest_conventions lint rule ([69541f1](https://github.com/drmartinwimmer/review/commit/69541f14a957e7e864fcea2202d3dfbf6bbc2ec6))
* **purist:** add max_file_lines rule to enforce file length boundaries ([cbce6cb](https://github.com/drmartinwimmer/review/commit/cbce6cbc46a8d299267a77565ecfd89e4617958a))
* **purist:** add max_file_lines rule with AST line counting and Cargo.toml configuration ([ce30cd1](https://github.com/drmartinwimmer/review/commit/ce30cd1e0e1ad101a3ecac4c52bcad471fd0e508))
* **purist:** add no_double_negation lint rule ([6fcff2b](https://github.com/drmartinwimmer/review/commit/6fcff2b6aabcf4e40f6c33e56747ceb0cd6918aa))
* **purist:** add no_double_negation lint rule ([b98740b](https://github.com/drmartinwimmer/review/commit/b98740b0457dabff981b3b85302b77d0b12983b2))
* **purist:** add no_trivial_getters_setters rule to flag redundant accessors (YAGNI) ([e8e427f](https://github.com/drmartinwimmer/review/commit/e8e427fc76177095c09b9c4f748816d901112cc6))
* **purist:** add no_trivial_getters_setters rule to flag redundant accessors (YAGNI) ([6d510a4](https://github.com/drmartinwimmer/review/commit/6d510a4964174e4552fc471402c49f8f08a4eaa2))

## [0.2.0](https://github.com/drmartinwimmer/review/compare/purist-v0.1.0...purist-v0.2.0) (2026-10-04)

### Features

#### CLI & Core Engine

- **cli:** introduce standalone `purist` CLI with support for recursive directory scanning, path filtering, and `--format json|text` output modes ([932d732](https://github.com/drmartinwimmer/review/commit/932d732a32ae6ea94f9a4379de6d9d5dcb52418e))
- **diagnostics:** render rich terminal error/warning diagnostics with annotated source code snippets, line/column pointers, severity highlights, and actionable suggestions via `annotate-snippets` and ANSI styling ([932d732](https://github.com/drmartinwimmer/review/commit/932d732a32ae6ea94f9a4379de6d9d5dcb52418e))
- **configuration:** support granular rule level configuration (`allow`, `warn`, `deny`) via configuration files and inline file-level comment suppressions (`// purist: allow(<rule>)`) ([932d732](https://github.com/drmartinwimmer/review/commit/932d732a32ae6ea94f9a4379de6d9d5dcb52418e))
- **release:** configure Release Please automated release and publishing workflow for `purist` ([#17](https://github.com/drmartinwimmer/review/issues/17)) ([932d732](https://github.com/drmartinwimmer/review/commit/932d732a32ae6ea94f9a4379de6d9d5dcb52418e))

#### Encapsulation & Architectural Boundaries

- **clap_struct_encapsulation:** require CLI command structs to keep fields private and expose a `run(self)` execution method
- **cli_run_consumes_self:** require CLI `run(self)` methods to take `self` by value rather than by reference to eliminate redundant cloning
- **centralized_command_execution:** flag uncentralized process invocations (`std::process::Command`), requiring centralized runner abstractions
- **no_inline_mods:** require submodules to reside in dedicated files rather than inline module blocks in root files
- **free_functions:** forbid dummy empty namespace structs used solely to group static methods in favor of free functions

#### Code Hygiene & Readability

- **use_declarations_over_qualified_paths:** flag verbose, deeply nested inline paths in signatures and expressions in favor of module-level `use` declarations
- **no_wildcard_imports:** disallow wildcard `use path::*` imports outside test scopes and preludes to prevent namespace pollution and name collisions
- **no_redundant_wrappers:** detect and flag trivial wrapper functions that merely forward arguments without additional logic or transformations
- **no_println_in_libraries:** prohibit raw `println!` and `eprintln!` calls in library crates, requiring structured returns or diagnostics
- **clippy_suppress:** enforce hygienic lint suppressions by requiring an explicit `reason` attribute on all `#[allow(...)]` and `#[expect(...)]` directives

#### Idiomatic Rust Patterns

- **single_match_to_let_else:** suggest replacing single-variant `match` expressions with diverging early exits using idiomatic `let ... else` statements
- **idiomatic_option_bool_mapping:** suggest replacing verbose `if let Some(...) = opt { expr } else { false }` with expressive `Option::is_some_and` or combinators
- **no_redundant_conversions:** flag redundant sequential serialization and deserialization roundtrips (e.g. converting between JSON values and strings repeatedly)

#### Robustness & Error Handling

- **error_types:** disallow unstructured error types (`Result<T, String>` or `Result<T, &'static str>`) in favor of structured `thiserror` enums or `anyhow::Result`
- **no_boxed_dyn_error:** prohibit `Box<dyn Error>` in public and production function signatures
- **exit_code_hygiene:** disallow direct `std::process::exit` calls in library modules to preserve proper RAII unwinding and cleanup
- **path_resolution:** disallow unanchored relative path literals in production filesystem operations to prevent runtime working-directory sensitivity
- **no_env_access_outside_config:** restrict direct `std::env` reads to dedicated configuration modules

#### Testing Hygiene

- **test_patterns:** enforce descriptive three-part test names (`<verb>_<scenario>_<outcome>`), GoogleTest matchers, and error propagation (`?`) over panicking `.unwrap()`
- **no_test_prefix:** forbid redundant `test_` prefixes in test function names
- **no_unsafe_in_tests:** prohibit unsuppressed `unsafe` blocks in test functions
- **raii_temp_directories:** require RAII temporary directory guards over manual `fs::remove_dir_all` calls in test suites
- **test_matcher_borrow_simplification:** flag unnecessary `.as_str()` and `.as_slice()` conversion calls inside GoogleTest assertion matchers
