# Language customization notes

This document records the language-specific behaviour the engineering loop has
used, how each piece is applied, and what is known about whether it helps. It
exists so a general "language mode", or a customization that users can declare
outside the binary, can grow from what works in practice rather than from a
design made up front.

It is a living document. Update it when a language-specific behaviour is added,
removed, measured, or moved.

## Where the behaviour lives

All language-specific policy sits behind the `DomainProfile` trait in
`src/profile/mod.rs`. The `coding` profile (`src/profile/coding.rs`) holds it.
`tests/structural_coupling.rs` fails if a language, build tool or test-runner
name appears in the runtime core outside `src/profile/`.

The `DomainProfile` methods are already the seams a language mode would plug
into. The two kinds below differ in risk: guidance changes what the model does,
while recognition only changes what the harness notices.

## Kind 1: guidance the model reads

These lines steer the model. Until the coordinator/subagent change (see
`CHARTER.md`), every coding task got the Rust versions whatever the project's
language was.

| Seam | What it was | Where the model sees it | Mode |
|---|---|---|---|
| `system_guidance` | "Create Cargo.toml, src/lib.rs, and other generated source at the tool root unless the task says otherwise." | System prompt | Task mode |
| `system_guidance` | "Use timeout_secs 1800 for first cargo build, cargo test, or similarly expensive validation probes." | System prompt | Task mode |
| `system_guidance` | "For Rust projects after edits, prefer this validation ladder: cargo fmt --check, cargo clippy --all-targets --all-features -- -D warnings, focused tests, then cargo test." | System prompt | Task mode |
| `run_guidance` | "Complete this benchmark task inside the generated project workspace." and "Build or update the Rust project at the current tool root, not in a nested workspace/ directory." | First user message, wrapped around the goal | Task mode |
| `repair_ladder_suffix` | "If you edit, run the validation ladder immediately afterward: cargo fmt --check, cargo clippy, focused tests, then broad tests." | End of every validation-repair prompt | Both modes |

The repair suffix is the one leak into everyday use: a user in a Python project
who hit a failing check was told to run `cargo fmt --check`.

### The general shape underneath

With the tool names removed, the Rust guidance is three ideas. Each is a
candidate field for a language mode:

1. **Project layout.** Where new source goes, and the files that make a minimal
   project (`Cargo.toml` and `src/lib.rs` for Rust).
2. **Expensive commands.** Which commands are slow the first time (a cold
   `cargo build` compiles every dependency), and the timeout to give them.
3. **Validation ladder.** An ordered list of checks from cheapest to broadest:
   format check, lint, focused tests, full tests. The order is the policy; the
   commands are per language.

### Where the evidence stands

The Rust lines date from the Rust-first demo matrix (June 2026) and moved
unchanged into the profile in Slice 3 of `GENERALIZATION_PLAN.md`. No controlled
measurement isolates any of them. The timeout line has a mechanical reason: a
cold build can take minutes and would otherwise hit a default timeout. The
ladder's value is untested. The registered experiments live outside this
repository, so look there for any trial that varies these lines before you
claim an effect.

## Kind 2: recognition the harness applies

These tables change what the harness notices in commands and output. The model
never reads them. Most already cover several languages unevenly.

| Seam | What it does | Language coverage |
|---|---|---|
| `command_family` | Groups shell commands for repeated-failure counting (`cargo test`, `npm test`, `pytest`, `go test`, `make test`, `mvn test`, `gradle test`, …). Falls back to the first word | Rust, JS, Python, Go, Make, JVM |
| `validation_command_family` | Labels validation commands in resumed-validation prompts. Has Rust-specific cases (`cargo clippy`, `./cargo test`); falls back to the first two words | Rust only, plus a general fallback |
| `failure_details` | Pulls failure lines out of probe output: `---- name stdout ----`, `thread '…' panicked at`, `error[`, `error:`, `FAILED`, and the `failures:` block up to `test result:` | Mostly the Rust test runner and compiler; `error:` and `FAILED` happen to match other tools |
| `is_ignored_dir` | Build and dependency directories left out of inspection and change detection (`target`, `node_modules`, `.venv`, `__pycache__`, `build`, `dist`, …) | Several ecosystems |
| `path_requires_validation_after_write` | Which writes need re-validation. Documentation (`.md`, `README`, `LICENSE`, …) is exempt | Language-neutral |
| `resolve_legacy_contract` | Turns fenced shell commands in a plain-text task into probes when they start with a known test or build command (`cargo test`, `npm test`, `pytest`, `go test`, `mix test`, …) | Rust, JS, Python, Go, Elixir |
| `action_intent_phrases` | Phrases that count as the model announcing an action; adds "run cargo" to the general verbs | Rust only |

Gaps show where a language mode would help most. For example, `failure_details`
finds nothing useful in pytest or Jest output, so repair prompts for those
projects carry only the raw failure text.

## Toward a language mode

These are questions to answer from practice, not decisions:

- **Selection.** Detect the language from marker files (`Cargo.toml`,
  `pyproject.toml`, `package.json`, `go.mod`, `mix.exs`), let the run contract
  name it, or both? A project can hold several languages.
- **Declaration.** Each guidance field above is data: layout hints, slow
  commands with timeouts, an ordered validation ladder. Most recognition tables
  are data too: command prefixes, ignored directories, failure-line patterns. A
  declared file (in the project, or beside the user's `config.json`) could hold
  them. `failure_details` needs patterns at least, and maybe more than patterns
  for some runners.
- **Authority.** Ladder steps would stay advisory guidance. Only probes in the
  run contract decide completion (see `CHARTER.md`). A language mode must not
  become a hidden source of completion rules.
- **Project guidance overlap.** A project's `AGENTS.md` often states its own
  checks. Decide whether a language mode is a default that `AGENTS.md`
  overrides.
- **Measurement.** Bring back a language-specific line only with a trial that
  shows it helps. The general wording is the baseline to beat.

## Current state

The model-facing guidance is language-neutral, as the first slice of the
coordinator/subagent migration in `PUBLIC_READINESS.md`. It keeps the three
ideas above in general wording. The original Rust text is recorded in the
Kind 1 table. The recognition tables are unchanged.
