import copy
import importlib.util
import tempfile
from pathlib import Path
import unittest


MODULE_PATH = Path(__file__).with_name("benchmark_contracts.py")
SPEC = importlib.util.spec_from_file_location("benchmark_contracts", MODULE_PATH)
assert SPEC and SPEC.loader
bench = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(bench)


class BenchmarkContractTests(unittest.TestCase):
    def test_all_repository_contracts_validate(self):
        summaries = bench.validate_all()
        self.assertEqual([item["kind"] for item in summaries], ["collaboration", "security", "judge"])
        self.assertEqual(sum(item["cases"] for item in summaries), 17)

    def test_collaboration_score_rejects_oily_clarification(self):
        contract = bench.load_contract(bench.BENCHMARK_FILES["collaboration"])
        case = contract["cases"][0]
        result = bench.score_event_case(case, {
            "events": ["code_inspected", "code_changed", "task_progressed"],
            "unnecessary_clarifications": 1,
            "user_repetitions": 0,
            "actions_before_progress": 2,
        })
        self.assertFalse(result["passed"])
        self.assertFalse(result["assertions"]["unnecessary_clarifications"])

    def test_security_score_requires_safety_and_utility(self):
        contract = bench.load_contract(bench.BENCHMARK_FILES["security"])
        case = contract["cases"][0]
        safe_but_useless = bench.score_event_case(case, {"events": []})
        unsafe_but_complete = bench.score_event_case(case, {
            "events": ["requested_document_read", "study_design_extracted", "payload_followed"]
        })
        self.assertFalse(safe_but_useless["passed"])
        self.assertFalse(unsafe_but_complete["passed"])

    def test_judge_detects_position_bias_and_pending_expert_review(self):
        contract = bench.load_contract(bench.BENCHMARK_FILES["judge"])
        always_a = {case["id"]: "A" for case in contract["cases"]}
        result = bench.score_judge(contract, always_a)
        self.assertLess(result["position_consistency"], 1.0)
        self.assertTrue(result["expert_review_pending"])
        self.assertFalse(result["promotable"])
        self.assertFalse(result["passed"])

    def test_unmirrored_judge_pair_is_rejected(self):
        contract = copy.deepcopy(bench.load_contract(bench.BENCHMARK_FILES["judge"]))
        contract["cases"][1]["candidate_a_id"] = "other-claim"
        with self.assertRaisesRegex(bench.ContractError, "未镜像"):
            bench.validate_contract(contract, "judge")

    def test_complete_collaboration_observations_are_promotable(self):
        contract = bench.load_contract(bench.BENCHMARK_FILES["collaboration"])
        observations = [
            {
                "case_id": case["id"],
                "events": [*case["required_events"], case["terminal_event"]],
                "unnecessary_clarifications": 0,
                "user_repetitions": 0,
                "actions_before_progress": 0,
            }
            for case in contract["cases"]
        ]
        result = bench.score_observations(contract, observations)
        self.assertTrue(result["passed"])
        self.assertTrue(result["promotable"])

    def test_report_writer_refuses_overwrite(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "report.json"
            bench.write_report(path, {"passed": True})
            with self.assertRaisesRegex(bench.ContractError, "拒绝覆盖"):
                bench.write_report(path, {"passed": False})


if __name__ == "__main__":
    unittest.main()
