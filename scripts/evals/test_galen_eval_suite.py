import copy
import importlib.util
from pathlib import Path
import unittest


MODULE_PATH = Path(__file__).with_name("galen_eval_suite.py")
SPEC = importlib.util.spec_from_file_location("galen_eval_suite", MODULE_PATH)
assert SPEC and SPEC.loader
suite = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(suite)


class GalenEvalSuiteTests(unittest.TestCase):
    def setUp(self):
        self.manifest = suite.load_manifest(suite.DEFAULT_MANIFEST)

    def test_repository_manifest_is_valid(self):
        warnings = suite.validate_manifest(self.manifest)
        self.assertTrue(any("agent-foundation" in warning for warning in warnings))

    def test_duplicate_lane_is_rejected(self):
        invalid = copy.deepcopy(self.manifest)
        invalid["lanes"].append(copy.deepcopy(invalid["lanes"][0]))
        with self.assertRaisesRegex(suite.ManifestError, "lane id 重复"):
            suite.validate_manifest(invalid)

    def test_unknown_dimension_is_rejected(self):
        invalid = copy.deepcopy(self.manifest)
        invalid["lanes"][0]["dimensions"].append("hand_wavy_quality")
        with self.assertRaisesRegex(suite.ManifestError, "未声明维度"):
            suite.validate_manifest(invalid)

    def test_stage_selection_cannot_cross_stage_boundary(self):
        with self.assertRaisesRegex(suite.ManifestError, "不存在 lane"):
            suite.selected_lanes(self.manifest, "pr", ["agent-foundation"])

    def test_delegated_lane_cannot_be_reported_as_passed(self):
        results = [{"lane_id": "agent-foundation", "status": "delegated"}]
        self.assertFalse(suite.report_passed(results))

    def test_changed_files_select_owned_lanes(self):
        lanes = suite.selected_lanes(self.manifest, "pr", [])
        selected = suite.lanes_for_changes(
            lanes,
            ["rust/crates/galen/src-tauri/src/execution_context.rs"],
        )
        self.assertEqual([lane["id"] for lane in selected], ["protocol"])


if __name__ == "__main__":
    unittest.main()
