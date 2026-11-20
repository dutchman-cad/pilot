# SPDX-License-Identifier: Apache-2.0
"""Exercise native calls and strict transport behavior without a model server."""
import contextlib
import io
import tempfile
import json
from pathlib import Path
import sys
import time
from types import SimpleNamespace
import unittest
from unittest.mock import MagicMock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import run
import report


class FakeCAD:
    tools = [{'name': 'execute', 'inputSchema': {'type': 'object'}}]
    def __init__(self):
        self.deadline = time.monotonic() + 10
        self.calls = []
    def tool(self, name, arguments):
        self.calls.append((name, arguments))
        return {'content': [{'type': 'text', 'text': '{}'}]}


def answer(message):
    return io.BytesIO(json.dumps({'choices': [{'message': message}]}).encode())


class RunnerTests(unittest.TestCase):
    def test_native_calls_execute_and_preserve_conversation(self):
        cad = FakeCAD()
        responses = [answer({'role':'assistant','content':None,'tool_calls':[
            {'id':'one','type':'function','function':{'name':'execute','arguments':'{"command":"solid.box","params":{"length":20}}'}}]}),
            answer({'role':'assistant','content':'Done'})]
        messages = []
        with patch('run.open_model', side_effect=responses):
            reply, _ = run.converse(cad, messages, 'Make a box', 'model', 'http://example.invalid/v1')
        self.assertEqual(reply, 'Done')
        self.assertEqual(len(cad.calls), 1)
        self.assertEqual(messages[2]['role'], 'tool')
        self.assertEqual(messages[2]['tool_call_id'], 'one')

    def test_prose_json_never_executes(self):
        cad = FakeCAD()
        with patch('run.open_model', return_value=answer({'role':'assistant','content':'{"tool":"execute","args":{}}'})):
            run.converse(cad, [], 'Make a box', 'model', 'http://example.invalid/v1')
        self.assertEqual(cad.calls, [])

    def test_endpoint_normalization(self):
        for suffix in ['', '/', '/api', '/v1', '/v1/']:
            self.assertEqual(run.endpoint_url('http://example.invalid'+suffix), 'http://example.invalid/v1')
        with self.assertRaises(ValueError):
            run.endpoint_url('http://user:secret@example.invalid')

    def test_pilot_lifecycle_uses_the_requested_upstream(self):
        with tempfile.TemporaryDirectory() as directory:
            args = type('Args', (), {'endpoint':'http://example.invalid', 'through_pilot':'pilot'})()
            with patch('run.socket.socket') as sock, patch('run.socket.create_connection'), patch('run.subprocess.Popen') as child:
                sock.return_value.__enter__.return_value.getsockname.return_value = ('127.0.0.1', 12345)
                child.return_value.poll.return_value = None
                with run.endpoint(args, Path(directory)) as url:
                    self.assertEqual(url, 'http://127.0.0.1:12345/v1')
                self.assertEqual(child.call_args.args[0], ['pilot','model','--listen','127.0.0.1:12345','--upstream','http://example.invalid/v1','--no-direct-recipes','--log',str((Path(directory)/'pilot-rewrites.jsonl').resolve())])
                child.return_value.terminate.assert_called_once()

    def test_pilot_mcp_uses_run_log_before_passthrough_args(self):
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory) / 'pilot-rewrites.jsonl'
            with patch('run.subprocess.Popen') as child, patch('run.threading.Thread'), patch('run.agents.kill_tree'), patch.object(run.MCP, 'rpc', side_effect=[{}, {'tools': []}]), patch.object(run.MCP, 'send'):
                with run.MCP('solidcraft', Path('part.solidcraft'), 'pilot', log):
                    self.assertEqual(child.call_args.args[0], [
                        'pilot', 'mcp', '--solidcraft-cli', 'solidcraft',
                        '--log', str(log), '--', '--in', 'part.solidcraft'])

    def test_report_pairs_and_rejects_duplicate_trials(self):
        rows = [{'format':1, 'set':'v1', 'model':'model', 'task':'cube', 'rep':1,
                 'through_pilot':flag, 'mode':'model', 'pass':flag} for flag in [False, True]]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'results.jsonl'
            path.write_text(''.join(json.dumps(row)+'\n' for row in rows))
            output = io.StringIO()
            with contextlib.redirect_stdout(output):
                report.report([path])
            self.assertIn('| 0/1 | 1/1 | +1/1 pairs | 0/0 |', output.getvalue())
            with contextlib.redirect_stdout(io.StringIO()), self.assertRaisesRegex(ValueError, 'duplicate'):
                report.report([path, path])

    def test_execution_error_is_never_a_rejected_negative(self):
        with patch('run.MCP', side_effect=RuntimeError('broken fixture')), patch('run.reference', return_value=[]):
            with self.assertRaisesRegex(RuntimeError, 'broken fixture'):
                run.run_one(type('Args', (), {'solidcraft':'unused'})(), Path('.'), {'setup_steps':0}, 'negative', None)


