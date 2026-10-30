# Pilot

[![Pilot: helps small local models drive SolidCraft](docs/social-preview.png)](https://dutchman-cad.github.io/view.html?src=https://raw.githubusercontent.com/dutchman-cad/pilot/main/docs/social-preview.png&alt=Pilot%3A%20helps%20small%20local%20models%20drive%20SolidCraft)

Pilot lets a small AI model running on your own computer build parts in SolidCraft:
ask for "a 42 mm cube with a 5 mm hole through the middle" and it builds it, checked.

<a href="https://dutchman-cad.github.io/view.html?src=https://raw.githubusercontent.com/dutchman-cad/pilot/main/docs/images/assistant-plate.gif&alt=SolidCraft%27s%20Assistant%3A%20gemma4%20e4b%20fills%20Pilot%27s%20plate%20form%20and%20SolidCraft%20builds%20a%20plate%20with%20four%20holes%20on%20a%20bolt%20circle"><img src="docs/images/assistant-plate.gif" width="100%" alt="SolidCraft's Assistant: gemma4 e4b fills Pilot's plate form and SolidCraft builds a plate with four holes on a bolt circle"></a>

<sub>gemma4 e4b filling Pilot's plate form in SolidCraft 0.27.1. The wait for the model is shortened.</sub>

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

Each of these was built by Pilot in SolidCraft 0.27.1 from the request under it.

<table>
  <tr>
    <td align="center" valign="top" width="25%"><a href="https://dutchman-cad.github.io/view.html?src=https://raw.githubusercontent.com/dutchman-cad/pilot/main/docs/images/tiles/enclosure.png&alt=an%2080%20x%2060%20x%2030%20mm%20enclosure%20with%202%20mm%20walls%20and%20a%20lip%20for%20the%20lid"><img src="docs/images/tiles/enclosure.png" width="240" alt="an 80 x 60 x 30 mm enclosure with 2 mm walls and a lip for the lid"></a><br><sub>“an 80 x 60 x 30 mm enclosure with 2 mm walls and a lip for the lid”</sub></td>
    <td align="center" valign="top" width="25%"><a href="https://dutchman-cad.github.io/view.html?src=https://raw.githubusercontent.com/dutchman-cad/pilot/main/docs/images/tiles/boltcircle.png&alt=a%20100%20x%2060%20x%206%20mm%20plate%20with%20four%205%20mm%20holes%20on%20a%2040%20mm%20bolt%20circle"><img src="docs/images/tiles/boltcircle.png" width="240" alt="a 100 x 60 x 6 mm plate with four 5 mm holes on a 40 mm bolt circle"></a><br><sub>“a 100 x 60 x 6 mm plate with four 5 mm holes on a 40 mm bolt circle”</sub></td>
    <td align="center" valign="top" width="25%"><a href="https://dutchman-cad.github.io/view.html?src=https://raw.githubusercontent.com/dutchman-cad/pilot/main/docs/images/tiles/lprofile.png&alt=an%20L%20profile%2050%20x%2030%20x%204%20mm%2C%20200%20mm%20long"><img src="docs/images/tiles/lprofile.png" width="240" alt="an L profile 50 x 30 x 4 mm, 200 mm long"></a><br><sub>“an L profile 50 x 30 x 4 mm, 200 mm long”</sub></td>
    <td align="center" valign="top" width="25%"><a href="https://dutchman-cad.github.io/view.html?src=https://raw.githubusercontent.com/dutchman-cad/pilot/main/docs/images/tiles/cchannel.png&alt=a%20C%20channel%2060%20mm%20wide%20with%2030%20mm%20flanges%2C%204%20mm%20thick%20and%20150%20mm%20long"><img src="docs/images/tiles/cchannel.png" width="240" alt="a C channel 60 mm wide with 30 mm flanges, 4 mm thick and 150 mm long"></a><br><sub>“a C channel 60 mm wide with 30 mm flanges, 4 mm thick and 150 mm long”</sub></td>
  </tr>
  <tr>
    <td align="center" valign="top" width="25%"><a href="https://dutchman-cad.github.io/view.html?src=https://raw.githubusercontent.com/dutchman-cad/pilot/main/docs/images/tiles/crossdrilled.png&alt=a%2040%20mm%20cube%20with%208%20mm%20holes%20through%20all%20six%20faces"><img src="docs/images/tiles/crossdrilled.png" width="240" alt="a 40 mm cube with 8 mm holes through all six faces"></a><br><sub>“a 40 mm cube with 8 mm holes through all six faces”</sub></td>
    <td align="center" valign="top" width="25%"><a href="https://dutchman-cad.github.io/view.html?src=https://raw.githubusercontent.com/dutchman-cad/pilot/main/docs/images/tiles/tube.png&alt=a%20tube%2040%20mm%20outside%20diameter%2C%2032%20mm%20inside%20diameter%2C%20100%20mm%20long"><img src="docs/images/tiles/tube.png" width="240" alt="a tube 40 mm outside diameter, 32 mm inside diameter, 100 mm long"></a><br><sub>“a tube 40 mm outside diameter, 32 mm inside diameter, 100 mm long”</sub></td>
    <td align="center" valign="top" width="25%"><a href="https://dutchman-cad.github.io/view.html?src=https://raw.githubusercontent.com/dutchman-cad/pilot/main/docs/images/tiles/cubehole.png&alt=a%2042%20mm%20cube%20with%20a%205%20mm%20hole%20through%20the%20middle"><img src="docs/images/tiles/cubehole.png" width="240" alt="a 42 mm cube with a 5 mm hole through the middle"></a><br><sub>“a 42 mm cube with a 5 mm hole through the middle”</sub></td>
    <td align="center" valign="top" width="25%"><a href="https://dutchman-cad.github.io/view.html?src=https://raw.githubusercontent.com/dutchman-cad/pilot/main/docs/images/tiles/gridplate.png&alt=a%20100%20x%2080%20x%205%20mm%20plate%20with%208%20mm%20rounded%20corners%20and%20a%203%20by%204%20grid%20of%206%20mm%20holes%2020%20mm%20apart"><img src="docs/images/tiles/gridplate.png" width="240" alt="a 100 x 80 x 5 mm plate with 8 mm rounded corners and a 3 by 4 grid of 6 mm holes 20 mm apart"></a><br><sub>“a 100 x 80 x 5 mm plate with 8 mm rounded corners and a 3 by 4 grid of 6 mm holes 20 mm apart”</sub></td>
  </tr>
  <tr>
    <td align="center" valign="top" width="25%"><a href="https://dutchman-cad.github.io/view.html?src=https://raw.githubusercontent.com/dutchman-cad/pilot/main/docs/images/tiles/tsection.png&alt=a%20T%20section%2060%20mm%20wide%2C%2040%20mm%20tall%2C%205%20mm%20thick%2C%20120%20mm%20long"><img src="docs/images/tiles/tsection.png" width="240" alt="a T section 60 mm wide, 40 mm tall, 5 mm thick, 120 mm long"></a><br><sub>“a T section 60 mm wide, 40 mm tall, 5 mm thick, 120 mm long”</sub></td>
    <td align="center" valign="top" width="25%"><a href="https://dutchman-cad.github.io/view.html?src=https://raw.githubusercontent.com/dutchman-cad/pilot/main/docs/images/tiles/copies.png&alt=a%20cylinder%2010%20mm%20across%20and%2010%20mm%20tall%20then%20make%2012%20copies%20of%20it%20in%20a%20row%2C%2015%20mm%20apart"><img src="docs/images/tiles/copies.png" width="240" alt="a cylinder 10 mm across and 10 mm tall then make 12 copies of it in a row, 15 mm apart"></a><br><sub>“a cylinder 10 mm across and 10 mm tall” then “make 12 copies of it in a row, 15 mm apart”</sub></td>
    <td align="center" valign="top" width="25%"><a href="https://dutchman-cad.github.io/view.html?src=https://raw.githubusercontent.com/dutchman-cad/pilot/main/docs/images/tiles/fillet.png&alt=a%2060%20x%2040%20x%2020%20mm%20box%20then%20round%20all%20edges%203%20mm"><img src="docs/images/tiles/fillet.png" width="240" alt="a 60 x 40 x 20 mm box then round all edges 3 mm"></a><br><sub>“a 60 x 40 x 20 mm box” then “round all edges 3 mm”</sub></td>
  </tr>
</table>

The full list:

- Boxes, cylinders, tubes, and spheres, with holes through them
- Holes, bolt circles, hole grids, honeycomb cuts, and cross-drilled blocks
- Plates with holes, bores, and rounded corners
- L, T, U, C, and rectangular profiles
- Enclosures with walls, a floor, and a lip for the lid
- Joints between parts, including a revolute joint on a pin
- Patterns of parts you already have
- Edits: fillets, chamfers, shells, mirrors, revolves, moves, and renames, plus measurements

Each kind and its limits are in [recipes](docs/recipes.md).

## What Pilot actually does

Pilot is mostly a recipe engine, with a safety net for everything else.

1. **Recipes (almost all of the gain).** For a request that fits a known part (plates,
   profiles, holes and hole patterns, enclosures, patterns of parts), Pilot picks the
   recipe and asks the model only to fill in its short typed form: sizes, counts, and
   positions. Pilot then builds the part with checked SolidCraft commands and measures it.
   When the request states every value, Pilot skips the model altogether.
2. **Free-form help (small gain).** When no recipe fits, the model drives SolidCraft's
   tools itself. Pilot repairs broken tool calls, finds the right command, adds short hints,
   retries, and trims old results so the context stays small. That helps a little, but a
   small model still rarely finishes anything with several steps on its own.
3. **Extras:** `/report` bug reports, an opt-in `--planner`, and the SolidCraft 98 driver.

Every set of commands waits for your approval in SolidCraft, and SolidCraft checks every
command before it runs.

## How well it works

Pilot is an early beta. On four blind test sets (56 requests never used during development,
three runs each), gemma4 e4b on 0.1.0-beta.8:

| How the part was built | Attempts | Built correctly |
| --- | ---: | ---: |
| The model filled in a recipe's form | 150 | 100 (67%) |
| Pilot read every value from the request, no model | 4 | 3 |
| The model drove SolidCraft itself, with Pilot's repairs | 14 | 3 (21%) |
| **All** | **168** | **106 (63%)** |

The same model without Pilot built 4 of the 56 (7%). So the recipes are what make a small
model useful today; outside them, expect little. 54 of the 56 test requests have a recipe,
so these numbers say more about recipe parts than about everything people ask for. Full
numbers, and results with other models, are in
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
Same model (gemma4 e4b), same request ("Round the four upright edges of the box with a 3 mm
radius."), same 40 mm box:

<a href="https://dutchman-cad.github.io/view.html?src=https://raw.githubusercontent.com/dutchman-cad/pilot/main/docs/images/solidcraft98-before.png&alt=SolidCraft%2098%20with%20a%2040%20mm%20box%2C%20next%20to%20its%20chat%20window%3A%20gemma4%20e4b%20without%20Pilot%20looks%20for%20a%20fillet%20command%20and%20gives%20up"><img src="docs/images/solidcraft98-before.png" width="100%" alt="SolidCraft 98 with a 40 mm box, next to its chat window: gemma4 e4b without Pilot looks for a fillet command and gives up"></a>

<sub>Without Pilot: the model searches the menus for a fillet command, finds none, and gives up.</sub>

<a href="https://dutchman-cad.github.io/view.html?src=https://raw.githubusercontent.com/dutchman-cad/pilot/main/docs/images/solidcraft98-after.png&alt=SolidCraft%2098%20with%20the%20same%20box%2C%20its%20four%20upright%20edges%20rounded%203%20mm%2C%20next%20to%20the%20chat%20window%3A%20gemma4%20e4b%20through%20Pilot"><img src="docs/images/solidcraft98-after.png" width="100%" alt="SolidCraft 98 with the same box, its four upright edges rounded 3 mm, next to the chat window: gemma4 e4b through Pilot"></a>

<sub>With Pilot: the model opens the box's sketch and sets its corner radius, which rounds the
four upright edges. Rounding the top and bottom edges requires SolidCraft 2000.</sub>

Both are replays of recorded runs, so they skip the model's thinking time. The run shown is the
second of two recorded tries; the first typed "3 mm" into a numbers-only field, which Pilot now
fixes. It is not reliable yet: in an earlier 11-task test with two runs each, gemma4 e4b built 2
of 22 parts through Pilot and 6 of 22 on its own; qwen2.5 7B, granite4 3B, and Muse Glimmer 30B
built none either way.

Licensed under [Apache-2.0](LICENSE). See [NOTICE](NOTICE).
