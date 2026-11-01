"""Check that relative links and images in the repository's Markdown files point at files that exist."""
import pathlib
import re
import subprocess
import sys

LINK = re.compile(r"!?\[[^\]]*\]\(([^)\s]+)(?:\s+\"[^\"]*\")?\)|<img[^>]*\ssrc=\"([^\"]+)\"")
files = subprocess.run(["git", "ls-files", "*.md"], capture_output=True, text=True, check=True).stdout.split()
bad = []
for name in files:
    text = pathlib.Path(name).read_text(encoding="utf-8")
    # Code blocks hold examples, not links.
    text = re.sub(r"```.*?```", "", text, flags=re.S)
    for match in LINK.finditer(text):
        target = match.group(1) or match.group(2)
        if re.match(r"[a-z][a-z0-9+.-]*:", target, re.I) or target.startswith("#"):
            continue
        path = target.split("#", 1)[0]
        if path and not (pathlib.Path(name).parent / path).exists():
            bad.append(f"{name}: {target}")
print("\n".join(bad) or f"{len(files)} Markdown files: every relative link resolves")
sys.exit(1 if bad else 0)
