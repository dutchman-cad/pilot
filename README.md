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

## How well it works

Pilot is an early beta. On four blind test sets (56 requests never used during development),
gemma4 e4b builds 37.7 correctly through Pilot on 0.1.0-beta.8 (fixed model settings,
average of three runs). Full numbers, and results with other models, are in
[the results issue](https://github.com/dutchman-cad/pilot/issues/1).

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

Also: Pilot can drive [SolidCraft 98](docs/solidcraft98.md).

Licensed under [Apache-2.0](LICENSE). See [NOTICE](NOTICE).
