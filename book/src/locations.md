# Files and locations

Microgentic keeps its files outside your project. It follows the XDG base
directory conventions.

| What | Location |
| --- | --- |
| Settings | `$XDG_CONFIG_HOME/microgentic/config.json`, or `~/.config/microgentic/config.json` |
| Personal guidance | `$XDG_CONFIG_HOME/microgentic/AGENTS.md`, or `~/.config/microgentic/AGENTS.md` |
| Agent files for `--agent` | `$XDG_CONFIG_HOME/microgentic/agents/`, or `~/.config/microgentic/agents/`, then `~/.claude/agents/` |
| Runs, traces, and conversations | `$XDG_STATE_HOME/microgentic/runs`, or `~/.local/state/microgentic/runs` |

Each `run` and each conversation has its own directory below `runs`. A
conversation is saved as `session-*/conversation.json`, with private file
permissions. Traces stay on disk when you start a new conversation.

Each session may also contain `intent.json`, its cached intent sentence, and
`intent-traces/`, the advisory summary requests. `owner.lock` and `worker.lock`
protect active sessions. Their files can remain after exit; the operating
system releases the locks when their owners close or exit.

## Credentials

Microgentic does not save credentials. It reads `OMLX_API_KEY` from the
environment. Traces and provider snapshots do not contain credentials for the
known provider variables.

## Remove everything

After `brew uninstall microgentic`, delete these directories to remove all
settings and history:

```sh
rm -rf ~/.config/microgentic ~/.local/state/microgentic
```

If you set `XDG_CONFIG_HOME` or `XDG_STATE_HOME`, delete the `microgentic`
directory below each of them instead.
