import importlib.util
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).with_name("scicode_benchmark.py")
SPEC = importlib.util.spec_from_file_location("scicode_benchmark", MODULE_PATH)
BENCH = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(BENCH)


class SciCodeBenchmarkTests(unittest.TestCase):
    def test_hidden_gold_counts_raw_quality_before_filtering(self):
        summary, quality = BENCH._gold(BENCH.HIDDEN_DATASETS["quality_edge_cases"])
        self.assertEqual(quality, {
            "row_count": 9,
            "verified_rows": 8,
            "duplicate_rows": 1,
            "missing_required_values": 1,
            "complete_athletes": 2,
        })
        self.assertEqual([row["rehab_id"] for row in summary], ["EDGE-1", "EDGE-2"])
        self.assertIsNone(summary[1]["rmssd_recovery_pct_72h"])
        self.assertIsNone(summary[1]["cmj_recovery_pct_72h"])

    def test_static_gate_rejects_hardcoding_and_process_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            script = Path(directory) / "bad.py"
            script.write_text(
                "import subprocess\nATHLETE = 'ATH-001'\nsubprocess.run(['echo', ATHLETE])\n",
                encoding="utf-8",
            )
            passed, problems, _ = BENCH._static_gate(script)
        self.assertFalse(passed)
        self.assertTrue(any("import" in problem for problem in problems))
        self.assertTrue(any("hardcoded" in problem for problem in problems))


if __name__ == "__main__":
    unittest.main()
