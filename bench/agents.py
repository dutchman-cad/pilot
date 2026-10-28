# SPDX-License-Identifier: Apache-2.0
"""Installed-agent execution and a bounded stdio bridge to the scored design."""
import contextlib
import json
import os
from pathlib import Path
import secrets
import signal
import socket
import socketserver
import subprocess
import sys
import tempfile
import threading
import time

MAX_BYTES = 16 * 1024 * 1024


def kill_tree(process):
    if os.name == 'posix':
        with contextlib.suppress(ProcessLookupError):
            os.killpg(process.pid, signal.SIGKILL)
    else:
        subprocess.run(['taskkill', '/PID', str(process.pid), '/T', '/F'],
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, timeout=5)
    process.wait(timeout=5)


def execute(command, prompt, deadline):
    for name in ('BENCH_API_KEY', 'ANTHROPIC_API_KEY'):
        key = os.environ.get(name)
        if key and any(key in argument for argument in command):
            raise ValueError('agent command must not contain an API key')
    # Inherit the user's agent sign-in, but do not expose benchmark keys to agents.
    env = {k: v for k, v in os.environ.items()
           if k not in ('BENCH_API_KEY', 'ANTHROPIC_API_KEY')}
    process = subprocess.Popen(command, stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                               stderr=subprocess.STDOUT, env=env,
                               start_new_session=os.name == 'posix')
    chunks = []
    overflow = threading.Event()
    if len(prompt.encode()) > MAX_BYTES:
        kill_tree(process)
        process.stdin.close()
        process.stdout.close()
        raise ValueError('agent prompt exceeds 16 MiB')

    def read():
        total = 0
        while True:
            chunk = process.stdout.read(65536)
            if not chunk:
                break
            total += len(chunk)
            if total > MAX_BYTES:
                overflow.set()
                break
            chunks.append(chunk)

    def write():
        with contextlib.suppress(OSError):
            process.stdin.write(prompt.encode())
            process.stdin.close()

    reader = threading.Thread(target=read, daemon=True)
    writer = threading.Thread(target=write, daemon=True)
    reader.start()
    writer.start()
    try:
        while process.poll() is None or reader.is_alive():
            if overflow.is_set():
                raise ValueError('agent output exceeds 16 MiB')
            if time.monotonic() >= deadline:
                raise TimeoutError('agent time budget exhausted')
            time.sleep(0.02)
        if overflow.is_set():
            raise ValueError('agent output exceeds 16 MiB')
        if process.returncode:
            raise RuntimeError('agent exited unsuccessfully')
        return b''.join(chunks).decode('utf-8', errors='replace').strip()
    finally:
        kill_tree(process)
        reader.join(timeout=5)
        writer.join(timeout=5)
        process.stdout.close()
        if not process.stdin.closed:
            process.stdin.close()


def binary(args):
    return args.agent_bin or ('claude' if args.agent == 'claude-code' else 'codex')


def version(args):
    return execute([binary(args), '--version'], '', time.monotonic() + 10)


class Bridge(socketserver.ThreadingTCPServer):
    allow_reuse_address = False
    daemon_threads = True


