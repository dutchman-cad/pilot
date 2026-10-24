# SPDX-License-Identifier: Apache-2.0
"""Rescore recorded SolidCraft 0.20 documents without invoking SolidCraft."""
import copy
import json
from pathlib import Path
import sys
import unittest

BENCH = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(BENCH))
import check
import run


class RecordedChecks(unittest.TestCase):
    def test_start_comparison_rejects_changes_and_missing_measurements(self):
        original = {
            'bodies': [{'name': 'Part', 'volume_mm3': 1000,
                        'bbox': {'min': [0, 0, 0], 'max': [10, 10, 10]}}],
            'params': [{'name': 'width', 'value': 10}],
            'timeline': [{'type': 'BoxFeature'}],
        }
        check.compare_start(original, copy.deepcopy(original))
        within_tolerance = copy.deepcopy(original)
        within_tolerance['bodies'][0]['volume_mm3'] = 1000.9
        check.compare_start(original, within_tolerance)
        for field, replacement in [
            ('bodies', []),
            ('bodies', [{'name': 'Part', 'volume_mm3': 1002,
                         'bbox': {'min': [0, 0, 0], 'max': [10, 10, 10]}}]),
            ('bodies', [{'name': 'Part', 'volume_mm3': 1000,
                         'bbox': {'min': [1, 0, 0], 'max': [11, 10, 10]}}]),
            ('bodies', [{'name': 'Part', 'volume_mm3': 1000}]),
            ('bodies', [{'name': 'Part', 'volume_mm3': float('nan'),
                         'bbox': {'min': [0, 0, 0], 'max': [10, 10, 10]}}]),
            ('bodies', [{'name': 'Part', 'volume_mm3': 1000,
                         'bbox': {'min': [0], 'max': [10]}}]),
            ('params', [{'name': 'width', 'value': 11}]),
            ('params', [{'name': 'length', 'value': 10}]),
            ('params', [{'name': 'width', 'value': float('nan')}]),
            ('timeline', []),
            ('timeline', [{'type': 'ExtrudeFeature'}]),
            ('timeline', [{'type': 'BoxFeature', 'error': 'failed'}]),
        ]:
            with self.subTest(field=field, replacement=replacement):
                changed = copy.deepcopy(original)
                changed[field] = replacement
                with self.assertRaises(ValueError):
                    check.compare_start(original, changed)

    def test_pass_and_fail_recordings_for_every_set(self):
        for directory in sorted((BENCH / 'sets').iterdir()):
            if directory.name == 'sc98':
                continue  # The screen-protocol fixtures are checked by tests/bench_sc98.rs.
            with self.subTest(set=directory.name):
                fixture = json.loads((BENCH/'tests'/'fixtures'/(directory.name+'.json')).read_text())
                self.assertEqual({True, False}, {r['pass'] for r in fixture['cases']})
                for record in fixture['cases']:
                    with self.subTest(case=record['id']):
                        passed, reason = check.evaluate(record['check'], record['document'],
                            record['reply'], record['before'], record['queries'])
                        self.assertEqual(record['pass'], passed, reason)

    def test_missing_start_cannot_prove_change_or_preservation(self):
        for spec in [{'changed': True}, {'unchanged': True}, {'keep_features': True}]:
            with self.subTest(check=spec):
                self.assertFalse(check.evaluate(spec, {})[0])

    def test_wrong_top_chamfer_rejected_with_correct_volume(self):
        fixture = json.loads((BENCH/'tests/fixtures/v2.json').read_text())
        record = next(r for r in fixture['cases'] if r['id'] == '07-chamfer-top-bottom-final')
        self.assertTrue(check.evaluate({'volume':record['check']['volume'], 'tol':.003}, record['document'])[0])
        passed, reason = check.evaluate(record['check'], record['document'], record['reply'], record['before'], record['queries'])
        self.assertFalse(passed)
        self.assertIn('centre of mass', reason)

    def test_missing_measurement_fails_closed(self):
        fixture = json.loads((BENCH/'tests/fixtures/v2.json').read_text())
        record = next(r for r in fixture['cases'] if r['id'] == '01-bolt-circle-bracket-final')
        self.assertFalse(check.evaluate(record['check'], record['document'], queries={})[0])

    def test_unchanged_question_rejects_geometry_change(self):
        fixture = json.loads((BENCH/'tests/fixtures/v1.json').read_text())
        record = next(r for r in fixture['cases'] if r['id'] == '09-volume-question-final')
        state = copy.deepcopy(record['document'])
        state['bodies'][0]['name'] = 'Altered'
        self.assertFalse(check.evaluate(record['check'], state, record['reply'], record['before'])[0])

    def test_task_and_reference_files_load(self):
        for directory in (BENCH/'sets').iterdir():
            if directory.name == 'sc98':
                continue  # The Rust checker validates this application's references.
            tasks = run.task_file(directory/'tasks.json')
            controls = run.load(directory/'do-nothing.json')['expectations']
            self.assertEqual({t['id'] for t in tasks}, {t['id'] for t in controls})
            for task in tasks + run.task_file(directory/'negatives/tasks.json'):
                commands = run.reference(directory, task)
                run.setup(directory, task, commands)
                turns = task.get('turns')
                if turns:
                    ends = [t['reference_end'] for t in turns]
                    self.assertEqual(ends, sorted(set(ends)))
                    self.assertEqual(ends[-1], len(commands))
                    self.assertGreater(ends[0], task['setup_steps'])


if __name__ == '__main__':
    unittest.main()
