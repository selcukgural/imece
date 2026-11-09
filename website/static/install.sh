#!/bin/sh
# imece's installer: puts the prebuilt imece for this machine on PATH.
#
#   curl -fsSL https://selcukgural.github.io/imece/install.sh | sh
#
# It downloads the release archive for this system (macOS or Linux, x86_64
# or arm64) from https://github.com/selcukgural/imece/releases, checks it
# against the release's SHA256SUMS (nothing is installed when it does not
# match), puts `imece` in ~/.local/bin and, when that is not on PATH yet,
# adds it in your shell's startup file once. Running it again updates.
#
# Settings, from the environment:
#   IMECE_VERSION=0.1.0      a version instead of the latest
#   IMECE_INSTALL_DIR=DIR    where imece goes (default: ~/.local/bin)
#   IMECE_NO_MODIFY_PATH=1   never touch a startup file
#   IMECE_NO_ICON=1          no icon for imece's tab (iTerm2, Konsole)
#   IMECE_DOWNLOAD_URL=URL   where the release files are: a mirror's
#                            https:// address, a file:// directory or
#                            a directory's absolute path (the tests')
#
# It also gives imece's tab its mark where a terminal shows a program's
# icon: iTerm2 on macOS (iterm_icon), Konsole on Linux (theme_icon).
#
# Uninstall: rm ~/.local/bin/imece (and the "# imece" lines it added; the
# icon: on macOS graphic_imece.png and the "imece" entries in
# ~/Library/Application Support/iTerm2/graphic_icons.json and
# graphic_colors.json, on Linux
# ~/.local/share/icons/hicolor/scalable/apps/imece.svg).
#
# Everything runs from `main` at the very end, so a download cut short
# runs nothing.

set -eu

REPO="https://github.com/selcukgural/imece"

say() {
    printf 'imece: %s\n' "$*"
}

die() {
    printf 'imece: %s\n' "$*" >&2
    exit 1
}

has() {
    command -v "$1" >/dev/null 2>&1
}

# The release target this machine runs: its CPU and system as Rust names
# them. Linux builds are static (musl), so any distribution takes them.
target() {
    os=$(uname -s)
    arch=$(uname -m)
    case "$os" in
        Darwin) os=apple-darwin ;;
        Linux) os=unknown-linux-musl ;;
        *) die "imece runs on macOS and Linux (on Windows: inside WSL); this is $os" ;;
    esac
    case "$arch" in
        x86_64 | amd64) arch=x86_64 ;;
        arm64 | aarch64) arch=aarch64 ;;
        *) die "no imece build for $arch; build from source: $REPO" ;;
    esac
    # A shell under Rosetta on Apple silicon says x86_64: the native build
    # is the one to have.
    if [ "$os" = apple-darwin ] && [ "$arch" = x86_64 ] &&
        [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || true)" = 1 ]; then
        arch=aarch64
    fi
    printf '%s-%s\n' "$arch" "$os"
}

# Fetches $1 to the file $2, over HTTPS only (a mirror given in
# IMECE_DOWNLOAD_URL may also be a local directory, as file://).
download() {
    if has curl; then
        proto='=https'
        if [ -n "${IMECE_DOWNLOAD_URL:-}" ]; then
            proto='=https,file'
        fi
        curl --proto "$proto" --proto-redir '=https' --tlsv1.2 \
            --fail --silent --show-error --location --retry 3 \
            --output "$2" "$1" || die "could not download $1"
    elif has wget; then
        wget --https-only --quiet --output-document="$2" "$1" ||
            die "could not download $1"
    else
        die "curl or wget is needed to download imece"
    fi
}

sha256() {
    if has sha256sum; then
        sha256sum "$1" | cut -d ' ' -f 1
    elif has shasum; then
        shasum -a 256 "$1" | cut -d ' ' -f 1
    else
        die "sha256sum or shasum is needed to check the download"
    fi
}

# The line that puts $1 on PATH in the startup file of the person's shell,
# and that file, as "FILE|LINE". `$PATH` is written as it is, for the shell
# to expand when it starts.
# shellcheck disable=SC2016
path_line() {
    case "$(basename "${SHELL:-sh}")" in
        zsh)
            printf '%s|export PATH="%s:$PATH"\n' "${ZDOTDIR:-$HOME}/.zshrc" "$1"
            ;;
        bash)
            # macOS terminals open login shells, which read .bash_profile.
            if [ "$(uname -s)" = Darwin ]; then
                rc="$HOME/.bash_profile"
            else
                rc="$HOME/.bashrc"
            fi
            printf '%s|export PATH="%s:$PATH"\n' "$rc" "$1"
            ;;
        fish)
            printf '%s|fish_add_path "%s"\n' \
                "${XDG_CONFIG_HOME:-$HOME/.config}/fish/conf.d/imece.fish" "$1"
            ;;
        *)
            printf '%s|export PATH="%s:$PATH"\n' "$HOME/.profile" "$1"
            ;;
    esac
}

