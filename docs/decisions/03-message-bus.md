# Message bus v0

Message bus v0 (2026-10-28): one tool, `send(to, type, body)`, served by
imece itself over MCP: one Unix socket per agent in a fresh 0700 temp
directory; the session starts `imece mcp-relay <socket>`, which only
copies bytes. The sender is the socket, never an argument. Allowed by
exact name (`--allowedTools mcp__imece__<tool>`, each tool); `init` must report
exactly imece's server, connected, and exactly its tools. Mail waits per
agent; waking types start the next turn when the agent has no prompt
left, inform/progress/notice ride along. Caps: body 4000 chars, 20
unread, 12 message-driven turns per agent until the person speaks again;
a stopped agent gets no waking mail, nobody does once the budget is spent.
