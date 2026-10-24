#!/bin/sh
# Install the two commands from an extracted Pilot archive.
set -eu
prefix=${HOME:?HOME is required}/.local/bin
dry_run=false
while [ "$#" -gt 0 ]; do
    case $1 in
        --prefix) [ "$#" -ge 2 ] && [ -n "$2" ] || { echo '--prefix needs a directory' >&2; exit 1; }; prefix=$2; shift 2 ;;
        --dry-run) dry_run=true; shift ;;
        --help|-h) echo 'Usage: sh install.sh [--prefix BIN_DIRECTORY] [--dry-run]'; exit 0 ;;
        *) echo "Unknown option: $1" >&2; exit 1 ;;
    esac
done
case $prefix in /*) ;; *) prefix=$PWD/$prefix ;; esac
if [ -d "$prefix" ]; then prefix=$(CDPATH= cd -P "$prefix" && pwd); fi
source_dir=$(CDPATH= cd -P "$(dirname "$0")" && pwd)
for name in dutchman-pilot dutchman; do
    [ -f "$source_dir/$name" ] && [ -x "$source_dir/$name" ] || {
        echo "Missing executable beside installer: $name" >&2; exit 1;
    }
done
probe_dir=$(mktemp -d "${TMPDIR:-/tmp}/pilot-install.XXXXXXXX")
trap 'rm -rf "$probe_dir"' 0
trap 'exit 1' HUP INT TERM
is_launcher() {
    # Limit output to eight file blocks and execution to three seconds.
    /bin/sh -c 'ulimit -f 8 || exit 1; exec "$1" --version' pilot-install-probe "$1" >"$probe_dir/version" 2>/dev/null &
    probe_pid=$!
    marker=$probe_dir/$probe_pid.running
    : >"$marker"
    /bin/sh -c 'sleep 3; if [ -f "$1" ]; then kill -s KILL "$2" 2>/dev/null || :; fi' pilot-install-timer "$marker" "$probe_pid" >/dev/null 2>&1 &
    probe_ok=false
    if wait "$probe_pid"; then probe_ok=true; fi
    rm -f "$marker"
    version=
    IFS= read -r version <"$probe_dir/version" || :
    case $version in 'dutchman launcher'*) "$probe_ok" ;; *) return 1 ;; esac
}
keep_full=false
check_existing() {
    if [ -e "$1" ] || [ -L "$1" ]; then
        if ! is_launcher "$1"; then keep_full=true; fi
    fi
}
check_existing "$prefix/dutchman"
remaining=${PATH-}
count=0
while :; do
    count=$((count + 1))
    [ "$count" -le 256 ] || { echo 'PATH exceeds 256 entries' >&2; exit 1; }
    case $remaining in
        *:*) entry=${remaining%%:*}; remaining=${remaining#*:}; last=false ;;
        *) entry=$remaining; last=true ;;
    esac
    entry=${entry:-.}
    if [ -x "$entry/dutchman" ] && [ ! -d "$entry/dutchman" ]; then
        check_existing "$entry/dutchman"
    fi
    "$last" && break
done
copy_command() {
    if [ "$source_dir/$1" = "$prefix/$1" ]; then
        echo "Already installed: $prefix/$1"
    elif "$dry_run"; then
        echo "Would install: $prefix/$1"
    else
        mkdir -p "$prefix"
        # Remove a destination symlink rather than writing through it.
        rm -f "$prefix/$1"
        cp "$source_dir/$1" "$prefix/$1"
        chmod 755 "$prefix/$1"
        echo "Installed: $prefix/$1"
    fi
}
copy_command dutchman-pilot
if "$keep_full"; then
    echo 'Existing Dutchman kept; the existing Dutchman will run dutchman pilot.'
else
    copy_command dutchman
fi
printf 'PATH hint: add %s to PATH to run the installed commands.\n' "$prefix"
