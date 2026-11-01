#!/bin/sh
# Runs website/static/install.sh end to end against a release made here
# from the binary given (CI gives its own build; any imece that answers
# --version does): the download, the checksum, the install, the PATH line
# once per shell, the refusals. Nothing outside a temporary directory is
# touched.
#
#   website/test-install.sh target/debug/imece

set -eu

bin=${1:?usage: website/test-install.sh PATH-TO-IMECE}
here=$(cd "$(dirname "$0")" && pwd)
installer="$here/static/install.sh"
work=$(mktemp -d 2>/dev/null || mktemp -d -t imece-test)
trap 'rm -rf "$work"' EXIT

fail() {
    printf 'test-install: FAIL: %s\n' "$*" >&2
    exit 1
}

sha256() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | cut -d ' ' -f 1
    else
        shasum -a 256 "$1" | cut -d ' ' -f 1
    fi
}

# The target install.sh asks for here (as it works it out; not under
# Rosetta).
case "$(uname -s)" in
    Darwin) os=apple-darwin ;;
    Linux) os=unknown-linux-musl ;;
    *) fail "unknown system" ;;
esac
case "$(uname -m)" in
    x86_64 | amd64) arch=x86_64 ;;
    arm64 | aarch64) arch=aarch64 ;;
    *) fail "unknown cpu" ;;
esac
archive="imece-$arch-$os.tar.gz"

# A release as release.yml makes it: the archive with imece at its top,
# and SHA256SUMS.
release="$work/release"
mkdir -p "$release" "$work/pack"
cp "$bin" "$work/pack/imece"
tar -C "$work/pack" -czf "$release/$archive" imece
(cd "$release" && printf '%s  %s\n' "$(sha256 "$archive")" "$archive" >SHA256SUMS)

# install.sh with a clean home and a PATH without its directory.
run() {
    home=$1
    shift
    env -i HOME="$home" PATH="/usr/bin:/bin:/usr/sbin:/sbin" \
        IMECE_DOWNLOAD_URL="file://$release" "$@" sh "$installer"
}

# 1. A bash user: installed in ~/.local/bin, the line added once.
h="$work/bash"
mkdir -p "$h"
out=$(run "$h" SHELL=/bin/bash) || fail "install failed: $out"
"$h/.local/bin/imece" --version >/dev/null || fail "the installed imece does not run"
[ "$(uname -s)" = Darwin ] && rc="$h/.bash_profile" || rc="$h/.bashrc"
line="export PATH=\"$h/.local/bin:\$PATH\""
[ "$(grep -cxF "$line" "$rc")" = 1 ] || fail "no PATH line in $rc"
printf '%s\n' "$out" | grep -q "installed imece " || fail "no installed line: $out"
printf '%s\n' "$out" | grep -q "added $h/.local/bin to PATH in $rc" || fail "no added line: $out"
# Again: updated, the line still once.
run "$h" SHELL=/bin/bash >/dev/null || fail "a second install failed"
[ "$(grep -cxF "$line" "$rc")" = 1 ] || fail "the PATH line was added twice"
[ ! -e "$h/.local/bin/.imece.new" ] || fail "a half-installed file is left"

# 2. zsh, fish and any other shell get their own file.
h="$work/zsh"
mkdir -p "$h"
run "$h" SHELL=/bin/zsh >/dev/null || fail "zsh install failed"
grep -qxF "export PATH=\"$h/.local/bin:\$PATH\"" "$h/.zshrc" || fail "no PATH line in .zshrc"
h="$work/fish"
mkdir -p "$h"
run "$h" SHELL=/usr/bin/fish >/dev/null || fail "fish install failed"
grep -qxF "fish_add_path \"$h/.local/bin\"" "$h/.config/fish/conf.d/imece.fish" ||
    fail "no fish_add_path line"
h="$work/sh"
mkdir -p "$h"
run "$h" SHELL=/bin/dash >/dev/null || fail "sh install failed"
grep -qxF "export PATH=\"$h/.local/bin:\$PATH\"" "$h/.profile" || fail "no PATH line in .profile"

# 3. A directory of one's own, startup files left alone.
h="$work/own"
mkdir -p "$h"
run "$h" SHELL=/bin/zsh IMECE_INSTALL_DIR="$h/bin" IMECE_NO_MODIFY_PATH=1 >/dev/null ||
    fail "install to a directory of one's own failed"
[ -x "$h/bin/imece" ] || fail "not installed in IMECE_INSTALL_DIR"
[ ! -e "$h/.zshrc" ] || fail "IMECE_NO_MODIFY_PATH=1 wrote a startup file"

# 4. A directory already on PATH: no startup file.
h="$work/onpath"
mkdir -p "$h/.local/bin"
env -i HOME="$h" PATH="$h/.local/bin:/usr/bin:/bin" SHELL=/bin/zsh \
    IMECE_DOWNLOAD_URL="file://$release" sh "$installer" >/dev/null ||
    fail "install with the directory on PATH failed"
[ ! -e "$h/.zshrc" ] || fail "a startup file was written though the directory is on PATH"

# 5. A download that does not match its checksum installs nothing.
h="$work/bad"
mkdir -p "$h"
cp "$release/SHA256SUMS" "$work/sums"
printf '%s  %s\n' "0000000000000000000000000000000000000000000000000000000000000000" "$archive" \
    >"$release/SHA256SUMS"
if out=$(run "$h" SHELL=/bin/bash 2>&1); then
    fail "a wrong checksum was installed"
fi
printf '%s\n' "$out" | grep -q "does not match its SHA256SUMS" || fail "wrong checksum not said: $out"
[ ! -e "$h/.local/bin/imece" ] || fail "a wrong checksum left an imece"
# 6. Nor does a SHA256SUMS that does not list the archive.
printf '%s  %s\n' "$(sha256 "$release/$archive")" "imece-other.tar.gz" >"$release/SHA256SUMS"
if out=$(run "$h" SHELL=/bin/bash 2>&1); then
    fail "an archive missing from SHA256SUMS was installed"
fi
printf '%s\n' "$out" | grep -q "SHA256SUMS lists no $archive" || fail "missing entry not said: $out"
[ ! -e "$h/.local/bin/imece" ] || fail "a missing entry left an imece"
cp "$work/sums" "$release/SHA256SUMS"

# 7. Nothing there to download: said, nothing installed.
h="$work/gone"
mkdir -p "$h"
if env -i HOME="$h" PATH="/usr/bin:/bin" SHELL=/bin/bash \
    IMECE_DOWNLOAD_URL="file://$work/nowhere" sh "$installer" >/dev/null 2>&1; then
    fail "a missing release was installed"
fi
[ ! -e "$h/.local/bin/imece" ] || fail "a missing release left an imece"

printf 'test-install: ok (%s)\n' "$archive"
