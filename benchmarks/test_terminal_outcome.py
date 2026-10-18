import json
import unittest
from pathlib import Path

from terminal_outcome import classify_terminal, outcome_label

CONTRACT = Path(__file__).parent / 'fixtures' / 'terminal_outcome_contract.json'


class TerminalOutcomeTests(unittest.TestCase):
    def classify(self,events,code=0):
        return classify_terminal(events,code,max_iterations=4,max_model_interactions=24)

    def test_native_policy_stop_despite_zero_exit(self):
        events=[{'kind':'agent.turn.started','payload':{'turn':3}},
                {'kind':'agent.turn.hidden_only_no_action_hard_failed','payload':{'turn':3}},
                {'kind':'run.finished','payload':{'final_summary':'turn 3 produced 2 consecutive hidden-only no-action responses'}}]
        self.assertEqual(self.classify(events),'hidden_only_no_action_stop')

    def test_interaction_limit_is_exact_provider_error_is_rejected(self):
        event={'kind':'run.failed','payload':{'phase':'agent.stream','error':'Maximum model interactions exceeded: limit 24'}}
        self.assertEqual(self.classify([event],1),'interaction_limit')
        event['payload']['error']='Unexpected HTTP disconnect'
        with self.assertRaises(ValueError):self.classify([event],1)

    def test_tokens_and_outer_budget(self):
        end={'kind':'run.finished','payload':{'final_summary':'DONE'}}
        self.assertEqual(self.classify([end]),'done')
        end['payload']['final_summary']='failing'
        with self.assertRaises(ValueError):self.classify([end])
        self.assertEqual(self.classify([{'kind':'agent.turn.started','payload':{'turn':4}},end]),'outer_turn_limit')
        end['payload']['final_summary']='FAIL blocked'
        self.assertEqual(self.classify([end]),'fail')


class StructuredOutcomeTests(unittest.TestCase):
    """Traces from binaries that record a typed outcome."""

    def classify(self,events,code=0):
        return classify_terminal(events,code,max_iterations=4,max_model_interactions=24)

    def test_every_contract_case_gets_its_label(self):
        # The same contract src/cli/run_outcomes.rs verifies through the CLI.
        contract=json.loads(CONTRACT.read_text())
        self.assertEqual(contract['schema_version'],'microgentic_terminal_outcome_contract.v1')
        for case in contract['cases']:
            with self.subTest(case['name']):
                outcome=dict(case['outcome'],detail='varies by run',reason='varies by run')
                event={'kind':case['terminal_kind'],'payload':{'final_summary':'unused','outcome':outcome}}
                code=0 if case['terminal_kind']=='run.finished' else 1
                self.assertEqual(self.classify([event],code),case['label'])

    def test_the_outcome_wins_over_summary_text_and_policy_events(self):
        # A stash rejection rewrites the summary to a FAIL line; the text rules
        # called it a FAIL, the outcome says the stash gate withheld DONE.
        events=[{'kind':'agent.turn.started','payload':{'turn':4}},
                {'kind':'agent.git_stash.done_rejected','payload':{}},
                {'kind':'run.finished','payload':{
                    'final_summary':'FAIL unresolved Git stash state at completion',
                    'outcome':{'kind':'withheld','gate':'stash','detail':'new stash object IDs: ["a1"]'}}}]
        self.assertEqual(self.classify(events),'withheld_stash')
        # A stale policy event earlier in the trace does not override it.
        events.insert(1,{'kind':'agent.turn.empty_response_hard_failed','payload':{}})
        self.assertEqual(self.classify(events),'withheld_stash')

    def test_a_stream_hard_stop_in_run_failed_is_classified(self):
        event={'kind':'run.failed','payload':{
            'phase':'agent.stream',
            'error':'stopped by the runtime (inspection loop): repeated list_tree',
            'outcome':{'kind':'hard_stop','cause':'inspection_loop','detail':'repeated list_tree'}}}
        self.assertEqual(self.classify([event],1),'inspection_loop_stop')
        event['payload']['outcome']['cause']='no_content_stream'
        self.assertEqual(self.classify([event],1),'no_content_stream_stop')
        # A failed run must not have exited cleanly.
        with self.assertRaises(ValueError):self.classify([event],0)

    def test_a_provider_error_has_no_outcome_and_stays_an_error(self):
        event={'kind':'run.failed','payload':{'phase':'agent.stream','error':'Unexpected HTTP disconnect','outcome':None}}
        with self.assertRaises(ValueError):self.classify([event],1)
        event['payload']['error']='Maximum model interactions exceeded: limit 24'
        self.assertEqual(self.classify([event],1),'interaction_limit')

    def test_unknown_or_non_benchmark_outcomes_are_rejected(self):
        for outcome in ({'kind':'withheld','gate':'mystery'},{'kind':'hard_stop','cause':'mystery'},
                        {'kind':'cancelled','signal':15},{'kind':'usage','message':'x'},{'kind':'error','message':'x'}):
            with self.subTest(outcome):
                with self.assertRaises(ValueError):outcome_label(outcome)
        self.assertEqual(outcome_label({'kind':'replied'}),'replied')

    def test_a_clean_finish_with_a_nonzero_exit_is_a_mismatch(self):
        event={'kind':'run.finished','payload':{'outcome':{'kind':'done'}}}
        with self.assertRaises(ValueError):self.classify([event],1)


if __name__=='__main__':
    unittest.main()
