# Code Review Toolkit: Skills, Rust CLI Tools & Evaluation Plan

> **For agentic workers:** Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish an end-to-end code review ecosystem consisting of modular agent skills, high-performance Rust-based CLI tools, and an automated evaluation suite powered by Inspect AI and the `fence` sandbox to systematically detect, flag, and prevent code quality issues in agent-generated code.

**Architecture:**
1. **Rust CLI Toolkit (`code-review` crate):** A modular binary offering:
   - `code-review check`: Aggregates and runs `cargo fmt`, `cargo clippy`, and custom linters, reporting normalized diagnostics.
   - `code-review configure-lints`: Modifies `Cargo.toml` using `toml_edit` to configure strict clippy lints while preserving formatting and comments.
   - `code-review opinionated`: AST-based static analysis engine checking code patterns beyond Clippy's scope (inline modules, dummy unit structs, VCS-relative paths, error enums, clippy suppression hygiene).
2. **Skills Suite (`skills/`):**
   - `skills/distilling-feedback`: Systematically mines past session transcripts and course corrections to distill new guidelines and linter rules.
   - Thematic review skills applied by dedicated subagents (`reviewing-spec-compliance`, `reviewing-rust-modularity`, `reviewing-rust-robustness`, `reviewing-rust-testing`, `reviewing-rust-lint-hygiene`, `reviewing-containment-safety`).
3. **Inspect AI Evaluation Suite (`evals/`):**
   - Managed with `uv` (`pyproject.toml`, `inspect-ai`).
   - Secure execution using `fence` sandbox to run configured agents (e.g., `agy`) against curated, simplified code examples abstracted from historical agent failures.
   - Deterministic and model-graded scorers evaluating issue detection accuracy, false positive rates, and recommendation quality.

**Tech Stack:** Rust (2024 edition, `syn`, `quote`, `toml_edit`, `clap`), Python 3.12+, `uv`, Inspect AI, `fence` sandbox CLI, Jujutsu (`jj`).

---

## System Overview & Directory Structure

```
.
├── Cargo.toml                          # Workspace / package configuration
├── flake.nix / rust-toolchain.toml     # Nix & Rust development environment
├── AGENTS.md                           # Repository rules and agent guidelines
├── PLAN.md                             # This implementation plan
├── SPEC.md                             # Global project specification
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
│   │   └── opinionated/                # Tool 3: Opinionated static analysis linter (`code-review opinionated`)
│   │       ├── mod.rs
│   │       ├── engine.rs               # AST visitor and analysis driver
│   │       └── rules/                  # Specific opinionated lint checks
│   │           ├── mod.rs
│   │           ├── no_inline_mods.rs   # Enforce submodules in separate files
│   │           ├── free_functions.rs   # Free functions over dummy unit structs
│   │           ├── path_resolution.rs  # VCS/manifest-relative paths vs CWD
│   │           ├── error_types.rs      # Structured thiserror/anyhow vs raw String
│   │           ├── clippy_suppress.rs  # Enforce justification on #[expect]/#[allow]
│   │           └── test_patterns.rs    # Test naming, assertions (expect_that!), no unwrap
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
│   ├── reviewing-rust-testing/         # Skill: Check test quality, gtest, expect_that, teardown
│   │   ├── SKILL.md
│   │   └── SPEC.md
│   ├── reviewing-rust-lint-hygiene/    # Skill: Check clippy cleanliness and suppression rules
│   │   ├── SKILL.md
│   │   └── SPEC.md
│   └── reviewing-containment-safety/   # Skill: Check sandbox bounds, paths, and timeouts
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
- **Features:**
  - Normalizes outputs from `rustfmt`, `clippy`, and `opinionated` into a single diagnostic stream (`file`, `line`, `col`, `rule`, `severity`, `message`, `suggested_fix`).
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

### 4. Skill 1: Feedback Reflection & Guideline Distillation (`skills/distilling-feedback`)
- **Purpose:** Provide agents with a repeatable methodology to analyze past agent session transcripts, identify user corrections and recurring failure patterns, and distill them into actionable review rules.
- **Workflow:**
  1. **Scan Transcripts:** Parse `transcript.jsonl` files for user intervention events, course corrections ("stop", "don't do that", "revert"), tool command exit errors, and manual user commits.
  2. **Categorize Root Causes:** Classify issues into themes (Modularity, Error Handling, Clippy Laziness, Timeout Loops, Sandbox Violations).
  3. **Distill Guidelines:** Format findings into new checklist items, rationalization tables, and before/after code snippets.
  4. **Propose Linter Rules:** Identify which guidelines are mechanically enforceable and draft specifications for new rules in `code-review opinionated`.
  5. **Generate Eval Cases:** Abstract the incident into a minimal, reproducible test case for the Inspect AI eval suite.

### 5. Skill Suite: Thematic Subagent Review Skills
Modular, specialized review skills designed for focused subagent execution:
- **`reviewing-spec-compliance`**: Verifies that implementation strictly satisfies requirements and invariants in `SPEC.md` and module specs without out-of-scope feature creep.
- **`reviewing-rust-modularity`**: Verifies single responsibility, separate submodule files, free functions over dummy structs, and lightweight dependencies.
- **`reviewing-rust-robustness`**: Enforces strict error handling, absence of unwraps/panics in production code, proper error enums, and no ignored results.
- **`reviewing-rust-testing`**: Verifies `#[gtest]`, `expect_that!` assertions, test naming conventions, `.or_fail()?`, and deterministic resource teardown (`finally` / drop guards).
- **`reviewing-rust-lint-hygiene`**: Ensures clean compilation under strict clippy, zero warnings, and absence of unjustified `#[expect]` or `#[allow]`.
- **`reviewing-containment-safety`**: Enforces VCS root-relative path resolution, avoids runaway commands, and ensures compliance with sandbox boundaries.

