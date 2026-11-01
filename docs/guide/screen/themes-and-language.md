---
title: Themes and language
description: The screen in English or Turkish, and its colours - the terminal's own, dark, light, mono, fifteen base16 schemes, or a colour file in Ghostty's or Kitty's format.
---

# Themes and language

The screen speaks English and Turkish and wears your terminal's own colours unless you choose otherwise. Both are a key away: `L` switches the language, `T` steps through the themes.

## Language

| How | What |
| --- | --- |
| `L`, `:lang` | the other language, at once, for this run |
| `:lang en`, `:lang tr` | the one named |
| `--lang en\|tr` | at start |
| `IMECE_LANG=tr` | the default when the flag is not given; anything but `en` or `tr` is refused at start |

English is the default. The language is not saved between runs: set `IMECE_LANG` or `--lang` for a lasting choice.

Every word on the screen lives in one place in both languages, with the same placeholders, so nothing is English-only: the help, the legend, the pane titles (WAITING FOR YOU is SENİ BEKLEYEN, JOURNAL is DEFTER, SALON stays SALON), the journal's lines, the flashes, the terminal's title (`imece · 2 seni bekliyor`), the exported Markdown's headings. The model stores events as data and words them at draw time, so `L` rewrites the whole journal, not only new lines. Keys and `:` commands are the same in both languages. The `?` help's `/` filter folds Turkish letters, so `calis` finds `çalış` from a keyboard without them.

What the agents say is in whatever language they write; imece never translates it. The raw stream (the trace) stays as the vendor sends it.

## Themes

| How | What |
| --- | --- |
| `T`, `:theme` | the next built-in theme, round to the first; the flash says `theme: nord` |
| `:theme NAME` (`:t NAME`) | the one named; the screen wears it while you type, `enter` keeps it, `esc` gives it up |
| `:theme FILE` | a colour file |
| `--theme NAME\|FILE` | at start |
| `IMECE_THEME` | the default when the flag is not given; an unknown name is refused at start |

The last choice (`T`, `:theme`) is remembered in the store for the next run, as a name or the file's absolute path; a preview is never remembered.

### Built-in

| Name | What it is |
| --- | --- |
| `terminal` | the default: the 16 ANSI colours, so your terminal's own theme (iTerm2, Ghostty, Kitty, …) colours imece and a change there shows at once. imece asks the terminal for its background and foreground to tell dark from light and to shade the selection and key hints |
| `dark`, `light` | the ANSI colours, assuming a dark or a light background instead of asking |
| `mono` | no colour: shapes and weight only. The glyphs are chosen so it still reads (`●` `✓` `✕` differ by shape) |

Then fifteen base16 schemes, copied from tinted-theming/schemes (MIT), in the order `T` cycles them: `dracula`, `nord`, `gruvbox-dark-medium`, `gruvbox-light-medium`, `catppuccin-mocha`, `catppuccin-latte`, `tokyo-night-dark`, `solarized-dark`, `solarized-light`, `onedark`, `rose-pine`, `everforest`, `kanagawa`, `github`, `monokai`. A scheme paints the whole screen, background included, so it looks the same in every terminal; base16's orange is imece's accent.

`:theme` completes the built-in names; `imece --help` names the first few.

### Colour files

`:theme FILE` or `--theme FILE` reads a colour file in Ghostty's format (`palette = 4=#rrggbb`, `background = `, `foreground = `) or Kitty's (`color4 #rrggbb`, `background #rrggbb`), 64 KiB at most. The file's stem is the theme's name, remembered by absolute path. All 16 slots, the background and the foreground must be there (`selection-background` may be); other keys are ignored, so a whole Ghostty config passes when it holds the colours. The slots give the roles (blue is the accent), background and foreground the rest. A name with a `/` in it, or one that is a file, is read as a file; anything else must be a built-in name. Not read: Alacritty's TOML and `.itermcolors`; convert them first (mbadolato's iTerm2-Color-Schemes has every format).

Without `COLORTERM=truecolor` or `24bit`, RGB colours are rounded to the 256-colour cube.

### Following the terminal

The `terminal` theme follows the terminal turning light or dark. Terminals that report a change of their own cannot be heard through imece's terminal library, so imece turns on focus reports and asks the terminal for its colours again whenever its window comes back into focus; the theme is built again only when they changed. A change while the window keeps the focus shows at the next focus; tmux passes focus only with `focus-events on`; a terminal that does not answer keeps what was known. A key pressed in the moment of the asking may be read with the answer.

At start the probe waits 200 ms for the colours and then for a cursor position report (up to a second), so a late answer never leaks into the key stream; `COLORFGBG` is taken as a hint.

### The dimmed backdrop

Whatever the theme, a window over the salon (a conversation, the help, the digest, where you left off, the quit list) dims everything around it, so the window reads first. Attention is one colour: what waits for you, a pot about to run out, a vendor window from 90% used.

## See also

- [The command line](command-line.md)
- [CLI reference](../reference/cli.md)
- [Commands reference](../reference/commands.md)
- [Keyboard](keyboard.md)
