# Agent files

An agent file describes how to approach the work: as a careful reviewer, as a
Rust craftsperson, as a documentation writer. Use `--agent` to load one into
the model's system context for a run.

```sh
microgentic --agent rust-craftsperson
microgentic run --agent rust-craftsperson --prompt 'Add a --verbose flag'
microgentic run --agent ./agents/reviewer.md --prompt 'Review this diff' < changes.diff
```

In the conversation, give `--agent` when you open it. It applies to every
message until you close the view. See [The conversation](conversation.md).

Agent files use the same format as Claude Code agent definitions, so the files
in `~/.claude/agents/` work as they are.

## Name or path

A value that contains `/` or ends in `.md` is a path, relative to the current
directory. Any other value is a name. Microgentic looks for `<name>.md` in
these directories, in order:

1. `agents/` beside your settings file:
   `$XDG_CONFIG_HOME/microgentic/agents/`, or `~/.config/microgentic/agents/`.
   With `--config <path>`, the `agents/` directory beside that file.
2. `~/.claude/agents/`.

If no file has that name, the error lists the directories it searched and the
agent names it found in each.

## The file

An agent file is Markdown. It can start with YAML frontmatter between two
`---` lines:

```markdown
---
name: careful-reviewer
description: Reviews changes before they land.
tools: Read, Grep
model: opus
---

Read every changed line before you judge it. Name the risk, then the fix.
```

Microgentic reads only `name`. Without it, the file name (without `.md`)
names the agent. Microgentic ignores all other keys, such as `tools`, `model`,
and `description`, and the trace lists them.

## Information, not authority

Like [project guidance](guidance.md), an agent file is information. It cannot
grant permissions, change the tools, override validation, select the model, or
change the stop rules. Microgentic sets those before it reads the file. That
is why the `tools` and `model` keys have no effect.

The agent file comes before project guidance in the system context. When the
two disagree, project guidance is more specific, and the model is told to
follow it.

`--agent` and `--no-project-guidance` are independent. You can use either one
or both.

## Who gets the file

`run` hands engineering work to an engineering subagent through its
`start_task` tool. The subagent does the work, so it gets the whole agent
file. The coordinator, the part of `run` that talks to you and writes the
brief, gets only one line that names the agent. This keeps its brief small.

The conversation does its work itself, so each message carries the whole
agent file.

## Size limit

The body of an agent file is limited to 6,000 estimated tokens. With a smaller
known context window the limit is one quarter of that window, and never less
than 500 tokens. This limit is separate from the project guidance limit.

Microgentic does not cut an agent file. If the body is too large, the run stops
before it starts. The error names the file, its estimated size, and the limit.
Use a shorter agent file, or a model with a larger context window.

The conversation checks the file against the model's known window before the
view opens. Each message checks it again against the window the provider
reports, so a message can fail with the same error if the window is smaller
than expected, or after `/model` selects a smaller model.

## See what loaded

In the conversation, the status bar shows `🎭` and the agent's name. `/agent`
reads the file again and shows its name, path, size, estimated tokens, and
the frontmatter keys Microgentic ignores. If you edit the file, the next
message uses the new text.

Each run's trace has a `context.agent.loaded` event. It records the file's
path, name, SHA-256, size, estimated tokens, the ignored frontmatter keys, and
whether that loop got the whole file or the one-line note. See
[Traces and transcripts](traces.md).

## Where `--agent` is not accepted

`microgentic benchmark run`, `benchmark run-sequential`, `run-sequential`, runs
with `--experiment`, `--goal`, or `--contract`, and the
`adaptive-agent-harness` compatibility executable refuse `--agent`. Their
provider requests do not change. There is no `config.json` setting for an
agent file: give `--agent` each time you want one.
