# Changelog

## [0.6.0](https://github.com/drmartinwimmer/purist/compare/purist-v0.5.0...purist-v0.6.0) (2026-10-08)


### Features

* **cli:** load Cargo.toml once, default to working directory, and restrict --path to workspace files ([7ae9c35](https://github.com/drmartinwimmer/purist/commit/7ae9c35198881d02ef398024cf91f031b63239b3))
* **cli:** load Cargo.toml once, default to working directory, and restrict --path to workspace files ([b3f26ea](https://github.com/drmartinwimmer/purist/commit/b3f26ea9a8bebd0ed10e38d26daee40ec5101caa))

## [0.5.0](https://github.com/drmartinwimmer/purist/compare/purist-v0.4.0...purist-v0.5.0) (2026-10-08)


### Features

* add --allow flag to disable triggered checks in Cargo.toml ([26931ad](https://github.com/drmartinwimmer/purist/commit/26931ad3f155663222027fe932d34d8a1435e9cc))
* add --allow flag to disable triggered checks in Cargo.toml ([3cf299a](https://github.com/drmartinwimmer/purist/commit/3cf299a6a02217f425852dffcff55fb7301e4ce3))

## [0.4.0](https://github.com/drmartinwimmer/purist/compare/purist-v0.3.0...purist-v0.4.0) (2026-10-07)


### Features

* **check:** reflect purist rename, add markdown/toml/json checks, and use affirmative boolean naming ([11e3dc7](https://github.com/drmartinwimmer/purist/commit/11e3dc7fda10be7a7a339d37a9f928f1fc888a3f))
* **diagnostics:** render console diagnostics with annotate-snippets and anstyle-query ([9af199f](https://github.com/drmartinwimmer/purist/commit/9af199f6b86e88f2b71de979bafa6f19c112d216))
* **diagnostics:** render console diagnostics with annotate-snippets and anstyle-query ([4630e48](https://github.com/drmartinwimmer/purist/commit/4630e48a8e218de1fbbe620bb07f6bf271f4f067))
* **m1:** implement core CLI architecture, multi-crate workspace, and cargo-toml lint configurator ([2480005](https://github.com/drmartinwimmer/purist/commit/248000576ac3a024bf54fcede570597fabfd62c7))
* Milestone 2 — opinionated linter, 23-rule suite, Cargo target discovery, manifest/in-code config, and command ownership refactor ([391c224](https://github.com/drmartinwimmer/purist/commit/391c22425e699286003cdc3ae1b90ba7d2de6062))
* Milestone 3 — linter & formatter runner aggregator (code-review check) ([cf4c469](https://github.com/drmartinwimmer/purist/commit/cf4c469f2b6291440f20562ef3b0eb28906759bf))
* **opinionated:** implement 15 AST rules, Cargo target discovery, and manifest/in-code lint configuration ([1ba5ed8](https://github.com/drmartinwimmer/purist/commit/1ba5ed8acf3266b11e24094b50259587ca7e134d))
* **publish:** configure purist crate and release-please workflow ([932d732](https://github.com/drmartinwimmer/purist/commit/932d732a32ae6ea94f9a4379de6d9d5dcb52418e))
* **publish:** configure purist crate and release-please workflow ([5574a86](https://github.com/drmartinwimmer/purist/commit/5574a86a5e8cd93bae2f6aa46c054aff60d28be2))
* **publish:** configure purist crate and release-please workflow ([b145e7c](https://github.com/drmartinwimmer/purist/commit/b145e7cbd27f16a4599ed6b2cb1d67c5e72f1618))
* **publish:** configure purist crate and release-please workflow ([#17](https://github.com/drmartinwimmer/purist/issues/17)) ([932d732](https://github.com/drmartinwimmer/purist/commit/932d732a32ae6ea94f9a4379de6d9d5dcb52418e))
* **purist:** add googletest_conventions lint rule ([0045346](https://github.com/drmartinwimmer/purist/commit/004534687742a087ff9bcbfe9fc2a0ebc6ef4fa5))
* **purist:** add googletest_conventions lint rule ([69541f1](https://github.com/drmartinwimmer/purist/commit/69541f14a957e7e864fcea2202d3dfbf6bbc2ec6))
* **purist:** add lib_facade_hygiene and no_negative_bool rules, and convert check flags to affirmative ([fc73e66](https://github.com/drmartinwimmer/purist/commit/fc73e6698ff0ff444a72e0bb03a734bb4f2f13b9))
* **purist:** add lib_facade_hygiene rule for concise library root files ([9b35fb7](https://github.com/drmartinwimmer/purist/commit/9b35fb7d6581be1e5528f37d99b4af39a45ebadc))
* **purist:** add max_file_lines rule to enforce file length boundaries ([cbce6cb](https://github.com/drmartinwimmer/purist/commit/cbce6cbc46a8d299267a77565ecfd89e4617958a))
* **purist:** add max_file_lines rule with AST line counting and Cargo.toml configuration ([ce30cd1](https://github.com/drmartinwimmer/purist/commit/ce30cd1e0e1ad101a3ecac4c52bcad471fd0e508))
* **purist:** add max_nesting_depth rule to enforce code readability ([5b1c11e](https://github.com/drmartinwimmer/purist/commit/5b1c11ec0e2db43a5be4762fe96b8b66f9090010))
* **purist:** add max_nesting_depth rule to enforce code readability ([c2e6479](https://github.com/drmartinwimmer/purist/commit/c2e647941e5d30b905050a0a1f9b8f0342133a3e))
* **purist:** add no_double_negation lint rule ([6fcff2b](https://github.com/drmartinwimmer/purist/commit/6fcff2b6aabcf4e40f6c33e56747ceb0cd6918aa))
* **purist:** add no_double_negation lint rule ([b98740b](https://github.com/drmartinwimmer/purist/commit/b98740b0457dabff981b3b85302b77d0b12983b2))
* **purist:** add no_trivial_getters_setters rule to flag redundant accessors (YAGNI) ([e8e427f](https://github.com/drmartinwimmer/purist/commit/e8e427fc76177095c09b9c4f748816d901112cc6))
* **purist:** add no_trivial_getters_setters rule to flag redundant accessors (YAGNI) ([6d510a4](https://github.com/drmartinwimmer/purist/commit/6d510a4964174e4552fc471402c49f8f08a4eaa2))


### Bug Fixes

* **opinionated:** avoid flagging .as_ref() in test_matcher_borrow_simplification ([20931a4](https://github.com/drmartinwimmer/purist/commit/20931a4f2767e5e6c17474ca413a722726e6f89b))
* **opinionated:** avoid flagging .as_ref() in test_matcher_borrow_simplification ([fc03bd0](https://github.com/drmartinwimmer/purist/commit/fc03bd0b0f45cb1142b1ea8dd63b96768bb2c4f0))
* **opinionated:** avoid flagging Path::join calls with relative arguments in path_resolution ([b7f5232](https://github.com/drmartinwimmer/purist/commit/b7f52329824399fe741492a6e255eb4287457453))
* **opinionated:** avoid flagging Path::join calls with relative arguments in path_resolution ([e51c5f4](https://github.com/drmartinwimmer/purist/commit/e51c5f4299c9e35e525e4c2ae1310f58e63bd875))
* **opinionated:** permit println and eprintln in CLI runner methods in no_println_in_libraries ([7d98330](https://github.com/drmartinwimmer/purist/commit/7d9833073229a2070be07469808ca316085212c9))
* **opinionated:** permit println and eprintln in CLI runner methods in no_println_in_libraries ([6df0ffd](https://github.com/drmartinwimmer/purist/commit/6df0ffd9ecf6d7aaec7141cc0213fa6a51f3f94e))
* **opinionated:** permit remove_dir_all in Drop implementations in raii_temp_directories ([c0b7290](https://github.com/drmartinwimmer/purist/commit/c0b729049e5411d9c13ae7619638959c18aa0073))
* **opinionated:** permit remove_dir_all in Drop implementations in raii_temp_directories ([c17084e](https://github.com/drmartinwimmer/purist/commit/c17084e379005c9ca47c94499f0e5edb8cf9c1a1))
* **opinionated:** permit roundtrip conversions in test functions and modules ([b19c50c](https://github.com/drmartinwimmer/purist/commit/b19c50c64385e21dec5b3a24946defac9ea6cb72))
* **opinionated:** permit roundtrip conversions in test functions and test modules ([0a71142](https://github.com/drmartinwimmer/purist/commit/0a71142ba91aa8560a84f51abbab2dd170838566))
* **opinionated:** permit single match when diverging arm uses pattern bindings or guards ([608a188](https://github.com/drmartinwimmer/purist/commit/608a188e5571715fefd844c146969244c34f8ef9))
* **opinionated:** permit single match when diverging arm uses pattern bindings or guards ([d6d9fb2](https://github.com/drmartinwimmer/purist/commit/d6d9fb2268e3c0b169ed47475d52df2afa36d6ca))
* **opinionated:** require 3+ segments and exempt primitive types in use_declarations ([a9e50b9](https://github.com/drmartinwimmer/purist/commit/a9e50b908e2b39ba3957c51266831d54bb3768d4))
* **opinionated:** require 3+ segments and exempt primitive types in use_declarations ([2d388c0](https://github.com/drmartinwimmer/purist/commit/2d388c01124e1fe5b85b0b6ac1b3992ea0e2d317))
* **purist:** adapt rules and AST visitors for syn 3.0 breaking changes ([bbeda1d](https://github.com/drmartinwimmer/purist/commit/bbeda1d0c091c63c1c7661aaec9933bfc1be09c7))
* **release:** synchronize Cargo workspace dependencies and lockfile for release-please ([d9f1009](https://github.com/drmartinwimmer/purist/commit/d9f1009d72a0fc9a31da0da4c2317b5223b763b7))

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
