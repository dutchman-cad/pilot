#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Bounded headless SolidCraft Assistant benchmark, using public MCP tools."""
import argparse
import contextlib
import json
import os
from pathlib import Path
import queue
import socket
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.parse
import urllib.request

import check

HERE = Path(__file__).resolve().parent
MAX_BYTES = 16 * 1024 * 1024
MAX_COMMANDS = 1024
MAX_REQUESTS = 24
MAX_TOOLS = 128
TIMEOUT = 600


def encode(value):
    data = json.dumps(value, allow_nan=False).encode()
    if len(data) > MAX_BYTES:
        raise ValueError("JSON exceeds 16 MiB limit")
    return data


def load(path):
    with Path(path).open('rb') as stream:
        data = stream.read(MAX_BYTES + 1)
    if len(data) > MAX_BYTES:
        raise ValueError("file exceeds 16 MiB limit")
    return json.loads(data)


def save(path, value):
    data = encode(value)
    Path(path).parent.mkdir(parents=True, exist_ok=True)
    Path(path).write_bytes(data + b'\n')


def task_file(path):
    value = load(path)
    if value.get('format') != 1:
        raise ValueError("unsupported task format; expected format 1")
    tasks = value['tasks']
    if not 0 <= len(tasks) <= 256:
        raise ValueError("task count exceeds 256")
    ids = set()
    for task in tasks:
        name = task['id']
        if not name or len(name) > 100 or any(c not in 'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-_' for c in name):
            raise ValueError("task id must be a safe filename of at most 100 characters")
        if name in ids:
            raise ValueError("duplicate task id")
        ids.add(name)
        turns = task.get('turns', [])
        if len(turns) > 8:
            raise ValueError("at most eight user turns are supported")
        if turns and task['prompt'] != turns[0]['prompt']:
            raise ValueError("top-level prompt must equal the first turn prompt")
    return tasks


def reference(root, task):
    path = (root / task['reference']).resolve()
    if root.resolve() not in path.parents:
        raise ValueError("reference must stay inside its set directory")
    commands = load(path)['commands']
    if len(commands) > MAX_COMMANDS:
        raise ValueError("reference exceeds 1024 commands")
    return commands


def setup(root, task, commands):
    count = task.get('setup_steps', 0)
    if not isinstance(count, int) or not 0 <= count <= len(commands):
        raise ValueError("invalid setup_steps")
    return task.get('setup', commands[:count])


def start_path(root, task):
    if not task.get('start'):
        return None
    path = (root / task['start']).resolve()
    if root.resolve() not in path.parents or path.suffix != '.solidcraft':
        raise ValueError("start must be a .solidcraft file inside its set directory")
    if not path.is_file() or path.stat().st_size > MAX_BYTES:
        raise ValueError("start is missing or exceeds 16 MiB")
    return path


def verify_start(binary, prefix, state):
    with MCP(binary) as baseline:
        replay(baseline, prefix)
        check.compare_start(baseline.call('document.inspect', {'measure': True}), state)


