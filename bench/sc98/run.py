#!/usr/bin/env python3
"""Serial SolidCraft 98 A (plain) / P (Pilot) benchmark. Python standard library only."""
import argparse
import contextlib
import fcntl
import json
import os
from pathlib import Path
import re
import selectors
import socket
import subprocess
import threading
import time

from score import fields, score, timeline

HERE = Path(__file__).resolve().parent
LOCK = '/tmp/solidcraft-pilot-c.lock'


@contextlib.contextmanager
def benchmark_lock(timeout=900):
    """Bounded wait; never bypass a benchmark already holding the shared flock."""
    deadline = time.monotonic() + timeout
    with open(LOCK, 'a') as lock:
        while True:
            try:
                fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                break
            except BlockingIOError:
                if time.monotonic() >= deadline:
                    raise TimeoutError('shared benchmark lock is busy')
                time.sleep(min(0.25, max(0, deadline - time.monotonic())))
        yield


class Remote:
    def __init__(self, program, directory):
        directory = Path(directory).resolve()
        directory.mkdir(parents=True, exist_ok=False)
        (directory / 'Documents').mkdir()
        self.process = subprocess.Popen([str(Path(program).resolve()), '--remote', '--headless'],
                                        stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                                        stderr=subprocess.DEVNULL, cwd=directory,
                                        env=dict(os.environ, HOME=str(directory), USERPROFILE=str(directory)), bufsize=0)
        self.buffer = b''
        self.commands = 0
        self.lock = threading.Lock()
        self.selector = selectors.DefaultSelector()
        self.selector.register(self.process.stdout, selectors.EVENT_READ)

    def send(self, command):
        with self.lock:
            data = command.encode('cp1252')
            if len(data) > 4096 or any(c in command for c in '\r\n\0') or command.lstrip().split(' ', 1)[0] == 'quit':
                raise ValueError('invalid, oversized or shutdown command')
            if self.commands >= 1000:
                raise ValueError('benchmark remote command cap (1000)')
            self.commands += 1
            self.process.stdin.write(data + b'\n')
            self.process.stdin.flush()
            deadline = time.monotonic() + 15
            result = []
            size = 0
            while True:
                while b'\n' not in self.buffer:
                    if len(self.buffer) > 8192:
                        raise ValueError('remote line exceeds 8 KiB')
                    if not self.selector.select(max(0, deadline - time.monotonic())):
                        raise TimeoutError('remote answer deadline')
                    chunk = os.read(self.process.stdout.fileno(), 8192)
                    if not chunk:
                        raise EOFError('SolidCraft 98 closed mid-answer')
                    self.buffer += chunk
                line, self.buffer = self.buffer.split(b'\n', 1)
                line = line.rstrip(b'\r')
                if len(line) > 8192:
                    raise ValueError('remote line exceeds 8 KiB')
                if line == b'.':
                    return '\n'.join(result) + '\n'
                if line.startswith(b'..'):
                    line = line[1:]
                size += len(line) + 1
                result.append(line.decode('cp1252'))
                if size > 262144 or len(result) > 4096 or time.monotonic() >= deadline:
                    raise ValueError('remote answer cap/deadline')

    def close(self):
        self.process.stdin.close()
        try:
            self.process.wait(timeout=2)
        except subprocess.TimeoutExpired:
            self.process.kill()
            self.process.wait(timeout=2)
        self.process.stdout.close()
        self.selector.close()


def setup(remote, description):
    if not description:
        return
    match = re.fullmatch(r'box:(\d{1,4})x(\d{1,4})x(\d{1,4})', description)
    if not match or any(not 0 < int(n) <= 1000 for n in match.groups()):
        raise ValueError('unsupported setup')
    length, width, height = match.groups()
    for command in ('menu Sketch > Rectangle', f'type {length}', 'key tab', f'type {width}', 'key enter'):
        checked(remote, command)
    if 'Cal, the caliper' in remote.send('look'):
        checked(remote, 'click Just draw the box without help')
    for command in ('menu Features > Extrude', f'type {height}', 'key enter'):
        checked(remote, command)
    expected = f'{length} x {width} mm at (0, 0)'
    screen = remote.send('look')
    if expected not in screen or f'Extrude1 | Extrude | Sketch1, {height} mm | OK' not in screen:
        raise ValueError('box setup failed final-state verification')


def checked(remote, command):
    result = remote.send(command)
    if result.lower().startswith('error:'):
        raise ValueError(f'setup/probe {command}: {result}')
    return result


