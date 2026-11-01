# Install

Homebrew (Linux, and macOS on Apple silicon):

```sh
brew install dutchman-cad/tap/dutchman-pilot
```

A Homebrew install needs no first-run Gatekeeper approval on macOS.

Scoop (Windows):

```powershell
scoop bucket add dutchman-cad https://github.com/dutchman-cad/scoop-bucket
scoop install dutchman-cad/dutchman-pilot
```

Both install `dutchman-pilot` and `dutchman-pilot-eval`, and follow each new release.

## From a release archive

Download the archive for your platform from
[the latest release](https://github.com/dutchman-cad/pilot/releases/latest):
`x86_64-linux`, `aarch64-linux`, `aarch64-macos`, or `x86_64-windows`.
Archive names start with `pilot-VERSION-`. Each contains `dutchman-pilot`,
`dutchman-pilot-eval`, the shared `dutchman` launcher, installers, docs, corpus,
license notices, and a `LAUNCHER_REV` file identifying the launcher source commit.
Compare its SHA-256 digest with the adjacent `.sha256` file before extracting:

```sh
sha256sum -c <archive>.sha256
# macOS: shasum -a 256 -c <archive>.sha256
```

On Windows, run `Get-FileHash <archive>.zip -Algorithm SHA256` in PowerShell
and compare the digest with the checksum file. Extract the archive, enter its
directory, and run the installer:

```sh
# Linux and macOS: installs into ~/.local/bin
sh install.sh
# Or choose a bin directory; preview with --dry-run
sh install.sh --prefix "$HOME/bin" --dry-run
```

```powershell
# Windows: installs into %LOCALAPPDATA%\Programs\dutchman-cad\bin
.\install.ps1
# Or choose a bin directory; preview with -DryRun
.\install.ps1 -Prefix "$HOME\bin" -DryRun
# Optionally add the directory to your user PATH, then open a new terminal
.\install.ps1 -AddToPath
```

The installers copy `dutchman-pilot`, `dutchman-pilot-eval`, and `dutchman` into the chosen bin directory.
Add that directory to PATH as the installer suggests. They require no administrator
access. Only Windows `-AddToPath` changes your saved user PATH.
You can rerun either installer to update an older launcher. A full Dutchman install
at the destination or on PATH is kept; it also runs `dutchman pilot`.

Both command forms work:

```sh
dutchman pilot --version
dutchman-pilot --version
```

Pilot also runs as `dutchman pilot` from the [dutchman](https://github.com/dutchman-cad/dutchman)
command line, with the same arguments.

macOS binaries are ad-hoc signed, without notarization; use your system's approval
controls for a trusted download.
Linux archives are built on Ubuntu 24.04 with glibc; build from source for older
systems or other C libraries.

## From source

Build from a checkout with Rust 1.88 or newer:

```sh
cargo install --locked --path . --bins
dutchman-pilot --help
dutchman-pilot how "round all edges 3 mm"
```
