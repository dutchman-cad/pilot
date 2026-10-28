# SPDX-License-Identifier: Apache-2.0
import contextlib
import http.server
import io
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import threading
import time
from types import SimpleNamespace
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import agents
import report
import run


class CAD:
    tools = [{'name': 'execute', 'description': 'Run a CAD command',
              'inputSchema': {'type': 'object'}}]
    info = {'instructions': 'Use SolidCraft tools.', 'serverInfo': {'name': 'fake', 'version': '1'}}

    def __init__(self):
        self.deadline = time.monotonic() + 10
        self.calls = []

    def __enter__(self):
        return self

    def __exit__(self, *args):
        pass

    def tool(self, name, arguments):
        self.calls.append((name, arguments))
        return {'content': [{'type': 'text', 'text': '{}'}]}

    def rpc(self, method, params):
        return self.tool(params['name'], params['arguments'])

    def call(self, method, params):
        return {'bodies': [{'name': 'Box', 'volume_mm3': 8000}] if self.calls else []}


@contextlib.contextmanager
def fake_api(responses, captured):
    class Handler(http.server.BaseHTTPRequestHandler):
        def log_message(self, *args):
            pass

        def do_POST(self):
            captured.append((self.path, dict(self.headers), json.loads(self.rfile.read(int(self.headers['Content-Length'])))))
            value = responses.pop(0)
            if 'redirect' in value:
                self.send_response(302)
                self.send_header('Location', value['redirect'])
                self.end_headers()
                return
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            self.end_headers()
            self.wfile.write(json.dumps(value).encode())
    with http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler) as server:
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        try:
            yield 'http://127.0.0.1:' + str(server.server_port) + '/v1'
        finally:
            server.shutdown()
            thread.join()


FAKE_AGENT = '''#!/usr/bin/env python3
import json, os, pathlib, subprocess, sys, re
if '--version' in sys.argv:
    print('fake-agent 1.2.3')
    sys.exit(0)
assert 'BENCH_API_KEY' not in os.environ
assert 'ANTHROPIC_API_KEY' not in os.environ
prompt = sys.stdin.read()
if '-p' in sys.argv:
    path = pathlib.Path(sys.argv[sys.argv.index('--mcp-config')+1])
    config = json.loads(path.read_text())['mcpServers']['solidcraft']
else:
    table = sys.argv[sys.argv.index('-c')+1].split('=',1)[1]
    config = json.loads(table.replace('command=', '"command":').replace('args=', '"args":'))
    path = pathlib.Path(sys.argv[sys.argv.index('--output-last-message')+1]).parent/'mcp.toml'
pathlib.Path(__file__+'.capture').write_text(json.dumps({'argv':sys.argv, 'config':str(path), 'prompt':prompt}))
p = subprocess.Popen([config['command']] + config['args'], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
for i, (method, params) in enumerate([('initialize', {}), ('tools/list', {}), ('tools/call', {'name':'execute','arguments':{'command':'solid.box','params':{'size':20}}})]):
    p.stdin.write(json.dumps({'jsonrpc':'2.0','id':i,'method':method,'params':params})+'\\n')
    p.stdin.flush()
    assert 'result' in json.loads(p.stdout.readline())
p.stdin.close()
p.wait(timeout=5)
if '--output-last-message' in sys.argv:
    pathlib.Path(sys.argv[sys.argv.index('--output-last-message')+1]).write_text('Done')
print('Done')
'''


