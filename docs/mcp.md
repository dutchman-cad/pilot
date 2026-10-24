# Use Pilot with an MCP client

The mcp face is a stdio server wrapping `solidcraft-cli mcp`. The client launches
Pilot; Pilot launches SolidCraft's public MCP binary, loads its live command
catalogue, and forwards requests. Install both executables first and use absolute
paths when the client does not inherit your shell's PATH.

For a design file:

```sh
dutchman pilot mcp --solidcraft-cli /path/to/solidcraft-cli -- --in /path/to/part.solidcraft
```

For an existing SolidCraft session with its local MCP connection enabled:

```sh
dutchman pilot mcp --solidcraft-cli /path/to/solidcraft-cli -- --connect 127.0.0.1:7878
```

Everything after Pilot's `--` belongs to `solidcraft-cli mcp`, including --in,
--connect, and any SolidCraft permission options. Follow the installed CLI's
public help for the connection port and any authentication requirements.
Pilot sends protocol messages on stdout and diagnostics on stderr.

## Claude Code

Register a local stdio server using the
[Claude Code MCP command](https://code.claude.com/docs/en/mcp):

```sh
claude mcp add --transport stdio solidcraft -- /path/to/dutchman-pilot mcp --solidcraft-cli /path/to/solidcraft-cli -- --connect 127.0.0.1:7878
claude mcp get solidcraft
```

Use `/mcp` in the client to check the connection. Replace --connect and its value
with --in and an absolute design path for a file session.

## Codex

Register the same command with [Codex's MCP configuration](https://developers.openai.com/codex/mcp/):

```sh
codex mcp add solidcraft -- /path/to/dutchman-pilot mcp --solidcraft-cli /path/to/solidcraft-cli -- --connect 127.0.0.1:7878
codex mcp list
```

Or add this table to `~/.codex/config.toml`:

```toml
[mcp_servers.solidcraft]
command = "/path/to/dutchman-pilot"
args = ["mcp", "--solidcraft-cli", "/path/to/solidcraft-cli", "--", "--connect", "127.0.0.1:7878"]
```

Use `/mcp` in the client to inspect active servers.

## Other stdio clients

For clients accepting the common mcpServers JSON format:

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

On Windows use the .exe paths and escape backslashes in JSON/TOML, or use forward
slashes. Configure the client to launch a program directly, without a shell.

## What changes

Pilot advertises SolidCraft's tools plus `how` and `plan`. Ask `how` with
`{"task":"round all edges 3 mm"}` for catalogue matches and example calls;
ask `plan` with `{"task":"make a 20 mm cube"}` for checked command steps.
Neither executes the suggested calls. Failed forwarded calls can carry a
corrected suggestion; hints describe design state and partial results are marked.
The client must explicitly choose subsequent calls.

Use --no-how, --no-plan, or --no-hints before `--` to disable those additions.
--log FILE records rewrites; otherwise the platform data directory under dutchman-pilot
contains rewrites.jsonl. Do not register both Pilot and the unwrapped SolidCraft
server for the same session: choose Pilot as the replacement entry.
See [behavior and limits](details.md) and [security](../SECURITY.md).
