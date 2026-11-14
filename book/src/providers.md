# Providers and models

Microgentic supports four local providers:

| Provider | `--provider` value |
| --- | --- |
| [oMLX](https://github.com/jundot/omlx) | `omlx` |
| [Ollama](https://ollama.com) | `ollama` |
| [vLLM](https://docs.vllm.ai) | `vllm` |
| [LM Studio](https://lmstudio.ai) | `lmstudio` |

The default provider is oMLX. Choose your default with `microgentic setup`, or
for one run with `--provider`.

vLLM and LM Studio have limits that the other providers do not have. Read
[vLLM](#vllm) or [LM Studio](#lm-studio) before you use one of them.

## Server addresses and credentials

Microgentic reads server addresses and credentials from environment variables.
Setup does not store them.

| Variable | Meaning | Default |
| --- | --- | --- |
| `OMLX_HOST` | oMLX server URL, without `/v1` | `http://localhost:8000` |
| `OMLX_API_KEY` | Bearer token for an authenticated oMLX server | Not set |
| `OMLX_TIMEOUT` | Timeout for calls that do not stream, in milliseconds | `600000` |
| `OLLAMA_HOST` | Ollama server URL | `http://localhost:11434` |
| `VLLM_HOST` | vLLM server URL, without `/v1` | `http://localhost:8000` |
| `VLLM_API_KEY` | Bearer token for a vLLM server started with `--api-key` | Not set |
| `LMSTUDIO_HOST` | LM Studio server URL, without `/v1` | `http://localhost:1234` |
| `LM_API_TOKEN` | Bearer token for an LM Studio server that requires authentication | Not set |

For oMLX, vLLM, and LM Studio, the gateway adds `/v1` to the URL. Microgentic
sends no `Authorization` header when the key variable is empty or not set.
vLLM and oMLX have the same default address. If you run both, set one of the
host variables.

Calls to vLLM and LM Studio that do not stream have a timeout of 10 minutes.
No variable changes it. Streamed replies have no fixed timeout.
Microgentic decides how long to wait from the model's measured throughput, so a
slow local model gets time to finish.

## Models

| Provider | Default model |
| --- | --- |
| oMLX | `Qwen3.8-27B-MLX-8bit` |
| Ollama | `qwen3.8:27b-mxfp8` |
| vLLM | `Qwen/Qwen3.8-27B` |
| LM Studio | `qwen/qwen3.8-27b` |

A vLLM server gives a model the name that you set when you start the server.
An LM Studio server names a model by its key. If your server uses a different
ID, give that ID to Microgentic.

To use another installed model, give its ID to `setup`, to `run --model`, or to
`/model` in the conversation. `/model` with no ID shows the models the provider
reports.

## Reasoning effort

The effort levels are `disabled`, `enabled`, `low`, `medium`, `high`, and
`xhigh`. If you do not set an effort, the model uses its own default.

The provider receives the effort in its own form:

| Effort | oMLX | Ollama | vLLM | LM Studio |
| --- | --- | --- | --- | --- |
| Not set | Nothing | Nothing | Nothing | Nothing |
| `disabled` | `enable_thinking: false` | `think: false` | `chat_template_kwargs.enable_thinking: false` | `reasoning_effort: "none"` |
| `enabled` | `enable_thinking: true` | `think: true` | `chat_template_kwargs.enable_thinking: true` | Nothing |
| `low`, `medium`, `high`, `xhigh` | `reasoning_effort` | `think` set to the level name | `reasoning_effort` | `reasoning_effort` |

The real effect depends on the model's chat template.

## Output and context limits

These `run` flags control request size:

| Flag | Default | Effect |
| --- | --- | --- |
| `--max-tokens` | 16384 | Maximum generated tokens per request, including reasoning |
| `--context-window-tokens` | 262144 for the default oMLX and Ollama models | Context window for Microgentic's accounting; Ollama also receives it as `num_ctx` |

The output limit is also available for new conversations and resumed sessions:

```sh
microgentic --max-tokens 32768
microgentic session resume SESSION_ID --max-tokens 32768
microgentic run --max-tokens 32768 --prompt 'Continue the implementation'
```

To make 32K the default for new conversations and everyday runs, use:

```sh
microgentic setup --max-tokens 32768 --apply
```

This saves `max_tokens` in the settings file. Explicit flags override it.
Resumed sessions retain their saved limit unless you provide an override.
The limit includes reasoning, answer text, and tool calls; increasing it can
allow a longer reasoning pass. Benchmark mode ignores personal defaults.

For oMLX, vLLM, and LM Studio, set the real context limit on the server. Only
Ollama receives `num_ctx`.

Microgentic asks the provider for the model's context window and uses it for
accounting:

| Provider | Source of the window |
| --- | --- |
| oMLX | `max_model_len` from `/v1/models` |
| Ollama | The `num_ctx` parameter or `<arch>.context_length` from `/api/show` |
| vLLM | `max_model_len` from `/v1/models` |
| LM Studio | The loaded instance's `config.context_length` from `/api/v1/models`. If the model is not loaded, its `max_context_length` |

The default vLLM and LM Studio models have no built-in window. If the server
does not answer, the window is unknown unless you give
`--context-window-tokens`.

When the provider reports an output limit, Microgentic stops with an explanation
and reports the generated token count when available. Reasoning can consume the
whole allowance before any answer or tool call appears. Partial output remains
incomplete, and unfinished tool calls are not executed.

Try a smaller task, or rerun with a larger `--max-tokens` value. To reduce
thinking, try `--reasoning-effort low` or `--reasoning-effort disabled`. Support
depends on the model. Microgentic does not automatically retry the request or
change these settings. In the conversation, see
[Output token limit](conversation.md#output-token-limit).

## vLLM

Microgentic sends OpenAI chat completion requests to `/v1/chat/completions` on
the vLLM server. It asks for token usage on each streamed reply.

Start the vLLM server with tool calling enabled for your model. Microgentic
needs tool calls for all work. Refer to the vLLM documentation for the
`--enable-auto-tool-choice` and `--tool-call-parser` options.

**Limit: Microgentic does not see vLLM reasoning text.** Current vLLM versions
return reasoning in a field named `reasoning`. The Mojentic 3.0.1 gateway reads
only `reasoning_content`. As a result, with vLLM:

- The conversation does not show thinking.
- Traces and context accounting contain no reasoning.
- The limits on thinking without an answer do not apply.
- `--preserve-reasoning` has nothing to keep.

Answers and tool calls are not affected. The model can still reason, and its
reasoning still uses output tokens. A correction needs a Mojentic release that
reads `reasoning`.

## LM Studio

Microgentic sends OpenAI chat completion requests to `/v1/chat/completions` on
the LM Studio server. It asks for token usage on each streamed reply. Use
LM Studio 0.4.8 or later. LM Studio supports `reasoning_effort` on this
endpoint from 0.4.8, and the context window query from 0.4.0.

To see reasoning, set this option in LM Studio, in App Settings, Developer:
"When applicable, separate reasoning_content and content in API responses".
If the option is off, LM Studio puts the reasoning in the answer text.
Microgentic then cannot show it as thinking or count it as reasoning.

LM Studio does not document the `reasoning_effort` values that this endpoint
accepts. Microgentic sends `none` for `disabled`, and sends the level name for
`low`, `medium`, `high`, and `xhigh`. A model or an LM Studio version can
ignore or reject a value. For `enabled`, Microgentic sends no effort, and the
model uses its default.

LM Studio does not document `chat_template_kwargs` for this endpoint, so
Microgentic does not send it. `--preserve-reasoning` still sends earlier
reasoning back as `reasoning_content` in the message history.

## Settings file

Setup saves its choices in `$XDG_CONFIG_HOME/microgentic/config.json`, or in
`~/.config/microgentic/config.json` when `XDG_CONFIG_HOME` is not set. Use
`--config <path>` to read a different file. Flags on a command override the
saved values.

Benchmark mode and the `adaptive-agent-harness` compatibility executable do not
read this file.

## Advanced context settings

Two settings exist for experiments that measure how much standing context a
model handles well. They are not in setup or the conversation view. Set them in
`config.json`, or with the matching flag for one run. A flag overrides the file,
and the file overrides the default.

| Key in `config.json` | Flag | Default | Effect |
| --- | --- | --- | --- |
| `standing_context_limit` | `--standing-context-limit` | 0.15 | All standing context (project guidance now) as a fraction of the context window. Traces record each request against it. It is not enforced yet. |
| `guidance_fraction` | `--guidance-fraction` | 0.125 | The project guidance budget as a fraction of the window, between 500 and 8,000 tokens |

Each value must be above 0 and at most 1. Traces record the effective values
beside each context ledger. Benchmark mode ignores the file and records a flag
only when you give it.

## Native reasoning

The Mojentic gateways represent native assistant reasoning separately from
content, keep named Ollama effort values distinct, and support xhigh. Coding
requests use temperature `1.0`.

Use `--reasoning-profile qwen3.8 --reasoning-effort medium` to validate the model's
supported controls before HTTP. The profile accepts disabled, low, medium and
xhigh; high is rejected rather than silently translated. Omitted effort still
means an unknown provider default. An omitted profile delegates capability
validation to the server; unknown model names are never matched heuristically.

`--preserve-reasoning` opts into native completed-response history. It is off by
default so retention can be varied independently from effort. The worker keeps
reasoning with its assistant tool request or final response, includes its size
in context accounting, and exposes it in exact snapshots and native transcripts.
File-read consolidation retains reasoning in its original assistant turn while
projecting the tool evidence separately; it does not attach reasoning to an
invented tool request. Existing context rewriting policies still apply.

Cancelled/capped partial chains remain separate from completed native history;
`--reasoning-checkpoint-tokens` retains its independent experimental meaning.
For oMLX and vLLM, the request includes chat_template_kwargs.preserve_thinking=true
when native retention is enabled. For LM Studio it does not. Forced server settings can override that request, so
benchmark registration must record server settings too. Sampling, stop budgets,
and advisory defaults are unchanged by these flags.
