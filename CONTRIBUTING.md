# Contributing to Pilot

Read [SPEC.md](SPEC.md) before changing behavior. Rust 1.88 or newer is required;
Cargo.toml is the source of truth for the minimum version. Build and check with:

```sh
cargo build --locked --bins
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked --all-targets
rustup target add wasm32-unknown-unknown
cargo build --target wasm32-unknown-unknown --no-default-features --features core
cargo clippy --locked --all-targets --no-default-features --features core -- -D warnings
cargo test --locked --all-targets --no-default-features --features core
```

The default features are `core` and `proxy`. The `core` feature alone builds
the library without host I/O; see [plugin preparation](docs/plugin.md).

Use Result with clear errors, no unsafe code, and bounded sizes, counts, and
retries. Production code must not use unwrap, expect, panic, todo, or unreachable.
Tests use synthetic inputs and fake HTTP/MCP peers; no model or SolidCraft is
required. Keep the implementation in Rust.

Before changing recipes, also run the full suite against the public SolidCraft
0.20.0 and 0.25.1 binaries. This enables both the geometry tests in
`tests/recipes_cli.rs` and the model-face public MCP tests. Use an absolute path:

```sh
buildq run pilot /home/ray/Documents/GitHub/sc-pilot-sc25-compat -- cargo test --locked --config 'env.PILOT_SOLIDCRAFT_CLI="/var/home/ray/Documents/pilot-bench/bin-0.20/solidcraft-cli"' -- --nocapture --test-threads=2
buildq run pilot /home/ray/Documents/GitHub/sc-pilot-sc25-compat -- cargo test --locked --config 'env.PILOT_SOLIDCRAFT_CLI="/var/home/ray/Documents/pilot-bench/dl25/solidcraft-0.25.1-linux-x86_64/bin/solidcraft-cli"' -- --nocapture --test-threads=2
```

Allow substantial time: the public geometry suite includes hundreds of honeycomb
cuts across many orientations. An unrestricted run took 75 minutes on this
machine; the two-thread 0.25.1 run took 26 minutes.
Two test threads limit simultaneous CAD sessions; `--nocapture` exposes failure
details immediately while long geometry tests are still running.

The build queue does not forward shell environment variables; pass the path with
Cargo's `--config`. On another machine, substitute its checkout and binary paths
(or omit the `buildq run ... --` prefix when no queue is installed).
CI currently has no SolidCraft download job. Public release asset URLs could not
be verified from the restricted development environment, so these checks remain
local until a public download can be verified.

The optional tests load `commands` from the configured binary, so expansions are
checked against its live catalogue, including the model-face public MCP tests.
Without that setting, deterministic tests continue using the bundled snapshot;
the snapshot is a fixed test fixture, with no requirement to track new releases.

### SolidCraft 0.20.0 → 0.25.1 catalogue audit

Compared the JSON from both public binaries' `commands` interfaces for the 44
command IDs referenced by Pilot's planners and recipe expanders. No emitted ID
was removed or renamed, and no emitted parameter was removed or renamed.
The initial full suite against 0.25.1 passed without recipe command changes;
there were no failing command/parameter pairs to list.
The following parameter documentation changed; all other referenced commands
have identical parameter documentation:

| Commands | Changes in 0.25.1 |
| --- | --- |
| `sketch.create` | Adds optional `frame: world\|component`; explicit planes default to world coordinates. |
| `sketch.circle.center` | Adds optional `construction`. |
| `sketch.point` | Adds snap, model-vertex and model-edge point forms. Numeric points remain valid. |
| `sketch.dimension` | Adds midpoint references, `distance_x`/`distance_y` aliases, `#name` definitions and more measurement cases. Existing dimension types remain valid. |
| `sketch.constraint.coincident` | Adds coincident circles/arcs and projected model references. |
| `sketch.constraint.equal` | Adds bounded `curves` lists and projected model references. |
| `sketch.constraint.midpoint`, `sketch.constraint.collinear`, `sketch.constraint.symmetry` | Add projected model references. |
| `solid.extrude` | Adds optional `exact_regions`; clarifies taper sign, profile picks and nested-loop defaults. Existing explicit profile forms remain valid. |
| `solid.revolve`, `solid.pattern.circular` | Add edge/face axis forms; revolve also accepts a typed world-axis object. Existing world axes and origin/direction objects remain valid. |
| `solid.cylinder` | Adds optional edge `references` for base placement. |
| `solid.hole` | Adds edge `references`, `handedness`, `thread_depth`, and more thread standards. Existing numeric positions, directions, diameters and depths remain valid. |
| `occurrence.copy` | Adds optional `in_place` and `parent_path`; default still copies the selected transform within its parent. |
| `timeline.paste` | Adds optional `component` and `component_path`; documents preservation of world placement across components. Copies within their own component keep their coordinates. |

Recipe compatibility uses shared parameters. Future differences must be selected
by the runtime catalogue, never by a hard-coded SolidCraft version. Check actual
geometry and measured state as well as catalogue acceptance: documentation alone
cannot establish that command behavior stayed compatible.

Validation with live catalogues on both 0.20.0 and 0.25.1: 403 passed, zero failed,
seven ignored on each. One additional unrestricted parallel 0.25.1 run marked
`public_binary_recipe_geometry` as failed before a buildq restart returned 125;
its captured command/parameter diagnostic was unavailable. The test passed seven
focused runs and the complete two-thread suite. The local commands above bound
concurrency and expose diagnostics immediately rather than waiting for the
longest geometry test to finish.

Use only SolidCraft's public docs and binaries. Never read, copy, or search its
source, or any GPL/LGPL CAD source.

Run the paired benchmark and corpus evaluation as described in
[Measure it](docs/measure.md). Keep the endpoint, model, and task set fixed;
include commands and results with behavior changes. Do not commit keys or logs
containing private design data.

After changing CLI help, regenerate [the flags reference](docs/flags.md):

```sh
cargo run --locked --example generate_flags
```

The display name lives in src/identity.rs. Keep the crate, executable, data-directory,
and protocol names stable when updating product wording.

Adding a recipe: add the file and one line in src/recipe_library/files.rs (the test lists any missing one).

To release, follow [Releasing Pilot](docs/releasing.md).
