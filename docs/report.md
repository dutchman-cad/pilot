# Local bug reports

`dutchman-pilot report` writes and prints a redacted Markdown report and a pre-filled
GitHub issue URL. Review the entire file, edit it if necessary, and submit it yourself.
Pilot does not call GitHub, use a token, submit an issue, or send a report to a service.
`--open` explicitly asks your operating system to open the URL in your browser; the
browser then visits GitHub. Omit that flag to keep the whole operation local.

## From the Assistant chat

Type `/report` in SolidCraft's Assistant while it is connected to Pilot's model
face. `report this`, `report that`, and `/report <note>` also work (case-insensitive,
with surrounding whitespace allowed). Notes are limited to 200 characters.
Only these forms trigger a report; ordinary requests such as “report the body count”
continue to the model.

Pilot replies with a **Pilot report:** prefix, the local saved file path, a prefilled
GitHub issue link, and a fenced, paste-ready summary for the Dutchman CAD Discord
**#pilot-help** channel. The summary has at most 12 lines. If the URL would exceed
the length limit, the link contains only the title and the summary tells you to
review and paste the file. Nothing is sent; open the link to review and submit.

Chat reports use the latest failed captured turn, otherwise the latest turn, and
always omit request words and file contents. The optional note is included only
after the same redaction as the report. Reports go to the default reports directory
(under `--profile-dir` if configured). The report command itself is never captured
or counted in model profiles; its Pilot-authored reply is recorded in the rewrite log.
It makes no upstream call and works with `--upstream none`, streaming responses,
and `--dialect solidcraft98`, whose model face also has capture.

If the ring is empty or disabled with `--no-report-buffer`, Pilot explains that
in one line and suggests the CLI's offline `--from-request` option. Restarting
Pilot clears the ring; an offline report requires a separately recorded request file.

## From the command line

```sh
dutchman-pilot report
dutchman-pilot report --listen 127.0.0.1:11500 --turn 12 --out report.md
dutchman-pilot report --include-request
dutchman-pilot report --from-request recorded-request.json --out replay-report.md
```

The embedded command is `dutchman pilot report` with the same arguments.
The default output directory is the user's data directory, `dutchman-pilot/reports/`.
The filename starts with `report-` and a timestamp. `--out FILE` chooses another file.
An explicitly chosen existing output file is replaced.

## Capture and selection

The model face retains the last **five completion requests**, excluding chat report
commands, in a memory-only ring.
Each has a stable increasing turn id. `--turn N` selects that id; expired ids fail
clearly. Without it, report chooses the most recent failed/declined/fallback turn,
otherwise the latest turn. Restarting the model face clears the ring.
`model --no-report-buffer` disables capture completely.

`GET /pilot/turns` on the model face's listen address returns the ring as JSON.
It accepts only loopback TCP peers (IPv4 or IPv6); other peers get HTTP 403 regardless
of their Host or forwarded headers. It never appears in `/v1/models`.
The report fetcher accepts only literal loopback addresses and uses one bounded TCP
request, without DNS, proxies, redirects, or retries.

Capture includes the latest user request, planner intent and typed slots, diagnostic
reasons, audit event kinds, tool names, command ids, errors, completion finish reasons,
model, Pilot version, and effective behavior flags. Tool payloads are immediately
reduced to body counts, indexed per-body volumes, bounding-box **sizes** (not positions),
feature types, and error text. Full design JSON, conversation prompts, credentials, and
Authorization headers are not retained in the report ring. Planner-selected names are
kept only as a bounded dictionary for consistent redaction.

Limits: five turns; 16 KiB of UTF-8 request text per turn (original byte length is
recorded); 256 events per turn; 2 KiB of facts per event (oversized facts become an
explicit truncation marker); 256 structured names; 64 facts/commands per reduction;
24 levels of capture traversal. Capture does not grow with a long conversation.
The existing rewrite log and model profiles are independent features; reporting never
reads them or writes its ring to them. Disabling the ring does not disable those logs.

## Offline replay

`--from-request FILE` reads a recorded chat-completions request, including its original
tool offers and inspection context, and replays `planner::plan` against the bundled public
catalogue. It neither starts a model face nor contacts any upstream. It reports the
planner's result, rather than claiming to reconstruct model replies that were not
recorded. Matched partial plans contribute slots to declined compound requests.
Offline input is bounded to 8 MiB and also subject to the planner/parser's smaller
size, depth and message limits. `--turn` and `--from-request` are mutually exclusive.

## Request consent

By default, request words are omitted. Reports contain the detected intent, extracted
slots and values, original request length, and decline diagnostics. Decline tokens come
from Pilot's actual guard/unsupported-modifier lists and shared multiple-intent checks.
The additional token-shape view allows only fixed planner vocabulary; all other words
appear as `[word]`. Both views are computed from redacted request text. Replacement
labels such as `[email]` remain single opaque tokens; their contents never contribute
vocabulary to decline diagnostics. Other request words echoed in diagnostics are also
masked without consent. Unknown or ambiguous slots remain unknown, rather than being guessed.
`--include-request` explicitly includes the redacted captured text under
**Request text (explicitly included; redacted)**. Dimensions and units remain useful.

## Redaction

`src/redaction.rs` is the single report-scoped redactor. All variable report strings,
including model, flags, slots, events, reasons, and optional request text, pass through it.
It replaces:

- Absolute POSIX and Windows paths, relative paths, and recognizable bare filenames.
- URLs, email addresses, IP addresses, and network addresses.
- Local account, home directory, and system identity values.
- Secrets beginning `sk-`, `ghp_`, `gho_`, `ghu_`, `ghs_`, `github_pat_`, `xox`, `AKIA`,
  and the token following `Bearer`. Other tokens of at least 24 bytes use Shannon entropy
  (at least 3.5 bits/character with alphabetic content).
- Structured and contextually labelled body/component/sketch/parameter/file names,
  using consistent `Body1`, `Body2`, `Component1`, `Sketch1`, `Param1`, `File1`, etc.

Replacement occurrences are counted, not just distinct strings. The report starts with
`redacted: 2 paths, 1 body name` (as applicable) and repeats the summary near the footer.
The same name has the same replacement everywhere in one report. Numeric values,
fractions, mm values, quoted dimensions, catalogue command ids, and Pilot's fixed text
stay readable. Names and secrets outside recognizable contexts may need manual editing;
always review the printed report before submitting it.

The report contains a title, version/model/flags, an event-derived explanation, event
table, request shape, redaction summary, and a review statement in its footer. It makes
no claim that a proposed tool call was executed. Markdown table cells escape pipes,
line breaks, and HTML angle brackets.

The issue URL is form-URL-encoded and strictly shorter than 7,500 bytes. If the full
report would exceed that cap, Pilot prints **paste the file** and a title-only URL.
After editing the report, paste the edited file instead of using the original URL body.
Capture, redaction, and reporting compile only with the `proxy` feature; they are absent
from the wasm core.
