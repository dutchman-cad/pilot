#!/usr/bin/env python3
"""Scripted OpenAI-compatible HTTP peer. Replays replies; never loads a model."""
import argparse
import json
from collections import deque
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
import threading


class Replay:
    def __init__(self, replies):
        if not isinstance(replies, list) or len(replies) > 800:
            raise ValueError('expected a list of at most 800 scripted replies')
        if any(not isinstance(r, (str, dict)) or len(json.dumps(r)) > 65536 for r in replies):
            raise ValueError('each reply must be a string/object of at most 64 KiB')
        self.replies = deque(replies)
        self.requests = []
        self.lock = threading.Lock()
        replay = self

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                length = int(self.headers.get('Content-Length', '0'))
                if self.path != '/v1/chat/completions' or not 0 < length <= 1048576:
                    self.send_error(400, 'invalid path or body size')
                    return
                try:
                    body = json.loads(self.rfile.read(length))
                except (ValueError, OSError):
                    self.send_error(400, 'invalid JSON')
                    return
                with replay.lock:
                    if len(replay.requests) >= 800:
                        self.send_error(429, 'script request cap')
                        return
                    replay.requests.append(body)
                    message = replay.replies.popleft() if replay.replies else 'COMMAND: done scripted replies exhausted'
                if isinstance(message, str):
                    message = {'role': 'assistant', 'content': message}
                data = json.dumps({'id': 'scripted', 'object': 'chat.completion',
                                   'choices': [{'index': 0, 'message': message, 'finish_reason': 'tool_calls' if message.get('tool_calls') else 'stop'}],
                                   'usage': {'total_tokens': 100}}).encode()
                self.send_response(200)
                self.send_header('Content-Type', 'application/json')
                self.send_header('Content-Length', str(len(data)))
                self.end_headers()
                self.wfile.write(data)

            def log_message(self, *_args):
                pass

        self.server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.server.daemon_threads = True
        self.server.timeout = 1
        self.url = 'http://127.0.0.1:%s/v1' % self.server.server_address[1]
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def close(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=2)

    def __enter__(self):
        return self

    def __exit__(self, *_args):
        self.close()


TASK_ONE = ['COMMAND: ' + command for command in (
    'menu Sketch > Rectangle', 'type 20', 'key tab', 'type 20', 'key enter',
    'menu Features > Extrude', 'type 20', 'key enter', 'done Made a 20 mm cube.')]


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--replies', type=Path, help='JSON array of strings or assistant message objects; default task 1')
    p.add_argument('--port-file', type=Path)
    args = p.parse_args()
    if args.replies and args.replies.stat().st_size > 1048576:
        p.error('script exceeds 1 MiB')
    replies = json.loads(args.replies.read_text()) if args.replies else TASK_ONE
    with Replay(replies) as peer:
        print(peer.url, flush=True)
        if args.port_file:
            args.port_file.write_text(peer.url + '\n')
        try:
            peer.thread.join()
        except KeyboardInterrupt:
            pass


if __name__ == '__main__':
    main()
