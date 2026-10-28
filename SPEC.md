# Pilot: specification

A local helper that lets small language models drive SolidCraft reliably, **from outside
SolidCraft**. SolidCraft's core uses native tool calling only: no text-call scraping, JSON
repair, or fallback prompt blocks, and it never runs commands a model only described in prose.
Help for weaker models fits best in an optional external adapter or proxy. This is that adapter.

One Rust binary, two faces, one shared library. No SolidCraft code is copied; SolidCraft is
reached only through its public interfaces (an OpenAI-compatible client in its Assistant, and its
MCP server, `solidcraft-cli mcp`, documented in SolidCraft's docs/mcp.md).

## Principles

1. **Opt-in and visible.** Nothing changes unless the user points SolidCraft (or an agent) at
   Pilot. Every rewrite Pilot makes is logged, and can be shown to the user.
2. **Explicit calls and selected recipes.** Pilot turns explicit JSON calls into native
   calls. In model-face plan mode only, an action pledge matching one fresh user recipe's
   intent and typed slots can become that recipe's catalogue-checked calls. Arbitrary prose
   never supplies new arguments. Every recipe and conversion is audited. Plan-text
   recipes require `--planner`; `--no-plan` is an accepted no-op alias for the default.
   Independent direct answering returns exact calls and measured answers by default
   through the same client approval and SolidCraft validation checks, with replies marked
   "Exact match (no model):". `--no-direct-recipes` sends everything to the model;
   `--direct-recipes` remains a no-op alias for the default and conflicts with the opt-out.
   `--upstream none` works by default and rejects `--no-direct-recipes`. Direct answering
   stays disabled for `--dialect solidcraft98`, including with `--direct-recipes`.
   The MCP face continues to offer suggestions only.
3. **Same checks as a native call.** Anything Pilot produces still goes through SolidCraft's
   own tool validation; Pilot only reshapes, it never bypasses.
4. **Measured.** Every feature is judged by SolidCraft's benchmark (`solidcraft-cli
   bench-assistant`, tangency-labs/solidcraft#4930) run with and without Pilot on the same
   models.
5. **Never crash, bounded.** No panics (`Result` everywhere), caps on message sizes, tool-call
   counts, retries, and nesting.

## Face 1: the model proxy (`dutchman pilot model`)

An OpenAI-compatible HTTP endpoint (`/v1/chat/completions`, `/v1/models`) on localhost. In
SolidCraft: add an API agent with base URL `http://127.0.0.1:11500/v1`. Pilot forwards to the
real endpoint (`--upstream http://localhost:11434/v1`, Ollama by default; any OpenAI-compatible
URL; an API key is passed through from the client's Authorization header, never stored).

Request side:
- With SolidCraft tools, `--offer-recipe-tools` offers local `pilot_find` and `pilot_build`
  functions (default off; unavailable in the SolidCraft 98 dialect); `--no-shortcuts` disables both. Search the embedded Markdown
  recipe library first with `{request}`; retrieve three ids, descriptions, schemas and BM25
  scores. A score below 1.8 returns an explicit miss and guidance to describe supported
  recipes or ask one question, without blocking native tools. Build with `{recipe, args}`.
  `--recipes-dir DIR` replaces the embedded library for development. Its legacy `shape` union accepts typed
  profiles, plates, enclosures and revolute joints, in mm (convert inches first).
  Shared recipe validators and catalogue-checked expanders replace a valid call with
  real client tools through the direct-recipe completion and measured verification path.
  Exact English recipes still win. Reject invalid arguments inside the turn with a field
  and rule tool error, at most two retries, then hand back with the shortcut removed.
  Audit `shortcut` (arguments and rule) and `shortcut_rejected` (reason). Local shortcut
  calls and results never reach SolidCraft and are stripped from returned client history.
  Audit `find` (query, top ids/scores) and `find_hit`/`find_miss`; profiles count finds,
  hits and misses. Two local searches per turn; existing wire/retry caps still apply.
  `recipe new --from-audit FILE --turn N` drafts the Nth recorded recipe emission to
  `recipe-review/`; it never activates a recipe. Reviewed sequences substitute typed
  parameters in at most 64 catalogue-checked calls. See docs/recipes.md for file format.
- On a fresh user turn after exact direct matching, Pilot searches with the same ranker.
  A top score >= 4.5 (find hit/miss remains 1.8), with available recipe state or
  inspection first, offers only `pilot_fill`: the typed recipe schema plus boolean
  `decline`, with forced function `tool_choice`. Fill and route are separate extraction
  requests built from client model/sampling settings, one short Pilot system message,
  the latest user message only, and exactly one tool. No client system prompt, history,
  or tools are included. Existing-target recipes first request public inspection through
  the client, then append a compact design summary (at most 8,192 characters and 12
  bodies): names, bounds, centres, last created body, last feature and definition,
  selection, and component/contact geometry for joints. Implicit this/these/it targets
  use selection, then the last created body, then the only body; ambiguous targets
  produce a clear form error. Fill requests ask for mm (inches × 25.4, cm × 10), omitted
  unstated optionals, and decline for mismatches. Reuse build validation and expansion.
  Scores from 1.5 inclusive to 4.5 exclusive offer a top-four `pilot_route`
  question first; lower scores use ordinary tools. A declined fill never routes.
  Invalid fills get at most two field-error retries (three attempts). An off-form or text
  reply gets one retry of the same clean request with a one-line problem note, then
  restores ordinary tools. Decline or a third invalid fill also restores ordinary tools.
  All form calls including retries share the four-request per-turn cap; the wire cap
  remains eight. Audit `form_request` records clean request bytes/characters, and
  `request_size` and profile accounting include every extraction request. Local forms never
  reach the client. Audit `fill_offered` (recipe, score), `fill_accepted`, `fill_declined`,
  `fill_invalid`, `fill_fallback`, and `fill_defaulted` for inferred non-dimensional choices
  and explicitly requested lip sizes derived from the wall (height = wall, width = wall / 2).
  Plate corner treatments may select upright corners by side or name. Enclosures may
  use clear inside dimensions: outside X/Y add two walls, height adds one floor.
  centred/centered/centre/center name the same placement field; centred plates centre
  in the sketch plane and extend from that plane through their thickness. open:true
  means the default top opening; explicit top/bottom choices remain available.
  Existing-part edit verbs may replace creation forms with edit forms after inspection
  confirms a body; pattern and joint forms retain their routing, and creation requests
  retain creation forms. Matching
  supplied shell dimensions are discarded after comparison with inspected bounds
  (0.001 mm or one part per million); mismatches name the measured size. Flat field
  names accept unambiguous singular/plural word variants. An omitted revolute angle
  uses a single explicit request rotation, including a quarter turn, with an
  inference audit; supplied angles are never overridden.
  A corner-only form defers requests to add holes as well as edit corners to
  native tools, retaining the complete request instead of completing only a subset.
  Offer once per user turn; no continuation or SC98
  fills. Forced fill and route do not require `--offer-recipe-tools`. `--no-fill` skips
  fill and route; `--no-shortcuts` disables forms and local recipe tools even with the offer flag.
  No upfront find-first fact is added.
- Facts and hints are sent once per retained conversation, with existing-history deduplication.
  Relevant guidance is always sent regardless of the client's character count.
  `how` remains available unless disabled with `--no-how`; its shorter schema saves
  64 characters. MCP retains its separate tool-list behavior.
- Pass other request fields through unchanged unless a feature below applies.
- `--fallback-prompt auto|on|off`: for a model that can't do native tools (detected from Ollama's
  `/api/show` capabilities, or after a turn that wrote a call as text), append ONE conditional
  block to the system prompt: "If you cannot call tools, and only then, reply with exactly one
  JSON object {"tool": ..., "args": {...}}. Only a tool call gets its arguments checked before
  the user sees the change." (Conditional wording: an unconditional one makes models stop
  calling tools natively.)

