# Security

Pilot passes the client's Authorization header to the configured model endpoint
in memory. It never stores API keys in configuration, profiles, or its audit log.
The evaluation runner reads optional authorization from OPENAI_API_KEY without
recording it. Put credentials in the client or environment, never in URLs, prompts,
command arguments or repository files. Upstream URLs containing credentials are
rejected; redirects and environment HTTP proxies are disabled.

The model endpoint binds to 127.0.0.1:11500 by default. Keep --listen on a loopback
address (127.0.0.1 or ::1). The option can bind other interfaces, but the endpoint
has no incoming authentication or TLS; do not expose it to a network. Local
processes able to reach it can use the configured upstream. The MCP face uses
stdio and starts solidcraft-cli mcp as a local child; it adds no network listener.
Use loopback for SolidCraft's --connect address as well.

Audit logs contain before/after tool content. Evaluation reports and the pass
cache retain model replies and prompts. Treat these files as design data and
review them before sharing. Headers are not logged, but content sent by a client
or echoed by an upstream can contain secrets; do not put keys in that content.

Suggestions are never executed automatically. Forwarded calls remain subject to
SolidCraft's validation and permissions. Requests, replies, retries, and catalogue
queries have fixed caps. Use trusted binaries for --solidcraft-cli.

Report a suspected vulnerability through this repository's GitHub private
vulnerability reporting channel when enabled. Otherwise ask the maintainer for
a private contact in an issue without publishing credentials or exploit details.
