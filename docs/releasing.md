# Releasing Pilot

A release is a tag on `main`. Pushing the matching `vVERSION` tag runs
[the release workflow](../.github/workflows/release.yml), which builds Linux
(x86_64 and aarch64), macOS (Apple silicon), and Windows archives and attaches
them, with checksum files, to a draft on
[GitHub Releases](https://github.com/dutchman-cad/pilot/releases). The draft is
published once its notes are written.

## Before the tag

1. **Measure.** Run the paired benchmark from [Measure it](measure.md) on the
   held-out sets and the field set, directly and through Pilot, with the same
   endpoint, models, and task sets as the last release. A change ships only if
   held-out results go up and nothing regresses. Keep the numbers; the release
   notes quote them.
2. **Check.** Run every command in [CONTRIBUTING.md](../CONTRIBUTING.md) on the
   commit to be tagged, including the `core` build for `wasm32-unknown-unknown`.
3. **Version.** Set `version` in `Cargo.toml` to the new version, and rename
   the `Unreleased` heading in [CHANGELOG.md](../CHANGELOG.md) to
   `## <version> (<date>)`. The workflow refuses a tag that does not match
   `Cargo.toml`.
4. **Image.** Every release shows at least one new image of what is new: a
   screenshot of SolidCraft's Assistant building something the release
   enables, or a render of the part (`solidcraft-cli snapshot` or `render` of
   the commands Pilot sent). Save it as
   `docs/images/releases/<version>/<name>.png`, under 1 MB, with no private
   designs, names, or paths in view. The release notes show it, and
   announcements reuse the same file.

## The release notes

- What is new, in plain words, one line per change, linking issues and pull
  requests.
- The measured numbers from step 1: before and after, with the model, mode,
  and task set.
- The image from step 4, with a one-line caption saying what it shows.
- Upgrade notes for any changed flag or default.

## After publishing

1. Update the [results issue](https://github.com/dutchman-cad/pilot/issues/1)
   and the README with the new numbers and the release link.
2. Bump the pinned Pilot revision in projects that depend on it, such as
   [dutchman-cad/plugins](https://github.com/dutchman-cad/plugins), and check
   that they still build.
3. Answer field reports that the release fixes, with the release link, the
   measured number, and the image.