Response side (non-streaming and streaming; for streaming, buffer the assistant turn, then emit
it as a well-formed stream):
1. **Text-written tool calls → native tool calls.** If the reply has no `tool_calls` but its text
   contains a JSON object that names a tool in the request's `tools` list, convert it. Accepted
   shapes: `{"tool","args"}`, `{"name","arguments"}`, `{"function":{"name","arguments"}}`, and a
   bare `{"<tool name>": {...}}`. Strip ``` fences first. Take the first balanced object only.
   If the "tool" isn't a tool but IS a SolidCraft command id (from the catalogue), rewrite it as
   the `execute` tool with that command. Prose with no JSON object is left as text except
   for the selected-recipe exception in principle 2.
2. **Argument repair.** For each tool call (native or converted): if the tool's schema expects an
   array or object and the argument is a string that parses as JSON of exactly that type (also
   inside a fence), use the parsed value; do the same for each `batch` step's `params`. Try
   strict JSON, then JSON5-style (unquoted keys, single quotes, trailing commas), then bracket
   balancing. Anything still unparseable is left alone (SolidCraft's own error comes back).
3. **Empty reply retry.** An empty assistant turn is retried once with a one-line nudge
   ("your reply was empty; answer or call a tool"). Twice empty: pass it through.
4. **Logging.** Each rewrite is recorded (`--log FILE`, JSON lines: time, model, kind, before,
   after). A response header `x-dutchman-pilot: rewrites=N` tells clients something changed.

Pilot does not depend on SolidCraft crates; it talks to SolidCraft only through its public
interfaces.

## Face 2: the tools proxy (`dutchman pilot mcp`)

An MCP server over stdio that an agent launches instead of `solidcraft-cli mcp`. Pilot starts
the real server as a child process (`--solidcraft-cli PATH`, plus pass-through args such as
`--connect 127.0.0.1:7878` or `--in part.solidcraft`), forwards every message, and adds:

1. **`how` tool.** Input: a plain-language task ("round all edges 3 mm", "rename a body",
   "what is the volume"). Output: the best matching command(s) from SolidCraft's live catalogue
   (`list_commands` / `describe_command`), each with a ready-to-run example call built from its
   parameter docs, or "SolidCraft has no command for that" with the nearest alternative. Matching
   is deterministic (ids, labels, params docs, a small synonym table: round→fillet, bevel→chamfer,
   hollow→shell, ...), no model involved.
2. **Errors that carry the fix.** When a forwarded call fails with an unknown command, an unknown
   or misnamed parameter, or a missing required parameter, append a `suggested` corrected call
   when exactly one correction is unambiguous (closest id; renamed key by case/punctuation/word
   match; a single unclaimed value of the right type for a single missing parameter). Never
   re-run it automatically.
3. **Next-step hints.** After `inspect` (and after `execute`/`batch` results), append one short
   hint for the current state: an empty design → start with a sketch or a primitive; an open
   sketch → finish it before solid features; a body count or volume unchanged by a change call
   → "the design didn't change"; a sketch with free degrees of freedom → say how many.
4. **Closest-command hint** on unknown ids (`unknown command x; closest: a, b`). If SolidCraft
   already supplies a "closest" or "did you mean" hint, keep that hint without adding another.
   Match case, whitespace, and punctuation variations. Still append an unambiguous `suggested`
   corrected call.

The tool list Pilot advertises = SolidCraft's tools + `how`. Everything else passes through
byte-for-byte.

## Shared library

- `catalog`: load and cache SolidCraft's command catalogue (`solidcraft-cli commands` JSON, or
  the MCP `list_commands` result); search by id, label, synonyms; closest-id by edit distance and
  word overlap.
- `repair`: JSON extraction from text, fence stripping, JSON5-style parsing, bracket balancing,
  stringified-container repair against a JSON schema.
- `model`: the HTTP proxy.
- `mcp`: the stdio proxy.
- `cli`: `dutchman pilot model [--listen 127.0.0.1:11500] [--upstream URL] [--fallback-prompt …]
  [--log FILE]`, `dutchman pilot mcp [--solidcraft-cli PATH] -- <args for solidcraft-cli mcp>`,
  `dutchman pilot how "<task>"` (the how tool from the command line, for testing), `--version`,
  `--help`.

## Dependencies

Keep them few and mainstream: serde/serde_json; a small blocking HTTP server and client (e.g.
tiny_http + ureq) rather than an async stack; no network access except to the upstream URL the
user gives. Rust only. Builds on Linux, macOS and Windows.

## Tests

- Unit tests per module with synthetic inputs (every accepted call shape; repair cases incl.
  refusals; closest-id; how-tool matching on a fixed catalogue snapshot in tests/; hint rules).
- An end-to-end model-proxy test with a fake upstream (a local test server returning canned
  replies, including a text-written call and an empty reply).
- An end-to-end MCP test with a fake child server.
- The real measure: `solidcraft-cli bench-assistant --base-url http://127.0.0.1:11500/v1` vs
  the model directly, v1 and v2 task sets, same models (gemma4:12b, qwen3:14b, qwen2.5:7b,
  gpt-oss:20b, muse-glimmer).

