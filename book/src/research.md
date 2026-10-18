# Research controls

These controls serve registered experiments and provider diagnostics. They are
off by default, and everyday use does not need them. Settings that change what
the model sees or when a run stops belong in a registered experiment, so that
the result can be compared with others.

## Request recovery

Recovery is disabled by default, with one wire attempt per completion. Opt in
explicitly; wire attempts do not consume model interactions or replay tools:

```bash
microgentic run --experiment /work/new-cell --provider ollama --json \
  --recover-requests --recovery-max-attempts 2 \
  --recovery-base-delay-ms 100 --recovery-delay-ceiling-ms 30000 \
  --recovery-budget-ms 60000 \
  --recovery-admission-command /opt/operator/owned-receipt \
  --recovery-capture-dir /work/private-wire
```

The trusted admission executable must be outside the model tool root. It receives
safe failure metadata and the immutable encoded request SHA-256 on stdin and must
return proof bound to the previous attempt and logical request. A plain allow
flag, aggregate activity, cancellation or model residency cannot admit a retry.
Pending admission is cancellable and expires after five seconds. Semantic partial
output ends with interruption and cannot be continued by another wire attempt.

`--recovery-capture-dir` is also allowed without `--recover-requests`, providing
explicit one-attempt capture. Credential checks cover each reasoning/content
channel and partial tool argument, name and ID fields, including JSON-escaped
values. Unrelated content cannot release a held reasoning credential prefix.
Captures exclude headers, withhold credential echoes
and label redaction and exactness. SIGINT and SIGTERM persist terminal lifecycle
and failure evidence before exit. Recovery budgets begin after failure; they are
not total-generation timeouts.

The admission executable is canonicalized outside the model tool root and
invoked directly, without a shell. It reads this JSON on stdin:

```json
{
  "failure": {
    "identity": {
      "logical_request_id": "<UUID>",
      "attempt_id": "<UUID>",
      "wire_attempt": 1
    },
    "http_status": 503,
    "provider_request_id": "<validated ID or null>",
    "progress": { "observed": {}, "delivered": {} }
  },
  "request_sha256": "<SHA-256 of the immutable encoded HTTP body>"
}
```

It must answer on stdout with proof bound to that failure:

```json
{
  "decision": "allow",
  "evidence_kind": "not_forwarded",
  "previous_attempt_id": "<exact previous UUID>",
  "logical_request_id": "<exact logical UUID>",
  "provider_request_id": "<exact input ID or null>",
  "request_sha256": "<exact input SHA-256>"
}
```

`exact_request_terminated` is the other `evidence_kind`, for an operator with
proof that the exact request terminated. Every binding must match. A missing or
ambiguous proof, a plain allow, a digest mismatch, a nonzero exit, malformed or
oversized output, a timeout, or a credential echo rejects admission. The child
has a five-second bound, a 16 KiB output limit and discarded stderr, and it is
killed on cancellation.

## Native initial-context assembly

Every run constructs one authoritative initial-context packet before the
worker starts. By default that packet contains the resolved task and profile
guidance. `run` and `run-sequential` can also accept an adapter-owned guidance
catalog:

```sh
cargo run -- run \
  --experiment ./cell \
  --initial-context-catalog context-catalog.json \
  --semantic-advisor-model Qwen3.8-27B-MLX-8bit
```

Catalog records have one harness-enforced disposition:

- `required` records always enter the worker packet and never enter the
  advisory call.
- `selectable` records are the only records visible to an isolated structured
  semantic advisory.
- `excluded` records must omit content and enter neither model packet.

The advisory has no tools or mutation authority. It proposes optional record
IDs; the initial-context assembler validates schema, disposition, IDs,
uniqueness, confidence, selection count, and the combined required-plus-selected
guidance budget. Invalid proposals fail closed. The trace records exact advisory
inputs and outputs plus the assembler's authoritative components and decision.
`max_advisory_chars` bounds the complete advisory request, including the task,
instructions, and selectable records; oversized calls are rejected before the
provider is invoked.

```json
{
  "schema_version": "initial_context_catalog.v2",
  "max_selected": 2,
  "max_total_guidance_chars": 6000,
  "max_advisory_chars": 20000,
  "min_confidence": 0.75,
  "records": [
    {
      "id": "release-safety",
      "disposition": "required",
      "description": "Release claims require observed evidence",
      "content": "Never claim validation that was not observed.",
      "source": "release-safety.md"
    },
    {
      "id": "release-format",
      "disposition": "selectable",
      "description": "Release-note layout used only for release tasks",
      "content": "Use a title followed by exactly two bullet points.",
      "source": "release-format.md"
    },
    {
      "id": "private-roadmap",
      "disposition": "excluded",
      "description": "Material prohibited from worker context",
      "source": "private-roadmap.md"
    }
  ]
}
```

