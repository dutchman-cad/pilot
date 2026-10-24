# SPDX-License-Identifier: Apache-2.0
"""Exercise native calls and strict transport behavior without a model server."""
import contextlib
import io
import tempfile
import json
from pathlib import Path
import sys
import time
import unittest
from unittest.mock import patch

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
        with patch('run.urllib.request.urlopen', side_effect=responses):
            reply, _ = run.converse(cad, messages, 'Make a box', 'model', 'http://example.invalid/v1')
        self.assertEqual(reply, 'Done')
        self.assertEqual(len(cad.calls), 1)
        self.assertEqual(messages[2]['role'], 'tool')
        self.assertEqual(messages[2]['tool_call_id'], 'one')

    def test_prose_json_never_executes(self):
        cad = FakeCAD()
        with patch('run.urllib.request.urlopen', return_value=answer({'role':'assistant','content':'{"tool":"execute","args":{}}'})):
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
                self.assertEqual(child.call_args.args[0], ['pilot','model','--listen','127.0.0.1:12345','--upstream','http://example.invalid/v1'])
                child.return_value.terminate.assert_called_once()

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


if __name__ == '__main__':
    unittest.main()
