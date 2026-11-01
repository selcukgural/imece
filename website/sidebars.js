// Copied from kule@c09ec13545c331fd56d74ab76cb5f8557594a16c website/sidebars.js
// and reworked for imece (see VENDORED.md).
//
// The order of the docs. Every page in ../docs/guide is listed here by its
// id (its path without .md), so a page left out shows up as a broken build.
// @ts-check

/** @type {import('@docusaurus/plugin-content-docs').SidebarsConfig} */
const sidebars = {
  docs: [
    'index',
    'videos',
    {
      type: 'category',
      label: 'Getting started',
      collapsed: false,
      items: ['getting-started/install', 'getting-started/first-run', 'getting-started/demo'],
    },
    {
      type: 'category',
      label: 'Concepts',
      items: [
        'concepts/how-imece-works',
        'concepts/team',
        'concepts/messages-and-board',
        'concepts/changes',
        'concepts/approvals-and-questions',
        'concepts/vendors',
        'concepts/budget-and-limits',
        'concepts/projects',
        'concepts/sessions',
        'concepts/security',
      ],
    },
    {
      type: 'category',
      label: 'The screen',
      items: [
        'screen/salon',
        'screen/waiting',
        'screen/conversations',
        'screen/cards',
        'screen/board-and-code-map',
        'screen/changes',
        'screen/command-line',
        'screen/journal',
        'screen/themes-and-language',
        'screen/keyboard',
      ],
    },
    {
      type: 'category',
      label: 'Scenarios',
      link: { type: 'doc', id: 'scenarios/index' },
      items: [
        'scenarios/one-fix',
        'scenarios/mixed-vendors',
        'scenarios/review-with-notes',
        'scenarios/many-projects',
        'scenarios/next-morning',
        'scenarios/own-shell-and-commit',
      ],
    },
    {
      type: 'category',
      label: 'Reference',
      items: ['reference/cli', 'reference/commands', 'reference/message-types', 'reference/files'],
    },
    'troubleshooting',
  ],
};

export default sidebars;