class EndpointAgentTests(unittest.TestCase):
    def test_anthropic_tool_loop_and_key_precedence(self):
        for env, expected in [({'ANTHROPIC_API_KEY': 'fallback'}, 'fallback'),
                              ({'BENCH_API_KEY': 'primary', 'ANTHROPIC_API_KEY': 'fallback'}, 'primary')]:
            with self.subTest(env=env):
                captured = []
                responses = [{'content': [{'type': 'text', 'text': 'Building'},
                    {'type': 'tool_use', 'id': 'one', 'name': 'execute', 'input': {'command': 'solid.box'}},
                    {'type': 'tool_use', 'id': 'two', 'name': 'execute', 'input': {'command': 'solid.box'}}]},
                    {'content': [{'type': 'text', 'text': 'Done'}]}]
                cad = CAD()
                with patch.dict(os.environ, env, clear=True), fake_api(responses, captured) as url:
                    reply, _ = run.converse(cad, [{'role': 'system', 'content': 'CAD instructions'}], 'Make a box', 'chosen', url, 'anthropic')
                self.assertEqual(reply, 'Done')
                self.assertEqual(len(cad.calls), 2)
                path, headers, body = captured[0]
                self.assertEqual(path, '/v1/messages')
                self.assertEqual(headers['X-Api-Key'], expected)
                self.assertEqual(headers['Anthropic-Version'], '2023-06-01')
                self.assertNotIn('Authorization', headers)
                self.assertEqual(body['system'], 'CAD instructions')
                self.assertEqual(body['tools'][0]['input_schema'], cad.tools[0]['inputSchema'])
                self.assertNotIn('seed', body)
                history = captured[1][2]['messages']
                self.assertEqual([b['tool_use_id'] for b in history[-1]['content']], ['one', 'two'])
                self.assertEqual(history[-2]['content'][1]['input'], {'command': 'solid.box'})

    def test_http_failure_does_not_echo_secret(self):
        with patch.dict(os.environ, {'BENCH_API_KEY': 'secret'}), patch('run.open_model', side_effect=OSError('secret in URL')):
            with self.assertRaisesRegex(RuntimeError, '^model HTTP request failed$'):
                run.converse(CAD(), [], 'box', 'm', 'http://local/v1', 'anthropic')
        with patch.dict(os.environ, {'BENCH_API_KEY': 'secret'}):
            with self.assertRaisesRegex(ValueError, 'must not contain'):
                run.endpoint_url('http://local/secret')

    def test_redirect_never_forwards_key(self):
        for kind in ('openai', 'anthropic'):
            captured = []
            with patch.dict(os.environ, {'BENCH_API_KEY': 'private-key'}), fake_api([{'redirect': '/leak'}], captured) as url:
                with self.assertRaisesRegex(RuntimeError, 'status 302'):
                    run.converse(CAD(), [], 'box', 'model', url, kind)
            self.assertEqual(len(captured), 1)

    @unittest.skipUnless(os.name == 'posix', 'fake executable uses a POSIX shebang')
    def test_timeout_removes_mcp_config(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory)/'agent'
            capture = Path(directory)/'config-path'
            binary.write_text('#!/usr/bin/env python3\nimport pathlib,sys,time\n'
                              'pathlib.Path(' + repr(str(capture)) + ').write_text(sys.argv[sys.argv.index("--mcp-config")+1])\n'
                              'time.sleep(60)\n')
            binary.chmod(0o700)
            cad = CAD()
            cad.deadline = time.monotonic()+10
            clock = time.monotonic
            def now():
                return cad.deadline+1 if capture.exists() else clock()
            args = SimpleNamespace(agent='claude-code', agent_bin=str(binary))
            with patch('agents.time.monotonic', side_effect=now), self.assertRaises(TimeoutError):
                agents.converse(cad, [], 'Make a box', args)
            self.assertFalse(Path(capture.read_text()).exists())

    def test_agent_tools_score_same_design_and_delete_configs(self):
        with tempfile.TemporaryDirectory() as directory:
            binary = Path(directory)/'fake-agent'
            binary.write_text(FAKE_AGENT)
            binary.chmod(0o700)
            root = Path(directory)
            run.save(root/'reference.json', {'commands': []})
            task = {'id': 'box', 'prompt': 'Make a box', 'reference': 'reference.json', 'check': {'bodies': 1}}
            for agent in ('claude-code', 'codex'):
                args = SimpleNamespace(agent=agent, agent_bin=str(binary), solidcraft='unused', through_pilot='pilot', replay_setup=True)
                with patch.dict(os.environ, {'BENCH_API_KEY': 'never-print-this'}):
                    self.assertEqual(agents.version(args), 'fake-agent 1.2.3')
                    cad = CAD()
                    with patch('run.MCP', return_value=cad) as mcp:
                        result = run.run_one(args, root, task, 'model', None)
                    self.assertTrue(result['pass'], result['reason'])
                    mcp.assert_called_once_with('unused', None, 'pilot')
                    capture = json.loads(Path(str(binary)+'.capture').read_text())
                    self.assertNotIn('never-print-this', json.dumps(capture))
                    self.assertFalse(Path(capture['config']).exists())
                    self.assertEqual(result['final']['reply'], 'Done')
                    self.assertEqual(len(cad.calls), 1)

    @unittest.skipUnless(os.name == 'posix', 'process group test uses POSIX')
    def test_timeout_kills_agent_and_child(self):
        with tempfile.TemporaryDirectory() as directory:
            pidfile = Path(directory)/'pids'
            code = ('import os,subprocess,sys,time; '
                    'p=subprocess.Popen([sys.executable,"-c","import time;time.sleep(60)"]); '
                    'open(sys.argv[1],"w").write(str(os.getpid())+" "+str(p.pid));time.sleep(60)')
            deadline = time.monotonic()+10
            clock = time.monotonic
            def now():
                return deadline+1 if pidfile.exists() else clock()
            with patch('agents.time.monotonic', side_effect=now), self.assertRaises(TimeoutError):
                agents.execute([sys.executable, '-c', code, str(pidfile)], '', deadline)
            for pid in map(int, pidfile.read_text().split()):
                stat = Path('/proc')/str(pid)/'stat'
                for _ in range(200):
                    if not stat.exists() or stat.read_text().split()[2] == 'Z':
                        break
                    time.sleep(0.01)
                self.assertTrue(not stat.exists() or stat.read_text().split()[2] == 'Z')

    def test_reports_and_output_redact_echoed_key(self):
        with tempfile.TemporaryDirectory() as directory:
            argv = ['run.py', '--model', 'm', '--solidcraft', 'unused', '--out', directory]
            record = {'task': 'box', 'pass': False, 'matched': False, 'reason': 'echo secret-key',
                      'turns': [{'response': {'text': 'secret-key'}}]}
            with patch.dict(os.environ, {'BENCH_API_KEY': 'secret-key'}), patch.object(sys, 'argv', argv), patch('run.task_file', return_value=[{'id':'box'}]), patch('run.run_one', return_value=record), contextlib.redirect_stdout(io.StringIO()) as output:
                self.assertEqual(run.main(), 1)
            self.assertNotIn('secret-key', output.getvalue())
            for path in (Path(directory)/'results.jsonl', Path(directory)/'01-box.json'):
                self.assertNotIn('secret-key', path.read_text())
                self.assertIn('[REDACTED]', path.read_text())
            self.assertEqual(json.loads((Path(directory)/'results.jsonl').read_text())['reached_via'], 'api-openai')

    def test_argument_errors_redact_environment_key(self):
        with patch.dict(os.environ, {'BENCH_API_KEY': 'private-key'}), patch.object(sys, 'argv', ['run.py', '--solidcraft', 'unused', '--out', '/tmp/unused', '--endpoint-kind', 'private-key']), contextlib.redirect_stderr(io.StringIO()) as output:
            with self.assertRaises(SystemExit):
                run.main()
        self.assertNotIn('private-key', output.getvalue())
        self.assertIn('[REDACTED]', output.getvalue())

    def test_report_separates_protocols_agents_and_versions(self):
        rows = []
        for via, version in [('api-openai', ''), ('api-anthropic', ''), ('agent-codex', 'codex 1'), ('agent-codex', 'codex 2'), ('agent-claude-code', 'claude 1')]:
            rows.append(dict(format=1, set='v1', model='same', task='box', rep=1, through_pilot=False, mode='model', reached_via=via, agent_version=version, **{'pass':True}))
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'results.jsonl'
            path.write_text(''.join(json.dumps(row)+'\n' for row in rows))
            with contextlib.redirect_stdout(io.StringIO()) as output:
                report.report([path])
            text = output.getvalue()
            self.assertEqual(text.count('| 1/1 |'), 5)
            self.assertIn('agent prompt; separate from API scores', text)
            self.assertNotIn('pairs', text)


if __name__ == '__main__':
    unittest.main()