class VersionGateTests(unittest.TestCase):
    def setUp(self):
        self.task = {'id': 'refusal', 'prompt': 'Make it left-handed.',
                     'max_solidcraft': '0.20', 'check': {'honest': True}}

    def test_numeric_inclusive_release_line_and_patch_bounds(self):
        for version in ('0.19.0', '0.20', '0.20.0', '0.20.99'):
            with self.subTest(version=version):
                self.assertIsNone(run.skip_reason(self.task, {'version': version}))
        for version in ('0.21.0', '0.44.0', '0.100.0', '1.0.0', '0.21.0-rc.1+build.2'):
            with self.subTest(version=version):
                self.assertIn(version, run.skip_reason(self.task, {'version': version}))
        task = dict(self.task, max_solidcraft='0.20.1')
        self.assertIsNone(run.skip_reason(task, {'version': '0.20.1'}))
        self.assertIsNotNone(run.skip_reason(task, {'version': '0.20.2'}))
        self.assertIsNone(run.skip_reason({}, {}))

    def test_unknown_versions_fail_instead_of_skipping_or_scoring(self):
        for version in (None, '', 'unknown', '0.20garbage', 20, '9' * 101):
            with self.subTest(version=version), self.assertRaisesRegex(ValueError, 'invalid SolidCraft version'):
                run.skip_reason(self.task, {'version': version})

    def test_task_loader_validates_gate(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'tasks.json'
            for task in (dict(self.task, max_solidcraft='bad'),
                         dict(self.task, check={'bodies': 1})):
                run.save(path, {'format': 1, 'tasks': [task]})
                with self.assertRaises(ValueError):
                    run.task_file(path)

    def test_new_versions_skip_all_modes_before_setup_model_and_checks(self):
        args = SimpleNamespace(solidcraft='unused', replay_setup=True)
        for mode in ('model', 'reference', 'negative', 'nothing'):
            app = MagicMock()
            app.__enter__.return_value = app
            app.info = {'serverInfo': {'version': '0.44.0'}}
            with self.subTest(mode=mode), patch('run.reference', return_value=[]), \
                    patch('run.MCP', return_value=app), patch('run.replay') as replay, \
                    patch('run.converse') as converse, patch('run.score') as score:
                record = run.run_one(args, Path('.'), self.task, mode, None)
                self.assertTrue(record['skipped'])
                self.assertIsNone(record['pass'])
                self.assertIsNone(record['matched'])
                self.assertEqual(record['solidcraft']['version'], '0.44.0')
                replay.assert_not_called()
                converse.assert_not_called()
                score.assert_not_called()
                app.call.assert_not_called()

    def test_old_version_still_scores_the_original_refusal(self):
        app = MagicMock()
        app.__enter__.return_value = app
        app.info = {'serverInfo': {'version': '0.19.0'}}
        args = SimpleNamespace(solidcraft='unused', replay_setup=True)
        with patch('run.reference', return_value=[]), patch('run.MCP', return_value=app), \
                patch('run.replay'), patch('run.check.inspect_for_task', return_value={}), \
                patch('run.score', return_value={'pass': True, 'reason': 'ok'}) as score:
            record = run.run_one(args, Path('.'), self.task, 'reference', None)
        self.assertTrue(record['pass'])
        self.assertTrue(record['matched'])
        self.assertNotIn('skipped', record)
        self.assertEqual(score.call_count, 2)

    def test_skip_is_recorded_and_printed_without_failure_exit(self):
        with tempfile.TemporaryDirectory() as directory:
            argv = ['run.py', '--model', 'm', '--solidcraft', 'unused', '--out', directory]
            skipped = {'task': 'refusal', 'pass': None, 'matched': None, 'skipped': True,
                       'reason': 'version gate', 'solidcraft': {'version': '0.44.0'}}
            with patch.object(sys, 'argv', argv), patch('run.task_file', return_value=[self.task]), \
                    patch('run.run_one', return_value=skipped), contextlib.redirect_stdout(io.StringIO()) as output:
                self.assertEqual(run.main(), 0)
            self.assertIn('refusal: SKIP', output.getvalue())
            for filename in ('results.jsonl', '01-refusal.json'):
                row = json.loads((Path(directory)/filename).read_text())
                self.assertTrue(row['skipped'])
                self.assertIsNone(row['pass'])

    def test_report_excludes_skips_from_scores_and_either_side_of_pairs(self):
        rows = []
        for task, skips in [('both', (True, True)), ('direct', (True, False)),
                            ('pilot', (False, True)), ('neither', (False, False))]:
            for flag in (False, True):
                rows.append(dict(format=1, set='v3', model='m', task=task, rep=1,
                                 mode='model', through_pilot=flag, skipped=skips[flag],
                                 **{'pass': None if skips[flag] else flag}))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'results.jsonl'
            path.write_text(''.join(json.dumps(row)+'\n' for row in rows))
            with contextlib.redirect_stdout(io.StringIO()) as output:
                report.report([path])
            self.assertIn('Skipped (direct/Pilot)', output.getvalue())
            self.assertIn('| 0/2 | 2/2 | +1/1 pairs | 0/0 | 2/2 |', output.getvalue())
            path.write_text(''.join(json.dumps(row)+'\n' for row in rows[:2]))
            with contextlib.redirect_stdout(io.StringIO()) as output:
                report.report([path])
            self.assertIn('| — | — | — | 0/0 | 1/1 |', output.getvalue())


if __name__ == '__main__':
    unittest.main()