def snapshot(remote):
    screen = remote.send('look')
    snapshot = {'screen': screen, 'fields': {}, 'probes': [], 'incomplete': False}
    if 'Dialog "' in screen:
        snapshot['incomplete'] = True
        return snapshot
    for row in timeline(screen):
        if not re.fullmatch(r'(Sketch|Extrude|Revolve|Hole)\d{1,4}', row['name']):
            snapshot['incomplete'] = True
            continue
        checked(remote, 'doubleclick ' + row['name'])
        probe = remote.send('look')
        snapshot['probes'].append({'feature': row['name'], 'screen': probe})
        snapshot['fields'][row['name']] = fields(probe)
        # Cancel every read-only field inspection; never apply a scorer edit.
        if 'Dialog "' in probe:
            checked(remote, 'key esc')
        else:
            snapshot['incomplete'] = True
    return snapshot


class Bridge:
    """Expose the already-set-up pipe session to drive98 over loopback TCP."""
    def __init__(self, remote):
        self.remote = remote
        self.socket = socket.socket()
        self.socket.bind(('127.0.0.1', 0))
        self.socket.listen(1)
        self.socket.settimeout(0.2)
        self.address = '%s:%s' % self.socket.getsockname()
        self.stop = threading.Event()
        self.error = None
        self.thread = threading.Thread(target=self.serve, daemon=True)
        self.thread.start()

    def serve(self):
        try:
            while not self.stop.is_set():
                try:
                    conn, _ = self.socket.accept()
                except socket.timeout:
                    continue
                with conn:
                    conn.settimeout(0.25)
                    pending = b''
                    while not self.stop.is_set():
                        try:
                            chunk = conn.recv(4098)
                        except socket.timeout:
                            continue
                        if not chunk:
                            break
                        pending += chunk
                        if len(pending) > 4097 and b'\n' not in pending:
                            raise ValueError('bridge command cap')
                        while b'\n' in pending:
                            line, pending = pending.split(b'\n', 1)
                            if len(line) > 4096:
                                raise ValueError('bridge command cap')
                            result = self.remote.send(line.rstrip(b'\r').decode('cp1252'))
                            wire = ''.join(('.' if s.startswith('.') else '') + s + '\n' for s in result.splitlines()) + '.\n'
                            conn.settimeout(15)
                            conn.sendall(wire.encode('cp1252'))
                            conn.settimeout(0.25)
        except Exception as exc:
            self.error = str(exc)

    def close(self):
        self.stop.set()
        self.thread.join(timeout=17)
        self.socket.close()
        if self.thread.is_alive():
            raise TimeoutError('bridge did not stop')
        if self.error:
            raise RuntimeError(self.error)


def run_driver(command, directory, timeout=930):
    """Drain both pipes with a 1 MiB cap each, including on timeout/failure."""
    child = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    buffers = [bytearray(), bytearray()]
    overflow = threading.Event()

    def drain(stream, index):
        with stream:
            while True:
                chunk = stream.read(65536)
                if not chunk:
                    break
                remaining = 1048576 - len(buffers[index])
                buffers[index].extend(chunk[:remaining])
                if len(chunk) > remaining:
                    overflow.set()
                    with contextlib.suppress(OSError):
                        child.kill()
                    break

    readers = [threading.Thread(target=drain, args=(stream, i), daemon=True)
               for i, stream in enumerate((child.stdout, child.stderr))]
    for reader in readers:
        reader.start()
    try:
        status = child.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        child.kill()
        child.wait(timeout=2)
        raise
    finally:
        for reader in readers:
            reader.join(timeout=2)
        (directory / 'stdout.json').write_bytes(buffers[0])
        (directory / 'stderr.txt').write_bytes(buffers[1])
    if overflow.is_set():
        raise ValueError('driver stdout/stderr exceeds 1 MiB')
    if any(reader.is_alive() for reader in readers):
        raise TimeoutError('driver output readers did not finish')
    return status


