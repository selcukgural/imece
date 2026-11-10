#!/bin/sh
# Makes the world an acceptance run happens in (docs/acceptance/README.md):
# the candidate's imece, its own state and config directories, the sample
# checkouts, a release directory for the installer, a home for Lonca's
# daemon, and env.sh to source. Nothing outside DIR is written but the
# release build in target/.
#
#   tools/acceptance/setup.sh /tmp/imece-acc
#   . /tmp/imece-acc/env.sh
#
# DIR must not exist yet, and must not be under a scratchpad directory of
# Claude Code's on macOS (a binary there may hang at exec): /tmp/… is fine.
# IMECE_ACC_BIN=path takes an imece already built instead of building one.

set -eu

dir=${1:?usage: tools/acceptance/setup.sh DIR}
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)

case "$dir" in
    /*) ;;
    *) dir="$PWD/$dir" ;;
esac
if [ -e "$dir" ]; then
    printf 'setup: %s is there already; an acceptance run starts from nothing\n' "$dir" >&2
    exit 1
fi

if [ -n "${IMECE_ACC_BIN:-}" ]; then
    bin=$IMECE_ACC_BIN
else
    (cd "$root" && cargo build --release --locked -p imece)
    bin="$root/target/release/imece"
fi
[ -x "$bin" ] || { printf 'setup: no imece at %s\n' "$bin" >&2; exit 1; }

mkdir -p "$dir"
chmod 700 "$dir"
mkdir -p "$dir/bin" "$dir/state" "$dir/config/imece" "$dir/home0" "$dir/nobody" \
    "$dir/out" "$dir/rel" "$dir/pack" "$dir/lonca" "$dir/humans" "$dir/repos"
cp "$bin" "$dir/bin/imece"
chmod 755 "$dir/bin/imece"

# What a release would put up for this machine, for INST-1.
host=$(rustc -vV | sed -n 's/^host: //p')
case "$host" in
    *-linux-gnu) target=${host%-gnu}-musl ;;
    *) target=$host ;;
esac
cp "$dir/bin/imece" "$root/LICENSE" "$dir/pack/"
tar -C "$dir/pack" -czf "$dir/rel/imece-$target.tar.gz" imece LICENSE
if command -v sha256sum >/dev/null 2>&1; then
    (cd "$dir/rel" && sha256sum "imece-$target.tar.gz" > SHA256SUMS)
else
    (cd "$dir/rel" && shasum -a 256 "imece-$target.tar.gz" > SHA256SUMS)
fi

# The sample checkouts: one commit each, no hooks, no signing.
cp -R "$here/repos/shelf" "$here/repos/tally" "$here/repos/plain" "$dir/repos/"
chmod 755 "$dir/repos/shelf/.imece/check" "$dir/repos/shelf/.imece/"*.sh
for r in shelf tally; do
    git -C "$dir/repos/$r" init -q -b main
    git -C "$dir/repos/$r" add -A
    git -C "$dir/repos/$r" -c user.name=acceptance -c user.email=acceptance@localhost \
        -c commit.gpgsign=false -c core.hooksPath=/dev/null \
        commit -q -m "The sample as the acceptance run starts it"
done
printf 'SAMPLE_SETTING=not-a-secret\n' > "$dir/repos/shelf/.env"
printf 'colour = blue\n' > "$dir/repos/shelf/local.cfg"

# A colour file in Ghostty's format, for SCR-9.
cat > "$dir/config/mine.theme" <<'EOF'
background = #1b1d23
foreground = #d8dee9
palette = 1=#e06c75
palette = 2=#98c379
palette = 3=#e5c07b
palette = 4=#61afef
palette = 5=#c678dd
palette = 6=#56b6c2
EOF

cat > "$dir/env.sh" <<EOF
# Sourced by the acceptance run (tools/acceptance/setup.sh made it).
export ACC='$dir'
export PATH='$dir/bin':"\$PATH"
export XDG_STATE_HOME='$dir/state'
export XDG_CONFIG_HOME='$dir/config'
export LONCA_HOME='$dir/lonca'
export LONCA_HUMANS='$dir/humans'
export EDITOR=vi
EOF

# The run's imece: the standard tier sonnet, the cheap haiku, a pot that
# stops a run gone wrong. Any flag given goes after these and wins.
cat > "$dir/bin/imece-acc" <<EOF
#!/bin/sh
cd '$dir/repos/shelf' || exit 1
exec '$dir/bin/imece' --model sonnet --tier cheap=haiku --budget 15 "\$@"
EOF
chmod 755 "$dir/bin/imece-acc"

printf 'setup: ready in %s (imece %s); now: . %s/env.sh\n' \
    "$dir" "$("$dir/bin/imece" --version | sed 's/^imece //')" "$dir"
