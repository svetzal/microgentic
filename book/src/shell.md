# Shell use with `run`

`microgentic run` sends one prompt and writes the reply to stdout. It never
opens the conversation view, so it works in pipes, scripts, and redirections.

## Give the prompt

Give the prompt with `--prompt`, or on stdin:

```sh
microgentic run --prompt 'Explain this project'
echo 'Explain this project' | microgentic run
microgentic run < task.txt
```

When you give `--prompt` and also pipe data to stdin, the prompt is the
instruction and stdin is reference material, for example a diff or a build
log:

```sh
git diff --no-ext-diff | microgentic run --prompt 'Review this diff' > review.md
microgentic run --prompt 'Explain this build failure' < build.log
```

Commands in the prompt and in reference material are text for the model to
read. They do not become validation probes.

These inputs fail before any provider call:

- an empty prompt, or a prompt that has only whitespace
- more than one `--prompt`
- a prompt given as a positional argument
- stdin that is not valid UTF-8, or that is larger than 1 MiB
- no `--prompt` when stdin is a terminal

## How a run ends

A run is one exchange, like one message in the conversation view. The prompt
reaches the model as you wrote it. A greeting gets a greeting, and a question
gets an answer.

The model can use its tools, and the run continues while it does. When you ask
for a change, it can read files, edit them, and run a check before it replies.
The first reply that has text and no tool calls ends the run. Before it ends,
the runtime still asks the model to check unvalidated source changes and to fix
a failed check.

The model does not have to end with DONE or FAIL, and the run does not ask it
to continue after it replies.

A termination signal also ends the run: SIGINT from Ctrl-C, SIGTERM from
`kill`, or SIGHUP when the terminal closes. The model request is cancelled, a
project command that is still running is killed, and the run exits with the
signal's conventional status, 128 plus the signal number. If the work has not
stopped two seconds later, or a second signal arrives, the process exits at
once with the same status.

`run --experiment`, `--goal` or `--contract` starts a task run instead. Task
runs keep the task instructions, the DONE/FAIL ending, and declared probes.
See [Benchmark mode](benchmark.md).

## Get the result

The reply goes to stdout, without blank lines before or after it. The streamed
reply, progress, and diagnostics go to stderr. Redirect them separately:

```sh
microgentic run < task.txt > result.txt 2> diagnostics.log
microgentic run --prompt 'Summarize the public API' >> summaries.txt
```

`--json` writes the complete run summary as JSON instead of the reply text.
Its `outcome` field says how the run ended: `{"kind": "replied"}`,
`{"kind": "done"}`, `{"kind": "failed", "reason": "…"}`,
`{"kind": "withheld", "gate": "…", "detail": "…"}`, or
`{"kind": "hard_stop", "cause": "…", "detail": "…"}`. The other fields are
unchanged.

## Exit status

The exit status says how the run ended, so a script can branch on it without
reading the reply:

| Code | Meaning |
| --- | --- |
| 0 | The reply finished, or the runtime accepted a task's `DONE` |
| 1 | A provider, environment, or internal error |
| 2 | A usage error |
| 3 | The model returned `FAIL` |
| 4 | A gate withheld the claimed completion until the iteration budget ran out |
| 5 | A runtime rule stopped the run |
| 128 + signal | A termination signal ended the run: 130, 143, or 129 |

When the status is not 0, stderr has one line that starts with `microgentic:`.
It names the outcome and gives the trace path:

```text
microgentic: FAIL returned: FAIL the fixture file is missing; evidence: /Users/you/.local/state/microgentic/runs/…/trace.jsonl
```

stdout still has the reply, or the JSON summary with `--json`.

```sh
if microgentic run --prompt 'Fix the parser' > reply.md; then
  echo 'finished'
else
  case $? in
    3) echo 'the model returned FAIL' ;;
    4|5) echo 'the run stopped before it finished; see the trace' ;;
    *) echo 'the run did not complete' ;;
  esac
fi
```

See [Commands](commands.md#exit-status) for the outcome names and the gates
and causes behind codes 4 and 5.

## Choose the project

`--dir` sets the project root for file and shell tools. The default is the
current directory.

```sh
microgentic run --prompt 'Fix the parser' --dir ./project
```

Run inputs and traces are kept outside the project, in the run-state directory.
Each run has its own directory. See [Files and locations](locations.md).

## Settings for one run

`run` uses your saved setup defaults. These flags override them for one run:

| Flag | Effect |
| --- | --- |
| `--provider omlx\|ollama` | Use this provider |
| `--model <id>` | Use this model |
| `--reasoning-effort <level>` | Use this reasoning effort |
| `--no-project-guidance` | Do not read `AGENTS.md` or `CLAUDE.md` |
| `--config <path>` | Read settings from a different file |

If you change the provider and do not give `--model`, the run uses that
provider's default model.

## Limits

- `run` can edit files and run commands in the project. Preview and approval
  are not available yet.
- Exit 0 means the harness finished cleanly. It does not mean an independent
  check passed. The summary does not yet list changed paths or the checks
  that ran.
- Streamed JSONL output is not available yet.
