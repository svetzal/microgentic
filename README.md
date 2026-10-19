# Microgentic

An adaptive coding agent optimized for small models that need extra help staying
on task. Microgentic provides a small terminal interface over an experimental
runtime for inspecting, editing, validating, and repairing scoped artifacts.

User documentation: <https://vetzal.ca/microgentic/>

## Install and start

```sh
brew install svetzal/tap/microgentic
microgentic setup --apply
microgentic
```

Run an oMLX or Ollama server before sending your first message. Setup lets you
select the provider, model, and effort level. It makes no provider request.
The TUI opens a conversation in your current project; ask for work when ready.

For a single prompt, use the normal shell input and output channels:

```sh
echo 'Explain this project' | microgentic run
microgentic run --prompt 'Review this diff' < changes.diff > review.txt
```

Release packages support Apple Silicon and Intel Macs on macOS 15 or later,
and Linux x86-64 with glibc 2.35 or later. Downloadable binaries and checksums are
available from [GitHub Releases](https://github.com/svetzal/microgentic/releases).
The Homebrew package also includes the compatibility executable. `mg` remains
an optional shell alias; no `mg` command is installed.

Uninstall with `brew uninstall microgentic`. Configuration and traces remain in
their documented XDG locations for future upgrades or reinstalls.

The harness is built in Rust on top of
[Mojentic](https://github.com/svetzal/mojentic-ru). It emphasizes deterministic
validation, strict workspace boundaries, traceable runtime decisions, and
evidence-backed experimentation. It does not assume one task, language, or
benchmark suite, and its benchmark commands are meant for any harness.

## Status

Microgentic is an early release under active development. Runtime policies,
contracts, trace schemas, and command-line interfaces may change as experiments
reveal better boundaries.

The [Microgentic public CLI readiness checklist](PUBLIC_READINESS.md) records
preparation for an adaptive coding agent optimized for small models, with a
small, simple, fast experience inspired by Pi. It covers provider setup, terminal
and shell workflows, session recovery, packaging, and benchmark-adapter migration
to the user-focused control surface.

## Capabilities

- Ongoing terminal conversations with streamed replies and retained context
- Provider setup and prompt runs through stdin or `--prompt`
- Typed run contracts for scope, guidance, probes, budgets, and terminal rules
- Scoped filesystem and tool execution
- Mutation-aware validation freshness
- Structured JSONL traces with exact provider-bound requests and deterministic
  trace analysis
- Native self-contained HTML transcript generation
- Bounded repair and retained-artifact retry policies
- Throughput-aware patience for slow local models
- Domain profiles that keep task-specific policy out of the runtime core
- A `calculate` tool for exact arithmetic in conversations and everyday runs

## Requirements

- An oMLX or Ollama server with a model installed; Qwen models are the benchmark
  defaults
- For source builds: Rust 1.99 or later and network access for the locked Mojentic
  Git dependency

## Configuration

`microgentic setup` saves the provider, default model, and reasoning effort.
Settings live in `$XDG_CONFIG_HOME/microgentic/config.json`, or
`~/.config/microgentic/config.json`. Server addresses and credentials come from
the environment: `OMLX_HOST`, `OMLX_API_KEY`, `OMLX_TIMEOUT`, and `OLLAMA_HOST`.
Explicit flags override saved defaults. Benchmark mode and the compatibility
executable never read personal settings. See
[Providers and models](https://vetzal.ca/microgentic/providers.html).

## Documentation

The [user guide](https://vetzal.ca/microgentic/) covers install, setup, the
conversation, shell use, providers, project guidance, tools, commands, file
locations, traces, benchmark mode, and research controls. Its source is in
[`book/src/`](book/src/). Other documents:

- [`CHANGELOG.md`](CHANGELOG.md) records each release.
- [`PUBLIC_READINESS.md`](PUBLIC_READINESS.md) is the backlog.
- [`CHARTER.md`](CHARTER.md) is the product charter.
- [`CONTEXT-RESIDENCY.md`](CONTEXT-RESIDENCY.md) frames context residency.
- [`TRACE_SCHEMA.md`](TRACE_SCHEMA.md) defines the trace format.
- [`GENERALIZATION_PLAN.md`](GENERALIZATION_PLAN.md) is the history of the
  architecture and its experimental decisions.
- [`RELEASING.md`](RELEASING.md) describes the release process.

## Build and Test

```sh
cargo build
cargo test --all-targets
cargo clippy --all-targets -- -D warnings
```

The end-to-end tests are Rust and run under `cargo test`. `tests/tui.rs`
drives the real TUI in a pseudo-terminal and reads the rendered screen.
`src/cli/everyday_runs.rs` covers the calculator tool and project guidance.
`src/cli/benchmark_continuity.rs` checks benchmark and legacy task dispatch
against fixed complete gateway requests, assembled packets and context ledgers.
These CLI suites use an in-process gateway double. The TUI and termination
integration suites retain the mock oMLX and Ollama server from `tests/support/`,
never a model. The TUI tests need a Unix pseudo-terminal.

Run benchmark continuity, including contract conversation isolation, with:

```sh
cargo test --locked --lib cli::benchmark_continuity
```

The historical socket-based binary comparison has been retired. Versioned
expectations now guard both dispatch paths against drifting together; see
[`tests/README.md`](tests/README.md) for the characterization and coverage limits.

## License

Licensed under the MIT License. See [`LICENSE`](LICENSE).
