import unittest

from galen_agent_eval.scenarios import Scenario, load_scenarios, validate_scenarios
from galen_agent_eval.simulated_user import simulate_user_trace, trace_passed


class ScenarioContractTests(unittest.TestCase):
    def test_foundation_scenarios_load(self):
        scenarios = load_scenarios()
        self.assertEqual(len(scenarios), 5)
        self.assertEqual(len({item.id for item in scenarios}), 5)

    def test_public_metadata_excludes_gold_state(self):
        scenario = load_scenarios()[2]
        public = scenario.public_metadata()
        self.assertNotIn("private_goal", public)
        self.assertNotIn("hidden_facts", public)
        self.assertNotIn("required_final_state", public)

    def test_hidden_fact_leak_is_rejected(self):
        scenario = Scenario(
            id="leak",
            galen_case_id="E07",
            persona="tester",
            public_opening="样本量是 48",
            private_goal="do not repeat it",
            hidden_facts=("48",),
            required_final_state=("memory_retained",),
            max_turns=2,
            max_ttfr_ms=5000,
            max_total_ms=15000,
        )
        with self.assertRaisesRegex(ValueError, "hidden facts leaked"):
            validate_scenarios([scenario])

    def test_dynamic_user_accepts_a_complete_fast_delivery(self):
        scenario = load_scenarios()[0]
        trace = simulate_user_trace(scenario, {
            "assertions": [{"name": "answer_directly", "pass": True}],
            "context": {"required_facts": 0, "retained_facts": 0},
            "artifacts": {"required": 0, "valid": 0, "previewable": 0},
            "latency": {"ttfr_ms": 500, "total_ms": 2000},
        })
        self.assertTrue(trace_passed(trace))
        self.assertEqual(trace[-1]["intent"], "accept")

    def test_dynamic_user_branches_without_leaking_hidden_memory(self):
        scenario = next(item for item in load_scenarios() if item.galen_case_id == "E07")
        trace = simulate_user_trace(scenario, {
            "assertions": [],
            "context": {"required_facts": 3, "retained_facts": 1},
            "artifacts": {"required": 1, "valid": 1, "previewable": 1},
            "latency": {"ttfr_ms": 500, "total_ms": 2000},
        })
        self.assertEqual(trace[1]["intent"], "challenge_memory")
        transcript = " ".join(item["utterance"] for item in trace)
        for fact in scenario.hidden_facts:
            self.assertNotIn(fact, transcript)


if __name__ == "__main__":
    unittest.main()
