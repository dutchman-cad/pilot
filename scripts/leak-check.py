#!/usr/bin/env python3
"""Fail when a test or source file shares a run of words with a private benchmark request.

tests/blind-fingerprints.txt holds salted hashes only, so the requests stay private. It is generated
outside this repository; wording already on main when it was generated is left out.
"""
import hashlib, os, re, sys

SALT = "pilot-blind-v1:"
EXACT, MASKED = 6, 8
PRINTS = "tests/blind-fingerprints.txt"


def words(text):
    return re.sub(r"[^a-z0-9./ ]", " ", text.lower().replace('"', " in ")).split()


def grams(ws):
    masked = ["#" if re.fullmatch(r"[0-9./]+", w) else w for w in ws]
    for n, kind, seq in ((EXACT, "x", ws), (MASKED, "m", masked)):
        for i in range(len(seq) - n + 1):
            yield kind + ":" + " ".join(seq[i:i + n])


prints = set(open(PRINTS).read().split())
hits = 0
for top in ("tests", "src"):  # Rust unit tests live in src.
    for root, _, files in os.walk(top):
        for name in files:
            path = os.path.join(root, name)
            if path == PRINTS:
                continue
            for number, line in enumerate(open(path, errors="ignore"), 1):
                if any(hashlib.sha256((SALT + g).encode()).hexdigest()[:20] in prints for g in grams(words(line))):
                    print(f"{path}:{number}: wording matches a private benchmark request; write a different example")
                    hits += 1
print(f"leak check: {hits} line(s)")
sys.exit(1 if hits else 0)
