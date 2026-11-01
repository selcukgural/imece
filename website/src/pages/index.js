// Copied from kule@c09ec13545c331fd56d74ab76cb5f8557594a16c website/src/pages/index.js
// and reworked for imece (see VENDORED.md).
import React from 'react';
import Layout from '@theme/Layout';
import Link from '@docusaurus/Link';
import CodeBlock from '@theme/CodeBlock';
import useBaseUrl from '@docusaurus/useBaseUrl';
import Translate, { translate } from '@docusaurus/Translate';

const loop = [
  translate({ id: 'home.loop.ask', message: 'tell the leader' }),
  translate({ id: 'home.loop.hire', message: 'it hires workers' }),
  translate({ id: 'home.loop.work', message: 'they work in worktrees' }),
  translate({ id: 'home.loop.review', message: 'review each change' }),
  translate({ id: 'home.loop.apply', message: 'apply or send it back' }),
];

const features = [
  {
    title: translate({ id: 'home.f.team.title', message: 'A team, not one agent' }),
    text: translate({
      id: 'home.f.team.text',
      message:
        'A leader reads your checkout and hires workers as the work needs them, each on its own card. Workers hire helpers for parts of a task. You talk to the leader, not to thirty agents.',
    }),
  },
  {
    title: translate({ id: 'home.f.vendors.title', message: 'Many vendors on one screen' }),
    text: translate({
      id: 'home.f.vendors.text',
      message:
        'Claude Code, Codex and Gemini CLI run as long-lived sessions; the screen never sees a wire format. Codex may lead while Claude and Gemini work, in cheap, standard or strong tiers.',
    }),
  },
  {
    title: translate({ id: 'home.f.gate.title', message: 'Your checkout is never touched' }),
    text: translate({
      id: 'home.f.gate.text',
      message:
        'Every worker changes its own git worktree. Its done shows up as a change (Δ) in WAITING FOR YOU: read the diff, note lines, apply all or only the hunks you choose, or send it back with your reasons.',
    }),
  },
  {
    title: translate({ id: 'home.f.bus.title', message: 'Agents talk to each other' }),
    text: translate({
      id: 'home.f.bus.text',
      message:
        'A message bus with typed envelopes: assign, ask, answer, done, blocked. A board where idle workers claim cards. Questions and shell commands that need you arrive in one queue.',
    }),
  },
  {
    title: translate({ id: 'home.f.pot.title', message: 'One pot, one ceiling' }),
    text: translate({
      id: 'home.f.pot.text',
      message:
        'One budget for the whole team, kept between runs; each session capped at what is left. The header shows the burn rate, how long the pot lasts, and each vendor plan’s tightest usage window.',
    }),
  },
  {
    title: translate({ id: 'home.f.remember.title', message: 'It remembers, you come back cheap' }),
    text: translate({
      id: 'home.f.remember.text',
      message:
        'Projects, cards, conversations and the journal live in one SQLite file. Agents sleep until you write to them, and wake in the session they had. Back from another window, the away digest shows at once.',
    }),
  },
];

const installLine = 'curl -fsSL https://selcukgural.github.io/imece/install.sh | sh';

// Copies `text` the old way, for a browser without the Clipboard API (or
// a page not served over HTTPS): a selected, hidden text area.
function copyByHand(text) {
  const area = document.createElement('textarea');
  area.value = text;
  area.setAttribute('readonly', '');
  area.style.position = 'fixed';
  area.style.opacity = '0';
  document.body.appendChild(area);
  area.select();
  let copied = false;
  try {
    copied = document.execCommand('copy');
  } catch (e) {
    copied = false;
  }
  document.body.removeChild(area);
  return copied;
}

