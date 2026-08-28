import json
import unittest

import ais_textbook_dataset as dataset


class AisTextbookDatasetTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        document = dataset.read_json(dataset.CASES_PATH)
        cls.cases = {case["case_id"]: case for case in document["cases"]}

    def test_temporal_task_hides_all_future_observations(self):
        case = self.cases["AIS-C021"]
        task = dataset.build_case_tasks(case, "development")[1]
        self.assertEqual(task["visible_event_ids"], ["baseline"])
        self.assertEqual(set(task["hidden_event_ids"]), {"in_brace", "follow_up"})
        self.assertTrue(all(item.startswith("C021-B-") for item in task["hard_gate_observation_ids"]))
        self.assertTrue(all("-I-" in item or "-F-" in item for item in task["forbidden_observation_ids"]))

    def test_disputed_source_value_never_enters_hard_gate(self):
        case = self.cases["AIS-C025"]
        tasks = dataset.build_case_tasks(case, "development")
        for task in tasks:
            self.assertNotIn("C025-I-T", task["hard_gate_observation_ids"])

    def test_sanitized_input_contains_only_visible_events(self):
        case = self.cases["AIS-C029"]
        task = dataset.build_case_tasks(case, "hidden")[1]
        record = dataset.build_input_record(case, task)
        self.assertEqual({event["event_id"] for event in record["events"]}, {"baseline"})
        self.assertEqual({item["event_id"] for item in record["observations"]}, {"baseline"})

    def test_pilot_has_four_tasks_per_case(self):
        splits = dataset.split_map(dataset.read_json(dataset.SPLITS_PATH))
        tasks = []
        for case_id, case in self.cases.items():
            tasks.extend(dataset.build_case_tasks(case, splits[case_id]))
        self.assertEqual(len(tasks), 40)


if __name__ == "__main__":
    unittest.main()
