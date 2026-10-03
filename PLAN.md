# Code Review Toolkit: Skills, Rust CLI Tools & Evaluation Plan

> **For agentic workers:** Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish an end-to-end code review ecosystem consisting of modular agent skills, high-performance Rust-based CLI tools, an automated release and SemVer verification pipeline, and an automated evaluation suite powered by Inspect AI and the `fence` sandbox to systematically detect, flag, and prevent code quality, security, and API issues in agent-generated code.

**Architecture:**
1. **Rust CLI Toolkit (`code-review` crate):** A modular binary offering:
   - `code-review check`: Aggregates and runs `cargo fmt`, `cargo clippy`, custom linters, `cargo audit`, and API drift/coverage gates, reporting normalized diagnostics.
   - `code-review configure-lints`: Modifies `Cargo.toml` using `toml_edit` to configure strict clippy lints while preserving formatting and comments.
   - `code-review opinionated`: AST-based static analysis engine checking code patterns beyond Clippy's scope (inline modules, dummy unit structs, VCS-relative paths, error enums, clippy suppression hygiene).
   - `code-review api`: Introspects and generates a checked-in API manifest (`API.md`) for libraries (public items), binary CLIs (subcommands/flags), and HTTP services (endpoints/methods), verifying intentionality and catching drift.
   - `code-review coverage`: Drives `cargo-llvm-cov` to measure source-based coverage, enforce minimum coverage thresholds, and report uncovered code paths.
2. **Automated SemVer & Release Pipeline:**
   - **Release Please** governs developer intent via Conventional Commits (`feat:`, `fix:`, `feat!:`, `BREAKING CHANGE:`), automatically updating `Cargo.toml` versions, changelogs, and release PRs.
   - **`cargo-semver-checks`** enforces code reality via `rustdoc` JSON analysis:
     - In feature/subagent PRs: validates changes against `origin/main` to ensure breaking API changes are not mislabeled as non-breaking.
     - On Release Please candidate PRs: verifies that the computed version bump in `Cargo.toml` strictly satisfies all accumulated public API changes before publishing to `crates.io`.
3. **Skills Suite (`skills/`):**
   - `skills/distilling-feedback`: Systematically mines past session transcripts and course corrections to distill new guidelines and linter rules.
   - Thematic review skills applied by dedicated subagents (`reviewing-spec-compliance`, `reviewing-rust-modularity`, `reviewing-rust-robustness`, `reviewing-rust-testing`, `reviewing-rust-lint-hygiene`, `reviewing-containment-safety`, `reviewing-api-surface`, and `reviewing-dependencies`).
4. **Inspect AI Evaluation Suite (`evals/`):**
   - Managed with `uv` (`pyproject.toml`, `inspect-ai`).
   - Secure execution using `fence` sandbox to run configured agents (e.g., `agy`) against curated, simplified code examples abstracted from historical agent failures (including API drift, dependency bloat/risks, and coverage holes).
   - Deterministic and model-graded scorers evaluating issue detection accuracy, false positive rates, and recommendation quality.

**Tech Stack:** Rust (2024 edition, `syn`, `quote`, `toml_edit`, `clap`, `cargo-llvm-cov`, `cargo-semver-checks`, `cargo-audit`), Python 3.12+, `uv`, Inspect AI, `fence` sandbox CLI, Jujutsu (`jj`).

---

## Common Practices: API Manifests, SemVer & Dependency Governance

In modern software development and especially in the Rust ecosystem, several established practices govern public API surfaces, automated releases, and supply chain security:

