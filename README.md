# Pilot

[![Discord](https://img.shields.io/badge/Discord-Dutchman%20CAD-5865F2?logo=discord&logoColor=white)](https://discord.gg/yTcseBEdeu)

Pilot is a local helper that lets small language models drive
SolidCraft through its public interfaces. It repairs explicit tool calls,
finds commands, and adds bounded hints. With `--planner`, a matching
action pledge can become a catalogue-checked recipe; SolidCraft validates every call.

Run it as `dutchman pilot` from the [dutchman](https://github.com/dutchman-cad/dutchman)
command line. The same commands also come as a standalone download,
`dutchman-pilot`, which takes the same arguments; examples below use
`dutchman pilot`.

**Status:** Early beta: results are still being measured; expect rough edges.

- [SolidCraft 98](docs/solidcraft98.md) screen tools, a bounded model driver, and
  offline help, using its public remote protocol (SolidCraft 98 1.2 or later).
- Recipe forms: Pilot finds the recipe that fits a request and asks the model only to
  fill in its typed form, in a short separate request, then builds the part with checked
  calls. Recipes cover primitives, hole layouts, profiles (L, T, U, C, rectangle), plates
  with bores, bolt circles, and rounded corners, enclosures with a locating lip,
  cross-drilled boxes, honeycomb hole patterns, patterns of existing parts, a revolute
  joint on a pin, edge finishes, movement, renaming, mirror, shell, revolve, and
  measured queries.
- Fewer input tokens: older tool results are trimmed inside long tasks (`--no-trim` to
  keep them), and Pilot adds no unused tool schemas.
- Millimetres, metric and inch units, fractions, and deterministic expansions.
- Opt-in `--planner`: recipe suggestions, matching prose conversion, continuations,
  and one measured completion check. These plan-text features are off by default;
  `--no-plan` (alias `--no-planner`) is accepted as a no-op.
- Text tool mode for models without native tools, with automatic retry and saved profiles.
- Exact matches use native tool proposals and measured replies without a model
  by default, with normal client approval; `--upstream none` supports recipe-only use.
  Use `--no-direct-recipes` to send everything to the model.
- Argument repair, text-call pickup, command lookup, hints, buffered streaming,
  and bounded retries remain enabled by default, with audited rewrites.
- Local, redacted [bug reports](docs/report.md) from five memory-only turns or offline
  planner replay: type `/report` in the Assistant. Chat reports omit request words;
  the CLI can include them with `--include-request`. You review and submit.

For SolidCraft 98, start `dutchman pilot mcp98 --program /path/to/sc98 --headless`
as an MCP server, or attach with `--connect 127.0.0.1:9898` to an application started with `/REMOTE`
(`/REMOTE:ANY` when Pilot runs on another machine or a virtual machine's host). See the
[SolidCraft 98 quick start](docs/solidcraft98.md) for the model driver and limits.

See [recipes and their limits](docs/recipes.md).

**Small models:** For 3–8B models, recipe forms and exact matches are on by default with
your upstream URL. On four blind test sets (56 tasks), gemma4 e4b passes about 33 without
exact matches and 35 with them on 0.1.0-beta.6, against 30 and 30 on beta.5 and 2 and 2
before recipe forms (averaged over repeated runs; [results](https://github.com/dutchman-cad/pilot/issues/1)).
Use `--no-direct-recipes` to send everything to the model. Exact matches bypass the model,
and their replies are marked **"Exact match (no model):"**.
The proposed calls still go through SolidCraft's approval and validation.
Other requests go to the model. Keep plan-text recipes off unless you want to try
`--planner`; they reduced reliability in measurements with small models.

Choose the face that fits your client:

| Client | Start | Connect |
| --- | --- | --- |
| SolidCraft's Assistant | `dutchman pilot model --upstream http://localhost:11434/v1` | Add an API agent at `http://127.0.0.1:11500/v1`. |
| An MCP client | `dutchman pilot mcp --solidcraft-cli /path/to/solidcraft-cli -- --in part.solidcraft` | Configure a local stdio server with this command. |

Pilot does not run a model or embed SolidCraft. Install SolidCraft separately;
use Ollama, LM Studio, or another chat-completions endpoint for the model face.

## Install

Homebrew (Linux, and macOS on Apple silicon):

```sh
brew install dutchman-cad/tap/dutchman-pilot
```

A Homebrew install needs no first-run Gatekeeper approval on macOS.

Scoop (Windows):

```powershell
scoop bucket add dutchman-cad https://github.com/dutchman-cad/scoop-bucket
scoop install dutchman-cad/dutchman-pilot
```

Both install `dutchman-pilot` and `dutchman-pilot-eval`, and follow each new release.

Or download the archive for your platform from
[GitHub Releases](https://github.com/dutchman-cad/pilot/releases):
`x86_64-linux`, `aarch64-linux`, `aarch64-macos`, or `x86_64-windows`.
Archive names start with `pilot-VERSION-`. Each contains `dutchman-pilot`,
`dutchman-pilot-eval`, the shared `dutchman` launcher, installers, docs, corpus,
license notices, and a `LAUNCHER_REV` file identifying the launcher source commit.
Compare its SHA-256 digest with the adjacent `.sha256` file before extracting:

```sh
sha256sum -c <archive>.sha256
# macOS: shasum -a 256 -c <archive>.sha256
```

On Windows, run `Get-FileHash <archive>.zip -Algorithm SHA256` in PowerShell
and compare the digest with the checksum file. Extract the archive, enter its
directory, and run the installer:

```sh
# Linux and macOS: installs into ~/.local/bin
sh install.sh
# Or choose a bin directory; preview with --dry-run
sh install.sh --prefix "$HOME/bin" --dry-run
```

```powershell
# Windows: installs into %LOCALAPPDATA%\Programs\dutchman-cad\bin
.\install.ps1
# Or choose a bin directory; preview with -DryRun
.\install.ps1 -Prefix "$HOME\bin" -DryRun
# Optionally add the directory to your user PATH, then open a new terminal
.\install.ps1 -AddToPath
```

The installers copy `dutchman-pilot`, `dutchman-pilot-eval`, and `dutchman` into the chosen bin directory.
Add that directory to PATH as the installer suggests. They require no administrator
access. Only Windows `-AddToPath` changes your saved user PATH.
You can rerun either installer to update an older launcher. A full Dutchman install
at the destination or on PATH is kept; it also runs `dutchman pilot`.

Both command forms now work:

```sh
dutchman pilot --version
dutchman-pilot --version
```

macOS binaries are ad-hoc signed, without notarization; use your system's approval
controls for a trusted download.
Linux archives are built on Ubuntu 24.04 with glibc; build from source for older
systems or other C libraries.

Or build from a checkout with Rust 1.88 or newer:

```sh
cargo install --locked --path . --bins
dutchman-pilot --help
dutchman-pilot how "round all edges 3 mm"
```

## Quick start

For Ollama, start Ollama, pull a model, and start Pilot:

```sh
ollama pull qwen2.5:7b
dutchman pilot model --upstream http://localhost:11434/v1
```

In SolidCraft's Assistant, add an API agent with base URL
`http://127.0.0.1:11500/v1` and model `qwen2.5:7b`.
See the [Assistant guide](docs/assistant.md) for setup details.

For an MCP client, configure a local stdio server:

```json
{
  "mcpServers": {
    "solidcraft": {
      "command": "/path/to/dutchman-pilot",
      "args": ["mcp", "--solidcraft-cli", "/path/to/solidcraft-cli", "--", "--in", "/path/to/part.solidcraft"]
    }
  }
}
```

Use absolute paths for the installed binaries and design file. See the
[MCP guide](docs/mcp.md) for client-specific setup and live connections.

Run the public [SolidCraft Assistant model benchmarks](bench/) directly or through Pilot.

## Guides

- [Use Pilot with SolidCraft's Assistant](docs/assistant.md): Ollama and LM Studio.
- [Use Pilot with an MCP client](docs/mcp.md): Claude Code, Codex, and other clients.
- [Measure it](docs/measure.md): paired benchmark runs and corpus evaluation.
- [Flags reference](docs/flags.md): generated from both binaries' help.
- [Behavior and limits](docs/details.md): repair, planning, profiles, and reports.
- [Plugin preparation](docs/plugin.md): portable logic and future plugin boundaries.
- [Changelog](CHANGELOG.md), [contributing](CONTRIBUTING.md),
  [security](SECURITY.md), and [the contract](SPEC.md).

## Help and community

- Questions and discussion: the [Dutchman CAD Discord](https://discord.gg/yTcseBEdeu).
- Bugs: type `/report` in SolidCraft's Assistant, or run `dutchman pilot report`. Either
  writes a redacted report and a prefilled [issue](https://github.com/dutchman-cad/pilot/issues)
  link you review before submitting.
- Results with your own model: add them to [#1](https://github.com/dutchman-cad/pilot/issues/1).

Licensed under [Apache-2.0](LICENSE). See [NOTICE](NOTICE).
