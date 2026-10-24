#!/bin/sh
set -eu
repo=$(CDPATH= cd -P "$(dirname "$0")/.." && pwd)
root=$(mktemp -d "${TMPDIR:-/tmp}/pilot-install-test.XXXXXXXX")
trap 'rm -rf "$root"' 0
trap 'exit 1' HUP INT TERM
mkdir -p "$root/archive" "$root/path"
cp "$repo/install.sh" "$root/archive/"
printf '#!/bin/sh\necho "dutchman launcher 1.0.0"\n' >"$root/archive/dutchman"
printf '#!/bin/sh\necho "dutchman-pilot 1.0.0"\n' >"$root/archive/dutchman-pilot"
chmod +x "$root/archive/dutchman" "$root/archive/dutchman-pilot"
# Isolate from any Dutchman in the caller's PATH.
for tool in dirname mktemp rm sleep mkdir cp chmod grep; do
    ln -s "$(command -v "$tool")" "$root/path/$tool"
done
PATH=$root/path
export PATH
run_install() { /bin/sh "$root/archive/install.sh" --prefix "$root/$1"; }
assert_same() { /usr/bin/cmp "$1" "$2"; }
run_install empty
assert_same "$root/archive/dutchman" "$root/empty/dutchman"
assert_same "$root/archive/dutchman-pilot" "$root/empty/dutchman-pilot"
run_install empty
assert_same "$root/archive/dutchman" "$root/empty/dutchman"
mkdir "$root/full" "$root/old"
printf '#!/bin/sh\necho "dutchman 0.9.0"\n' >"$root/full/dutchman"
printf '#!/bin/sh\necho "dutchman launcher 0.0.1"\n' >"$root/old/dutchman"
chmod +x "$root/full/dutchman" "$root/old/dutchman"
cp "$root/full/dutchman" "$root/full-original"
run_install full >"$root/full-output"
grep -Fx 'Existing Dutchman kept; the existing Dutchman will run dutchman pilot.' "$root/full-output"
assert_same "$root/full-original" "$root/full/dutchman"
assert_same "$root/archive/dutchman-pilot" "$root/full/dutchman-pilot"
run_install old
assert_same "$root/archive/dutchman" "$root/old/dutchman"
# A full command on PATH also prevents installing a shadowing launcher.
cp "$root/full/dutchman" "$root/path/dutchman"
run_install path-full
[ ! -e "$root/path-full/dutchman" ]
assert_same "$root/full-original" "$root/path/dutchman"
# This also holds when the destination has an older launcher.
printf '#!/bin/sh\necho "dutchman launcher 0.0.1"\n' >"$root/old/dutchman"
cp "$root/old/dutchman" "$root/old-original"
run_install old
assert_same "$root/old-original" "$root/old/dutchman"
rm "$root/path/dutchman"
/bin/sh "$root/archive/install.sh" --prefix "$root/dry run" --dry-run
[ ! -e "$root/dry run" ]
run_install 'with spaces'
assert_same "$root/archive/dutchman" "$root/with spaces/dutchman"
# Failed identification must preserve the existing command.
printf '#!/bin/sh\nexit 1\n' >"$root/full/dutchman"
cp "$root/full/dutchman" "$root/failed-original"
run_install full
assert_same "$root/failed-original" "$root/full/dutchman"
# A stalled version command must be kept without stalling installation.
printf '#!/bin/sh\ntrap "" TERM\nwhile :; do :; done\n' >"$root/full/dutchman"
cp "$root/full/dutchman" "$root/stalled-original"
run_install full
assert_same "$root/stalled-original" "$root/full/dutchman"
printf 'POSIX installer tests passed.\n'
