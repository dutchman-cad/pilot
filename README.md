# Pilot

[![Pilot: helps small local models drive SolidCraft](docs/social-preview.png)](https://github.com/dutchman-cad/pilot/releases/latest)

Pilot lets a small AI model running on your own computer build parts in SolidCraft:
ask for "a 42 mm cube with a 5 mm hole through the middle" and it builds it, checked.

![SolidCraft's Assistant building an enclosure with a lip through Pilot](docs/images/assistant-enclosure-day.png#gh-light-mode-only)
![SolidCraft's Assistant building an enclosure with a lip through Pilot](docs/images/assistant-enclosure-night.png#gh-dark-mode-only)

[![Discord](https://img.shields.io/badge/Discord-Dutchman%20CAD-5865F2?logo=discord&logoColor=white)](https://discord.gg/yTcseBEdeu)

## Get started

1. Install Pilot from [the latest release](https://github.com/dutchman-cad/pilot/releases/latest):

   | System | Command |
   | --- | --- |
   | macOS (Apple silicon) and Linux, with Homebrew | `brew install dutchman-cad/tap/dutchman-pilot` |
   | Windows, with Scoop | `scoop bucket add dutchman-cad https://github.com/dutchman-cad/scoop-bucket; scoop install dutchman-cad/dutchman-pilot` |
   | Any, from the release archive | `sh install.sh` (or `.\install.ps1` on Windows) in the extracted folder |

2. Run a local model with [Ollama](https://ollama.com), then start Pilot in front of it:

   ```sh
   ollama pull gemma4:e4b
   dutchman-pilot model --upstream http://localhost:11434/v1
   ```

3. In SolidCraft's Assistant, add an API agent with base URL `http://127.0.0.1:11500/v1`
   and model `gemma4:e4b`. Then ask for a part.

Stuck? See [First-run help](docs/first-run.md). More options are in the
[Assistant guide](docs/assistant.md) (LM Studio, and Pilot without a model) and the
[install guide](docs/install.md).

## What it can build today

- Boxes, cylinders, tubes, and spheres, with holes through them
- Holes, bolt circles, hole grids, honeycomb cuts, and cross-drilled blocks
- Plates with holes, bores, and rounded corners
- L, T, U, C, and rectangular profiles
- Enclosures with walls, a floor, and a lip for the lid
- Joints between parts, including a revolute joint on a pin
- Patterns of parts you already have
- Edits: fillets, chamfers, shells, mirrors, revolves, moves, and renames, plus measurements

Each kind and its limits are in [recipes](docs/recipes.md).

## How it works

1. You ask SolidCraft's Assistant for a part in plain words.
2. Pilot finds the recipe that fits, such as a plate, a profile, or an enclosure.
3. The model fills in that recipe's short typed form: sizes, counts, and positions. When
   Pilot can read every value from the request itself, it skips the model.
4. Pilot turns the form into SolidCraft commands, you approve them, and SolidCraft builds
   the part. Pilot measures the result and reports it.

A small model is much better at filling in a short form than at writing CAD commands from
scratch, and SolidCraft checks every command before it runs.

## How well it works

Pilot is an early beta. On four blind test sets (56 requests never used during development),
gemma4 e4b builds about 38 of the 56 requests (67%) correctly through Pilot on 0.1.0-beta.8
(fixed model settings, average of three runs). Full numbers, and results with other models, are in
[the results issue](https://github.com/dutchman-cad/pilot/issues/1).

## Models that work

- **gemma4 e4b** (`ollama pull gemma4:e4b`) is the recommended model and the one Pilot
  is measured with. It is small enough to run on many laptops.
- Larger models such as gemma4 12B and gemma4 26B did well in earlier tests. qwen2.5 7B,
  qwen3 14B, gpt-oss 20B, and llama 3.1 and 3.2 have also been tried.
- Any model served through Ollama, LM Studio, or another chat-completions endpoint can be
  used. Numbers for each are in [the results issue](https://github.com/dutchman-cad/pilot/issues/1).

## Privacy

Pilot runs on your computer and sends requests only to the model server you choose; with
Ollama or LM Studio, that is your own computer too. Pilot sends nothing anywhere else. Reports from `/report` are redacted, saved locally, and
only leave your computer if you submit the issue yourself.

## Limits

- It is an early beta: about a third of the blind test requests still fail, most often
  because the model fills in a wrong value.
- It builds the kinds of parts listed above. Freeform shapes, arbitrary sketches, and large
  edits to existing designs go to the model on its own, which a small model rarely gets right.
- Every set of commands waits for your approval in SolidCraft, and a part Pilot built without
  the model says so in its reply.

## Contributing

The most useful thing is a request that went wrong: type `/report` in the Assistant and
submit the issue it prepares. Ideas and questions are welcome on the
[Discord](https://discord.gg/yTcseBEdeu); code changes are described in
[contributing](CONTRIBUTING.md).

## Help and more

- Something wrong? Type `/report` in SolidCraft's Assistant. Pilot writes a redacted report and a
  prefilled [issue](https://github.com/dutchman-cad/pilot/issues) link you review before
  sending.
- Questions and discussion: the [Dutchman CAD Discord](https://discord.gg/yTcseBEdeu).
- Your own model's results: add them to [#1](https://github.com/dutchman-cad/pilot/issues/1).
- [Command-line options](docs/command-line.md): MCP clients, exact matches, trimming,
  text tool mode, and the planner. Every flag is in the [flags reference](docs/flags.md).
- [Behavior and limits](docs/details.md), [measuring it](docs/measure.md),
  [public benchmarks](bench/), and [plugin preparation](docs/plugin.md).
- [Changelog](CHANGELOG.md), [contributing](CONTRIBUTING.md),
  [security](SECURITY.md), and [the contract](SPEC.md).

## SolidCraft 98

A side extra: Pilot can also drive [SolidCraft 98](docs/solidcraft98.md) through its screen.

<!-- sc98 screenshot -->

Licensed under [Apache-2.0](LICENSE). See [NOTICE](NOTICE).