Semantic advisories are a reusable proposal-only harness effect with explicit
kinds for initial-context selection, situation analysis, and failure
classification. Initial-context selection is the first active consumer. When
the catalog is omitted, or contains no selectable records, no advisory call is
made. The semantic-selection policy is experimental; native context assembly is
not.

## Acceptance coverage ledger

`run --acceptance-ledger` enables an experimental pre-worker planning phase.
The isolated advisor first produces a deterministically validated atomic
acceptance plan, then a validated set of high-risk interactions between those
requirements. The accepted entries are rendered once into the retained worker
packet with their public provenance and suggested evidence.

The treatment adds `submit_acceptance_evidence`. The worker may cite one
observation for several stable ledger IDs. A submission records coverage at the
current workspace-mutation epoch; a later evidence-invalidating mutation makes
that coverage stale. `DONE` is withheld until every atomic and interaction ID
has current coverage.

Ledger coverage is explicitly advisory. It does not convert model-authored
tests, inspections, or claims into validation authority. Adapter-declared
probes retain their existing terminal authority and must still pass after the
latest relevant mutation. If either planning stage exhausts its bounded
structured-output attempts, the treatment run fails closed rather than
silently falling back to a control packet.

## Bounded reasoning continuity

`run --reasoning-checkpoint-tokens N` enables an experimental continuity aid
for hidden-only no-action turns. The harness retains at most the approximate
`N`-token tail of the model's own hidden reasoning and supplies that tail once
in the next ordinary user turn. The checkpoint is explicitly labeled as
incomplete, self-generated continuity state rather than task authority.

The default is zero, which preserves the prior prompt and provider request.
Enabling the treatment does not disable reasoning or narrow the tool surface.
Only the newest checkpoint is retained. Trace events record size, omitted
prefix size, and a SHA-256 digest; the exact checkpoint remains available in
the provider-request snapshot rather than being duplicated into summary
events.

## Declared probe delivery

Adapter-owned probes in an explicit run contract are part of the initial worker
packet and terminal authority. Both command probes and artifact assertions are
rendered by stable ID. Their executor-owned command bytes or expected content
stay outside model-facing provider requests. The worker invokes either kind
through `execute_probe`; the returned evidence identifies the probe and command
digest without exposing its registered implementation. The worker is told not
to substitute a synthetic approximation. After a relevant mutation, every
declared probe must pass in the current mutation generation before `DONE` is
accepted.

When a resolved contract declares probes, the harness adds `execute_probe` to
the active model-facing tool set even when the selected domain profile does not
include it by default. This keeps probe reachability tied to the contract while
leaving probe-free runs and their tool surfaces unchanged.

Declared command identity remains stable throughout repair handling. Runtime
repair state and every model-facing repair or escalation prompt use
`probe:<id>` and `declared_command_probe`; the registered command bytes remain
executor-owned even after repeated failures.

Command probes use the same scoped shell executor, finite timeout, process-group
cleanup, mutation sensing, repair policy, and lifecycle-phase tracing as other
shell effects. Their declared stable ID, rather than inferred command text,
grants validation authority. Model-authored `shell_command` calls are opaque
observations: neither a familiar command name nor a zero exit status can clear
pending evidence or trigger terminal readiness. A probe-free contract therefore
does not invent a self-validation gate; independent evaluation remains separate.

The optional `--repair-handoff-policy constrained` mode gives an authoritative
failed declared probe invoked through `execute_probe` immediate control of the
next provider request. It ends the in-flight tool turn, removes superseded
agent-loop user guidance, places the validation-repair action contract after
the failure evidence, and limits the next tool surface to `write_file`,
`edit_file`, `shell_command`, and `execute_probe`. Model-authored
`shell_command` failures do not activate this authority. The default
`text-only` mode preserves the legacy continuation behavior so the two policies
can be compared on one pinned binary.