// The install line with a copy button, as install pages have it: the
// command in full, selectable, and a button that copies it and says so
// for two seconds.
function InstallLine() {
  const [copied, setCopied] = React.useState(false);
  const done = () => {
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };
  const copy = () => {
    if (navigator.clipboard && window.isSecureContext) {
      navigator.clipboard.writeText(installLine).then(done, () => {
        if (copyByHand(installLine)) done();
      });
    } else if (copyByHand(installLine)) {
      done();
    }
  };
  return (
    <div className="imece-install">
      <span className="imece-install-label">
        <Translate id="home.install.label">macOS and Linux</Translate>
      </span>
      <div className="imece-install-line">
        <code>
          <span className="imece-install-prompt" aria-hidden="true">
            ${' '}
          </span>
          {installLine}
        </code>
        <button
          type="button"
          className="imece-install-copy"
          onClick={copy}
          aria-label={translate({ id: 'home.install.copyLabel', message: 'Copy the install command' })}>
          {copied ? (
            <Translate id="home.install.copied">Copied</Translate>
          ) : (
            <Translate id="home.install.copy">Copy</Translate>
          )}
        </button>
      </div>
    </div>
  );
}

const session = `cd your/checkout
imece                                  # a leader reads the checkout and waits for you
imece --agents 8 --budget 10           # up to 8 workers, a $10 pot
imece --lead codex --confine strict    # Codex leads; file tools only, commands through run
imece --demo --projects 3 --agents 30  # fake agents, to see the screen at work`;

export default function Home() {
  return (
    <Layout
      title={translate({ id: 'home.title', message: 'One terminal for many coding agents' })}
      description={translate({
        id: 'home.description',
        message:
          'imece runs a team of coding agents from many vendors on one codebase, on one screen: a leader hires workers, they work in worktrees, and you review each change before it reaches your checkout.',
      })}>
      <header className="imece-hero">
        <h1>imece</h1>
        <p>
          <Translate id="home.hero">
            One terminal for many coding agents from many vendors. A leader splits your request into
            cards and hires workers; they talk to each other and work in their own worktrees; every
            change waits for you before it reaches your checkout.
          </Translate>
        </p>
        <InstallLine />
        <Link className="button button--primary button--lg" to="/docs/getting-started/install">
          <Translate id="home.cta.install">Install</Translate>
        </Link>
        <Link className="button button--secondary button--lg" to="/docs/getting-started/first-run">
          <Translate id="home.cta.firstrun">First run</Translate>
        </Link>
        <Link className="button button--secondary button--lg" to="/docs/videos">
          <Translate id="home.cta.video">Watch the tours</Translate>
        </Link>
        <div className="imece-loop" aria-label="The loop">
          {loop.map((step, i) => (
            <React.Fragment key={step}>
              {i > 0 && <span className="arrow">→</span>}
              <span>{step}</span>
            </React.Fragment>
          ))}
        </div>
      </header>
      <main>
        <section className="imece-video">
          <video
            controls
            preload="metadata"
            playsInline
            poster={useBaseUrl('/video/imece-tour.jpg')}>
            <source src={useBaseUrl('/video/imece-tour.mp4')} type="video/mp4" />
            <track
              kind="captions"
              srcLang="en"
              label="English"
              src={useBaseUrl('/video/imece-tour.vtt')}
              default
            />
          </video>
        </section>
        <section className="imece-features container">
          <div className="row">
            {features.map((f) => (
              <div key={f.title} className="col col--4">
                <h3>{f.title}</h3>
                <p>{f.text}</p>
              </div>
            ))}
          </div>
        </section>
        <section className="imece-terminal">
          <h2>
            <Translate id="home.terminal.title">Start it where your code is</Translate>
          </h2>
          <CodeBlock language="sh">{session}</CodeBlock>
          <p>
            <Translate id="home.terminal.more">Every flag is explained in the</Translate>{' '}
            <Link to="/docs/reference/cli">
              <Translate id="home.terminal.link">command line reference</Translate>
            </Link>
            .
          </p>
        </section>
      </main>
    </Layout>
  );
}
