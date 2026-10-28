# SPDX-License-Identifier: Apache-2.0
"""Fixed sampling transport, temporary settings lifecycle and comparison isolation."""
import contextlib
import io
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import MagicMock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import report
import run
from test_agents_api import CAD, fake_api

SETTINGS = {'temperature': 0.0, 'seed': 42, 'num_ctx': 16384}


class SettingsTests(unittest.TestCase):
    def test_settings_reach_fake_upstreams_on_every_request(self):
        for kind in ('openai', 'ollama'):
            with self.subTest(kind=kind):
                captured = []
                tool = {'name': 'execute', 'arguments': {'command': 'solid.box'}}
                calls = [{'role': 'assistant', 'content': '', 'tool_calls': [
                    {'id': 'one', 'type': 'function', 'function': tool}]},
                    {'role': 'assistant', 'content': 'Done'}]
                if kind == 'openai':
                    tool['arguments'] = json.dumps(tool['arguments'])
                    responses = [{'choices': [{'message': m}]} for m in calls]
                    settings = {k: v for k, v in SETTINGS.items() if k != 'num_ctx'}
                else:
                    responses = [{'message': m} for m in calls]
                    settings = SETTINGS
                cad = CAD()
                with fake_api(responses, captured) as url:
                    reply, _ = run.converse(cad, [], 'Make a box', 'm', url, kind, settings)
                self.assertEqual(reply, 'Done')
                self.assertEqual(len(cad.calls), 1)
                self.assertEqual(len(captured), 2)
                for path, _, body in captured:
                    self.assertEqual(path, '/api/chat' if kind == 'ollama' else '/v1/chat/completions')
                    values = body['options'] if kind == 'ollama' else body
                    for key, value in settings.items():
                        self.assertEqual(values[key], value)
                if kind == 'ollama':
                    history = captured[1][2]['messages']
                    self.assertIsInstance(history[1]['tool_calls'][0]['function']['arguments'], dict)
                    self.assertEqual(history[2]['tool_name'], 'execute')

    def test_unset_settings_are_not_sent(self):
        for kind in ('openai', 'ollama', 'anthropic'):
            captured = []
            response = ({'message': {'role': 'assistant', 'content': 'Done'}} if kind == 'ollama' else
                        {'content': [{'type': 'text', 'text': 'Done'}]} if kind == 'anthropic' else
                        {'choices': [{'message': {'role': 'assistant', 'content': 'Done'}}]})
            with fake_api([response], captured) as url:
                run.converse(CAD(), [], 'Box', 'm', url, kind)
            body = captured[0][2]
            for key in SETTINGS:
                self.assertNotIn(key, body)
                self.assertNotIn(key, body.get('options', {}))

    def test_pilot_settings_file_lives_until_exit_and_is_removed_on_failure(self):
        for failure in ('none', 'body', 'startup', 'spawn'):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as directory:
                args = SimpleNamespace(endpoint='http://local', through_pilot='pilot', model='m', **SETTINGS)
                paths = []
                child = MagicMock()
                def spawn(command, **kwargs):
                    path = Path(command[command.index('--model-settings') + 1])
                    paths.append(path)
                    self.assertEqual(json.loads(path.read_text()), {'m': SETTINGS})
                    if failure == 'spawn':
                        raise OSError('spawn failed')
                    return child
                with patch('run.subprocess.Popen', side_effect=spawn), patch('run.socket.create_connection'), patch('run.socket.socket'), patch('run.time.sleep'):
                    child.poll.return_value = 1 if failure == 'startup' else None
                    manager = self.assertRaises((OSError, RuntimeError)) if failure != 'none' else contextlib.nullcontext()
                    with manager:
                        with run.endpoint(args, Path(directory)):
                            self.assertTrue(paths[0].exists())
                            if failure == 'body':
                                raise RuntimeError('run failed')
                    self.assertEqual(len(paths), 1)
                    self.assertFalse(paths[0].exists())
                    if failure != 'spawn':
                        child.terminate.assert_called_once()

    def test_task_runner_forwards_settings_and_uses_openai_for_pilot(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            run.save(root/'reference.json', {'commands': []})
            task = {'id': 'box', 'prompt': 'Make a box', 'reference': 'reference.json',
                    'check': {'bodies': 0}}
            for pilot in (None, 'pilot'):
                args = SimpleNamespace(solidcraft='unused', replay_setup=True, model='m',
                    endpoint_kind='ollama', through_pilot=pilot, **SETTINGS)
                with patch('run.MCP', return_value=CAD()), patch('run.converse', return_value=('Done', [])) as converse:
                    run.run_one(args, root, task, 'model', 'http://local/v1')
                self.assertEqual(converse.call_args.args[-2:],
                                 ('openai' if pilot else 'ollama', SETTINGS))

    def test_rows_artifacts_and_report_include_settings_even_for_errors(self):
        for fails in (False, True):
            with tempfile.TemporaryDirectory() as directory:
                argv = ['run.py', '--model', 'm', '--solidcraft', 'unused', '--out', directory,
                        '--endpoint-kind', 'ollama', '--temperature', '0', '--seed', '42', '--num-ctx', '16384']
                result = {'task': 'box', 'pass': True, 'matched': True, 'reason': 'ok'}
                with patch.object(sys, 'argv', argv), patch('run.task_file', return_value=[{'id': 'box'}]), patch('run.run_one', side_effect=RuntimeError('broken') if fails else None, return_value=result), contextlib.redirect_stdout(io.StringIO()):
                    self.assertEqual(run.main(), 1 if fails else 0)
                for name in ('results.jsonl', '01-box.json'):
                    self.assertEqual(json.loads((Path(directory)/name).read_text())['model_settings'], SETTINGS)
                with contextlib.redirect_stdout(io.StringIO()) as output:
                    report.report([Path(directory)/'results.jsonl'])
                for key, value in SETTINGS.items():
                    self.assertIn(json.dumps(key) + ': ' + json.dumps(value), output.getvalue())

    def test_matching_fixed_settings_are_paired(self):
        rows = [dict(format=1, set='v1', model='m', task='box', rep=1, mode='model',
                     through_pilot=flag, model_settings=SETTINGS, **{'pass': flag})
                for flag in (False, True)]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'results.jsonl'
            path.write_text(''.join(json.dumps(row)+'\n' for row in rows))
            with contextlib.redirect_stdout(io.StringIO()) as output:
                report.report([path])
            self.assertIn('| 0/1 | 1/1 | +1/1 pairs |', output.getvalue())
            self.assertEqual(output.getvalue().count('| Set |'), 1)

    def test_mixed_settings_get_separate_tables_and_no_cross_pairing(self):
        variants = [SETTINGS, dict(SETTINGS, temperature=0.1), dict(SETTINGS, seed=43),
                    dict(SETTINGS, num_ctx=8192), {k: None for k in SETTINGS}, None]
        rows = []
        for index, settings in enumerate(variants):
            row = dict(format=1, set='v1', model='m', task='box', rep=1, mode='model',
                       through_pilot=bool(index % 2), **{'pass': True})
            if settings is not None:
                row['model_settings'] = settings
            rows.append(row)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'results.jsonl'
            path.write_text(''.join(json.dumps(row)+'\n' for row in rows))
            with contextlib.redirect_stdout(io.StringIO()) as output:
                report.report([path])
            text = output.getvalue()
            self.assertEqual(text.count('| Set |'), len(variants))
            self.assertIn('separate comparison tables', text)
            self.assertIn('legacy (settings unrecorded)', text)
            self.assertNotIn('pairs', text)

    def test_invalid_or_unsupported_settings_fail_before_loading_tasks(self):
        options = [['--temperature', 'nan'], ['--temperature', '0.3'], ['--temperature', '-1'],
                   ['--seed', str(2**63)], ['--num-ctx', '8191'], ['--num-ctx', '131073'],
                   ['--num-ctx', '8192'], ['--endpoint-kind', 'anthropic', '--seed', '1'],
                   ['--agent', 'codex', '--seed', '1'], ['--verify-reference', '--seed', '1']]
        for extra in options:
            with self.subTest(extra=extra), patch.object(sys, 'argv', ['run.py', '--solidcraft', 'unused', '--out', '/tmp/unused'] + ([] if '--agent' in extra or '--verify-reference' in extra else ['--model', 'm']) + extra), patch('run.task_file') as tasks, contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                try:
                    run.main()
                finally:
                    tasks.assert_not_called()


if __name__ == '__main__':
    unittest.main()