# imece's mark (five hands round one job), 32 x 32, black on transparent:
# iTerm2 tints it. Made by tools/tab_icon.py; website/static/img/imece-tab.png
# is the same bytes (website/test-install.sh checks).
ICON_PNG='iVBORw0KGgoAAAANSUhEUgAAACAAAAAgCAYAAABzenr0AAAA7klEQVR42u1W2w3DIAzMBozACBmBURiBERiBERiFEToKI1AquT+WAfNII6WcZOUnPhz7fOQ4NjbG4XJECHfH4QnFT4uIRAHxrwq4fQSXi1Dm8DkCPMUAh0AcsicRz/jVWYSAHKwRFoch5vsJ3VGALnAYTrKdSW58hOUkq0KyJAr9itASGqI41EgLI9F+xSDXSEt6RMWqY0y20lGxek3VTHtXoaaB58GAo7mGo0l4J3SubRWecDRZOBw7qJ89/Czss2PekAk4lqo9QYsxwqz5cC+mki2bwk/KtAdgR/NMvcQR92u54snUzSXut7FxCd5AfZ+plmag8AAAAABJRU5ErkJggg=='

# iTerm2 shows the icon of the program running in a tab from its own table
# of programs; imece is not in the one it ships, so its tab shows a letter
# in a circle. iTerm2 reads two tables of the person's from its folder over
# its own (graphic_icons.json: an icon's name and the programs that show
# it; graphic_colors.json: a name and its colour) and the icon as
# graphic_NAME.png beside them, when it starts. imece's mark goes there in
# imece's amber. What the person wrote stays: a table that already names
# imece is not touched, and one that is not a JSON object, or is a link, is
# left as it is and said.
iterm_icon() {
    support="$HOME/Library/Application Support/iTerm2"
    [ -d "$support" ] || return 0
    if ! has plutil || ! has base64; then
        say "iTerm2: plutil and base64 are needed for imece's tab icon; left as it was"
        return 0
    fi
    icon="$support/graphic_imece.png"
    if [ -d "$icon" ] && [ ! -L "$icon" ]; then
        say "iTerm2: $icon is a directory; imece's tab icon left out"
        return 0
    fi
    # Next to it, then moved over it (over a link too, never through one).
    printf '%s' "$ICON_PNG" | base64 -D >"$support/.graphic_imece.png.new" 2>/dev/null ||
        { rm -f "$support/.graphic_imece.png.new"; say "iTerm2: could not write $icon"; return 0; }
    chmod 644 "$support/.graphic_imece.png.new"
    mv -f "$support/.graphic_imece.png.new" "$icon"
    left=""
    iterm_table "$support/graphic_icons.json" -json '["imece"]' || left="$left graphic_icons.json"
    iterm_table "$support/graphic_colors.json" -string '#E3A93F' || left="$left graphic_colors.json"
    if [ -z "$left" ]; then
        say "iTerm2: imece's tab shows its mark from iTerm2's next start"
    else
        say "iTerm2: left$left in $support as it was (not a JSON object, or a link)"
    fi
}

# Adds imece to the iTerm2 table $1 with the value given as $2 $3 (plutil's
# words) when it has no imece; true when imece is in it now, false when the
# table is not one to write.
iterm_table() {
    if [ -L "$1" ]; then
        return 1
    fi
    new="$1.imece.new"
    if [ -e "$1" ]; then
        [ -f "$1" ] || return 1
        [ "$(tr -d ' \t\r\n' <"$1" | cut -c 1)" = "{" ] || return 1
        if plutil -extract imece raw -o - "$1" >/dev/null 2>&1; then
            return 0
        fi
        cp "$1" "$new"
    else
        printf '{}\n' >"$new"
    fi
    # Through a plist: plutil takes a bare {} for the old OpenStep form,
    # which it cannot write; a null, which a plist cannot hold, stops it.
    if ! { plutil -convert xml1 "$new" && plutil -insert imece "$2" "$3" "$new" &&
        plutil -convert json -r "$new"; } >/dev/null 2>&1; then
        rm -f "$new"
        return 1
    fi
    chmod 644 "$new"
    mv -f "$new" "$1"
}