## Out of scope

Anything inside SolidCraft; storing keys; running a model; acting on arbitrary prose
outside the selected model-face recipe exception.

## Techniques, mapped

Every weak-model technique Pilot provides, and where it lives.
Items marked (M) are in the model face, (T) in the tools face.

1. **Deterministic planning for common requests (M), plan-text opt-in.** A rule-based
   recogniser for frequent requests ("make a 20 mm cube", "a cylinder 30 mm across and 50 tall",
   "a plate with a hole through the middle", "move it 50 mm along X", "rename the body to X",
   "what is the volume") computes the exact command plan from the request and, where needed, the
   design's state (read through SolidCraft's MCP `inspect`). With `--planner`, the plan goes to the model as the
   answer to that part of the message: "run this plan; don't reinvent it; handle anything else
   asked". Counting, totals, and "which is biggest" are computed, never left to the model. Track
   which requests fail most in the benchmark and move those into the planner first.
   The beta.2 recipe contract is [docs/recipes.md](docs/recipes.md): typed bounded slots,
   metric / inch / fractional lengths, deterministic catalogue-checked expansions, no firing
   with missing or ambiguous slots or multiple intents, and no new action recipe after the
   request's `acted` check. A selected edge recipe may continue after its topology lookup;
   accepted creation prefixes are never replayed. Matching model prose can be converted only
   in model plan mode. In that mode, the model receives all results and writes the
   reply. Direct recipes return measured replies without a model by default (opt out with
   `--no-direct-recipes`); all proposed calls still require SolidCraft approval and validation. Every fired
   recipe audits rule, slots, calls and expected state. After the final reply, compare the
   latest complete measured state with expected body count and computable volume (0.1% or
   0.01 mm³ tolerance), plus available rename / move facts. Missing measurements or a
   difference get one hand-back within existing retry budgets, including after accepted
   calls. Queries and unrelated requests keep their existing guards. Evaluation prompts
   are never recipe rules; tests use independently authored phrasings.
