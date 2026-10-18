"""Classify native Harness endpoints independently of outer process success.

Microgentic records a typed `outcome` in the terminal trace event: in the
`run.finished` summary, and in `run.failed` when a hard stop raised inside a
model stream ended the run. When it is present, the label comes from it and
from nothing else. Traces written before outcomes existed have no such field;
for those, the earlier rules below still apply, so frozen evidence classifies
as it always did.

Labels are the earlier vocabulary where a case existed before, so counts stay
comparable: `done`, `fail`, `outer_turn_limit`, `interaction_limit`, and the
`*_stop` policy labels. The typed outcome adds cases the text rules could not
see:

- `withheld_<gate>` (`stash`, `validation`, `probes`, `acceptance_ledger`): the
  model claimed completion, a gate rejected it, and the iteration budget ran
  out. The text rules called these `fail` (a stash rejection rewrites the
  summary to a FAIL line) or `outer_turn_limit`.
- `inspection_loop_stop` and `no_content_stream_stop`: hard stops raised
  inside a stream, which the text rules rejected as unexpected errors.
- `replied`: an everyday reply; benchmark mode never produces it.

Reaching the model-interaction limit is still a runtime error with no typed
outcome, so `interaction_limit` is matched on its exact error text.
"""
POLICY_EVENTS = {
    'agent.turn.hidden_only_no_action_hard_failed': 'hidden_only_no_action_stop',
    'agent.turn.empty_response_hard_failed': 'empty_response_stop',
    'agent.validation.repair_hard_failed': 'repair_stop',
    'agent.validation.repair_depth_hard_failed': 'repair_depth_stop',
    'agent.action_boundary.hard_failed': 'action_boundary_stop',
}

HARD_STOP_CAUSES = {
    'action_boundary': 'action_boundary_stop',
    'thinking_only': 'hidden_only_no_action_stop',
    'empty_response': 'empty_response_stop',
    'repair_no_action': 'repair_stop',
    'repair_depth': 'repair_depth_stop',
    'inspection_loop': 'inspection_loop_stop',
    'no_content_stream': 'no_content_stream_stop',
    'iteration_budget': 'outer_turn_limit',
}

WITHHELD_GATES = ('stash', 'validation', 'probes', 'acceptance_ledger')


def outcome_label(outcome):
    """The label for a typed Microgentic outcome; ValueError for one that a
    benchmark run must not end with or that this reader does not know."""
    kind = outcome.get('kind')
    if kind == 'done':
        return 'done'
    if kind == 'failed':
        return 'fail'
    if kind == 'replied':
        return 'replied'
    if kind == 'withheld' and outcome.get('gate') in WITHHELD_GATES:
        return 'withheld_' + outcome['gate']
    if kind == 'hard_stop' and outcome.get('cause') in HARD_STOP_CAUSES:
        return HARD_STOP_CAUSES[outcome['cause']]
    raise ValueError('Unrecognized Harness outcome: ' + str(outcome))


def classify_terminal(events, return_code, *, max_iterations, max_model_interactions):
    terminals=[e for e in events if e['kind'] in ('run.finished','run.failed')]
    if not terminals:
        raise ValueError('Missing Harness terminal event')
    terminal=terminals[-1]
    payload=terminal['payload']
    outcome=payload.get('outcome')
    if terminal['kind']=='run.failed':
        if outcome is not None:
            if return_code==0:
                raise ValueError('Harness terminal/process status mismatch')
            return outcome_label(outcome)
        if return_code==1 and payload.get('phase')=='agent.stream' and payload.get('error')==f'Maximum model interactions exceeded: limit {max_model_interactions}':
            return 'interaction_limit'
        raise ValueError('Unexpected Harness error: '+str(payload))
    if return_code!=0:
        raise ValueError('Harness terminal/process status mismatch')
    if outcome is not None:
        return outcome_label(outcome)
    for event in reversed(events):
        if event['kind'] in POLICY_EVENTS:
            return POLICY_EVENTS[event['kind']]
    lines=payload.get('final_summary','').splitlines()
    if any(line.strip()=='DONE' for line in lines):
        return 'done'
    if any(line.strip().split(maxsplit=1)[:1]==['FAIL'] for line in lines):
        return 'fail'
    if max((e['payload'].get('turn',0) for e in events if e['kind']=='agent.turn.started'),default=0)==max_iterations:
        return 'outer_turn_limit'
    raise ValueError('Unrecognized Harness terminal')