**Subagent Orchestration Pattern:**
When an agent reviews code, it dispatches specialized review subagents in parallel with dedicated review prompts, then aggregates their structured feedback into a consolidated report.

### 6. Evaluation Suite: Inspect AI + Fence Sandbox (`evals/`)
- **Environment:** Isolated Python virtual environment managed via `uv` (`uv run inspect eval ...`).
- **Sandbox Architecture (`fence`):**
  - Uses the `fence` CLI sandbox (`fence -t code -- ...`) to contain the agent under test.
  - Network access is denied/restricted to prevent external side effects.
  - The host filesystem is read-only; each eval run operates in an ephemeral target directory containing the test crate.
- **Agent Under Test:** Configured agent binary (e.g. `agy --print "<prompt>" --mode accept-edits --dangerously-skip-permissions`).
- **Evaluation Dataset:** Abstracted, minimal reproduction cases created from past feedback:
  - Positive examples (bad patterns): Deliberate bugs (unwrapped panics, inline submodules, unjustified clippy suppression, CWD path dependencies, leaky test resources).
  - Negative examples (clean patterns): Fully compliant Rust crates to measure false positive rates.
- **Inspect AI Tasks & Metrics:**
  - `@task`: Loads datasets and wires the sandbox solver and evaluation scorer.
  - `@solver`: Executes the agent inside `fence` with the review skills, directing it to review the target directory and emit a review report.
  - `@scorer`: Evaluates the agent's review output against ground-truth defect annotations:
    - **Detection Rate (Recall):** Did the agent identify the deliberate flaw?
    - **Precision:** Did the agent avoid false accusations on clean code?
    - **Actionability:** Did the agent suggest the idiomatic fix?
    - **Tool Synergy:** Did the agent invoke `code-review check` or `code-review opinionated` during its review?

---

## Implementation Milestones & Roadmap

### Milestone 1: Core CLI Architecture & Cargo.toml Lint Configurator Tool
- **Description:** Initialize the Rust CLI crate structure with `clap`, create unified diagnostic data structures, and implement `code-review configure-lints` using `toml_edit` to inject and update strict Clippy lint configurations in `Cargo.toml`.
- **Status:** `[ ] Pending`
- **Target Completion Date:** 2026-10-05
- **Actual Completion Date:** -
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
- **Description:** Implement `code-review check` to run `cargo fmt --check`, `cargo clippy`, and `code-review opinionated`, aggregating diagnostic outputs into console, JSON, and Markdown formats. Add Jujutsu changed-file filtering (`--changed-only`).
- **Status:** `[ ] Pending`
- **Target Completion Date:** 2026-10-12
- **Actual Completion Date:** -
- **Dependencies:** Milestone 2
- **Tasks File:** `plan/M3.md`
- **Feedback File:** `plan/FEEDBACK_M3.md`

### Milestone 4: Feedback Distillation & Reflection Skill
- **Description:** Create `skills/distilling-feedback/SKILL.md` and `SPEC.md` defining the workflow for analyzing past session transcripts (`transcript.jsonl`), categorizing failures, and distilling new review guidelines and test cases.
- **Status:** `[ ] Pending`
- **Target Completion Date:** 2026-10-15
- **Actual Completion Date:** -
- **Dependencies:** Milestone 3
- **Tasks File:** `plan/M4.md`
- **Feedback File:** `plan/FEEDBACK_M4.md`