class MCP:
    """One CLI process and one isolated design per task; bounded stdio JSON RPC."""
    def __init__(self, binary, start=None):
        self.binary = binary
        self.start = start
        self.process = None
        self.messages = queue.Queue(maxsize=256)
        self.sequence = 0
        self.deadline = time.monotonic() + TIMEOUT

    def __enter__(self):
        command = [self.binary, 'mcp']
        if self.start is not None:
            command.extend(['--in', str(self.start)])
        self.process = subprocess.Popen(command, stdin=subprocess.PIPE,
                                        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        threading.Thread(target=self._read, daemon=True).start()
        try:
            self.info = self.rpc('initialize', {'protocolVersion': '2024-11-05',
                'capabilities': {}, 'clientInfo': {'name': 'pilot-benchmark', 'version': '1'}})
            self.send({'jsonrpc': '2.0', 'method': 'notifications/initialized'})
            self.tools = self.rpc('tools/list', {})['tools']
            if len(self.tools) > MAX_TOOLS:
                raise ValueError("SolidCraft advertises more than 128 tools")
            return self
        except Exception:
            self.__exit__(None, None, None)
            raise

    def _read(self):
        try:
            for _ in range(10000):
                line = self.process.stdout.readline(MAX_BYTES + 1)
                if not line:
                    raise RuntimeError("SolidCraft closed its output")
                if len(line) > MAX_BYTES:
                    raise ValueError("SolidCraft response exceeds 16 MiB")
                self.messages.put(json.loads(line), timeout=1)
            raise ValueError("SolidCraft message count exceeded 10000")
        except Exception as error:
            with contextlib.suppress(queue.Full):
                self.messages.put(error, timeout=1)

    def send(self, message):
        self.process.stdin.write(encode(message) + b'\n')
        self.process.stdin.flush()

    def rpc(self, method, params):
        self.sequence += 1
        self.send({'jsonrpc': '2.0', 'id': self.sequence, 'method': method, 'params': params})
        for _ in range(256):
            remaining = self.deadline - time.monotonic()
            if remaining <= 0:
                raise TimeoutError("SolidCraft time budget exhausted")
            try:
                reply = self.messages.get(timeout=remaining)
            except queue.Empty as error:
                raise TimeoutError("SolidCraft did not respond before the deadline") from error
            if isinstance(reply, Exception):
                raise reply
            if reply.get('id') != self.sequence:
                continue
            if 'error' in reply:
                raise RuntimeError(f"MCP {method}: {reply['error']}")
            return reply['result']
        raise RuntimeError("too many unrelated MCP messages")

    def tool(self, name, arguments):
        result = self.rpc('tools/call', {'name': name, 'arguments': arguments})
        if result.get('isError'):
            raise RuntimeError(f"tool {name}: {result.get('content')}")
        return result

    @staticmethod
    def unpack(result):
        if 'structuredContent' in result:
            return result['structuredContent']
        text = '\n'.join(c.get('text', '') for c in result.get('content', []) if c.get('type') == 'text')
        try:
            return json.loads(text)
        except json.JSONDecodeError as error:
            raise ValueError("SolidCraft returned a non-JSON measurement") from error

    def execute(self, command, params):
        value = self.unpack(self.tool('execute', {'command': command, 'params': params}))
        if isinstance(value, dict) and (value.get('error') or value.get('ok') is False):
            raise RuntimeError(f"command {command} failed: {value}")
        return value

    def call(self, method, params):
        if method != 'document.inspect':
            raise ValueError("unsupported measurement method")
        return self.unpack(self.tool('inspect_design', params))

    def __exit__(self, *args):
        if self.process is not None:
            self.process.terminate()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.kill()
                self.process.wait(timeout=5)
            self.process.stdin.close()
            self.process.stdout.close()


class Queries:
    def __init__(self, app):
        self.app = app
        self.values = {}
        self.errors = []

    def execute(self, command, params):
        key = check.query_key(command, params)
        if key not in self.values:
            try:
                self.values[key] = self.app.execute(command, params)
            except Exception as error:
                self.errors.append(str(error))
                raise
        return self.values[key]


def score(app, item, before, reply):
    state = check.inspect_for_task(app, item)
    queries = Queries(app)
    # Read supplementary data once, then score from the recording alone.
    check.run_checks_v2(item['check'], queries, state, reply, before)
    if queries.errors:
        raise RuntimeError('measurement failed: ' + '; '.join(queries.errors))
    ok, reason = check.evaluate(item['check'], state, reply, before, queries.values)
    return {'pass': ok, 'reason': reason, 'document': state, 'before': before,
            'reply': reply, 'queries': queries.values, 'check': item['check']}


def replay(app, commands):
    if len(commands) > MAX_COMMANDS:
        raise ValueError("setup exceeds 1024 commands")
    for step in commands:
        app.execute(step['command'], step.get('params', {}))


def endpoint_url(url):
    parsed = urllib.parse.urlsplit(url)
    if parsed.scheme not in ('http', 'https') or not parsed.netloc or parsed.username or parsed.password or parsed.query or parsed.fragment:
        raise ValueError("endpoint must be an HTTP(S) URL without credentials, query or fragment")
    base = url.rstrip('/')
    if base.endswith('/api'):
        base = base[:-4]
    return base if base.endswith('/v1') else base + '/v1'


@contextlib.contextmanager
def endpoint(args, out):
    upstream = endpoint_url(args.endpoint)
    if not args.through_pilot:
        yield upstream
        return
    with socket.socket() as sock:
        sock.bind(('127.0.0.1', 0))
        port = sock.getsockname()[1]
    with (out / 'pilot.log').open('wb') as log:
        child = subprocess.Popen([args.through_pilot, 'model', '--listen', f'127.0.0.1:{port}',
                                  '--upstream', upstream], stdout=log, stderr=log)
        try:
            for _ in range(100):
                if child.poll() is not None:
                    raise RuntimeError("Pilot exited during startup; see pilot.log")
                try:
                    with socket.create_connection(('127.0.0.1', port), timeout=0.2):
                        break
                except OSError:
                    time.sleep(0.1)
            else:
                raise RuntimeError("Pilot did not listen within 30 seconds")
            yield f'http://127.0.0.1:{port}/v1'
        finally:
            child.terminate()
            try:
                child.wait(timeout=5)
            except subprocess.TimeoutExpired:
                child.kill()
                child.wait(timeout=5)


def converse(app, messages, prompt, model, url):
    messages.append({'role': 'user', 'content': prompt})
    tools = [{'type': 'function', 'function': {'name': t['name'],
        'description': t.get('description', ''), 'parameters': t['inputSchema']}} for t in app.tools]
    transcript = []
    calls_count = 0
    for _ in range(MAX_REQUESTS):
        remaining = app.deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError("turn exceeded 600 seconds")
        payload = {'model': model, 'messages': messages, 'tools': tools, 'stream': False,
                   'temperature': 0, 'seed': 1, 'max_tokens': 4096}
        request = urllib.request.Request(url + '/chat/completions', data=encode(payload),
                                         headers={'Content-Type': 'application/json'})
        key = os.environ.get('BENCH_API_KEY')
        if key:
            request.add_header('Authorization', 'Bearer ' + key)
        with urllib.request.urlopen(request, timeout=remaining) as response:
            raw = response.read(MAX_BYTES + 1)
        if len(raw) > MAX_BYTES:
            raise ValueError("model response exceeds 16 MiB")
        result = json.loads(raw)
        message = result['choices'][0]['message']
        # Keep only standard chat-completions fields in the next request.
        message = {k: v for k, v in message.items() if k in ('role', 'content', 'tool_calls')}
        messages.append(message)
        transcript.append({'response': result})
        tool_calls = message.get('tool_calls') or []
        if not tool_calls:
            return message.get('content') or '', transcript
        calls_count += len(tool_calls)
        if calls_count > MAX_TOOLS:
            raise ValueError("turn exceeded 128 tool calls")
        allowed = {t['name'] for t in app.tools}
        for call in tool_calls:
            function = call['function']
            try:
                if function['name'] not in allowed:
                    raise ValueError("unknown tool")
                arguments = json.loads(function['arguments'])
                if not isinstance(arguments, dict):
                    raise ValueError("tool arguments must be an object")
                if function['name'] == 'batch' and len(arguments.get('commands', [])) > MAX_COMMANDS:
                    raise ValueError("batch exceeds 1024 commands")
                value = app.tool(function['name'], arguments)
            except (ValueError, RuntimeError) as error:
                value = {'isError': True, 'content': [{'type': 'text', 'text': str(error)}]}
            transcript.append({'tool': function['name'], 'arguments': function['arguments'], 'result': value})
            messages.append({'role': 'tool', 'tool_call_id': call['id'], 'content': json.dumps(value)})
    raise RequestLimit(transcript)


class RequestLimit(RuntimeError):
    """The model used every request in a turn; the design is still scored as it stands."""

    def __init__(self, transcript):
        super().__init__("turn exceeded 24 model requests")
        self.transcript = transcript


def run_one(args, root, task, mode, url):
    started = time.monotonic()
    commands = reference(root, task)
    prefix = setup(root, task, commands)
    start = None if getattr(args, 'replay_setup', False) else start_path(root, task)
    with MCP(args.solidcraft, start) as app:
        if start is None:
            replay(app, prefix)
        elif mode != 'model':
            verify_start(args.solidcraft, prefix, app.call('document.inspect', {'measure': True}))
        initial = check.inspect_for_task(app, task)
        messages = [{'role': 'system', 'content': app.info.get('instructions', '')}]
        turns = task.get('turns') or [task]
        if task.get('turns'):
            ends = [t['reference_end'] for t in turns]
            if any(not isinstance(n, int) for n in ends) or ends != sorted(set(ends)) or ends[0] <= len(prefix) or ends[-1] != len(commands):
                raise ValueError("turn boundaries must increase and consume the complete reference")
        position = task.get('setup_steps', 0)
        records = []
        reply = ''
        for item in turns:
            stopped = None
            app.deadline = time.monotonic() + TIMEOUT
            before = check.inspect_for_task(app, item)
            transcript = []
            if mode in ('reference', 'negative'):
                end = item.get('reference_end', len(commands))
                replay(app, commands[position:end])
                position = end
                reply = item.get('reference_reply', task.get('reference_reply', ''))
            elif mode == 'model':
                try:
                    reply, transcript = converse(app, messages, item['prompt'], args.model, url)
                except RequestLimit as limit:
                    reply, transcript = '', limit.transcript
                    stopped = 'request limit'
            record = score(app, item, before, reply)
            record['transcript'] = transcript
            if stopped:
                record['stopped'] = stopped
            records.append(record)
        final = score(app, task, initial, reply)
        passed = all(r['pass'] for r in records) and final['pass']
        expected = mode not in ('negative', 'nothing')
        if mode == 'nothing':
            expectations = load(root / 'do-nothing.json')['expectations']
            expected = next(v['pass'] for v in expectations if v['id'] == task['id'])
        # Every multi-turn revision must reject doing nothing.
        matched = passed == expected
        if mode == 'nothing' and not expected:
            matched = not any(r['pass'] for r in records) and not final['pass']
        return {'task': task['id'], 'pass': passed, 'expected': expected, 'matched': matched,
                'reason': '; '.join(r['reason'] for r in records + [final] if not r['pass']) or 'ok',
                'turns': records, 'final': final, 'solidcraft': app.info['serverInfo'],
                'wall_s': round(time.monotonic() - started, 3)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--set', default='v1', choices=sorted(p.name for p in (HERE/'sets').iterdir() if p.is_dir() and p.name != 'sc98'))
    parser.add_argument('--model')
    parser.add_argument('--endpoint', default='http://127.0.0.1:11434')
    parser.add_argument('--through-pilot', metavar='PATH')
    parser.add_argument('--solidcraft', required=True, metavar='PATH')
    parser.add_argument('--replay-setup', action='store_true', help='replay setup commands instead of opening start files')
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument('--verify-reference', action='store_true')
    modes.add_argument('--do-nothing', action='store_true')
    modes.add_argument('--negatives', action='store_true')
    parser.add_argument('--reps', type=int, default=1)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    mode = 'reference' if args.verify_reference else 'nothing' if args.do_nothing else 'negative' if args.negatives else 'model'
    if not 1 <= args.reps <= 20:
        parser.error('--reps must be between 1 and 20')
    if mode == 'model' and not args.model:
        parser.error('--model is required for model runs')
    root = HERE/'sets'/args.set
    tasks = task_file(root / ('negatives/tasks.json' if args.negatives else 'tasks.json'))
    if args.negatives:
        # Near misses share a public starting design when their setup is identical.
        starts = {}
        for public in task_file(root / 'tasks.json'):
            if public.get('start'):
                prefix = setup(root, public, reference(root, public))
                starts[encode(prefix)] = public['start']
        for task in tasks:
            prefix = setup(root, task, reference(root, task))
            if encode(prefix) in starts:
                task.setdefault('start', starts[encode(prefix)])
    args.out.mkdir(parents=True, exist_ok=True)
    results = args.out/'results.jsonl'
    if results.exists():
        parser.error('--out already contains results.jsonl; use a new directory')
    failures = 0
    with results.open('w', encoding='utf-8') as output:
        manager = endpoint(args, args.out) if mode == 'model' else contextlib.nullcontext(None)
        with manager as url:
            for rep in range(1, args.reps + 1):
                for task in tasks:
                    try:
                        record = run_one(args, root, task, mode, url)
                    except Exception as error:
                        record = {'task': task['id'], 'pass': False, 'matched': False,
                                  'error': str(error), 'reason': 'execution error'}
                    record.update({'format': 1, 'set': args.set, 'mode': mode, 'rep': rep,
                                   'model': args.model, 'through_pilot': bool(args.through_pilot)})
                    artifact = f"{rep:02d}-{task['id']}.json"
                    save(args.out/artifact, record)
                    summary = {k: v for k, v in record.items() if k not in ('turns', 'final')}
                    summary['artifact'] = artifact
                    output.write(encode(summary).decode() + '\n')
                    output.flush()
                    failures += not record['matched']
                    print(f"{task['id']}: {'PASS' if record['matched'] else 'FAIL'} {record['reason']}", flush=True)
    return 1 if failures else 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (OSError, ValueError, RuntimeError, TimeoutError) as error:
        print(f"benchmark: {error}", file=sys.stderr)
        sys.exit(2)
