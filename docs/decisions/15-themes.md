# Themes v1

Themes v1 (2026-10-29, user's ask: the first themes changed too little,
and a terminal's own theme should count; no icons or images, no cost
per draw): `theme.rs` builds one `Theme` (roles, now with `bg`, `fg`
and `border`) from three sources, once when chosen. `terminal` (the
default) names the 16 ANSI colours, so the terminal's theme (iTerm2,
Ghostty, Kitty, …) colours imece and a change there shows at once;
`probe.rs` asks the terminal for its background and foreground (OSC
11 / OSC 10, 200 ms, before the screen starts, `COLORFGBG` as a hint)
to say dark or light and to shade the selection and key hints;
`dark` / `light` assume instead of asking; `mono` is unchanged.
Built-in schemes (`schemes.rs`: 15 base16 palettes copied from
tinted-theming/schemes, MIT, commit noted in the file header) map the
base16 slots to roles (base09 orange = accent, as imece's own colour;
base0F left out) and paint the whole screen (`ui::paint`, also under
every window after `Clear`), so they look the same everywhere. A
colour file in Ghostty's (`palette = 4=#rrggbb`) or Kitty's (`color4
#rrggbb`) format, from `--theme FILE` or `:theme FILE` (64 KiB cap,
the stem is the name, remembered by absolute path): the slots give
the roles (blue = accent), background and foreground the rest.
Without `COLORTERM=truecolor|24bit` RGB is rounded to the 256-colour
cube. `:theme NAME|FILE` (`:t`) with completion of the built-in
names, `T` cycles them. Preferences, flags and `IMECE_THEME` take
names or paths. Tests: every scheme's roles stand off its background
by luma; speakers that repeat (Rose Pine's orange is its purple) fade
apart. The probe is followed by a cursor position report (`CSI 6 n`)
and reads until its answer (deadline 1 s), so a late colour answer
never leaks into the key stream (2026-10-29: Terminal.app answered
after the first probe's 200 ms, and the `rgb:…` opened the code map
and the composer at start). Known gaps: no Alacritty (TOML) or
`.itermcolors` (plist) reader, convert with mbadolato's tools.
