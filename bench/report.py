#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Print a paired direct/Pilot table from results.jsonl files."""
import argparse
import collections
import json
from pathlib import Path
import sys


def report(paths):
    groups = collections.defaultdict(dict)
    for path in paths:
        with Path(path).open(encoding='utf-8') as stream:
            for index, line in enumerate(stream):
                if index >= 100000 or len(line) > 1024 * 1024:
                    raise ValueError('report input exceeds row or line limit')
                row = json.loads(line)
                if row.get('format') != 1:
                    raise ValueError('unsupported result format')
                if row['mode'] != 'model':
                    continue
                key = (row['set'], row['model'])
                trial = (row['task'], row['rep'], row['through_pilot'])
                if trial in groups[key]:
                    raise ValueError('duplicate task/repetition/mode; supply each run once')
                groups[key][trial] = row
    print('| Set | Model | Direct | Pilot | Paired Δ | Errors (direct/Pilot) |')
    print('| --- | --- | --- | --- | --- | --- |')
    for (name, model), trials in sorted(groups.items()):
        modes = [{(t, r): row for (t, r, p), row in trials.items() if p == flag} for flag in (False, True)]
        def score(rows):
            return f"{sum(bool(r['pass']) and not r.get('error') for r in rows.values())}/{len(rows)}" if rows else '—'
        common = modes[0].keys() & modes[1].keys()
        delta = sum(bool(modes[1][k]['pass']) - bool(modes[0][k]['pass']) for k in common)
        change = f'{delta:+d}/{len(common)} pairs' if common else '—'
        errors = '/'.join(str(sum(bool(r.get('error')) for r in rows.values())) for rows in modes)
        print(f"| {name} | {model.replace('|', '/')} | {score(modes[0])} | {score(modes[1])} | {change} | {errors} |")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('results', nargs='+', help='results.jsonl from direct and Pilot runs')
    args = parser.parse_args()
    try:
        report(args.results)
    except (OSError, ValueError, KeyError) as error:
        parser.exit(2, f'report: {error}\n')


if __name__ == '__main__':
    main()
