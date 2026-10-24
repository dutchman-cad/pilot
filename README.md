# Pilot

Pilot is a local helper that lets small language models drive
SolidCraft through its public interfaces. It repairs explicit tool calls,
finds commands, and adds bounded plans and hints. Prose alone never becomes an
action; SolidCraft still validates every forwarded call.

Run it as `dutchman pilot` from the [dutchman](https://github.com/dutchman-cad/dutchman)
command line. The same commands also come as a standalone download,
`dutchman-pilot`, which takes the same arguments; examples below use
`dutchman pilot`.

**Status:** Early beta: results are still being measured; expect rough edges.

Choose the face that fits your client:

| Client | Start | Connect |
| --- | --- | --- |
| SolidCraft's Assistant | `dutchman pilot model --upstream http://localhost:11434/v1` | Add an API agent at `http://127.0.0.1:11500/v1`. |
| An MCP client | `dutchman pilot mcp --solidcraft-cli /path/to/solidcraft-cli -- --in part.solidcraft` | Configure a local stdio server with this command. |

Pilot does not run a model or embed SolidCraft. Install SolidCraft separately;
use Ollama, LM Studio, or another chat-completions endpoint for the model face.

## Install

Download the archive for your platform from
[GitHub Releases](https://github.com/dutchman-cad/pilot/releases):
`x86_64-linux`, `aarch64-linux`, `aarch64-macos`, or `x86_64-windows`.
Archive names start with `SolidCraft-Pilot-VERSION-`. Each contains the standalone
`dutchman-pilot` and `dutchman-pilot-eval` binaries, docs, corpus, and license notices.
Compare its SHA-256 digest with the adjacent `.sha256` file before extracting:

```sh
sha256sum -c <archive>.sha256
# macOS: shasum -a 256 -c <archive>.sha256
```

On Windows, run `Get-FileHash <archive>.zip -Algorithm SHA256` in PowerShell
and compare the digest with the checksum file. Extract the archive and put its
binaries in a directory on PATH. macOS binaries are ad-hoc signed, without
notarization; use your system's approval controls for a trusted download.
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

## Guides

- [Use Pilot with SolidCraft's Assistant](docs/assistant.md): Ollama and LM Studio.
- [Use Pilot with an MCP client](docs/mcp.md): Claude Code, Codex, and other clients.
- [Measure it](docs/measure.md): paired benchmark runs and corpus evaluation.
- [Flags reference](docs/flags.md): generated from both binaries' help.
- [Behavior and limits](docs/details.md): repair, planning, profiles, and reports.
- [Plugin preparation](docs/plugin.md): portable logic and future plugin boundaries.
- [Changelog](CHANGELOG.md), [contributing](CONTRIBUTING.md),
  [security](SECURITY.md), and [the contract](SPEC.md).

Licensed under [Apache-2.0](LICENSE). See [NOTICE](NOTICE).
