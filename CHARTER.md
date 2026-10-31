# Microgentic charter

## Purpose

Microgentic's primary development checkout is `~/Work/Projects/Personal/microgentic`.
The upstream repository is `svetzal/microgentic`. This repository is general and
self-contained: it does not refer to any particular experiment repository, and
experiment repositories depend on it, never the reverse. General-purpose product
development proceeds independently of registered experiments; preserve their
pinned evidence.
Versioned releases and Homebrew packages follow `RELEASING.md`.

This Rust application studies how an adaptive harness can make smaller local
models useful. It changes the size, context, validation, and feedback of work
packets according to measured capability.

The harness owns continuity. The model reasons about a scoped packet.
Deterministic checks establish whether work satisfies its contract.
A model claim alone cannot establish success.

The public product is Microgentic: an adaptive coding agent optimized for small
models that need extra help staying on task. Its CLI and terminal conversation
should feel small, simple, and fast, with Pi as an interaction reference and
selectable Mojentic gateways. Build session and presentation around the general
runtime, preserving validation, evidence, and local-model patience.

Design the control surface for end users. Benchmark harnesses of any kind adapt
to its public non-interactive interface and documented advanced configuration.
The benchmark commands serve any harness, not one benchmark suite; today's
adapters are the first consumers. Verify equivalent
resolved inputs and runtime behavior before new trials; preserve frozen evidence
through pinned binaries, adapter revisions, and configuration. The preparation
backlog is in `PUBLIC_READINESS.md`; proposed features are not implemented behavior.

The initial Microgentic CLI keeps task preparation and presentation outside the
runtime policy. Benchmark adapters supply explicit providers and authoritative
contracts instead of inheriting personal setup defaults. Prove migration fidelity
with provider requests after tool feedback, not only matching option names.

Conversation continuity retains native user, assistant, and tool history, while
each request resolves fresh task authority and validation state. Never replay old
system/probe/repair instructions as new task policy or incomplete streams as
completed responses. Repair pruning must protect the current user task as well as
earlier conversation prompts. Stream presentation observes the existing runtime;
benchmark entry points never load ambient conversation history.

Microgentic works as two nested agents. The conversation is the coordinator.
An open TUI or everyday `run` has no assumed engineering task: the user and the
coordinator investigate, discuss, and work out a thought until they agree on
work worth doing. Ordinary replies finish an exchange without task status
tokens. The coordinator inspects the project but does not change it. Its
`start_task` tool begins engineering work.

`start_task` gathers what should be known before building as structured fields:
the goal, the relevant parts of the conversation, constraints, and how success
will be checked. The fields exist to help the user and the coordinator think the
work through. The user reviews the brief before work starts. The coordinator
then writes it as a run contract.

The run contract is the engineering subagent's only task input. Its guidance is
plain text: the task as a person would hand it over. Beside the text, as data,
it may carry acceptance probes and read and write scopes, which the harness
enforces. Checks travel as probes rather than prose, so no model has to notice
them for them to count. The subagent starts from the contract and the
workspace, not from the conversation, and runs until its stop rules end the
work. It owns task prompting, probe execution, validation authority, repair,
and terminal rules. Quoted examples stay data, and observational shell checks
stay separate from probes. It returns its outcome, changes, and checks to the
coordinator, which reports them in conversation.

Today's code predates this model: everyday conversation and benchmark task mode
are separate loops. `PUBLIC_READINESS.md` tracks the migration.

Show the TUI evidence footer for project-tool activity in the current exchange.
Ordinary chat stays free of that footer, including conversational replies after
completed work. Keep all traces saved and use structured activity events rather
than prompt classification to control presentation. This changes no task authority.

Benchmark mode is glue. `microgentic benchmark run` and
`benchmark run-sequential` read a run contract from experiment files (or a plain
`task.md`, resolved into one), hand it straight to the engineering subagent, and
emit JSON. The contract format is the shared interface between `start_task` and
any benchmark harness. They skip the coordinator, personal
setup, stdin intake, and conversation history, and change nothing about how the
subagent works. A benchmark score therefore measures the same engineering loop
users get. Exact historical reproduction still needs pinned binaries,
dependencies, adapters, and provider configuration.

## Responsibilities

The harness maintains goals, acceptance criteria, budgets, and stopping rules.
It assembles bounded context and preserves conversation history.
It runs scoped tools, validates their effects, classifies failures, and records
traces that explain each decision.

Mojentic supplies provider completion, messages, tool abstractions, and tracing
primitives. Provider-level recovery belongs in Mojentic. The harness owns resource
admission, experiment policy, session recovery, and trace persistence.

The harness is general. No benchmark task is its domain.
Do not couple runtime policy to one task, language, model, or repository layout.
Do not hide validation inside an unconstrained model phase.

## Evidence and scope

Use explicit contracts and deterministic probes. Preserve exact provider-bound
context when explaining a model failure. Distinguish model behavior from missing
context, tool defects, and environment failures.

A surprising trace needs a hypothesis and a controlled measurement before a
policy change. Local generation has no fixed wall-clock cutoff. The harness uses
measured progress and throughput-aware patience.

Keep historical experiments unchanged. Never restart a stopped benchmark to
validate a library change. Use a fresh registered experiment instead.

Research protocol and operating evidence for registered experiments live with
those experiments, outside this repository. The repository README describes the
current runtime and commands.

## Request recovery evidence

Wire attempts belong to one immutable completion and are separate from semantic
interactions and tool depth. Admission requires proof for the exact previous
attempt and encoded request. A syntactically valid provider identifier can still
echo a credential; filter known active credentials before persistence or admission.
Credential fragments across transport or semantic boundaries require buffering;
a redacted capture must never be presented as exact wire evidence.

## Workspace preservation

Capture Git stash object IDs before agent tools run, then compare inventories
before accepting DONE. Existing stashes are baseline evidence, not automatically
agent-owned work. Never automatically restore or discard stashes. Record shared
repository attribution as uncertain. A restored implementation must still pass
its independent verifier; recovery of missing edits alone does not prove quality.
