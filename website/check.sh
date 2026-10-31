#!/usr/bin/env bash
# The site's own checks, run by .github/workflows/docs.yml before the build:
#
#  1. every English page in docs/guide has its Turkish copy under
#     website/i18n/tr/…/current, and every Turkish copy has its English page;
#  2. every `:` command the screen completes (complete::COMMANDS) is named
#     in the commands reference, in both languages;
#  3. every key the `?` help lists (i18n::EN.help_keys) is named in the
#     keyboard page, in both languages.
#
# So a feature added to the screen without its page fails the build, and a
# page written in one language only does too.
set -euo pipefail
cd "$(dirname "$0")/.."

en=docs/guide
tr=website/i18n/tr/docusaurus-plugin-content-docs/current
fail=0

say() { printf '%s\n' "$*" >&2; }

# 1. Both languages, page for page.
while IFS= read -r f; do
  rel=${f#"$en"/}
  [ -f "$tr/$rel" ] || { say "no Turkish copy: $tr/$rel"; fail=1; }
done < <(find "$en" -type f \( -name '*.md' -o -name '*.mdx' \) | sort)
while IFS= read -r f; do
  rel=${f#"$tr"/}
  [ -f "$en/$rel" ] || { say "no English page: $en/$rel"; fail=1; }
done < <(find "$tr" -type f \( -name '*.md' -o -name '*.mdx' \) | sort)

# 2. Every `:` command, in both commands pages.
commands=$(perl -0ne 'print "$1\n" if /pub const COMMANDS: \[&str; \d+\] = \[(.*?)\];/s' \
  crates/imece-tui/src/complete.rs | grep -o '"[a-z]*"' | tr -d '"')
[ -n "$commands" ] || { say "no COMMANDS found in complete.rs"; exit 1; }
for page in "$en/reference/commands.md" "$tr/reference/commands.md"; do
  [ -f "$page" ] || { say "missing: $page"; fail=1; continue; }
  for c in $commands; do
    grep -q "\`:$c" "$page" || { say "$page: \`:$c\` is not named"; fail=1; }
  done
done

# 3. Every key the help lists, in both keyboard pages. The first string of
#    each (key, text) pair of EN.help_keys, split into its tokens; a token
#    must appear in backticks somewhere in the page.
keys=$(perl -0ne '
  if (/pub const EN: Text = Text \{.*?help_keys: \[(.*?)\n    \],/s) {
    my $body = $1;
    while ($body =~ /\(\s*"((?:[^"\\]|\\.)*)"\s*,/g) { print "$1\n"; }
  }' crates/imece-tui/src/i18n.rs)
[ -n "$keys" ] || { say "no help_keys found in i18n.rs"; exit 1; }
for page in "$en/screen/keyboard.md" "$tr/screen/keyboard.md"; do
  [ -f "$page" ] || { say "missing: $page"; fail=1; continue; }
  while IFS= read -r line; do
    for token in $line; do
      tok=${token//\\\\/\\}
      grep -qF "\`$tok" "$page" || { say "$page: key \`$tok\` (from \"$line\") is not named"; fail=1; }
    done
  done <<< "$keys"
done

if [ "$fail" -ne 0 ]; then
  say "website/check.sh: the site does not describe the screen; see above"
  exit 1
fi
say "website/check.sh: ok"