### Milestone 5: Thematic Subagent Review Skills Suite
- **Description:** Create thematic review skills under `skills/` (`reviewing-spec-compliance`, `reviewing-rust-modularity`, `reviewing-rust-robustness`, `reviewing-rust-testing`, `reviewing-rust-lint-hygiene`, `reviewing-containment-safety`) with frontmatter, checklists, rationalization tables, and subagent prompts.
- **Status:** `[ ] Pending`
- **Target Completion Date:** 2026-10-19
- **Actual Completion Date:** -
- **Dependencies:** Milestone 4
- **Tasks File:** `plan/M5.md`
- **Feedback File:** `plan/FEEDBACK_M5.md`

### Milestone 6: Inspect AI Eval Harness with Fence Sandbox
- **Description:** Initialize Python environment via `uv`, configure `pyproject.toml` with `inspect-ai`, build the `fence` sandbox runner, and create Inspect AI tasks, solvers, and scorers to evaluate agents reviewing code examples.
- **Status:** `[ ] Pending`
- **Target Completion Date:** 2026-10-23
- **Actual Completion Date:** -
- **Dependencies:** Milestone 5
- **Tasks File:** `plan/M6.md`
- **Feedback File:** `plan/FEEDBACK_M6.md`

### Milestone 7: Abstracted Dataset, Baseline Benchmarks & End-to-End Verification
- **Description:** Extract real historical feedback examples into `evals/datasets/`, run baseline Inspect AI benchmarks on configured agents (e.g., `agy`), verify detection accuracy and tool integration, and finalize documentation.
- **Status:** `[ ] Pending`
- **Target Completion Date:** 2026-10-26
- **Actual Completion Date:** -
- **Dependencies:** Milestone 6
- **Tasks File:** `plan/M7.md`
- **Feedback File:** `plan/FEEDBACK_M7.md`

---

## Detailed Task Breakdown

### Milestone 1: Core CLI Architecture & Cargo.toml Lint Configurator Tool
- [ ] **M1-T0: Update Specifications (`SPEC.md`, `src/tools/cargo_toml.spec.md`)**
  - Define invariants for `code-review` CLI subcommands and `Cargo.toml` modification safety (no comment stripping, preserving existing tables, idempotency).
  - Describe Jujutsu change: `jj describe -m "plan-M1-T0: docs: add specs for core CLI and cargo-toml configurator"`
- [ ] **M1-T1: CLI Dispatcher & Diagnostic Core Types**
  - Add `clap` and `serde` dependencies to `Cargo.toml`.
  - Create `src/cli.rs` defining commands: `check`, `configure-lints`, `opinionated`.
  - Create `src/common/diagnostics.rs` defining `Diagnostic`, `Severity`, `Span`, and `DiagnosticReport`.
  - Create `src/common/reporter.rs` supporting console output and structured JSON.
  - Describe Jujutsu change: `jj describe -m "plan-M1-T1: feat: add clap CLI dispatcher and unified diagnostic types"`
- [ ] **M1-T2: Cargo.toml Lint Injection Engine (`src/tools/cargo_toml.rs`)**
  - Add `toml_edit` dependency to `Cargo.toml`.
  - Write unit tests in `src/tools/cargo_toml.rs` verifying that running `configure_lints` on a minimal `Cargo.toml` preserves comments, inserts `[workspace.lints.clippy]` or `[lints.clippy]`, and sets `warn` on all required lints.
  - Implement `configure_lints` and `remove_lints` functions.
  - Wire `code-review configure-lints` subcommand in `src/main.rs`.
  - Verify with `cargo test`.
  - Describe Jujutsu change: `jj describe -m "plan-M1-T2: feat: implement Cargo.toml lint configurator using toml_edit"`

### Milestone 2: Opinionated Static Analysis Linter Engine & Rules
- [ ] **M2-T0: Update Specifications (`src/tools/opinionated/SPEC.md`)**
  - Document the contract, AST patterns, and false-positive criteria for each custom lint rule.
  - Describe Jujutsu change: `jj describe -m "plan-M2-T0: docs: add spec for opinionated linter rules"`
- [ ] **M2-T1: AST Visitor Framework (`src/tools/opinionated/engine.rs`)**
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
  - Aggregate all diagnostics into `DiagnosticReport`.
  - Describe Jujutsu change: `jj describe -m "plan-M3-T1: feat: implement subprocess runner and compiler json parser"`
- [ ] **M3-T2: Jujutsu Integration (`--changed-only`)**
  - Implement VCS query using `jj --no-pager diff --summary` to extract modified files.
  - Filter diagnostics to only report issues on modified files.
  - Describe Jujutsu change: `jj describe -m "plan-M3-T2: feat: add jj changed-file filtering to code-review check"`

### Milestone 4: Feedback Distillation & Reflection Skill
- [ ] **M4-T0: Skill Specification (`skills/distilling-feedback/SPEC.md`)**
  - Specify the distillation process contracts, transcript parsing schemas, and output artifact requirements.
  - Describe Jujutsu change: `jj describe -m "plan-M4-T0: docs: add spec for distilling-feedback skill"`