The `constrained-action-only` variant keeps that first bounded repair request.
If the request reaches its repair thinking cap without an action, the following
request uses native `enable_thinking=false` (oMLX) or `think=false` (Ollama),
retains the authoritative
failure packet and restricted repair tools, and explicitly states that the
interrupted hidden reasoning was not retained. The native action-only request is
reserved for this post-validation repair boundary. Before source mutation,
ordinary reasoning and tool use remain interleaved; hidden-only no-action turns
receive bounded continuation guidance rather than a forced native mode switch.
The action-only provider snapshot records `reasoning_effort: disabled`,
`thinking_disabled: true`, and the effective cap source `provider_disabled`.
The ordinary `constrained` variant retains the configured worker reasoning effort.

Every `llm.provider_request.assembled` snapshot records the harness-side
thinking limits that govern that request. The `harness_limits` object includes
the ordinary and repair caps, whether validation repair is active, and the
effective cap and source. A failed declared probe activates repair limits
immediately for any following request in the same tool turn as well as for a
constrained handoff.

Probe delivery is trace-visible as `agent.contract.probes.delivered`, including
the delivered IDs, probe kinds, and resulting worker-message size. Empty probe
lists leave the worker message unchanged and emit no delivery event. Probe
expectations remain adapter-owned: model-authored semantic advisories may
propose candidate evidence, but they do not gain terminal authority without
deterministic contract policy.

## Controlled provider diagnostics

The `provider_benchmark` example exercises both Mojentic gateways without the
adaptive coding policy. It provides a long-prefix reuse probe and a sequential
receipt-tool session. It saves exact requests, per-call timings and native
traces. By default, reasoning is disabled and each response is capped
at 128 tokens.

Build with `cargo build --release --locked --example provider_benchmark`, then
run `target/release/examples/provider_benchmark CONFIG.json` under the workspace
memory/ownership guard. Example configuration:

```json
{
  "provider": "omlx",
  "model": "Qwen3.8-27B-MLX-8bit",
  "workload": "tool_chain",
  "nonce": "unique-session-id",
  "output": "/absolute/path/to/new-output-directory",
  "prefix_lines": 256,
  "rounds": 24
}
```

Use `prefix` for unchanged, suffix-changed and early-changed prefix probes.
Use `ollama` with its model ID to select the other gateway. Host/authentication
settings come from the gateways' environment variables. The output directory
must not already exist. The receipt tool has no shell or file access.

Cache counters have provider-specific meanings; gateway action
latency is not automatically server prefill time. Unload between sessions does
not clear persistent caches. A semantic failure is recorded in result.json;
process success alone does not mean the task passed.

The diagnostic config also accepts `reasoning_effort` (default disabled, null
for provider default), `preserve_reasoning` (default false), `max_tokens`
(default 128, range 1–8192), `temperature` (default 0), and `reasoning_profile`.
The profile is a library capability object, for example:

```json
{
  "name": "qwen3.8",
  "supported_efforts": ["disabled", "low", "medium", "xhigh"],
  "supports_reasoning_history": true
}
```

Reasoning is permitted by semantic checks only when the configured effort
allows it; the same receipt/order/final-answer checks still apply. Current
production coding runs use oMLX; the controlled diagnostic can select either
provider. Register changed settings before running a new measured cell.

## Local benchmark activity counters

`benchmarks/provider_activity.py` interprets the verified oMLX 0.7 status API.
Its active count includes output collectors for waiting requests. One queued
request can therefore report both active=1 and waiting=1; these counts must not
be added or treated as separate clients. Multiple-request evidence is
`max(active, waiting) > 1`, while idle requires both zero. This helper does not
establish request ownership or replace model, memory and admission checks.
Run its deterministic tests with:

```sh
python3 -m unittest discover -s benchmarks -p 'test_provider_activity.py' -v
```

## External runner progress

Missing or incomplete process, residency, or accelerator telemetry remains
`ProgressUnknown` after the throughput projection expires. A quiet stream is
not sufficient to cancel inference. Stall confirmation requires error-free,
affirmative inactivity on every sampled activity channel, followed by repeated
checks beyond the projection. Current process/GPU samplers do not establish
all of those negatives for an external provider, so automatic stall cancellation
fails closed there. This does not prove remote termination or authorize a retry.

`fixtures/progress/worker_pause.py` exercises the public worker with scripted
Ollama and oMLX streams pausing 390 seconds before resuming. It asserts retained
reasoning and tool feedback, one tool execution, two logical requests and two
wire attempts. A synthetic throughput registry makes the normal projection
expire during the pause; it is not real model performance evidence. Run this
fixture with a portable worker wrapper and `PAUSE_WORKER_PORTABLE=1` to verify
unavailable external telemetry. Unit tests separately verify repeated reliable
inactivity and interruption of a genuinely quiet stream.
