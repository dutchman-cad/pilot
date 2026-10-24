#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Build or verify starting designs through the public SolidCraft CLI."""
import argparse
from pathlib import Path
import re
import subprocess
import sys
import tempfile

import check
import run


def cli(binary, arguments):
    # Keep child output out of memory until its size has been checked.
    with tempfile.TemporaryFile() as output, tempfile.TemporaryFile() as errors:
        result = subprocess.run([binary, *arguments], stdout=output, stderr=errors,
                                timeout=run.TIMEOUT, check=False)
        output.seek(0)
        errors.seek(0)
        data = output.read(run.MAX_BYTES + 1)
        diagnostic = errors.read(run.MAX_BYTES + 1)
        if max(len(data), len(diagnostic)) > run.MAX_BYTES:
            raise ValueError('SolidCraft output exceeds 16 MiB')
        if result.returncode:
            raise RuntimeError(f'SolidCraft exited {result.returncode}: '
                               f'{diagnostic.decode(errors="replace").strip()}')
        return data.decode('utf-8')


def version(binary):
    text = cli(binary, ['--version']).strip()
    match = re.fullmatch(r'solidcraft-cli (\d+\.\d+\.\d+)', text)
    if match is None:
        raise ValueError(f'unrecognised SolidCraft version: {text}')
    value = match.group(1)
    if tuple(map(int, value.split('.'))) < (0, 19, 0):
        raise ValueError('SolidCraft 0.19.0 or later is required')
    return value


def verify(readers, path, expected):
    for binary, reader_version in readers:
        try:
            with run.MCP(binary, path) as app:
                check.compare_start(expected, app.call('document.inspect', {'measure': True}))
        except (OSError, ValueError, RuntimeError, TimeoutError) as error:
            raise ValueError(f'opening with {reader_version}: {error}') from error


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--solidcraft', required=True, metavar='PATH',
                        help='builder and replay binary; use SolidCraft 0.19.0')
    sets = sorted(p.name for p in (run.HERE/'sets').iterdir() if p.is_dir())
    parser.add_argument('--set', choices=sets)
    parser.add_argument('--check', action='store_true', help='verify without writing')
    parser.add_argument('--open-with', action='append', default=[], metavar='PATH',
                        help='also verify opening with this binary (repeatable, at most eight)')
    args = parser.parse_args()
    if len(args.open_with) > 8:
        parser.error('at most eight --open-with binaries are supported')
    builder_version = version(args.solidcraft)
    readers = [(args.solidcraft, builder_version)]
    readers.extend((binary, version(binary)) for binary in args.open_with)
    print(f'SolidCraft {builder_version}: {"checking" if args.check else "building"} starts', flush=True)
    failed = count = 0
    for name in [args.set] if args.set else sets:
        root = run.HERE/'sets'/name
        tasks = [t for t in run.task_file(root/'tasks.json') if t.get('setup_steps')]
        if not tasks:
            continue
        directory = root/'start'
        if args.check:
            with (directory/'VERSION').open('rb') as stream:
                saved_version = stream.read(64).decode('ascii').strip()
            if not re.fullmatch(r'\d+\.\d+\.\d+', saved_version):
                raise ValueError(f'{name}: invalid start/VERSION')
        set_failed = False
        for task in tasks:
            count += 1
            try:
                if task.get('start') != f'start/{task["id"]}.solidcraft':
                    raise ValueError('missing or incorrect start field')
                prefix = run.setup(root, task, run.reference(root, task))
                with run.MCP(args.solidcraft) as app:
                    run.replay(app, prefix)
                    expected = app.call('document.inspect', {'measure': True})
                if args.check:
                    path = run.start_path(root, task)
                    verify(readers, path, expected)
                else:
                    directory.mkdir(parents=True, exist_ok=True)
                    with tempfile.TemporaryDirectory(dir=directory) as temporary:
                        script = Path(temporary)/'setup.json'
                        path = Path(temporary)/'start.solidcraft'
                        run.save(script, {'commands': prefix})
                        cli(args.solidcraft, ['run', str(script), '--save', str(path), '--quiet'])
                        if not path.is_file() or path.stat().st_size > run.MAX_BYTES:
                            raise ValueError('saved start is missing or exceeds 16 MiB')
                        verify(readers, path, expected)
                        path.replace(directory/f'{task["id"]}.solidcraft')
                print(f'{name}/{task["id"]}: PASS', flush=True)
            except (OSError, ValueError, RuntimeError, TimeoutError, subprocess.TimeoutExpired) as error:
                failed += 1
                set_failed = True
                print(f'{name}/{task["id"]}: FAIL {error}', flush=True)
        if not args.check and not set_failed:
            (directory/'VERSION').write_text(builder_version + '\n')
    print(f'{count - failed}/{count} starts verified; {failed} differences', flush=True)
    return int(bool(failed))


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (OSError, ValueError, RuntimeError, TimeoutError, subprocess.TimeoutExpired) as error:
        print(f'starts: {error}', file=sys.stderr)
        sys.exit(2)