- [ ] **M4-T1: Skill Playbook (`skills/distilling-feedback/SKILL.md`)**
  - Write concise, token-efficient playbook following `dev:writing-skills`.
  - Include triggers (`Use when analyzing past agent session logs...`), transcript parsing steps, failure categorization patterns, and guideline synthesis templates.
  - Describe Jujutsu change: `jj describe -m "plan-M4-T1: feat: create distilling-feedback skill playbook"`

### Milestone 5: Thematic Subagent Review Skills Suite
- [ ] **M5-T0: Skills Specifications (`skills/reviewing-*/SPEC.md`)**
  - Write module specs for each of the 6 thematic review skills.
  - Describe Jujutsu change: `jj describe -m "plan-M5-T0: docs: add specs for thematic review skills"`
- [ ] **M5-T1: Review Skills Playbooks (`skills/reviewing-*/SKILL.md`)**
  - Implement `reviewing-spec-compliance/SKILL.md`.
  - Implement `reviewing-rust-modularity/SKILL.md`.
  - Implement `reviewing-rust-robustness/SKILL.md`.
  - Implement `reviewing-rust-testing/SKILL.md`.
  - Implement `reviewing-rust-lint-hygiene/SKILL.md`.
  - Implement `reviewing-containment-safety/SKILL.md`.
  - Ensure all skills adhere to `dev:writing-skills` (<500 words, rationalization tables, red flags).
  - Describe Jujutsu change: `jj describe -m "plan-M5-T1: feat: author thematic review skill playbooks"`

### Milestone 6: Inspect AI Eval Harness with Fence Sandbox
- [ ] **M6-T0: Eval Suite Specification (`evals/SPEC.md`)**
  - Document eval contracts, fence isolation guarantees, sample schema, and scoring formulas.
  - Describe Jujutsu change: `jj describe -m "plan-M6-T0: docs: add spec for Inspect AI eval suite"`
- [ ] **M6-T1: Python Environment & Fence Sandbox Integration (`evals/sandbox/`)**
  - Initialize `pyproject.toml` with `inspect-ai>=0.3` using `uv`.
  - Implement `evals/sandbox/fence_driver.py` configuring fence parameters (`-t code`, read-only host, writable eval directory, blocked outbound network).
  - Implement `evals/sandbox/agent_runner.py` invoking `agy` inside `fence`.
  - Describe Jujutsu change: `jj describe -m "plan-M6-T1: feat: implement fence sandbox driver and agy runner"`
- [ ] **M6-T2: Inspect AI Tasks, Solvers & Scorers (`evals/tasks/`)**
  - Implement `evals/tasks/solvers.py`: Custom solver dispatching agent with target skill prompts in the sandbox.
  - Implement `evals/tasks/scorers.py`: Evaluating agent reviews against expected defect tags (True/False Positives).
  - Implement `evals/tasks/review_eval.py`: Inspect AI `@task` linking dataset, solver, and scorer.
  - Describe Jujutsu change: `jj describe -m "plan-M6-T2: feat: implement Inspect AI tasks, solvers, and scorers"`

### Milestone 7: Abstracted Dataset, Baseline Benchmarks & End-to-End Verification
- [ ] **M7-T0: Curate Abstracted Examples from Past Feedback (`evals/datasets/`)**
  - Build minimal codebases representing:
    - Inline modules (`inline_mods/`).
    - Unwrapped panics & swallowed errors (`unwrap_panics/`).
    - Unjustified Clippy suppressions (`unjustified_suppression/`).
    - CWD-relative path bugs (`cwd_path_resolution/`).
    - Leaked test resources (`leaky_tests/`).
    - Compliant clean crate (`clean_baseline/`).
  - Describe Jujutsu change: `jj describe -m "plan-M7-T0: test: add curated eval datasets from past feedback"`
- [ ] **M7-T1: Run Benchmark Evaluation & Validate Metrics**
  - Execute `uv run inspect eval evals/tasks/review_eval.py`.
  - Verify that `code-review check` and `code-review opinionated` are correctly leveraged by the agent.
  - Record baseline metrics in `evals/BENCHMARK_RESULTS.md`.
  - Describe Jujutsu change: `jj describe -m "plan-M7-T1: test: execute baseline Inspect AI benchmark runs"`

---

## Execution Handoff & Options

Plan complete and saved to `PLAN.md`.

Two execution options:
1. **Subagent-Driven (Recommended):** Dispatch a fresh subagent for each bite-sized task in the milestones, reviewing diffs between tasks.
2. **Inline Execution:** Execute tasks step-by-step in the current session.
