//! Built-in colour schemes: base16 palettes, 16 slots with fixed roles
//! (base00 background … base05 text … base08 red, base09 orange, base0A
//! yellow, base0B green, base0C cyan, base0D blue, base0E purple, base0F
//! brown), which `theme::scheme` turns into imece's roles.
//!
//! Copied from https://github.com/tinted-theming/schemes (MIT, Copyright
//! (c) 2022 Tinted Theming) at dd2a1379533372ad79560bb4392570782a8a0a9d,
//! branch spec-0.11, directory base16/, on 2026-10-29; the values only.
//! To add one: its 16 colours, in order, from `base16/<slug>.yaml`.

pub struct Scheme {
    /// The name on the command line and in `:theme`.
    pub slug: &'static str,
    /// Whether base00 is dark (a check on the values; the roles do not
    /// care).
    #[cfg_attr(not(test), allow(dead_code))]
    pub dark: bool,
    /// base00 … base0F as 0xRRGGBB.
    pub base: [u32; 16],
}

pub const SCHEMES: [Scheme; 15] = [
    Scheme {
        slug: "dracula",
        dark: true,
        base: [
            0x282a36, 0x21222c, 0x44475a, 0x6272a4, 0x9ea8c7, 0xf8f8f2, 0xf8f8f2, 0xffffff,
            0xff5555, 0xffb86c, 0xf1fa8c, 0x50fa7b, 0x8be9fd, 0xbd93f9, 0xff79c6, 0x993333,
        ],
    },
    Scheme {
        slug: "nord",
        dark: true,
        base: [
            0x2e3440, 0x3b4252, 0x434c5e, 0x4c566a, 0xd8dee9, 0xe5e9f0, 0xeceff4, 0x8fbcbb,
            0xbf616a, 0xd08770, 0xebcb8b, 0xa3be8c, 0x88c0d0, 0x81a1c1, 0xb48ead, 0x5e81ac,
        ],
    },
    Scheme {
        slug: "gruvbox-dark-medium",
        dark: true,
        base: [
            0x282828, 0x3c3836, 0x504945, 0x665c54, 0xbdae93, 0xd5c4a1, 0xebdbb2, 0xfbf1c7,
            0xfb4934, 0xfe8019, 0xfabd2f, 0xb8bb26, 0x8ec07c, 0x83a598, 0xd3869b, 0xd65d0e,
        ],
    },
    Scheme {
        slug: "gruvbox-light-medium",
        dark: false,
        base: [
            0xfbf1c7, 0xebdbb2, 0xd5c4a1, 0xbdae93, 0x665c54, 0x504945, 0x3c3836, 0x282828,
            0x9d0006, 0xaf3a03, 0xb57614, 0x79740e, 0x427b58, 0x076678, 0x8f3f71, 0xd65d0e,
        ],
    },
    Scheme {
        slug: "catppuccin-mocha",
        dark: true,
        base: [
            0x1e1e2e, 0x181825, 0x313244, 0x45475a, 0x585b70, 0xcdd6f4, 0xf5e0dc, 0xb4befe,
            0xf38ba8, 0xfab387, 0xf9e2af, 0xa6e3a1, 0x94e2d5, 0x89b4fa, 0xcba6f7, 0xf2cdcd,
        ],
    },
    Scheme {
        slug: "catppuccin-latte",
        dark: false,
        base: [
            0xeff1f5, 0xe6e9ef, 0xccd0da, 0xbcc0cc, 0xacb0be, 0x4c4f69, 0xdc8a78, 0x7287fd,
            0xd20f39, 0xfe640b, 0xdf8e1d, 0x40a02b, 0x179299, 0x1e66f5, 0x8839ef, 0xdd7878,
        ],
    },
    Scheme {
        slug: "tokyo-night-dark",
        dark: true,
        base: [
            0x1a1b26, 0x16161e, 0x2f3549, 0x444b6a, 0x787c99, 0xa9b1d6, 0xcbccd1, 0xd5d6db,
            0xc0caf5, 0xa9b1d6, 0x0db9d7, 0x9ece6a, 0xb4f9f8, 0x2ac3de, 0xbb9af7, 0xf7768e,
        ],
    },
    Scheme {
        slug: "solarized-dark",
        dark: true,
        base: [
            0x002b36, 0x073642, 0x586e75, 0x657b83, 0x839496, 0x93a1a1, 0xeee8d5, 0xfdf6e3,
            0xdc322f, 0xcb4b16, 0xb58900, 0x859900, 0x2aa198, 0x268bd2, 0x6c71c4, 0xd33682,
        ],
    },
    Scheme {
        slug: "solarized-light",
        dark: false,
        base: [
            0xfdf6e3, 0xeee8d5, 0x93a1a1, 0x839496, 0x657b83, 0x586e75, 0x073642, 0x002b36,
            0xdc322f, 0xcb4b16, 0xb58900, 0x859900, 0x2aa198, 0x268bd2, 0x6c71c4, 0xd33682,
        ],
    },
    Scheme {
        slug: "onedark",
        dark: true,
        base: [
            0x282c34, 0x353b45, 0x3e4451, 0x545862, 0x565c64, 0xabb2bf, 0xb6bdca, 0xc8ccd4,
            0xe06c75, 0xd19a66, 0xe5c07b, 0x98c379, 0x56b6c2, 0x61afef, 0xc678dd, 0xbe5046,
        ],
    },
    Scheme {
        slug: "rose-pine",
        dark: true,
        base: [
            0x191724, 0x1f1d2e, 0x26233a, 0x6e6a86, 0x908caa, 0xe0def4, 0xe0def4, 0x524f67,
            0xeb6f92, 0xf6c177, 0xebbcba, 0x31748f, 0x9ccfd8, 0xc4a7e7, 0xf6c177, 0x524f67,
        ],
    },
    Scheme {
        slug: "everforest",
        dark: true,
        base: [
            0x2d353b, 0x343f44, 0x475258, 0x859289, 0x9da9a0, 0xd3c6aa, 0xe6e2cc, 0xfdf6e3,
            0xe67e80, 0xe69875, 0xdbbc7f, 0xa7c080, 0x83c092, 0x7fbbb3, 0xd699b6, 0x9da9a0,
        ],
    },
    Scheme {
        slug: "kanagawa",
        dark: true,
        base: [
            0x1f1f28, 0x16161d, 0x223249, 0x54546d, 0x727169, 0xdcd7ba, 0xc8c093, 0x717c7c,
            0xc34043, 0xffa066, 0xc0a36e, 0x76946a, 0x6a9589, 0x7e9cd8, 0x957fb8, 0xd27e99,
        ],
    },
    Scheme {
        slug: "github",
        dark: false,
        base: [
            0xffffff, 0xf6f8fa, 0xd1d9e0, 0x818b98, 0x59636e, 0x454c54, 0x25292e, 0x1f2328,
            0xcf222e, 0x953800, 0x9a6700, 0x116329, 0x1b7c83, 0x0969da, 0x8250df, 0xa40e26,
        ],
    },
    Scheme {
        slug: "monokai",
        dark: true,
        base: [
            0x272822, 0x383830, 0x49483e, 0x75715e, 0xa59f85, 0xf8f8f2, 0xf5f4f1, 0xf9f8f5,
            0xf92672, 0xfd971f, 0xf4bf75, 0xa6e22e, 0xa1efe4, 0x66d9ef, 0xae81ff, 0xcc6633,
        ],
    },
];