### 1. Library Public APIs & Breaking Change Detection
- **Golden File Pattern (`public-api.txt` / `API.md`):** Tools like [`cargo-public-api`](https://github.com/cargo-public-api/cargo-public-api) leverage `rustdoc-json` to emit a normalized, sorted textual listing of all public items (types, traits, functions, methods, re-exports). Checking this file into version control ensures every PR changing the public API surface produces an explicit diff.
- **Breaking Change Detection (`cargo-semver-checks`):** [`cargo-semver-checks`](https://github.com/obi1kenobi/cargo-semver-checks) inspects the rustdoc JSON output across releases or git revisions to mechanically enforce Semantic Versioning rules, flagging accidental breaking changes (e.g. adding a non-default method to a trait, altering type signatures, or removing public items).

### 2. Automated SemVer Governance: Release Please + `cargo-semver-checks`
- **Intent vs. Reality Verification Loop:**
  - **Release Please** operates on **Developer Intent**: it parses commit messages written according to Conventional Commits (`feat:`, `fix:`, `feat!:`, `BREAKING CHANGE:`) and calculates version bumps in `Cargo.toml`.
  - **`cargo-semver-checks`** operates on **Code Reality**: it analyzes the compiled public API via `rustdoc` JSON to determine what level of SemVer bump the actual code changes legally require.
- **Workflow Synergy:**
  1. *Feature PRs:* `cargo-semver-checks check-release --baseline-rev origin/main` runs in CI. If a PR contains an unannotated breaking change (e.g. labeled `feat:` instead of `feat!:`), CI blocks the PR.
  2. *Release Candidate PRs:* When Release Please opens an automated release PR bumping `Cargo.toml`, `cargo-semver-checks check-release` verifies that the bumped version matches the actual API difference against the last crates.io release or git tag before merging and publishing.
- **Tooling Options:**
  - The official GitHub Action [`obi1kenobi/cargo-semver-checks-action@v2`](https://github.com/obi1kenobi/cargo-semver-checks-action) is maintained by the author of `cargo-semver-checks`, provides automatic toolchain resolution, and caches the crates.io index and rustdoc JSON artifacts.
  - Alternatively, [`taiki-e/install-action@v2`](https://github.com/taiki-e/install-action) with `tool: cargo-semver-checks` installs pre-built binaries instantly, allowing direct execution in workflow steps (consistent with `aiw`'s toolchain setup).

### 3. Dependency Hygiene & Supply Chain Security
- **Security Vulnerability Audits (`cargo-audit`):** Checks `Cargo.lock` against the RustSec Advisory Database for known CVEs, unmaintained crate alerts, and yanked releases.
- **Dependency Minimization (YAGNI):** Agents frequently add heavy third-party crates for trivial tasks (e.g. pulling in an entire async runtime, regex engine, or base64 crate when a 10-line helper or standard library suffices). A dedicated dependency review gate prevents code bloat.
- **Transitive Tree Budgeting (`cargo tree`):** Reviewing the depth and breadth of transitive dependencies to prevent supply chain explosion.
- **Build Script (`build.rs`) & Macro Execution Risk:** Crate dependencies executing procedural macros or arbitrary code during `build.rs` compilation represent high-risk attack vectors in sandboxed environments.
- **License Compliance (`cargo-deny`):** Enforcing allowed license expressions (e.g. MIT, Apache-2.0) and blocking copyleft contamination (GPL/AGPL) in commercial or permissive libraries.

### 4. Binary & CLI Surfaces
- **CLI Schema / Manpage Snapshots:** In tools built with `clap`, common practice involves using `clap::Command` reflection or `clap_mangen` to generate structured JSON manifests (`cli.json`) or markdown documentation (`docs/cli.md`).
- **Snapshot Testing (`trycmd` / `insta`):** CLI suites snapshot help output, command trees, argument parsers, and error messages to ensure flags or subcommands aren't added, renamed, or dropped unintentionally.

### 5. HTTP Server Endpoints
- **OpenAPI / Route Manifests:** Frameworks like Axum or Actix leverage crates like `utoipa` or `aide` to generate `openapi.json` at test or build time. Checking this specification into version control ensures that endpoint modifications (URL paths, HTTP methods, headers, request bodies, response status codes) are reviewed explicitly.

### 6. Deprecation Lifecycle & Minimalism
- **Least-Privilege Visibility:** A core principle in library design is keeping visibility internal (`pub(crate)`) unless external consumers strictly require access.
- **Deprecation Attributes:** When an API is superseded, the idiomatic pattern in Rust is decorating it with `#[deprecated(since = "x.y.z", note = "use `new_api` instead")]`.
- **Planned Sunset:** Established APIs maintain backwards compatibility for at least one minor release cycle (or major release cycle if breaking), pairing deprecation warnings with a documented migration guide before final removal.

### 7. Source-Based Code Coverage
- **`cargo-llvm-cov`:** The established standard for Rust code coverage, utilizing LLVM source-based code coverage instrumentation (`-C instrument-coverage`). It produces accurate line, branch, and region metrics without requiring debug-unfriendly ptrace wrappers, seamlessly outputting LCOV, JSON, and summary tables.

---

## System Overview & Directory Structure

```
.
├── Cargo.toml                          # Workspace / package configuration
├── flake.nix / rust-toolchain.toml     # Nix & Rust development environment
├── AGENTS.md                           # Repository rules and agent guidelines
├── PLAN.md                             # This implementation plan
├── SPEC.md                             # Global project specification
├── API.md                              # Checked-in API surface manifest
├── README.md                           # Project documentation and badges
├── LICENSE                             # MIT License
├── release-please-config.json          # Release Please configuration
├── .release-please-manifest.json       # Release Please version tracking manifest
├── .github/
│   ├── dependabot.yml                  # Weekly dependency maintenance
│   └── workflows/
│       ├── ci.yml                      # GitHub Actions CI (lint, test, nix, coverage, semver-checks)
│       └── release.yml                 # Release Please & Crates.io publishing (when ready)
├── src/                                # Rust CLI toolkit source code
│   ├── main.rs                         # Entry point and CLI subcommand dispatcher
│   ├── cli.rs                          # Clap CLI definition
│   ├── common/                         # Shared utilities, diagnostics, and reporting
│   │   ├── mod.rs
│   │   ├── diagnostics.rs              # Unified diagnostic data structures (file, line, span, severity)
│   │   └── reporter.rs                 # Formatted console and JSON/Markdown outputs
│   ├── tools/                          # Rust CLI Tool implementations
│   │   ├── mod.rs
│   │   ├── runner.rs                   # Tool 1: Formatter & linter runner aggregator (`code-review check`)
│   │   ├── cargo_toml.rs               # Tool 2: Cargo.toml linter configurator (`code-review configure-lints`)
│   │   ├── opinionated/                # Tool 3: Opinionated static analysis linter (`code-review opinionated`)
│   │   │   ├── mod.rs
│   │   │   ├── engine.rs               # AST visitor and analysis driver
│   │   │   └── rules/                  # Specific opinionated lint checks
│   │   │       ├── mod.rs
│   │   │       ├── no_inline_mods.rs   # Enforce submodules in separate files
│   │   │       ├── free_functions.rs   # Free functions over dummy unit structs
│   │   │       ├── path_resolution.rs  # VCS/manifest-relative paths vs CWD
│   │   │       ├── error_types.rs      # Structured thiserror/anyhow vs raw String
│   │   │       ├── clippy_suppress.rs  # Enforce justification on #[expect]/#[allow]
│   │   │       └── test_patterns.rs    # Test naming, assertions (expect_that!), no unwrap
│   │   ├── api/                        # Tool 4: API Manifest Generator & Auditor (`code-review api`)
│   │   │   ├── mod.rs
│   │   │   ├── engine.rs               # API inspection coordinator
│   │   │   ├── library.rs              # Library public item extractor (rustdoc JSON / syn)
│   │   │   ├── cli_extractor.rs        # Binary CLI command & flag extractor (clap introspection)
│   │   │   ├── http_extractor.rs       # HTTP router & endpoint extractor
│   │   │   ├── manifest.rs             # API.md generation, serialization, and formatting
│   │   │   └── diff.rs                 # Manifest drift comparison & breaking change detection
│   │   └── coverage.rs                 # Tool 5: LLVM Source-Based Coverage Engine (`code-review coverage`)
├── skills/                             # Agent Skills Directory
│   ├── distilling-feedback/            # Skill: Review past sessions and distill guidelines
│   │   ├── SKILL.md
│   │   └── SPEC.md
│   ├── reviewing-spec-compliance/      # Skill: Verify requirements and milestone scope
│   │   ├── SKILL.md
│   │   └── SPEC.md
│   ├── reviewing-rust-modularity/      # Skill: Check file separation and single responsibility
│   │   ├── SKILL.md
│   │   └── SPEC.md
│   ├── reviewing-rust-robustness/      # Skill: Check errors, panics, unwrap, and fallibility
│   │   ├── SKILL.md
│   │   └── SPEC.md
│   ├── reviewing-rust-testing/         # Skill: Check test quality, gtest, assertions, coverage
│   │   ├── SKILL.md
│   │   └── SPEC.md
│   ├── reviewing-rust-lint-hygiene/    # Skill: Check clippy cleanliness and suppression rules
│   │   ├── SKILL.md
│   │   └── SPEC.md
│   ├── reviewing-containment-safety/   # Skill: Check sandbox bounds, paths, and timeouts
│   │   ├── SKILL.md
│   │   └── SPEC.md
│   ├── reviewing-api-surface/          # Skill: Review API manifest, minimalism, and deprecations
│   │   ├── SKILL.md
│   │   └── SPEC.md
│   └── reviewing-dependencies/         # Skill: Review proposed dependencies, bloat, and security
│       ├── SKILL.md
│       └── SPEC.md
└── evals/                              # Inspect AI Evaluation Harness
    ├── pyproject.toml                  # Python environment managed via uv
    ├── uv.lock
    ├── README.md                       # Eval harness instructions
    ├── sandbox/                        # Sandbox runner & fence integration
    │   ├── __init__.py
    │   ├── fence_driver.py             # Fence container runner (network/fs isolation)
    │   └── agent_runner.py             # Driver executing `agy` non-interactively
    ├── datasets/                       # Curated, abstracted test cases from past feedback
    │   ├── inline_mods/                # Minimal cases for inline module smells
    │   ├── unwrap_panics/              # Minimal cases for unwrap / panic risks
    │   ├── unjustified_suppression/    # Minimal cases for clippy suppression abuse
    │   ├── cwd_path_resolution/        # Minimal cases for CWD vs root path resolution
    │   ├── leaky_tests/                # Minimal cases for unhandled test resource leaks
    │   ├── api_drift/                  # Minimal cases for unreviewed / leaking public APIs
    │   ├── missing_deprecations/       # Minimal cases for superseded APIs without #[deprecated]
    │   ├── dependency_bloat/           # Minimal cases for unnecessary / speculative dependencies
    │   ├── obscure_deps/               # Minimal cases for obscure crates where established alternatives exist
    │   ├── vulnerable_deps/            # Minimal cases for dependencies with known advisories
    │   ├── coverage_gaps/              # Minimal cases for uncovered error paths
    │   └── clean_baseline/             # Clean idiomatic crates (false-positive checks)
    ├── tasks/                          # Inspect AI task definitions
    │   ├── __init__.py
    │   ├── review_eval.py              # Main Inspect AI task suite
    │   ├── solvers.py                  # Custom solvers running agy with review skills in fence
    │   └── scorers.py                  # Evaluation scorers (detection rate, precision, recall)
    └── run_evals.py                    # Convenient CLI runner script
```

---

## Detailed Component Specifications

### 1. Tool 1: Linter & Formatter Runner Aggregator (`code-review check`)
- **Purpose:** Automatically detect project structure (standalone crate or multi-crate Cargo workspace), execute all relevant linters and formatters, and present an aggregated, unified diagnostic report.
- **Checks Executed:**
  - `cargo fmt --check`: Formatting compliance.
  - `cargo clippy --all-targets --all-features -- -D warnings`: Compiler and Clippy lint status.
  - `code-review opinionated`: Custom opinionated AST checks.
  - `code-review api check`: API manifest drift verification against checked-in `API.md`.
  - `cargo audit`: Security vulnerability scans of `Cargo.lock`.
  - `code-review coverage`: Optional coverage threshold verification (`--coverage`).
- **Features:**
  - Normalizes outputs from all sub-tools into a single diagnostic stream (`file`, `line`, `col`, `rule`, `severity`, `message`, `suggested_fix`).
  - Output formats: Colored terminal report for human interactive use, `--json` for machine tools, and `--format markdown` for subagent reviewers.
  - Filtering: `--fail-on [warnings|errors]`, `--path <dir>`, `--changed-only` (inspecting Jujutsu modified files via `jj diff --summary`).

### 2. Tool 2: Cargo.toml Linter Configurator (`code-review configure-lints`)
- **Purpose:** Programmatically update `Cargo.toml` to inject or update strict, production-grade linter configurations without breaking comments, existing table formatting, or custom configurations.
- **Implementation:** Built using `toml_edit` to ensure precise preservation of formatting, whitespace, and inline comments.
- **Configured Lint Categories:**
  - **Don't Panic:** `unwrap_used`, `indexing_slicing`, `string_slice`, `panic`, `todo`, `unimplemented`, `get_unwrap`, `unwrap_in_result`, `panic_in_result_fn`.
  - **Don't Fail Silently:** `let_underscore_future`, `let_underscore_must_use`, `unused_result_ok`, `map_err_ignore`, `assertions_on_result_states`.
  - **Don't Do Unsafe Things with Memory:** `mem_forget`, `undocumented_unsafe_blocks`, `multiple_unsafe_ops_per_block`.
  - **Don't Do Potentially Incorrect Things with Numbers:** `float_cmp`, `float_cmp_const`, `lossy_float_literal`, `cast_sign_loss`.
  - **Don't Do Bad Things That Are Easy to Avoid:** `rc_mutex`, `debug_assert_with_mut_call`, `dbg_macro`, `infallible_try_from`.
  - **Don't `allow` Your Way Around Lints:** `allow_attributes = "warn"`, `allow_attributes_without_reason = "warn"`.
- **Target Placement:** Automatically detects workspace root vs single crate and updates `[workspace.lints.clippy]` or `[lints.clippy]`. Supports preset profiles (`--profile strict`, `--profile standard`).

### 3. Tool 3: Opinionated Static Analysis Linter (`code-review opinionated`)
- **Purpose:** Check for stylistic, architectural, and behavioral anti-patterns that standard Clippy intentionally avoids checking or cannot inspect at the AST level.
- **AST Parsing Engine:** Implemented with `syn` and `quote` to inspect the syntax tree of Rust source files.
- **Opinionated Rules:**
  1. `no_inline_mods`: Forbids inline `mod foo { ... }` blocks with inline declarations in `main.rs` and `lib.rs` (excluding small `#[cfg(test)] mod tests`). Enforces modular submodules in separate files (`foo.rs` or `foo/mod.rs`).
  2. `free_functions`: Flags stateless dummy structs used solely as namespaces (e.g., `pub struct Parser; impl Parser { pub fn parse(...) }`) and advises idiomatic free functions in the module namespace.
  3. `path_resolution`: Flags relative path operations (`Path::new("relative/path")` or `std::fs::read("config.json")`) without resolving against the workspace root or `CARGO_MANIFEST_DIR`.
  4. `error_types`: Flags `Result<T, String>` or `Result<T, &str>` in non-test functions. Recommends structured error enums via `thiserror` or `anyhow::Result`.
  5. `clippy_suppression_hygiene`: Flags any `#[expect(...)]` or `#[allow(...)]` that either lacks `reason = "..."` or does not have an accompanying code comment on the preceding line explaining why the lint cannot be fixed.
  6. `test_patterns`: Validates test function conventions:
     - Naming: `<verb>_<description>_<outcome>`.
     - Flags `assert_eq!` in test files when `expect_that!` should be used.
     - Flags `.unwrap()` on `Option`/`Result` in test bodies where `?` or `.or_fail()?` is required.
  7. `no_redundant_conversions`: Flags redundant double-serialization patterns (e.g. `serde_json::to_string` followed immediately by `serde_json::from_str` within the same scope).

### 4. Tool 4: API Manifest Generator & Auditor (`code-review api`)
- **Purpose:** Provide a unified mechanism to generate and verify a checked-in API surface manifest (`API.md` or `.api/api-manifest.json`), ensuring all API changes are intentional, reviewed, and properly versioned.
- **Subcommands:**
  - `code-review api dump` (or `generate`): Auto-detects the project targets, extracts the public API surface, and generates or updates `API.md`.
  - `code-review api check`: Compares the current code against the checked-in `API.md`. Fails with detailed diffs if uncommitted additions, removals, or modifications are detected.
- **Target Surface Extractors:**
  1. **Library Targets (`library.rs`):**
     - Extracts all public items (`pub struct`, `pub enum`, `pub fn`, `pub trait`, `pub type`, `pub const`, and public re-exports).
     - Identifies item visibility, function signatures (parameters, return types), and implemented public traits.
     - Leverages `syn` AST traversal or rustdoc JSON output.
  2. **Binary / CLI Targets (`cli_extractor.rs`):**
     - Introspects `clap::Command` hierarchies to extract all subcommands, positional arguments, short/long flags, options, defaults, environment variable bindings, and doc summaries.
  3. **HTTP Web Service Targets (`http_extractor.rs`):**
     - Detects framework route registrations (e.g., Axum router or OpenAPI attributes via `utoipa`/`aide`).
     - Extracts endpoint path templates, HTTP methods (`GET`, `POST`, etc.), query parameters, request payloads, and response status codes.
- **Drift & Breaking Change Engine (`diff.rs`):**
  - Identifies:
    - **Additions**: Newly introduced public items, commands, or endpoints.
    - **Removals**: Removed or unexported items (breaking changes).
    - **Modifications**: Changed signatures, new required arguments without defaults, or modified endpoint paths.
  - Generates clear, human-readable markdown diffs suitable for PR reviews.

### 5. Tool 5: LLVM Source-Based Coverage Engine (`code-review coverage`)
- **Purpose:** Automate source-based code coverage collection, reporting, and threshold enforcement using `cargo-llvm-cov`.
- **Features:**
  - Non-interactive execution of `cargo llvm-cov` across `--all-targets` and `--all-features`.
  - Computes line, branch, and region coverage percentages.
  - Threshold enforcement: `--fail-under <pct>` (fails with non-zero exit code if coverage drops below target threshold).
  - Flags uncovered lines, untested functions, and ignored error-handling paths.
  - Outputs formats: Console summary table, LCOV (`--lcov`), and structured JSON (`--json`).

### 6. Automated SemVer & Release Pipeline: Release Please + `cargo-semver-checks`
- **Purpose:** Seamless, tamper-proof versioning and publishing on GitHub and Crates.io.
- **Components:**
  1. **`release-please-config.json` & `.release-please-manifest.json`:**
     - Configures `"release-type": "rust"` with automatic version synchronization for `Cargo.toml` and `flake.nix`.
     - Sets `"bump-minor-pre-major": true` for `0.x` SemVer semantics.
  2. **Feature PR Verification Workflow (`.github/workflows/ci.yml`):**
     - Steps install `cargo-semver-checks` via `taiki-e/install-action@v2` (or `obi1kenobi/cargo-semver-checks-action@v2`).
     - Runs `cargo semver-checks check-release --baseline-rev origin/main` to ensure any breaking changes are labeled with `feat!:` or `BREAKING CHANGE:`.
  3. **Release Please Candidate PR Verification:**
     - Runs `cargo semver-checks check-release` against the latest published crates.io version (or prior release tag).
     - Asserts that the version bump calculated by Release Please is sufficient for all accumulated code changes.
  4. **Release Workflow (`.github/workflows/release.yml`):**
     - Uses `googleapis/release-please-action@v5`.
     - When a release PR merges to `main`, tags the release and runs `cargo publish --token ...`.

### 7. Skill Suite: Thematic Subagent Review Skills

#### Skill 1: Feedback Reflection & Guideline Distillation (`skills/distilling-feedback`)
- **Purpose:** Provide agents with a repeatable methodology to analyze past agent session transcripts, identify user corrections and recurring failure patterns, and distill them into actionable review rules.
- **Workflow:**
  1. **Scan Transcripts:** Parse `transcript.jsonl` files for user intervention events, course corrections ("stop", "don't do that", "revert"), tool command exit errors, and manual user commits.
  2. **Categorize Root Causes:** Classify issues into themes (Modularity, Error Handling, Clippy Laziness, Timeout Loops, Sandbox Violations, API Bloat, Dependency Risk).
  3. **Distill Guidelines:** Format findings into new checklist items, rationalization tables, and before/after code snippets.
  4. **Propose Linter Rules:** Identify which guidelines are mechanically enforceable and draft specifications for new rules in `code-review opinionated` or `code-review api`.
  5. **Generate Eval Cases:** Abstract the incident into a minimal, reproducible test case for the Inspect AI eval suite.

#### Skill 2: API Surface & Deprecation Review (`skills/reviewing-api-surface`)
- **Purpose:** Explicitly focuses on reviewing public API changes in `API.md` (and underlying code), enforcing minimalism, and managing the deprecation lifecycle of superseded APIs.
- **Review Checklist:**
  1. **API Minimalism (Least Privilege):** Are any internal helpers accidentally exposed as `pub` instead of `pub(crate)`? Are new CLI options strictly required?
  2. **Deprecation of Superseded APIs:** Has an older function or flag been replaced? Is it decorated with `#[deprecated(since = "x.y.z", note = "...")]` with clear migration guidance?
  3. **Intentionality Audit:** Do changes in the checked-in `API.md` match feature requirements?
  4. **Ergonomics & Naming:** Are names idiomatic and consistent with RFC 430?

#### Skill 3: Dependency Hygiene & Security Review (`skills/reviewing-dependencies`)
- **Purpose:** Review any proposed changes to dependencies in `Cargo.toml`, ensuring the codebase stays lightweight, YAGNI-compliant, and secure from supply chain vulnerabilities.
- **Review Checklist:**
  1. **Necessity & YAGNI:**
     - Is this dependency strictly necessary, or can the functionality be implemented cleanly in 10-30 lines of standard library Rust?
     - Does the dependency duplicate functionality already provided by another crate in the workspace?
  2. **Popularity & Established Alternatives:**
     - How mature and widely adopted is the crate? Inspect ecosystem standing (crates.io download counts, reverse dependencies via `cargo info <crate>`, GitHub stars, and community adoption).
     - Is there an established, de facto standard alternative in the Rust ecosystem? (e.g., preferring `clap` over ad-hoc arg parsers, `serde` over niche JSON serializers, `thiserror`/`anyhow` over bespoke error crates, `tracing` over non-standard loggers).
     - Is the crate actively maintained? Check the date of the latest release, recent repository commit activity, and whether the project has a single bus factor or signs of abandonment.
     - Strictly flag obscure, one-off hobby crates when standard, audited ecosystem solutions exist.
  3. **Security & Advisories:**
     - Does `cargo audit` report any known CVEs or unmaintained alerts on the crate?
     - Does the crate name resemble a known package with slight typos (typosquatting detection)?
  4. **Transitive Dependency Weight:**
     - What is the transitive dependency footprint? Run `cargo tree -i <crate>` to verify it does not pull in an excessive number of sub-dependencies.
  5. **Macro & Build Script Execution Risks:**
     - Does the crate use arbitrary `build.rs` scripts or complex procedural macros that run untrusted code at build time?
     - Prefer pure-Rust, `#![forbid(unsafe_code)]` implementations when available.
  6. **License Compatibility:**
     - Is the crate licensed under an MIT/Apache-2.0 compatible permissive license? Strictly flag copyleft (GPL/AGPL) licenses that would contaminate the project.

#### Remaining Thematic Review Skills:
- **`reviewing-spec-compliance`**: Verifies that implementation strictly satisfies requirements and invariants in `SPEC.md` and module specs without out-of-scope feature creep.
- **`reviewing-rust-modularity`**: Verifies single responsibility, separate submodule files, free functions over dummy structs, and lightweight dependencies.
- **`reviewing-rust-robustness`**: Enforces strict error handling, absence of unwraps/panics in production code, proper error enums, and no ignored results.
- **`reviewing-rust-testing`**: Verifies `#[gtest]`, `expect_that!` assertions, test naming conventions, `.or_fail()?`, deterministic resource teardown, and **sufficient test coverage** (verifying tests cover public APIs and error paths).
- **`reviewing-rust-lint-hygiene`**: Ensures clean compilation under strict clippy, zero warnings, and absence of unjustified `#[expect]` or `#[allow]`.
- **`reviewing-containment-safety`**: Enforces VCS root-relative path resolution, avoids runaway commands, and ensures compliance with sandbox boundaries.

**Subagent Orchestration Pattern:**
When an agent reviews code, it dispatches specialized review subagents in parallel with dedicated review prompts, then aggregates their structured feedback into a consolidated report.

### 8. Evaluation Suite: Inspect AI + Fence Sandbox (`evals/`)
- **Environment:** Isolated Python virtual environment managed via `uv` (`uv run inspect eval ...`).
- **Sandbox Architecture (`fence`):**
  - Uses the `fence` CLI sandbox (`fence -t code -- ...`) to contain the agent under test.
  - Network access is denied/restricted to prevent external side effects.
  - The host filesystem is read-only; each eval run operates in an ephemeral target directory containing the test crate.
- **Agent Under Test:** Configured agent binary (e.g. `agy --print "<prompt>" --mode accept-edits --dangerously-skip-permissions`).
- **Evaluation Dataset:** Abstracted, minimal reproduction cases created from past feedback:
  - Positive examples (bad patterns): Deliberate bugs (unwrapped panics, inline submodules, unjustified clippy suppression, CWD path dependencies, leaky test resources, unreviewed API leaks, superseded APIs without deprecation, speculative/vulnerable/obscure dependencies, coverage gaps).
  - Negative examples (clean patterns): Fully compliant Rust crates to measure false positive rates.
- **Inspect AI Tasks & Metrics:**
  - `@task`: Loads datasets and wires the sandbox solver and evaluation scorer.
  - `@solver`: Executes the agent inside `fence` with the review skills, directing it to review the target directory and emit a review report.
  - `@scorer`: Evaluates the agent's review output against ground-truth defect annotations:
    - **Detection Rate (Recall):** Did the agent identify the deliberate flaw?
    - **Precision:** Did the agent avoid false accusations on clean code?
    - **Actionability:** Did the agent suggest the idiomatic fix?
    - **Tool Synergy:** Did the agent invoke `code-review check`, `code-review opinionated`, or `code-review api check` during its review?

---

## Implementation Milestones & Roadmap

### Milestone 1: Core CLI Architecture & Cargo.toml Lint Configurator Tool
- **Description:** Initialize the Rust CLI crate structure with `clap`, create unified diagnostic data structures, and implement `code-review configure-lints` using `toml_edit` to inject and update strict Clippy lint configurations in `Cargo.toml`.
- **Status:** `[x] Completed`
- **Target Completion Date:** 2026-10-05
- **Actual Completion Date:** 2026-10-02
- **Dependencies:** None
- **Tasks File:** `plan/M1.md`
- **Feedback File:** `plan/FEEDBACK_M1.md`

### Milestone 2: Opinionated Static Analysis Linter Engine & Rules
- **Description:** Implement the `code-review opinionated` tool with `syn` AST traversal, implementing rules for inline modules, dummy unit structs, VCS path resolution, raw string errors, and clippy suppression hygiene.
- **Status:** `[ ] Pending`
- **Target Completion Date:** 2026-10-09
- **Actual Completion Date:** -
- **Dependencies:** Milestone 1
- **Tasks File:** `plan/M2.md`
- **Feedback File:** `plan/FEEDBACK_M2.md`

### Milestone 3: Linter & Formatter Runner Aggregator (`code-review check`)
- **Description:** Implement `code-review check` to run `cargo fmt --check`, `cargo clippy`, `code-review opinionated`, and `cargo audit`, aggregating diagnostic outputs into console, JSON, and Markdown formats. Add Jujutsu changed-file filtering (`--changed-only`).
- **Status:** `[ ] Pending`
- **Target Completion Date:** 2026-10-12
- **Actual Completion Date:** -
- **Dependencies:** Milestone 2
- **Tasks File:** `plan/M3.md`
- **Feedback File:** `plan/FEEDBACK_M3.md`

### Milestone 4: API Manifest Engine & Auditor (`code-review api`)
- **Description:** Implement `code-review api dump` and `code-review api check` to inspect and dump public API surfaces for libraries (public items), binary CLIs (subcommands/flags), and HTTP services (endpoints). Integrate manifest drift checks into `code-review check`.
- **Status:** `[ ] Pending`
- **Target Completion Date:** 2026-10-15
- **Actual Completion Date:** -
- **Dependencies:** Milestone 3
- **Tasks File:** `plan/M4.md`
- **Feedback File:** `plan/FEEDBACK_M4.md`

### Milestone 5: Code Coverage Engine (`code-review coverage`) & CI Integration
- **Description:** Implement `code-review coverage` wrapping `cargo-llvm-cov` to measure source-based coverage, enforce thresholds, report uncovered error paths, and add a dedicated coverage check to the CI workflow.
- **Status:** `[ ] Pending`
- **Target Completion Date:** 2026-10-18
- **Actual Completion Date:** -
- **Dependencies:** Milestone 4
- **Tasks File:** `plan/M5.md`
- **Feedback File:** `plan/FEEDBACK_M5.md`

### Milestone 6: Release Pipeline & Automated SemVer Verification
- **Description:** Configure Release Please (`release-please-config.json`, `.release-please-manifest.json`, `.github/workflows/release.yml`) and integrate `cargo-semver-checks` into the CI pipeline (evaluating feature PRs against `origin/main` and release candidate PRs against crates.io).
- **Status:** `[ ] Pending`
- **Target Completion Date:** 2026-10-21
- **Actual Completion Date:** -
- **Dependencies:** Milestone 5
- **Tasks File:** `plan/M6.md`
- **Feedback File:** `plan/FEEDBACK_M6.md`

### Milestone 7: Feedback Distillation & Reflection Skill
- **Description:** Create `skills/distilling-feedback/SKILL.md` and `SPEC.md` defining the workflow for analyzing past session transcripts (`transcript.jsonl`), categorizing failures, and distilling new review guidelines and test cases.
- **Status:** `[ ] Pending`
- **Target Completion Date:** 2026-10-24
- **Actual Completion Date:** -
- **Dependencies:** Milestone 6
- **Tasks File:** `plan/M7.md`
- **Feedback File:** `plan/FEEDBACK_M7.md`

### Milestone 8: Thematic Subagent Review Skills Suite (including API & Dependencies)
- **Description:** Create thematic review skills under `skills/` (`reviewing-spec-compliance`, `reviewing-rust-modularity`, `reviewing-rust-robustness`, `reviewing-rust-testing`, `reviewing-rust-lint-hygiene`, `reviewing-containment-safety`, `reviewing-api-surface`, and `reviewing-dependencies`) with frontmatter, checklists, rationalization tables, and subagent prompts.
- **Status:** `[ ] Pending`
- **Target Completion Date:** 2026-10-27
- **Actual Completion Date:** -
- **Dependencies:** Milestone 7
- **Tasks File:** `plan/M8.md`
- **Feedback File:** `plan/FEEDBACK_M8.md`

### Milestone 9: Inspect AI Eval Harness with Fence Sandbox
- **Description:** Initialize Python environment via `uv`, configure `pyproject.toml` with `inspect-ai`, build the `fence` sandbox runner, and create Inspect AI tasks, solvers, and scorers to evaluate agents reviewing code examples.
- **Status:** `[ ] Pending`
- **Target Completion Date:** 2026-10-30
- **Actual Completion Date:** -
- **Dependencies:** Milestone 8
- **Tasks File:** `plan/M9.md`
- **Feedback File:** `plan/FEEDBACK_M9.md`

### Milestone 10: Curated Datasets (API Drift, Dependency Bloat) & Verification
- **Description:** Extract real historical feedback examples into `evals/datasets/` (including API leaks, superseded APIs without deprecation, unneeded dependencies, and coverage gaps), run baseline Inspect AI benchmarks on configured agents (e.g., `agy`), verify detection accuracy and tool integration, and finalize documentation.
- **Status:** `[ ] Pending`
- **Target Completion Date:** 2026-11-02
- **Actual Completion Date:** -
- **Dependencies:** Milestone 9
- **Tasks File:** `plan/M10.md`
- **Feedback File:** `plan/FEEDBACK_M10.md`

---

## Detailed Task Breakdown

### Milestone 1: Core CLI Architecture & Cargo.toml Lint Configurator Tool
- [x] **M1-T0: Update Specifications (`SPEC.md`, `src/tools/cargo_toml.spec.md`)**
  - Define invariants for `code-review` CLI subcommands and `Cargo.toml` modification safety (no comment stripping, preserving existing tables, idempotency).
  - Describe Jujutsu change: `jj describe -m "plan-M1-T0: docs: add specs for core CLI and cargo-toml configurator"`
- [x] **M1-T1: CLI Dispatcher & Diagnostic Core Types**
  - Add `clap` and `serde` dependencies to `Cargo.toml`.
  - Create `src/cli.rs` defining commands: `check`, `configure-lints`, `opinionated`, `api`, `coverage`.
  - Create `src/common/diagnostics.rs` defining `Diagnostic`, `Severity`, `Span`, and `DiagnosticReport`.
  - Create `src/common/reporter.rs` supporting console output and structured JSON.
  - Describe Jujutsu change: `jj describe -m "plan-M1-T1: feat: add clap CLI dispatcher and unified diagnostic types"`
- [x] **M1-T2: Cargo.toml Lint Injection Engine (`src/tools/cargo_toml.rs`)**
  - Add `toml_edit` dependency to `Cargo.toml`.
  - Write unit tests in `src/tools/cargo_toml.rs` verifying that running `configure_lints` on a minimal `Cargo.toml` preserves comments, inserts `[workspace.lints.clippy]` or `[lints.clippy]`, and sets `warn` on all required lints.
  - Implement `configure_lints` and `remove_lints` functions.
  - Wire `code-review configure-lints` subcommand in `src/main.rs`.
  - Verify with `cargo test`.
  - Describe Jujutsu change: `jj describe -m "plan-M1-T2: feat: implement Cargo.toml lint configurator using toml_edit"`
- [x] **M1-T3: Milestone Spec Remediation & Completion**
  - Create module specifications `src/cli.spec.md` and `src/common/SPEC.md`.
  - Tighten visibility of internal helper methods on `LintProfile` to `pub(crate)`.
  - Update `plan/M1.md` and `PLAN.md` roadmap status to completed.
  - Describe Jujutsu change: `jj describe -m "plan-M1-T3: docs: add cli and common specs, tighten visibility, and complete milestone 1"`

### Milestone 2: Opinionated Static Analysis Linter Engine & Rules
- [x] **M2-T0: Update Specifications (`crates/opinionated/SPEC.md`)**
  - Document the contract, AST patterns, and false-positive criteria for each custom lint rule.
  - Describe Jujutsu change: `jj describe -m "plan-M2-T0: docs: add spec for opinionated linter rules"`
- [x] **M2-T1: AST Visitor Framework (`crates/opinionated/src/engine.rs`)**
  - Add `syn` and `quote` dependencies to `Cargo.toml`.
  - Implement visitor engine traversing Rust files, handling syntax errors gracefully, and delegating to rule checkers.
  - Unit tests for AST traversal.
  - Describe Jujutsu change: `jj describe -m "plan-M2-T1: feat: implement opinionated AST visitor engine"`
- [ ] **M2-T2: Rule Implementations (`src/tools/opinionated/rules/`)**
  - Implement `no_inline_mods.rs`: Detect non-test inline modules in `main.rs`/`lib.rs`.
  - Implement `free_functions.rs`: Detect unit structs with pure associated methods.
  - Implement `path_resolution.rs`: Detect non-manifest relative paths.
  - Implement `error_types.rs`: Detect raw `Result<T, String>` signatures.
  - Implement `clippy_suppress.rs`: Detect `#[expect]` or `#[allow]` lacking `reason` or comments.
  - Implement `test_patterns.rs`: Detect test naming and assertion violations.
  - Unit test each rule with positive and negative snippets.
  - Describe Jujutsu change: `jj describe -m "plan-M2-T2: feat: implement opinionated static analysis rules"`
- [ ] **M2-T3: Opinionated Linter CLI Integration**
  - Connect engine to `code-review opinionated` CLI command.
  - Support `--path`, `--json`, and `--fix` stubs.
  - Describe Jujutsu change: `jj describe -m "plan-M2-T3: feat: connect opinionated linter to code-review CLI"`

### Milestone 3: Linter & Formatter Runner Aggregator (`code-review check`)
- [ ] **M3-T0: Update Specifications (`src/tools/runner.spec.md`)**
  - Document runner behavior, exit code aggregation, and multi-format reporting.
  - Describe Jujutsu change: `jj describe -m "plan-M3-T0: docs: add spec for linter runner aggregator"`
- [ ] **M3-T1: Subprocess Runners & Diagnostic Parsers (`src/tools/runner.rs`)**
  - Implement runners for `cargo fmt --check` and `cargo clippy --message-format=json`.
  - Implement JSON output parser converting rustc/clippy JSON compiler messages into `Diagnostic`.
  - Implement runner for `code-review opinionated`.
  - Implement runner for `cargo audit --json`.
  - Aggregate all diagnostics into `DiagnosticReport`.
  - Describe Jujutsu change: `jj describe -m "plan-M3-T1: feat: implement subprocess runner and compiler json parser"`
- [ ] **M3-T2: Jujutsu Integration (`--changed-only`)**
  - Implement VCS query using `jj --no-pager diff --summary` to extract modified files.
  - Filter diagnostics to only report issues on modified files.
  - Describe Jujutsu change: `jj describe -m "plan-M3-T2: feat: add jj changed-file filtering to code-review check"`

### Milestone 4: API Manifest Engine & Auditor (`code-review api`)
- [ ] **M4-T0: Update Specifications (`src/tools/api/SPEC.md`)**
  - Define invariants for API surface extraction, manifest formatting in `API.md`, drift comparison rules, and breaking change classification.
  - Describe Jujutsu change: `jj describe -m "plan-M4-T0: docs: add spec for API manifest engine"`
- [ ] **M4-T1: Surface Extractors for Library, CLI, and HTTP (`src/tools/api/`)**
  - Implement `library.rs`: Extract public structs, enums, functions, traits, and types.
  - Implement `cli_extractor.rs`: Introspect `clap::Command` to extract subcommands, arguments, flags, and help text.
  - Implement `http_extractor.rs`: Extract route paths, HTTP methods, and payload models.
  - Unit test extractors on sample crates.
  - Describe Jujutsu change: `jj describe -m "plan-M4-T1: feat: implement library, cli, and http API extractors"`
- [ ] **M4-T2: Manifest Formatter and Drift Differ (`src/tools/api/manifest.rs`, `diff.rs`)**
  - Implement `manifest.rs`: Serialize extracted API surface into clean markdown `API.md` (or JSON).
  - Implement `diff.rs`: Compare active surface against checked-in `API.md`, categorizing additions, removals, and modifications.
  - Implement CLI subcommands: `code-review api dump` and `code-review api check`.
  - Wire drift check into `code-review check`.
  - Describe Jujutsu change: `jj describe -m "plan-M4-T2: feat: implement API.md manifest formatting, drift differ, and CLI commands"`

### Milestone 5: Code Coverage Engine (`code-review coverage`) & CI Integration
- [ ] **M5-T0: Update Specifications (`src/tools/coverage.spec.md`)**
  - Define coverage metrics, reporting formats, and threshold enforcement rules.
  - Describe Jujutsu change: `jj describe -m "plan-M5-T0: docs: add spec for code coverage tool"`
- [ ] **M5-T1: LLVM Coverage Subprocess Driver (`src/tools/coverage.rs`)**
  - Implement runner invoking `cargo llvm-cov --all-targets --all-features --json`.
  - Parse coverage JSON to calculate line, function, and branch coverage.
  - Implement `--fail-under <pct>` threshold validation.
  - Wire `code-review coverage` CLI command.
  - Describe Jujutsu change: `jj describe -m "plan-M5-T1: feat: implement cargo-llvm-cov runner and threshold checker"`
- [ ] **M5-T2: CI Workflow Coverage Job**
  - Update `.github/workflows/ci.yml` to install `cargo-llvm-cov` and execute coverage checks in CI.
  - Describe Jujutsu change: `jj describe -m "plan-M5-T2: ci: add cargo-llvm-cov coverage job to CI workflow"`

### Milestone 6: Release Pipeline & Automated SemVer Verification
- [ ] **M6-T0: Release Pipeline Specification (`docs/release_pipeline.spec.md`)**
  - Document Release Please configuration, conventional commit contracts, and `cargo-semver-checks` gating invariants.
  - Describe Jujutsu change: `jj describe -m "plan-M6-T0: docs: add spec for release pipeline and semver verification"`
- [ ] **M6-T1: Release Please Manifests & Action Configuration**
  - Add `release-please-config.json` configuring Rust release type and extra files (`flake.nix`).
  - Add `.release-please-manifest.json` initializing root version `0.1.0`.
  - Create `.github/workflows/release.yml` with `googleapis/release-please-action@v5` and conditional `cargo publish`.
  - Describe Jujutsu change: `jj describe -m "plan-M6-T1: ci: configure Release Please manifests and release workflow"`
- [ ] **M6-T2: SemVer CI Verification with cargo-semver-checks**
  - Update `.github/workflows/ci.yml` to install `cargo-semver-checks`.
  - Add step verifying PR commits against `origin/main`.
  - Add step verifying release candidate PRs against baseline releases.
  - Describe Jujutsu change: `jj describe -m "plan-M6-T2: ci: add cargo-semver-checks verification steps to CI workflow"`

### Milestone 7: Feedback Distillation & Reflection Skill
- [ ] **M7-T0: Skill Specification (`skills/distilling-feedback/SPEC.md`)**
  - Specify the distillation process contracts, transcript parsing schemas, and output artifact requirements.
  - Describe Jujutsu change: `jj describe -m "plan-M7-T0: docs: add spec for distilling-feedback skill"`
- [ ] **M7-T1: Skill Playbook (`skills/distilling-feedback/SKILL.md`)**
  - Write concise, token-efficient playbook following `dev:writing-skills`.
  - Include triggers (`Use when analyzing past agent session logs...`), transcript parsing steps, failure categorization patterns, and guideline synthesis templates.
  - Describe Jujutsu change: `jj describe -m "plan-M7-T1: feat: create distilling-feedback skill playbook"`

### Milestone 8: Thematic Subagent Review Skills Suite
- [ ] **M8-T0: Skills Specifications (`skills/reviewing-*/SPEC.md`)**
  - Write module specs for each of the 8 thematic review skills (including `reviewing-api-surface`, `reviewing-dependencies`, and `reviewing-rust-testing`).
  - Describe Jujutsu change: `jj describe -m "plan-M8-T0: docs: add specs for thematic review skills"`
- [ ] **M8-T1: API Surface Review Skill (`skills/reviewing-api-surface/SKILL.md`)**
  - Author `skills/reviewing-api-surface/SKILL.md`.
  - Focus on API minimalism (`pub(crate)` vs `pub`), reviewing `API.md` diffs, deprecating superseded APIs (`#[deprecated]`), and migration path clarity.
  - Describe Jujutsu change: `jj describe -m "plan-M8-T1: feat: author reviewing-api-surface skill playbook"`
- [ ] **M8-T2: Dependency Hygiene & Security Review Skill (`skills/reviewing-dependencies/SKILL.md`)**
  - Author `skills/reviewing-dependencies/SKILL.md`.
  - Focus on YAGNI/minimalism (stdlib vs crate), popularity & established ecosystem alternatives (`cargo info`, download counts, de facto standards), maintenance health, vulnerability checking (`cargo audit`), transitive dependency weight (`cargo tree`), build script risks (`build.rs`), and license compliance.
  - Describe Jujutsu change: `jj describe -m "plan-M8-T2: feat: author reviewing-dependencies skill playbook"`
- [ ] **M8-T3: Remaining Review Skills Playbooks (`skills/reviewing-*/SKILL.md`)**
  - Implement `reviewing-spec-compliance/SKILL.md`.
  - Implement `reviewing-rust-modularity/SKILL.md`.
  - Implement `reviewing-rust-robustness/SKILL.md`.
  - Implement `reviewing-rust-testing/SKILL.md` (updating with coverage verification guidelines).
  - Implement `reviewing-rust-lint-hygiene/SKILL.md`.
  - Implement `reviewing-containment-safety/SKILL.md`.
  - Ensure all skills adhere to `dev:writing-skills` (<500 words, rationalization tables, red flags).
  - Describe Jujutsu change: `jj describe -m "plan-M8-T3: feat: author remaining thematic review skill playbooks"`

### Milestone 9: Inspect AI Eval Harness with Fence Sandbox
- [ ] **M9-T0: Eval Suite Specification (`evals/SPEC.md`)**
  - Document eval contracts, fence isolation guarantees, sample schema, and scoring formulas.
  - Describe Jujutsu change: `jj describe -m "plan-M9-T0: docs: add spec for Inspect AI eval suite"`
- [ ] **M9-T1: Python Environment & Fence Sandbox Integration (`evals/sandbox/`)**
  - Initialize `pyproject.toml` with `inspect-ai>=0.3` using `uv`.
  - Implement `evals/sandbox/fence_driver.py` configuring fence parameters (`-t code`, read-only host, writable eval directory, blocked outbound network).
  - Implement `evals/sandbox/agent_runner.py` invoking `agy` inside `fence`.
  - Describe Jujutsu change: `jj describe -m "plan-M9-T1: feat: implement fence sandbox driver and agy runner"`
- [ ] **M9-T2: Inspect AI Tasks, Solvers & Scorers (`evals/tasks/`)**
  - Implement `evals/tasks/solvers.py`: Custom solver dispatching agent with target skill prompts in the sandbox.
  - Implement `evals/tasks/scorers.py`: Evaluating agent reviews against expected defect tags (True/False Positives).
  - Implement `evals/tasks/review_eval.py`: Inspect AI `@task` linking dataset, solver, and scorer.
  - Describe Jujutsu change: `jj describe -m "plan-M9-T2: feat: implement Inspect AI tasks, solvers, and scorers"`

### Milestone 10: Curated Datasets (API Drift, Dependency Bloat) & Baseline Benchmarks
- [ ] **M10-T0: Curate Abstracted Examples from Past Feedback (`evals/datasets/`)**
  - Build minimal codebases representing:
    - Inline modules (`inline_mods/`).
    - Unwrapped panics & swallowed errors (`unwrap_panics/`).
    - Unjustified Clippy suppressions (`unjustified_suppression/`).
    - CWD-relative path bugs (`cwd_path_resolution/`).
    - Leaked test resources (`leaky_tests/`).
    - Unintentional API leaks & manifest drift (`api_drift/`).
    - Superseded APIs lacking deprecation annotations (`missing_deprecations/`).
    - Unnecessary / speculative dependencies (`dependency_bloat/`).
    - Obscure dependencies where established alternatives exist (`obscure_deps/`).
    - Dependencies with known advisories or build script risks (`vulnerable_deps/`).
    - Uncovered critical error paths (`coverage_gaps/`).
    - Compliant clean crate (`clean_baseline/`).
  - Describe Jujutsu change: `jj describe -m "plan-M10-T0: test: add curated eval datasets from past feedback"`
- [ ] **M10-T1: Run Benchmark Evaluation & Validate Metrics**
  - Execute `uv run inspect eval evals/tasks/review_eval.py`.
  - Verify that `code-review check`, `code-review opinionated`, and `code-review api check` are correctly leveraged by the agent.
  - Record baseline metrics in `evals/BENCHMARK_RESULTS.md`.
  - Describe Jujutsu change: `jj describe -m "plan-M10-T1: test: execute baseline Inspect AI benchmark runs"`

---

## Execution Handoff & Options

Plan complete and saved to `PLAN.md`.

Two execution options:
1. **Subagent-Driven (Recommended):** Dispatch a fresh subagent for each bite-sized task in the milestones, reviewing diffs between tasks.
2. **Inline Execution:** Execute tasks step-by-step in the current session.
