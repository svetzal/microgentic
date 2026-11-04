# Saved sessions

You can quit the conversation and reopen it later:

```sh
microgentic session list
microgentic session resume session-AbCd12
```

Use an ID from the list. Resume opens the saved project and waits for your next
message. It restores completed messages, tool results, model, reasoning effort,
output token limit, and the agent-file selection. It does not run tools or
continue a stopped generation automatically.

To change the output limit while keeping the conversation, use
`microgentic session resume SESSION_ID --max-tokens 32768`. The new limit is
retained for later resumes, even if you quit before sending another message.

## Find a conversation

`session list` shows conversations for the current project, newest first. Each
entry includes an intent sentence, ID, last activity in UTC, state, project,
and model. The states are `active`, `ready`, and `stopped`.

To include other projects, use:

```sh
microgentic session list --all
```

To read the metadata in a script, use:

```sh
microgentic session list --all --json
```

The list reads saved data only. It does not contact a provider or change the
session files. Unreadable or incompatible sessions are skipped with a warning
on stderr.

## Intent sentences

After an exchange, Microgentic tries to summarize the apparent intent using
the session's model while the TUI is idle. This is a separate, bounded request
with thinking disabled and no tools. It uses excerpts of user messages, with
known active credentials removed. Its result labels the session; it never
becomes an instruction or conversation message.

The sentence is cached beside the conversation. New user messages allow an
updated sentence. Listing sessions never generates summaries. If a summary is
unavailable, the list shows the first user message instead.

Starting another request or leaving the TUI cancels its pending local summary work.
A missing summary does not prevent resume. Summary requests and their outcomes
are recorded in the session's `intent-traces/` directory.

## Resume safely

You can resume from a directory other than the saved project. Microgentic uses
the saved project and model settings rather than your current setup defaults.
The project and any saved agent file must still exist. Agent files and project
guidance are read again, so their current contents apply.

Earlier tool results appear as saved observations. The workspace may have
changed while the conversation was closed. Each resumed exchange starts with
fresh validation state, and the model is told to inspect the project before
relying on earlier results. Completed tool calls are not replayed. Partial
replies are excluded from the retained conversation.

Only one TUI can own a session. Resume refuses a session that is already open
or still has a running worker. Closing the owner releases its lock; a crashed
process does not leave a permanent PID lock.

Older saved conversations can be resumed too. They may lack an intent sentence
or agent-file selection. Supply an agent explicitly when needed:

```sh
microgentic session resume session-AbCd12 --agent rust-craftsperson
```

Older releases did not record TUI ownership. Close an older TUI before reopening
its saved conversation.

An engineering task's internal execution state, pending approvals, and
unfinished provider stream are not restored. Describe the next step in your
message after reopening.
