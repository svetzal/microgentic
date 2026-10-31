# Commands

Every command has `--help`, with examples.

## Everyday commands

| Command | What it does |
| --- | --- |
| `microgentic` | Open the conversation view in the current directory |
| `microgentic setup` | Show the provider, model, and effort settings it would save |
| `microgentic setup --apply` | Save those settings |
| `microgentic run` | Send one prompt from stdin or `--prompt`, and write the reply to stdout. Can edit the project |

All commands accept `--config <path>` to read settings from a different file.

`microgentic run --agent <name|path>` loads an agent file that describes how to
approach the work. See [Agent files](agents.md).

## Traces

| Command | What it does |
| --- | --- |
| `microgentic analyze-trace <paths>` | Summarize context assembly and tool payload size from JSONL traces |
| `microgentic render-transcript <paths> --output <file>` | Render traces as one self-contained HTML transcript |

See [Traces and transcripts](traces.md).

## Research commands

These commands support registered experiments. Everyday use does not need them.

| Command | What it does |
| --- | --- |
| `microgentic benchmark run` | Run an isolated task contract with historical defaults and JSON output |
| `microgentic benchmark run-sequential` | Run bounded attempts against one retained artifact |
| `microgentic run-sequential` | The earlier advanced form of sequential runs |
| `microgentic resolve-contract` | Resolve and print a run contract without a provider |
| `microgentic summarize-matrix` | Reproduce the preserved benchmark baseline counts |
| `microgentic plan-task` | Propose a measurement-only acceptance checklist |
| `microgentic plan-interactions` | Propose measurement-only scenarios over an acceptance plan |
| `microgentic author-interaction-evidence` | Author evidence for one interaction candidate |

`run --help` also lists advanced research flags, such as model interaction
budgets, reasoning interruption, and request recovery. They are for registered
experiments.

See [Benchmark mode](benchmark.md).

## Exit status

`microgentic run` reports how the run ended in its exit status. The same
outcome is the `outcome` field of the `--json` summary and, when the run did
not succeed, one `microgentic:` line on stderr.

| Code | Outcome | Meaning |
| --- | --- | --- |
| 0 | `replied`, `done` | An everyday reply finished, or the runtime accepted a task's `DONE` |
| 1 | `error` | A provider, environment, or internal error. The run has no summary |
| 2 | `usage` | Bad command-line input. Nothing ran |
| 3 | `failed` | The model returned `FAIL` |
| 4 | `withheld` | The model claimed completion, a gate withheld it, and the iteration budget ran out. The gate is `stash`, `validation`, `probes`, or `acceptance_ledger` |
| 5 | `hard_stop` | A runtime rule stopped the run. The cause is `action_boundary`, `thinking_only`, `empty_response`, `repair_no_action`, `repair_depth`, `inspection_loop`, `no_content_stream`, or `iteration_budget` |
| 128 + signal | `cancelled` | A termination signal ended the run: 130 for SIGINT, 143 for SIGTERM, 129 for SIGHUP |

A `DONE` from the model does not set exit 0 on its own. The runtime's gates
accept it, and only then is the outcome `done`. Exit 0 says the harness
finished cleanly. It does not say an independent verifier passed.

`microgentic benchmark run`, `benchmark run-sequential`, `run-sequential`, and
the `adaptive-agent-harness` executable keep their earlier behaviour: 0 when
they produce a summary, 1 on an error. Their JSON summaries carry the same
`outcome` field, and so does each attempt record of a sequential run, so an
adapter reads the outcome from the summary or from the trace's `run.finished`
event instead of the exit status. See [Benchmark mode](benchmark.md).

Every other command exits with 0 when it succeeds, 1 on an error, and 2 on a
usage error.
