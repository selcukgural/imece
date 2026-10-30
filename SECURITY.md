# Security policy

imece is an early prototype. It drives Claude Code, Codex and Gemini CLI
sessions on your own machine with your own accounts, switching on each
vendor's own tool restrictions and checking what the session reports it
runs with; it is not a sandbox, and an agent with a shell can do what your
account can. Security fixes go to the latest commit on `main`; there are
no releases or maintained branches yet.

## Reporting a vulnerability

Please do not open a public issue, pull request or discussion for a
security problem.

Report it privately through GitHub instead:
[Report a vulnerability](https://github.com/selcukgural/imece/security/advisories/new)
(the repository's Security tab → "Report a vulnerability").

Include what you can of:

- the affected crate and commit,
- what an attacker can do and under which conditions (for example a
  compromised agent, a malicious repository, or a local user),
- steps or a test that reproduces it.

You should get an answer within a week. Once a fix is ready, it is
published together with a GitHub security advisory that credits you,
unless you would rather stay anonymous.

## Scope

Of particular interest are agent text that reaches the terminal without
being cleaned (escape sequences, spoofed screen content), one agent
impersonating another or the person on the message bus, a session running
with more than imece asked for without imece noticing (a tool, an MCP
server, a hook or a setting the fail-closed checks miss), a worker's change
reaching the checkout past the person's review, and secrets that end up on
screen, in logs or in a model's context.
