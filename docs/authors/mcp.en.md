# MCP servers

Existing MCP (Model Context Protocol) servers plug straight into Miyu: the core starts the server process, speaks MCP over stdin and stdout, and registers the server's tools for the AI. Two ways: in personal settings (just for you, quickest), or as a package (to share). Chinese version: [mcp.md](mcp.md).

## Quickest: personal settings

Open `~/.miyu/home/admin/settings.toml` (or `miyu config edit`), one table per server:

```toml
[mcp.files]
command = ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/home/me/notes"]
```

For a server that only gives a URL (GitHub and the like), write `url`, and the token as `bearer`:

```toml
[mcp.github]
url = "https://api.githubcopilot.com/mcp/"
bearer = { secret = "github" }       # store it with miyu provider login or the settings page; sent as Authorization: Bearer …
```

It takes effect when saved, no restart. The Packages page of the terminal or web UI shows whether it's running; `miyu check` checks for mistakes.

For a server that only accepts browser authorization and can't issue a personal token (Notion, Linear and the like): write only `url`, no `bearer`. It waits as "needs login" (`needs_login`); run `miyu mcp login <server>` and agree in the browser to connect. The token goes into the secrets file; you never handle it, and the core renews it before it expires. If the browser is on another machine, paste the whole address it lands on back into the terminal. Run `miyu mcp logout <server>` when you no longer need it.

## As a package

```text
notes-mcp/
└── package.toml
```

```toml
[package]
name = { en = "Notes", zh = "笔记" }

[[mcp]]
name = "notes"                                    # server name: lowercase letters, digits, -, 1 to 32 characters
command = ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/srv/notes"]
env = { TOKEN = { secret = "notes" } }            # optional; values can be text, or refer to a secret { secret = … } or an environment variable { env = … }
venues = ["local"]                                # optional: which sessions; local sessions only by default
timeout_ms = 600000                               # optional: how long one call may take, 1000 to 3600000
need_load = false                                 # optional: loaded on demand unless written; false always sends full parameters
clicall = false                                   # optional: the AI cannot call this server's tools with miyu call inside a command
```

A package with only servers and no program doesn't need `protocol`, `[command]` or `[process]`. A package can bring several servers; extension packages can bring them too, and they start and stop with the extension.

Install: `miyu pkg install ./notes-mcp`. A package with only servers installs **switched off**; turn it on in the Packages page.

## Fields

| Field | Notes |
|---|---|
| `name` | Server name (the table name in personal settings): lowercase letters, digits, `-`, 1 to 32 characters; globally unique |
| `command` | Command and arguments. The first item is looked up in the package's `bin/`, then next to `miyu`, then on `PATH` (`npx`, `uvx`, `docker` and the like). Write this or `url` |
| `url` | The server's address: starts with `https://`; `http://` only for servers on this machine (`localhost`, `127.0.0.1`), others aren't connected so the token isn't sent in the clear. Write this or `command` |
| `bearer` | With `url`: the token, sent as `Authorization: Bearer <value>`; written like an `env` value, please as `{ secret = … }` |
| `headers` | With `url`: other request headers, like `{ X-Api-Key = { secret = "key" } }`, written like `env` |
| `env` | Environment variables for the server. Write secrets as `{ secret = "<name>" }` (stored with `miyu provider login` or the settings screen); don't put values in the manifest |
| `venues` | One or more of `local`, `private`, `group`: local sessions, private chats and groups on chat platforms |
| `timeout_ms` | How long one call may take |
| `need_load` | Loaded on demand unless written: the AI normally sees only each tool's name and one line, and loads the full parameters when it needs them, which saves money for servers with many tools. `false`: always send the full parameters; write it for small servers used every day |
| `clicall` | Whether the AI can call this server's tools with `miyu call` from a script it runs; unset means it can. Write `false` for servers that write or send things |

## What the AI sees

- Tools are named `mcp__<server>__<tool>`, with the server's descriptions and parameters as-is (descriptions over 2048 characters are cut).
- In presets each server is one feature (`mcp-<server>`): switch it off whole, or tool by tool.
- **Whether people are asked**: tools annotated `readOnlyHint: true` are never asked about; the rest are treated like network access and need approval at the "workspace" level. If you write servers, mark read-only tools.

## Server instructions, prompts and resources

- **Instructions** (`instructions`): the text your server gives goes at the end of the AI's system prompt, one section per server with its name, up to 4000 characters. Only servers whose tools are in that session get one. Say when your tools should be used: tools often show only their name and one line at first, and the AI uses your instructions to decide when to fetch the full tool. Instructions that change mid-session take effect on the next turn where the tools change (so the cache isn't broken every turn).
- **Prompts** (`prompts`): they become slash commands `/<server>:<prompt>`, shown in the command menu. The words a person types after the name are split on whitespace (a value with spaces goes in English double quotes, e.g. `/weather:forecast "San Francisco" CA`) and matched to your declared arguments in order, the last argument taking the rest; the text and images that come back are sent to the AI as that person's message. Characters outside `[A-Za-z0-9_-]` in a name become `_`; a name that clashes after that isn't registered, and `miyu check` says so. Only usable in local sessions.
- **Resources** (`resources`): material your server makes available (files, documents, table schemas…), one URI each, or URI templates such as `file:///logs/{day}.txt`. A server that declares `resources` gives the AI two more tools: `mcp__<server>__list_resources` lists resources and templates a page at a time, and `mcp__<server>__read_resource` reads one by URI. Text comes back to the AI as is, images are taken like images from tools, and other binary content is reduced to one line with its type and size. They follow the server's switch, venues and load-on-demand; if your server has its own tool with the same name, yours is used. A server with only resources and no tools works too.

## While running

- The working directory is `~/.miyu/state/mcp/<server>/`, outside the sandbox; stderr goes to `~/.miyu/state/logs/mcp-<server>.stderr`. Look there first when it won't start.
- Servers that fail to start or crash are retried with backoff and stopped after 5 failures in a row; the Packages page shows the reason and the last lines of stderr, and restarting tries again.
- Both old and new MCP versions are understood. Tools, the server's instructions and its prompts are used; resources, `sampling` and the like aren't.
- Miyu doesn't install Node or Python for people: for servers started with `npx` or `uvx`, say what must be installed first.

## Same name in personal settings and a package

The one in personal settings wins and covers the package's (the Packages page shows this).
