# Microgentic operating notes

## Product direction

Microgentic is an adaptive coding agent optimized for small models that need help
staying on task. Keep the everyday experience small, simple, and fast.
Treat `CHARTER.md` as the product charter and `PUBLIC_READINESS.md` as the backlog.

The primary development checkout is `~/Work/Projects/Personal/microgentic`.
This repository is self-contained and general. It must not name, depend on, or
assume any particular benchmark suite, experiment repository, or local path
outside itself. Experiment repositories depend on Microgentic, never the reverse.
Keep the benchmark commands usable by any benchmark harness; the current
adapters (DeepSWE/Pier and Terminal-Bench/Harbor) are the first two consumers,
not the design target.

## Runtime and UX boundaries

- Bare `microgentic` starts an ongoing conversation with no assumed work.
- `run` reads a prompt from stdin or `--prompt` and participates in shell pipes.
- `benchmark run` and `benchmark run-sequential` hand their run contract
  straight to the engineering subagent, isolated from conversation. The
  coordinator's `start_task` runs the same subagent. Do not give benchmark mode
  loop behavior of its own (see `CHARTER.md`; the migration is in
  `PUBLIC_READINESS.md`).
- Preserve validation authority, model controls, and stop rules when changing
  presentation. Compare exact provider requests when needed.
- Keep user preferences, stdin intake, and TUI history out of benchmark mode.
- Confine benchmark integration changes to adapters or `microgentic benchmark`.
  Benchmark-specific operation must not change everyday `run`, the TUI, or
  shared runtime defaults. Pass any benchmark customization explicitly through
  the benchmark entry point.
- Show evidence footers for project tools in the current exchange. Keep traces
  even when ordinary conversation does not display a footer.
- Preserve unrelated user changes and Git stashes.
- Retain the `adaptive_agent_harness` library name and compatibility executable
  until consumers complete an explicit migration. Do not install an `mg` command.

## Validation and commits

Commit after meaningful changes. Use the existing CLI UX conventions.
Run checks appropriate to the change. Before a release, run all required gates:

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets --all-features
python3 -m unittest discover -s benchmarks -p 'test_collection_validation.py'
python3 -m unittest discover -s benchmarks -p 'test_terminal_outcome.py'
cargo build --locked --release --bins
mdbook build book
```

No suite in this repository is designated integration or end-to-end. The
`tests/` Cargo layout is not an exception to the test I/O rules: tests must make
no real network connections, open no sockets or servers, and use in-process
gateway/transport doubles at application I/O boundaries. Drive the public run
API or actual CLI dispatcher, preserving validation, evidence and settlement.
Use injected clocks for deadline and elapsed-time behavior; never use real
sleeps or wall-clock thresholds as behavioral evidence. Preserve executable
signal/process-group checks and FIFO/readiness handshakes without settling delays.
Use Rust tests and the existing `tests/support/` workspace fixtures; do not add
Python scripts under `tests/`.

Benchmark continuity runs in-process with
`cargo test --locked --lib cli::benchmark_continuity`. Fixed gateway request,
packet and ledger expectations retain their existing versioned specification;
see `tests/README.md` for its coverage and limits. Registered experiment harnesses
in `fixtures/recovery/` and `fixtures/progress/` stay unchanged in Python and are
not invoked by repository tests. Stacey decided on 2026-10-15 that registered
experiments retain and hash-register those files. Do not port them unless she
revisits that decision.

Provider fixtures use in-process gateway/transport doubles. Do not start local
models for routine checks. Preserve model patience and let the runtime own stop
policy during live runs. Local-model comparisons stay within Qwen unless the
user requests otherwise.

## Local install and CI

Install locally with Cargo, not by waiting for a published release:

```sh
git worktree add --detach /tmp/mg-install <commit>
cargo install --locked --path /tmp/mg-install --target-dir target/install --force
git worktree remove /tmp/mg-install
```

Run every gate in `.hone-gates.json` on that revision immediately before you
install it; a local install is a deployment. Install from a committed revision,
not from a working tree that another agent may be editing. Do not reuse
`target/install` for a `cargo test` in the main checkout afterwards: the test
binaries bake in the worktree's `CARGO_MANIFEST_DIR`, Cargo does not rebuild
them for the new path, and every fixture-reading and provenance test fails on
a directory that no longer exists. `~/.cargo/bin` comes before Homebrew on the
PATH here, so the Cargo build replaces the Homebrew one without an uninstall.

Every push and tag starts GitHub Actions. Let those runs finish, including for
intermediate versions; never cancel them. Do not wait on them either: push, then
continue local work and local installs. Check a run's result later, when its
outcome decides the next step (for example, before updating the Homebrew tap).

## Documentation site

User documentation is an mdBook in `book/`. The `deploy-docs` job in
`.github/workflows/ci.yml` publishes it to <https://vetzal.ca/microgentic/>
after every push to `main` that passes CI. When a change alters what users see
or do, update the matching page in `book/src/` in the same commit.
`book/src/changelog.md` includes `CHANGELOG.md`; do not copy entries into it.
Use the mdBook version that CI pins (`MDBOOK_VERSION`, 0.5.4 today).

## Release process

See `RELEASING.md`. Public binaries need an embedded source revision, native
platform checks, checksums, and a smoke test outside the build checkout.
Publishing requires user authorization; a direct request to publish supplies it.
Never replace an existing release tag or asset silently. Update the Homebrew tap
only after the corresponding release assets pass validation.
