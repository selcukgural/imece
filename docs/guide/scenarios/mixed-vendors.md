---
title: "Codex leads, Claude and Gemini work"
description: "One team from three vendors: who leads, how the leader hires across vendors, what a tier means for each, tokens against dollars, and the header's usage limits."
---

# Codex leads, Claude and Gemini work

imece drives Claude Code, Codex and Gemini CLI, and a team may mix them: the leader comes from one vendor and hires workers from any that is on PATH. You meet this when more than one vendor is installed, or when a subscription makes one of them cheap. This scenario starts a Codex leader that hires a Claude worker and a Gemini worker, and follows what differs from a one-vendor team.

All three CLIs are on PATH and logged in: Codex with ChatGPT, Gemini with an API key.

## 1. Start with a leader named

```sh
imece --lead codex --tier claude:strong=opus --agents 6
```

`--lead codex` names the leader's vendor, so the WHO LEADS? question is skipped. Without the flag imece takes your primary vendor (set with `p` on that screen, or `:lead VENDOR`), else the only vendor on PATH, else it asks: there is no silent default among several.

`--tier claude:strong=opus` says what the `strong` tier means for Claude workers (opus is Claude's `strong` already; the flag is here for its shape, and `opus[1m]` or `sonnet` would change it). The vendor prefix matters: without one, `--tier` and `--model` speak of the leader's vendor, which is Codex here, and `strong=opus` would be read as a Codex model. `--agents 6` lets the leader hire up to six workers.

## 2. What a Codex leader is

Codex has no tool allowlist and always a shell, so imece confines it with Codex's own sandbox: `read-only` for a leader. It reads the checkout, searches the web (on under `standard`), and cannot write; a command that would need more simply fails, because a leader never asks (`approval_policy = "never"`). Its shell has no network in the read-only sandbox, so it can read the web only through web search.

The trace (`enter` twice on its row, or `t`) shows two lines imece cannot put in dollars:

```text
 TRACE · lead · raw stream
 codex runs on your ChatGPT plan: its turns cost the budget nothing
 tokens so far: 1200 in (1000 cached) · 34 out
```

Codex reports tokens, not dollars. On a ChatGPT plan its turns are booked at $0.00 in the pot, and the token totals come after each turn. A Codex logged in with an API key is refused at the start, since imece cannot price it yet: `is logged in with apikey, whose cost imece can not count yet; log in with ChatGPT (codex login)`.

Before the thread starts, imece checks what Codex reports it runs with (the config echo, no plugins, no hooks, exactly imece's MCP server) and stops the session on any mismatch. A Codex leader has no command line of its own: of the `/` commands only `/compact` works.

## 3. Hiring across vendors

Press `/` and ask for the work:

```text
 › to lead: port the rate limiter to the new config format and add tests
```

The `hire` tool takes a `tier` and a `vendor`: `cheap`, `standard` or `strong`, and `claude`, `codex` or `gemini` (the vendors on PATH that imece has an adapter for; the default is the hirer's own). The leader hires a Claude worker on `strong` for the design and a Gemini worker on `cheap` for the tests:

```text
 14:02 ◆ lead hired claude-1 (opus)
 14:02 ◆ lead hired gemini-1 (flash)
 14:02 ⇄ lead → claude-1 [assign]: limiter: config v2 in src/limit.rs
 14:02 ⇄ lead → gemini-1 [assign]: limiter tests in tests/limit.rs
```

Workers are numbered per vendor: `claude-1`, `gemini-1`, and a later Codex worker would be `codex-1`. Each row keeps its vendor's colour.

### What a tier means per vendor

| Tier | Claude | Codex | Gemini |
| --- | --- | --- | --- |
| `cheap` | `haiku` | `@low` (a low effort on its default model) | `flash` |
| `standard` | the vendor's own default | its default effort | `auto` |
| `strong` | `opus` | `@high` | `pro` |

Codex's tiers are reasoning efforts, not models: a tier value is `[MODEL][@EFFORT]`, so `--tier codex:strong=gpt-5.6-terra@xhigh` names both. A tier model your account does not have fails its first turn at no cost; imece restarts that worker on the standard model and says so:

```text
 14:03 ⚑ claude-1: model opus is not available here, using the default
```

## 4. Gemini's price table

Gemini also reports tokens. With an API key imece books each turn from the list prices it carries (prompts up to 200K tokens, thought tokens as output, cached tokens at the cached rate); the table in `gemini.rs` knows the `gemini-2.5` models (`pro`, `flash`, `flash-lite`) and the `gemini-3` line (`3-flash-preview`, `3.1-pro-preview`, `3.1-flash-lite`, `3.5-flash-lite`, `3.6-flash`, `3.8-flash`). A turn named only by an alias (`auto`, `pro`, `flash`) is booked at the dearest model the alias may mean. A model imece cannot price stops the session, so name one it knows with `--model gemini:…` or `--tier gemini:…`. With a Google login the turns cost the pot nothing. The trace says which:

```text
 gemini runs on your API key: its turns are booked at list price
```

Gemini has no `--restricted`, so imece runs it in a private directory of its own with the worktree as an included directory, and a policy file imece writes there allows exactly the tools of the role and the level and denies every other (a denied tool is not even shown to the model). The repository's `.gemini` settings, hooks and `.env` never reach the session.

## 5. A Gemini worker's shell asks

Under `standard` a Gemini writer's `run_shell_command` is `ask_user` in the policy: every command comes to imece. Routine ones that run in the agent's directory run at once (the worker is told to give each command its `dir_path`); any other waits:

```text
 WAITING FOR YOU (1)
 ⚑ 14:09 gemini-1 waits for your yes: shell: npm run lint:fix (in /Users/you/code/api)
   a allow · r refuse · enter focus
```

`a` allows once, `r` refuses: imece answers Gemini's ask with its own cancel option, the call fails and the turn goes on. A Codex worker asks the same way (`on-request`) for what needs more than its `workspace-write` sandbox gives, shown as `shell: …` with Codex's own shell wrapper taken off. The turn's end withdraws an ask still waiting (its item closes, the trace says `no longer asks`). The journal keeps every decision:

```text
 14:09 › you allowed gemini-1: shell: npm run lint:fix (in /Users/you/code/api)
```

## 6. The header's usage limits

A team on subscriptions can be stopped by a plan's five-hour or weekly window long before the pot runs dry. Claude and Codex sessions report their windows; the header shows each vendor's window nearest to full after the pot:

```text
 imece  ● 2 working │ spent $4.20 of $50.00 ($6.10/h, ~7 h 30 min left) │ claude 5h: 48% left, resets in 1 h 59 min │ codex 7d: 81% left  14:20
```

From 90% used a vendor's line turns the attention colour. Gemini reports no windows. The windows are the account's, not an agent's, and are not kept between runs: a session says them again on its first turn. Nothing stops a turn before a window is full; the vendor refuses it, and imece shows that refusal.

Beside the limits, the pot: `$6.10/h` is what the last ten minutes cost, `~7 h 30 min left` how long the pot lasts at that rate. Codex's turns add nothing to it here; Gemini's and Claude's do.

## 7. `--confine strict` as an alternative

Start again with `--confine strict` and every agent gets its vendor's file tools only; commands go through imece's `run` tool (the build, the tests, git's reading commands, your `--run` programs), and nothing asks you anything:

| Vendor | `standard` (default) | `strict` |
| --- | --- | --- |
| Claude | `--restricted`, file tools inside its directory, `Bash`, `WebFetch`, `WebSearch`; a shell command Claude Code would ask about comes to you | `--restricted`, file tools only; what would ask is refused |
| Codex | sandbox `workspace-write` with network, web search live; a worker asks before what the sandbox refuses | sandbox without network, web search off, approval `never`: what the sandbox refuses fails |
| Gemini | policy allows file tools, the web and a shell that asks; everything else denied | policy allows file tools only; no shell, no web; what would ask is refused |

At every level the leader never writes and the vendor's sub-agent tool stays off (a sub-agent spawned inside the CLI would skip the worktree, the budget, the bus and the screen). `--confine open` goes the other way: the vendor runs as you have it set up, your settings, hooks, MCP servers and extensions loaded. The journal says the level at every start that is not strict (`confine standard: …` or `confine open: …`). imece sandboxes nothing: it switches on each vendor's own restrictions and checks, fail closed, that the session runs with them; isolation is a container's job.

## See also

- [Vendors](../concepts/vendors.md): each adapter in detail
- [Budget and limits](../concepts/budget-and-limits.md)
- [Approvals and questions](../concepts/approvals-and-questions.md)
- [Security](../concepts/security.md): what imece guards and what it does not
- [Command-line options](../reference/cli.md)