2. **Per-model profiling (M).** Detect tool support (Ollama `/api/show` capabilities; otherwise
   learn from replies). Keep a per-model record (local JSON) of which path each turn took
   (native, text-written call, prose), empty replies, repairs, and hand-backs. Use it to turn on
   the conditional fallback block only for models that need it, and to report per-model stats
   (`dutchman pilot profile [model]`). Text-call frequency alone never disables native
   tools. Explicit capabilities lacking tools or an HTTP 400 containing "does not
   support tools" (case-insensitive) select model-face text tool mode: omit native
   tool fields, describe offered tools and JSON reply format in a bounded system
   block, and flatten tool history on the upstream wire. Persist explicit provider
   rejections and retry that turn once; recipes and repairs still apply. Audit all
   transformations. MCP clients continue calling tools themselves.
3. **The text-written call fallback, repair, and command-id rewrites (M).** As in Face 1, plus
   params packed into the id (`solid.cube {side: 20}`): split at the first space or `{`/`(`,
   look up the id, and repair the rest into `params`.
4. **Prompt patterns (M), opt-in (`--prompt-help`):** one conditional fallback block; a brief
   "action rail" (id: one-line purpose) filtered to what's relevant (sketch open → sketch
   commands); the rules that earned their place (data about the design comes from a tool call;
   a message that's only a name is a search, not "create"; ask "which one?" when two things
   match). One prompt builder, the same in tests and benchmarks.
5. **Model settings (M), opt-in per model:** temperature 0–0.2 on action turns; a fixed seed for
   benchmarks; `num_ctx` large enough (at least 8k for 7–8B) when the upstream is Ollama;
   JSON mode only on a retry where a JSON reply is already decided.
6. **In-turn guards with hand-backs (M), at most 2 per turn:** an unknown command id gets
   near-miss resolution, then "unknown x; closest a, b"; missing or misnamed arguments are
   reconciled (case/punctuation/word match; a single unclaimed value of the right type), else
   "x needs y"; **an instruction answered only in prose that names a command** gets "you
   described x; run it, or say why not", handed back to the MODEL unless the selected-recipe
   exception in principle 2 applies;
   a question about a specific thing answered with nothing looked up gets "look it up first"
   (narrow trigger); an empty reply gets one retry; provider errors (rate limit with retry time,
   bad key, missing model, out of quota) become one human sentence plus a machine code.
7. **Heuristic suggestions in the tools face (T):** `how`; errors with a `suggested` call;
   next-step hints after inspect and changes, including "you said done but the design didn't
   change"; **'Say "yes" to do: <command>'** when exactly one next step is obvious (from SolidCraft
   98's Tutor); "Did you mean" for unknown ids.
8. **Truthful paging (T):** every list-like result Pilot passes through starts with
   "shown N of TOTAL (partial)" when SolidCraft truncated it, so a model never reads a cut page as
   "none".
9. **The harness:** the benchmark (#4930) on the v1 and v2 task sets; later, a corpus of real
   requests with a claim each (command and key args, "asks which", or "answers in words"), in
   buckets with floors; ask three times and classify broken/noise/pass; record every reply;
   re-score old recordings when the judge changes; cache passes keyed by a prompt+tools
   fingerprint for about 14 days. Report pass rate per bucket and model, the path each turn took,
   hand-backs, argument vs command correctness, tokens, latency, and throttling (throttled = no
   verdict).

Build order (by measured impact on #4851/v2): repair + fallback (done in core) → model proxy with
guards and profiling → tools face (`how`, hints, suggested calls) → planner → prompt/settings
options → harness extensions. Each step is benchmarked before and after.
