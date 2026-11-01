// Copied from kule@c09ec13545c331fd56d74ab76cb5f8557594a16c website/docusaurus.config.js
// and reworked for imece (see VENDORED.md).
//
// imece's documentation site. The pages are the Markdown files in
// ../docs/guide (English) and i18n/tr/… (Turkish), so they read on GitHub
// as they are; this folder holds only the site.
// @ts-check

import { themes as prismThemes } from 'prism-react-renderer';

// Docusaurus builds each locale on its own and names it here, so the few
// strings the theme cannot translate from i18n/ follow it.
const tr = process.env.DOCUSAURUS_CURRENT_LOCALE === 'tr';

/** @type {import('@docusaurus/types').Config} */
const config = {
  title: 'imece',
  tagline: tr
    ? 'Birçok satıcıdan birçok kodlama ajanı için tek terminal'
    : 'One terminal for many coding agents from many vendors',
  favicon: 'img/favicon.svg',

  url: 'https://selcukgural.github.io',
  baseUrl: '/imece/',
  organizationName: 'selcukgural',
  projectName: 'imece',
  trailingSlash: false,

  onBrokenLinks: 'throw',
  onBrokenAnchors: 'throw',
  markdown: {
    format: 'detect',
    hooks: { onBrokenMarkdownLinks: 'throw' },
  },

  i18n: {
    defaultLocale: 'en',
    locales: ['en', 'tr'],
    localeConfigs: {
      en: { label: 'English', htmlLang: 'en' },
      tr: { label: 'Türkçe', htmlLang: 'tr' },
    },
  },

  presets: [
    [
      'classic',
      /** @type {import('@docusaurus/preset-classic').Options} */
      ({
        docs: {
          path: '../docs/guide',
          routeBasePath: 'docs',
          sidebarPath: './sidebars.js',
          editUrl: 'https://github.com/selcukgural/imece/edit/main/website/',
          showLastUpdateTime: false,
        },
        blog: false,
        theme: { customCss: './src/css/custom.css' },
      }),
    ],
  ],

  themeConfig:
    /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
    ({
      image: 'img/social-preview.png',
      colorMode: { respectPrefersColorScheme: true },
      announcementBar: {
        // A new id shows the bar again to those who closed the old one.
        id: 'early-install',
        content: tr
          ? 'imece <b>erken</b> bir sürümdür: Claude Code, Codex ve Gemini CLI ile çalışır; macOS ve Linux için tek satırla kurulur.'
          : 'imece is <b>early</b>: it runs Claude Code, Codex and Gemini CLI; one line installs it on macOS and Linux.',
        isCloseable: true,
      },
      navbar: {
        title: 'imece',
        logo: { alt: 'imece', src: 'img/imece-mark.svg' },
        items: [
          { type: 'docSidebar', sidebarId: 'docs', position: 'left', label: 'Docs' },
          { to: '/docs/scenarios', label: 'Scenarios', position: 'left' },
          { to: '/docs/screen/keyboard', label: 'Keys', position: 'left' },
          { to: '/docs/videos', label: 'Videos', position: 'left' },
          { type: 'localeDropdown', position: 'right' },
          { href: 'https://github.com/selcukgural/imece', label: 'GitHub', position: 'right' },
        ],
      },
      footer: {
        style: 'dark',
        links: [
          {
            title: 'Start',
            items: [
              { label: 'Install', to: '/docs/getting-started/install' },
              { label: 'First run', to: '/docs/getting-started/first-run' },
              { label: 'The simulation', to: '/docs/getting-started/demo' },
            ],
          },
          {
            title: 'Reference',
            items: [
              { label: 'Command line flags', to: '/docs/reference/cli' },
              { label: ': commands', to: '/docs/reference/commands' },
              { label: 'Keys', to: '/docs/screen/keyboard' },
            ],
          },
          {
            title: 'Project',
            items: [
              { label: 'GitHub', href: 'https://github.com/selcukgural/imece' },
              { label: 'Decision records', href: 'https://github.com/selcukgural/imece/tree/main/docs/decisions' },
              { label: 'Report a vulnerability', href: 'https://github.com/selcukgural/imece/security/policy' },
            ],
          },
        ],
        copyright: tr
          ? 'imece, Apache-2.0 lisansıyla açık kaynaktır.'
          : 'imece is open source under the Apache-2.0 license.',
      },
      prism: {
        theme: prismThemes.github,
        darkTheme: prismThemes.dracula,
        additionalLanguages: ['bash', 'toml', 'json', 'rust', 'diff'],
      },
    }),
};

export default config;
