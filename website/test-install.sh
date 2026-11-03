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

# 8. iTerm2's tab icon (macOS, iTerm2's folder there): the mark and imece
# in both tables, what the person wrote kept; elsewhere nothing.
icon="$here/static/img/imece-tab.png"
it() {
    printf '%s/Library/Application Support/iTerm2' "$1"
}
h="$work/iterm"
mkdir -p "$(it "$h")"
out=$(run "$h" SHELL=/bin/zsh) || fail "install with iTerm2 failed: $out"
if [ "$(uname -s)" = Darwin ]; then
    d=$(it "$h")
    cmp -s "$icon" "$d/graphic_imece.png" || fail "the tab icon is not imece-tab.png"
    [ "$(plutil -extract imece json -o - "$d/graphic_icons.json")" = '["imece"]' ] ||
        fail "graphic_icons.json lacks imece: $(cat "$d/graphic_icons.json")"
    [ "$(plutil -extract imece raw -o - "$d/graphic_colors.json")" = '#E3A93F' ] ||
        fail "graphic_colors.json lacks imece's amber: $(cat "$d/graphic_colors.json")"
    printf '%s\n' "$out" | grep -q "iTerm2: imece's tab shows its mark from iTerm2's next start" ||
        fail "the icon not said: $out"
    for f in "$d"/.*.new "$d"/*.new; do
        [ ! -e "$f" ] || fail "a half-written file is left: $f"
    done
    # Again: the same, nothing twice.
    cp "$d/graphic_icons.json" "$work/icons1"
    run "$h" SHELL=/bin/zsh >/dev/null || fail "a second install with iTerm2 failed"
    cmp -s "$work/icons1" "$d/graphic_icons.json" || fail "a second install changed graphic_icons.json"

    # The person's own tables: their entries kept, imece added, still JSON.
    h="$work/iterm-own"
    d=$(it "$h")
    mkdir -p "$d"
    printf '{\n  "vim": ["vim", "nvim"]\n}\n' >"$d/graphic_icons.json"
    printf '{"vim": "#00FF00"}\n' >"$d/graphic_colors.json"
    run "$h" SHELL=/bin/zsh >/dev/null || fail "install over own tables failed"
    [ "$(plutil -extract vim json -o - "$d/graphic_icons.json")" = '["vim","nvim"]' ] ||
        fail "the person's icon entry was lost: $(cat "$d/graphic_icons.json")"
    [ "$(plutil -extract imece json -o - "$d/graphic_icons.json")" = '["imece"]' ] ||
        fail "imece not added to the person's graphic_icons.json"
    [ "$(plutil -extract vim raw -o - "$d/graphic_colors.json")" = '#00FF00' ] ||
        fail "the person's colour was lost"
    [ "$(cut -c 1 <"$d/graphic_icons.json" | head -n 1)" = "{" ] || fail "not JSON any more"

    # A table that names imece already, one that is not a JSON object, a
    # link: left byte for byte, and said.
    h="$work/iterm-kept"
    d=$(it "$h")
    mkdir -p "$d"
    printf '{"imece": ["imece", "mine"]}' >"$d/graphic_icons.json"
    printf '<?xml version="1.0"?><plist version="1.0"><dict/></plist>' >"$d/graphic_colors.json"
    cp "$d/graphic_icons.json" "$work/kept-icons"
    cp "$d/graphic_colors.json" "$work/kept-colors"
    out=$(run "$h" SHELL=/bin/zsh) || fail "install over kept tables failed: $out"
    cmp -s "$work/kept-icons" "$d/graphic_icons.json" || fail "a table naming imece was rewritten"
    cmp -s "$work/kept-colors" "$d/graphic_colors.json" || fail "a table not JSON was rewritten"
    printf '%s\n' "$out" | grep -q "iTerm2: left graphic_colors.json in" || fail "not said: $out"
    # An empty table of one's own takes imece; one with a null (no plist
    # holds it) is left.
    h="$work/iterm-null"
    d=$(it "$h")
    mkdir -p "$d"
    printf '{}' >"$d/graphic_icons.json"
    printf '{"vim": null}' >"$d/graphic_colors.json"
    out=$(run "$h" SHELL=/bin/zsh) || fail "install over an empty and a null table failed: $out"
    [ "$(plutil -extract imece json -o - "$d/graphic_icons.json")" = '["imece"]' ] ||
        fail "imece not added to an empty table: $(cat "$d/graphic_icons.json")"
    [ "$(cat "$d/graphic_colors.json")" = '{"vim": null}' ] || fail "a table with a null was rewritten"
    printf '%s\n' "$out" | grep -q "iTerm2: left graphic_colors.json in" || fail "null not said: $out"
    for f in "$d"/.*.new "$d"/*.new; do
        [ ! -e "$f" ] || fail "a half-written file is left: $f"
    done
    h="$work/iterm-link"
    d=$(it "$h")
    mkdir -p "$d"
    printf '{}' >"$work/dotfile-icons.json"
    ln -s "$work/dotfile-icons.json" "$d/graphic_icons.json"
    printf 'theirs' >"$work/elsewhere.png"
    ln -s "$work/elsewhere.png" "$d/graphic_imece.png"
    out=$(run "$h" SHELL=/bin/zsh) || fail "install over links failed: $out"
    [ "$(cat "$work/dotfile-icons.json")" = '{}' ] || fail "written through a linked table"
    [ -L "$d/graphic_icons.json" ] || fail "a linked table was replaced"
    [ "$(cat "$work/elsewhere.png")" = theirs ] || fail "the icon was written through a link"
    cmp -s "$icon" "$d/graphic_imece.png" || fail "the linked icon was not replaced by the mark"
    printf '%s\n' "$out" | grep -q "iTerm2: left graphic_icons.json in" || fail "link not said: $out"

    # Asked not to, or no iTerm2: nothing.
    h="$work/iterm-no"
    d=$(it "$h")
    mkdir -p "$d"
    run "$h" SHELL=/bin/zsh IMECE_NO_ITERM_ICON=1 >/dev/null || fail "install with IMECE_NO_ITERM_ICON failed"
    [ -z "$(ls -A "$d")" ] || fail "IMECE_NO_ITERM_ICON=1 wrote $(ls -A "$d")"
    h="$work/no-iterm"
    mkdir -p "$h"
    run "$h" SHELL=/bin/zsh >/dev/null || fail "install without iTerm2 failed"
    [ ! -e "$h/Library" ] || fail "an iTerm2 folder was made where there was none"
else
    [ -z "$(ls -A "$(it "$h")")" ] || fail "iTerm2's folder was written on $(uname -s)"
    printf '%s\n' "$out" | grep -q iTerm2 && fail "iTerm2 said on $(uname -s): $out"
fi
# The icon install.sh carries is the one in the repository.
grep -q "^ICON_PNG='$(base64 <"$icon" | tr -d '\n')'\$" "$installer" ||
    fail "install.sh's ICON_PNG is not website/static/img/imece-tab.png"

printf 'test-install: ok (%s)\n' "$archive"
