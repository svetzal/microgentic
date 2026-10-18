# Traces and transcripts

Every run and every conversation exchange writes a JSONL trace. A trace records
what Microgentic sent to the provider, what came back, each tool call, and each
decision the runtime made. Use traces to find out why a run went the way it
did.

## What a trace contains

- an exact, ordered snapshot of each request sent to the provider
- a context ledger for each request, with the reason each part is included and
  its estimated size
- streamed reasoning and reply text
- each tool call and its result
- validation results, stop decisions, and the final outcome
- the project files and their uncommitted Git edits when the run started
  (`run.workspace.baseline`), and in `run.finished`, the files that the run
  changed and the checks that passed, failed, or did not run
- the project guidance that was discovered and delivered
- the model's context window and where it came from: your
  `--context-window-tokens`, the provider, or Microgentic's model table
- for each request, the time to the first token, what kind of token it was
  (reply text, thinking, or a tool call), and how long the request took
- for everyday runs and the conversation, the residency of each part of each
  request: why it is there, its size, the turn that added it, whether it is
  pinned, and whether Microgentic has shortened it. The standing context (the
  project guidance) is measured against a limit you can set; see
  [Providers and models](providers.md#advanced-context-settings).

The full schema is in
[`TRACE_SCHEMA.md`](https://github.com/svetzal/microgentic/blob/main/TRACE_SCHEMA.md).

## Find a trace

In the conversation, an exchange that uses project tools ends with an
`Evidence:` line that gives its trace location. All traces are below the
run-state directory. See [Files and locations](locations.md).

A trace file is named for the moment its run started, to the nanosecond, for
example `run-20261017T093005.123456789Z.jsonl`. Names sort in start order. If
two runs start at the same instant in one directory, the later one adds `_01`,
`_02`, and so on. A run never replaces an existing trace file.

## Summarize traces

```sh
microgentic analyze-trace ~/.local/state/microgentic/runs/run-*/traces/
```

The summary reports context assembly, tool payload size, snapshot coverage, and
tool-call counts. It also tells you if a trace is incomplete, so that you do not
draw conclusions from partial evidence.

## Render a transcript

```sh
microgentic render-transcript traces/ \
  --output transcript.html \
  --title "Agent session"
```

The transcript is one self-contained HTML file. It shows the provider input,
reasoning, replies, tool effects, runtime decisions, and context measurements as
one timeline. Give it trace files or directories. It finds the Microgentic
traces in them and ignores other JSONL files.

## Context and tool-call evidence

Every worker provider call has a compact context ledger and an exact ordered
request snapshot. `analyze-trace` reports the snapshot count and whether
coverage is complete, so you can spot legacy or truncated traces before you
draw conclusions about context.

Response-side tool-call coverage is measured separately from request coverage.
`analyze-trace` reports batch count, aggregate and normalized call counts,
largest batch, bounded-argument truncations, and whether per-call response
evidence is complete. Stream progress counts remain progress evidence only;
they never create executable calls.

Every successful read of an unchanged file is rendered as one authoritative,
event-sourced projection in provider context. Version-three projections put
the content in a line-numbered text view (`1|content`) and explicitly report
whether it is `complete` or `partial`; partial projections name exact missing
ranges. Later reads update that same path-and-mutation-epoch view instead of
adding another provider-visible read result. The content bound scales with the
actual provider window and is capped, while raw read events remain unchanged in
the trace only. Read-only shell effects do not split a file epoch, but confirmed
workspace mutations do.

## Independent verification

Independent verification is optional and harness-neutral. Supply a
`transcript_evidence.v1` JSON file with `--evidence`; entries match sessions by
canonical trace path or transcript label. Replicate directories shaped like
`rNN-<arm>` supply stable labels for arbitrary arm names. The complete schema
and fidelity rules are documented in [`TRACE_SCHEMA.md`](TRACE_SCHEMA.md).