def converse(app, messages, prompt, args):
    """Agents launch a stdio client; all calls reach the harness's live MCP server."""
    token = secrets.token_hex(32)
    transcript = []
    lock = threading.Lock()
    calls = 0
    transcript_bytes = 0
    failure = None

    class Handler(socketserver.StreamRequestHandler):
        def handle(self):
            nonlocal calls, transcript_bytes, failure
            self.connection.settimeout(max(0.01, app.deadline - time.monotonic()))
            if self.rfile.readline(256).strip().decode() != token:
                return
            for _ in range(10000):
                line = self.rfile.readline(MAX_BYTES + 1)
                if not line or len(line) > MAX_BYTES:
                    return
                request = json.loads(line)
                if 'id' not in request:
                    continue
                method = request.get('method')
                with lock:
                    try:
                        if time.monotonic() >= app.deadline:
                            raise TimeoutError('agent tool time budget exhausted')
                        if method == 'initialize':
                            result = app.info
                        elif method == 'tools/list':
                            result = {'tools': app.tools}
                        elif method == 'ping':
                            result = {}
                        elif method == 'tools/call':
                            calls += 1
                            if calls > 128 or failure:
                                failure = 'agent exceeded tool or transcript limit'
                                raise ValueError(failure)
                            params = request['params']
                            if params['name'] not in {t['name'] for t in app.tools}:
                                raise ValueError('unknown tool')
                            arguments = params.get('arguments', {})
                            if not isinstance(arguments, dict):
                                raise ValueError('tool arguments must be an object')
                            if params['name'] == 'batch' and len(arguments.get('commands', [])) > 1024:
                                raise ValueError('batch exceeds 1024 commands')
                            result = app.rpc(method, params)
                            entry = {'tool': params['name'], 'arguments': arguments, 'result': result}
                            transcript_bytes += len(json.dumps(entry).encode())
                            if transcript_bytes > MAX_BYTES:
                                failure = 'agent transcript exceeds 16 MiB'
                                raise ValueError(failure)
                            transcript.append(entry)
                        else:
                            raise ValueError('unsupported agent MCP method')
                        response = {'jsonrpc': '2.0', 'id': request['id'], 'result': result}
                    except Exception:
                        response = {'jsonrpc': '2.0', 'id': request['id'],
                                    'error': {'code': -32000, 'message': 'agent MCP call failed or limit exceeded'}}
                data = json.dumps(response).encode()
                if len(data) > MAX_BYTES:
                    return
                self.wfile.write(data + b'\n')
                self.wfile.flush()

    with Bridge(('127.0.0.1', 0), Handler) as server, tempfile.TemporaryDirectory(prefix='pilot-agent-') as directory:
        thread = threading.Thread(target=server.serve_forever, kwargs={'poll_interval': 0.05}, daemon=True)
        thread.start()
        try:
            config = {'command': sys.executable, 'args': [str(Path(__file__).resolve()),
                      str(server.server_address[1]), token]}
            path = Path(directory) / 'mcp.json'
            path.write_text(json.dumps({'mcpServers': {'solidcraft': config}}))
            if args.agent == 'claude-code':
                command = [binary(args), '-p', '--mcp-config', str(path), '--strict-mcp-config',
                           '--allowedTools', 'mcp__solidcraft__*']
            else:
                # Codex's documented -c accepts TOML inline tables. Keep CODEX_HOME
                # intact so the installed agent retains its own sign-in and model.
                table = '{command=' + json.dumps(config['command']) + ',args=' + json.dumps(config['args']) + '}'
                path = Path(directory) / 'mcp.toml'
                path.write_text('mcp_servers.solidcraft=' + table + '\n')
                last = Path(directory) / 'reply.txt'
                command = [binary(args), 'exec', '--skip-git-repo-check',
                           '-c', path.read_text().strip(), '-c', 'approval_policy="never"',
                           '--output-last-message', str(last), '-']
            messages.append({'role': 'user', 'content': prompt})
            text = '\n'.join(m['role'] + ': ' + (m.get('content') or '') for m in messages)
            if len(text.encode()) > MAX_BYTES:
                raise ValueError('agent prompt exceeds 16 MiB')
            reply = execute(command, text, app.deadline)
            # Finish any tool request already in flight before the checker uses MCP.
            if not lock.acquire(timeout=max(0, app.deadline - time.monotonic())):
                raise TimeoutError('agent MCP time budget exhausted')
            lock.release()
            if args.agent == 'codex':
                with last.open('rb') as stream:
                    data = stream.read(MAX_BYTES + 1)
                if len(data) > MAX_BYTES:
                    raise ValueError('agent reply exceeds 16 MiB')
                reply = data.decode('utf-8')
            if failure:
                raise ValueError(failure)
            messages.append({'role': 'assistant', 'content': reply})
            return reply, transcript
        finally:
            server.shutdown()
            thread.join(timeout=5)


def bridge_stdio(port, token):
    with socket.create_connection(('127.0.0.1', int(port)), timeout=600) as connection:
        connection.sendall(token.encode() + b'\n')
        stream = connection.makefile('rb')
        for _ in range(10000):
            line = sys.stdin.buffer.readline(MAX_BYTES + 1)
            if not line:
                return
            if len(line) > MAX_BYTES:
                raise ValueError('agent MCP message exceeds 16 MiB')
            request = json.loads(line)
            connection.sendall(line)
            if 'id' in request:
                reply = stream.readline(MAX_BYTES + 1)
                if not reply or len(reply) > MAX_BYTES:
                    raise ValueError('invalid agent MCP response')
                sys.stdout.buffer.write(reply)
                sys.stdout.buffer.flush()


if __name__ == '__main__':
    try:
        bridge_stdio(*sys.argv[1:])
    except Exception:
        sys.exit(1)