# imece's mark as an icon of the person's icon theme, `imece` in hicolor,
# the theme every other falls back to: imece names it to Konsole for its
# tab (OSC 32) while it runs. The same bytes as
# website/static/img/imece-mark.svg (website/test-install.sh checks).
theme_icon() {
    apps="${XDG_DATA_HOME:-$HOME/.local/share}/icons/hicolor/scalable/apps"
    if [ -d "$apps/imece.svg" ] && [ ! -L "$apps/imece.svg" ]; then
        say "$apps/imece.svg is a directory; imece's tab icon left out"
        return 0
    fi
    mkdir -p "$apps" 2>/dev/null || { say "could not make $apps; imece's tab icon left out"; return 0; }
    cat >"$apps/.imece.svg.new" <<'SVG' || { rm -f "$apps/.imece.svg.new"; return 0; }
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" width="64" height="64" role="img" aria-label="imece">
  <!-- Five hands round one job: a ring of five dots on a dark ground. -->
  <rect width="64" height="64" rx="14" fill="#15161b"/>
  <g fill="#e3a93f">
    <circle cx="32" cy="14" r="6"/>
    <circle cx="49.1" cy="26.4" r="6"/>
    <circle cx="42.6" cy="46.6" r="6"/>
    <circle cx="21.4" cy="46.6" r="6"/>
    <circle cx="14.9" cy="26.4" r="6"/>
  </g>
  <circle cx="32" cy="33" r="5" fill="#f4f4f6"/>
</svg>
SVG
    chmod 644 "$apps/.imece.svg.new"
    mv -f "$apps/.imece.svg.new" "$apps/imece.svg"
    say "icon: $apps/imece.svg (Konsole shows it on imece's tab)"
}

# The tab's icon where the terminal shows one per program.
tab_icon() {
    [ "${IMECE_NO_ICON:-}" = 1 ] && return 0
    case "$(uname -s)" in
        Darwin) iterm_icon ;;
        Linux) theme_icon ;;
    esac
}

main() {
    for tool in uname tar mkdir chmod cp mv rm grep cut basename mktemp; do
        has "$tool" || die "$tool is needed"
    done
    target=$(target)
    archive="imece-$target.tar.gz"
    if [ -n "${IMECE_DOWNLOAD_URL:-}" ]; then
        base="${IMECE_DOWNLOAD_URL%/}"
        # A directory's absolute path is a file:// address to curl; a
        # relative one is refused, as anything without a scheme is.
        case "$base" in
            https://* | file://*) ;;
            /*) base="file://$base" ;;
            *) die "IMECE_DOWNLOAD_URL must be an https:// or file:// address or an absolute directory: $IMECE_DOWNLOAD_URL" ;;
        esac
    elif [ -n "${IMECE_VERSION:-}" ]; then
        base="$REPO/releases/download/v${IMECE_VERSION#v}"
    else
        base="$REPO/releases/latest/download"
    fi
    dir="${IMECE_INSTALL_DIR:-$HOME/.local/bin}"

    tmp=$(mktemp -d 2>/dev/null || mktemp -d -t imece)
    trap 'rm -rf "$tmp"' EXIT
    trap 'exit 1' INT TERM HUP

    say "downloading $archive (${IMECE_VERSION:-latest})"
    download "$base/$archive" "$tmp/$archive"
    download "$base/SHA256SUMS" "$tmp/SHA256SUMS"
    want=$(grep "  $archive\$" "$tmp/SHA256SUMS" | cut -d ' ' -f 1 || true)
    [ -n "$want" ] || die "SHA256SUMS lists no $archive"
    got=$(sha256 "$tmp/$archive")
    [ "$got" = "$want" ] ||
        die "$archive does not match its SHA256SUMS (got $got, want $want); nothing installed"

    tar -xzf "$tmp/$archive" -C "$tmp" imece ||
        die "$archive holds no imece"
    mkdir -p "$dir"
    # Next to the old one, then moved over it: a running imece keeps its
    # file, and no half-written one is ever on PATH.
    cp "$tmp/imece" "$dir/.imece.new"
    chmod 755 "$dir/.imece.new"
    mv -f "$dir/.imece.new" "$dir/imece"
    version=$("$dir/imece" --version) || die "the installed $dir/imece does not run"
    say "installed $version at $dir/imece"

    case ":$PATH:" in
        *":$dir:"*)
            first=$(command -v imece || true)
            if [ -n "$first" ] && [ "$first" != "$dir/imece" ]; then
                say "note: $first comes first on PATH; remove it or put $dir before it"
            fi
            ;;
        *)
            if [ "${IMECE_NO_MODIFY_PATH:-}" = 1 ]; then
                say "$dir is not on PATH; add it to use imece by name"
            else
                spec=$(path_line "$dir")
                rc=${spec%%|*}
                line=${spec#*|}
                if ! grep -qsF "$line" "$rc"; then
                    mkdir -p "${rc%/*}"
                    printf '\n# imece\n%s\n' "$line" >>"$rc"
                    say "added $dir to PATH in $rc"
                fi
                say "open a new terminal, or run: $line"
            fi
            ;;
    esac

    tab_icon

    if ! has claude; then
        say "imece drives Claude Code: install it and log in"
        say "  (https://claude.com/claude-code)"
    fi
    say "start it in a git checkout: cd your-project && imece   (or imece --demo)"
}

main "$@"
