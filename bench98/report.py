#!/usr/bin/env python3
"""Print per-model A (plain) vs P (Pilot) and per-task state scores, offline."""
import argparse
from collections import defaultdict
import json
from pathlib import Path
from score import score


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('directory', type=Path)
    p.add_argument('--rescore', action='store_true', help='apply current checks to recorded states')
    args = p.parse_args()
    groups = defaultdict(list)
    paths = sorted(args.directory.glob('*/result.json'))
    if not paths or len(paths) > 20000:
        p.error('expected 1–20000 result files')
    for path in paths:
        if path.stat().st_size > 8 * 1048576:
            p.error(f'oversized result: {path}')
        run = json.loads(path.read_text())
        if args.rescore and 'final' in run:
            run['score'] = score(run['task'], run.get('summary', {}), run['final'], run.get('initial'))
        groups[run['model']].append(run)

    def aggregate(runs):
        passed = sum(r['score']['pass'] for r in runs)
        if not runs:
            return '—'
        return f'{passed}/{len(runs)} ({100 * passed / len(runs):.1f}%)'

    print('| Model | A plain | P Pilot | Δ percentage points | A/P tokens | A/P latency (s) | A/P actions | A/P errors | P rewrites |')
    print('|---|---:|---:|---:|---:|---:|---:|---:|---:|')
    for model, runs in sorted(groups.items()):
        a = [r for r in runs if r['plain']]
        pilot = [r for r in runs if not r['plain']]
        def mean(rows, key):
            values = [r['summary'][key] for r in rows if key in r.get('summary', {})]
            return sum(values) / len(values) if values else 0
        delta = (100 * sum(r['score']['pass'] for r in pilot) / len(pilot) - 100 * sum(r['score']['pass'] for r in a) / len(a)) if a and pilot else 0
        print(f'| {model} | {aggregate(a)} | {aggregate(pilot)} | {delta:+.1f} | {mean(a,"tokens"):.0f}/{mean(pilot,"tokens"):.0f} | {mean(a,"latency"):.2f}/{mean(pilot,"latency"):.2f} | {mean(a,"actions"):.1f}/{mean(pilot,"actions"):.1f} | {mean(a,"errors"):.1f}/{mean(pilot,"errors"):.1f} | {mean(pilot,"rewrites"):.1f} |')
    for model, runs in sorted(groups.items()):
        a = [r for r in runs if r['plain']]
        pilot = [r for r in runs if not r['plain']]
        print(f'\n{model} per task (A / P):')
        for task in sorted({r['task'] for r in runs}):
            ar = [r for r in a if r['task'] == task]
            pr = [r for r in pilot if r['task'] == task]
            print(f'  {task:2}: {aggregate(ar)} / {aggregate(pr)}')
            for r in ar + pr:
                if not r['score']['pass']:
                    print(f'      {"A" if r["plain"] else "P"} rep {r["rep"]}: ' + '; '.join(r['score']['reasons']))
        errors = sum('harness_error' in r for r in runs)
        if errors:
            print(f'  Infrastructure errors included as failures: {errors}')
        print()


if __name__ == '__main__':
    main()
