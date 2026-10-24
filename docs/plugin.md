# Plugin preparation

SolidCraft-Pilot's default features, `core` and `proxy`, retain both existing proxy faces
and their binaries. `proxy` depends on `core`. Building only `core` provides
catalogue search and the how matcher, the request planner, argument repair, and
state hint rules. It uses serde and serde_json, without HTTP, subprocesses,
networking, files, clocks, or threads. Rust's standard library is still used for
in-memory collections and strings.

Callers pass catalogue JSON to `Catalog::from_json`, then use `Catalog::how`
for serializable matches and examples. `planner::plan` takes a JSON request
containing messages, tool state, and available tools plus the catalogue. Its
optional `Plan` serializes to JSON with `rule`, `calls`, and `facts`. It only
proposes calls.
`repair::repair_arguments` takes arguments and a schema, which can come from
`Catalog::parameter_schema`. `guidance::state_hint` takes before/after state,
a change flag and an optional catalogue; `guidance::paging` reports partial
results. All these inputs come from the caller. None reads files or the clock;
any future time-dependent logic must accept a caller-supplied "now".
The bundled catalogue remains an embedded fallback; live JSON is preferable.
Existing byte, command, message, depth, and step limits still apply.

Check the portable build locally with:

```sh
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --no-default-features --features core
```

This builds a Rust library, not a WIT component. No plugin is built or exposed
here until SolidCraft ships plugins. A runtime, bindings, and a plugin
implementation depend on that interface.

## Future commands

The proposed interface described in tangency-labs/solidcraft#1221 and
SolidCraft's public `docs/plugins.md` is a WIT component importing
`run(command: string, args: string) -> result<string, string>` plus logging and
progress, and exporting `manifest() -> string` and
`execute(params: string) -> result<string, string>`. `run` uses SolidCraft's
JSON commands, the same set available through MCP. Network, files, clock and
threads are unavailable unless explicitly granted.

A future component could expose two plugin commands returning JSON:

- `pilot.how`: accept a task, obtain catalogue JSON through the host, and return
  `Catalog::how` matches, explicit examples, and nearest alternatives.
- `pilot.plan`: accept a request and current design state, obtain catalogue
  JSON through the host, and return the planner's optional plan and computed
  facts. Execution remains an explicit decision by the calling client.

These commands would wrap `core`; final schemas and bindings depend on the
interface SolidCraft ships.

## Proxy boundaries

The model proxy sits between a client and its model endpoint. It handles model
traffic, streaming, fallback prompts, retries, profiles, and rewrite logs. The
plugin interface has no hook into the Assistant's model traffic, so that proxy
cannot move inside a plugin.

The MCP wrapper sits between a client and SolidCraft's MCP server. It forwards
other tool calls and decorates their results with repairs, suggestions and
hints. A plugin has no hook into other clients' tool calls, so it cannot replace
that wrapper either. The pure repair and hint rules can be reused when a client
explicitly invokes a plugin command, but interception remains in the external
proxies.