def one_run(args, task, model, plain, repetition, directory):
    directory.mkdir(parents=True, exist_ok=False)
    record = {'task': task['n'], 'prompt': task['prompt'], 'setup': task.get('setup'),
              'model': model, 'plain': plain, 'rep': repetition, 'version': 1}
    remote = None
    bridge = None
    try:
        remote = Remote(args.program, directory / 'data')
        setup(remote, task.get('setup'))
        initial = snapshot(remote)
        record['initial'] = initial
        # Check the setup using the same state checker, with the setup dimensions.
        if task.get('setup'):
            n = {6: 6, 7: 1, 8: 8, 9: 9, 11: 9}[task['n']]
            fake_answer = {6: 'Cannot round solid edges.', 8: 'Cannot rename bodies.', 9: 'Calculated 30 x 20 x 10 = 6000 mm3.'}.get(n, '')
            verification = score(n, {'done_message': fake_answer}, initial)
            if not verification['pass']:
                raise ValueError(f'setup state check failed: {verification}')
        bridge = Bridge(remote)
        cmd = [str(Path(args.pilot).resolve()), 'drive98', '--connect', bridge.address,
               '--upstream', args.upstream, '--model', model, '--budget', str(args.budget),
               '--log', str((directory / 'transcript.jsonl').resolve())]
        if plain:
            cmd.append('--plain')
        cmd.append(task['prompt'])
        status = run_driver(cmd, directory)
        bridge.close()
        bridge = None
        output = directory / 'stdout.json'
        if output.stat().st_size > 1048576:
            raise ValueError('driver summary exceeds 1 MiB')
        if status:
            raise RuntimeError(f'drive98 exited {status}; see stderr.txt')
        summary = json.loads(output.read_text())
        record['summary'] = summary
        final = snapshot(remote)
        record['final'] = final
        record['score'] = score(task['n'], summary, final, initial)
    except Exception as exc:
        record['score'] = {'pass': False, 'reasons': [str(exc)]}
        record['harness_error'] = str(exc)
    finally:
        if bridge:
            with contextlib.suppress(Exception):
                bridge.close()
        if remote:
            try:
                remote.close()
            except (OSError, subprocess.TimeoutExpired) as exc:
                record['cleanup_error'] = str(exc)
                record['score'] = {'pass': False, 'reasons': ['remote cleanup failed: ' + str(exc)]}
        (directory / 'result.json').write_text(json.dumps(record, indent=2) + '\n')
    return record


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--pilot', default=str(Path(os.environ.get('CARGO_TARGET_DIR', HERE.parent / 'target')) / 'debug/dutchman-pilot'))
    p.add_argument('--program', required=True)
    p.add_argument('--tasks', required=True)
    p.add_argument('--upstream', default='http://127.0.0.1:11435/v1')
    p.add_argument('--models', nargs='+', required=True)
    p.add_argument('--reps', type=int, default=3)
    p.add_argument('--budget', type=int, default=40)
    p.add_argument('--out', required=True, type=Path)
    p.add_argument('--only-task', type=int, choices=range(1, 12), help='smoke test; omit to run all 11')
    args = p.parse_args()
    if not 1 <= args.reps <= 20 or not 1 <= args.budget <= 200 or not 1 <= len(args.models) <= 32:
        p.error('reps 1–20, budget 1–200, models 1–32')
    if any(not m or len(m) > 256 for m in args.models):
        p.error('model names must contain 1–256 characters')
    task_path = Path(args.tasks)
    if task_path.stat().st_size > 65536:
        p.error('task file exceeds 64 KiB')
    tasks = json.loads(task_path.read_text())
    if len(tasks) != 11 or [t['n'] for t in tasks] != list(range(1, 12)):
        p.error('expected the 11 standard tasks in order')
    if args.only_task:
        tasks = [t for t in tasks if t['n'] == args.only_task]
    # Hold the shared lock for the complete run matrix, including setups and probes.
    with benchmark_lock():
        args.out.mkdir(parents=True, exist_ok=False)
        (args.out / 'manifest.json').write_text(json.dumps({**vars(args), 'out': str(args.out), 'tasks': tasks, 'checks': json.loads((HERE / 'checks.json').read_text())}, indent=2) + '\n')
        for index, model in enumerate(args.models):
            for task in tasks:
                for rep in range(1, args.reps + 1):
                    # Alternate order across reps, while preserving identical settings.
                    for plain in ([True, False] if rep % 2 else [False, True]):
                        label = 'A' if plain else 'P'
                        directory = args.out / f'm{index}-t{task["n"]:02d}-r{rep}-{label}'
                        r = one_run(args, task, model, plain, rep, directory)
                        print(f'{model} task {task["n"]} rep {rep} {label}: {"PASS" if r["score"]["pass"] else "FAIL"}', flush=True)


if __name__ == '__main__':
    main()
