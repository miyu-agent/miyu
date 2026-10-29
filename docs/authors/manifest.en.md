# The manifest `package.toml`

Every table and field of a package manifest. Only these are accepted; unknown tables and keys are errors (`miyu pkg install` and `miyu check` say which line is wrong). Chinese version: [manifest.md](manifest.md).

**Text for people** (`name`, `summary`, `about`, a setting's `name` and `description`…) is written as "language to sentence": `{ en = "Hello", zh = "打招呼" }`. Only `zh`, `en`, `ja` are recognized; the UI picks the person's language and falls back to `en`, `zh`, `ja`.

**The kind of package** isn't written; it follows from the tables present: `[process]` makes an extension; no program but `[[mcp]]` makes an MCP server package; `[mascot]` makes a mascot package and `[theme]` a theme package (both at the end of this page); only `[package]` makes a package of skills, scripts and command templates. The UI, built-in and worker kinds are for Miyu's own use and aren't covered here.

## A complete extension manifest

```toml
[package]
version = "0.1.0"                                   # optional, for people only
protocol = [1, 1]                                   # required for extensions: protocol major versions [lowest, highest]; only 1 exists
name = { en = "Hello", zh = "打招呼" }
summary = { en = "Greets people by name", zh = "照名字打招呼" }   # optional
author = "Alice <alice@example.com>"               # optional
upstream = "https://github.com/alice/hello"         # source repository, optional
icon = "hand"                                       # a Lucide icon name, optional
logo = "art/logo.png"                               # a picture shipped in the package, optional
needs = ["images"]                                  # optional: off where images can't be shown (voice)
targets = ["linux-x86_64", "macos-aarch64"]         # optional: the systems its own programs run on

[command]                                           # required for extensions
name = "hello"                                      # `miyu hello …` is handed to your program
program = "miyu-hello"                              # program name under bin/, no path
about = { en = "Hello extension", zh = "打招呼扩展" }

[process]                                           # this makes it an extension
args = ["serve"]                                    # arguments the core starts it with
start = "manual"                                    # manual: started when switched on (default); always: started with the core
capabilities = ["tools", "widgets"]                 # what it needs, see "Capabilities"

[check]
args = ["check"]                                    # `miyu check` runs <program> check

[settings.greeting]                                 # a setting; its key is hello.greeting
type = "text"
max = 40
default = "Hello"
layers = ["system"]
applies = "now"
name = { en = "Greeting", zh = "问候语" }

[features.greet]                                    # one switch in presets
name = { en = "Greeting", zh = "打招呼" }
tools = ["hello_greet"]

[page]
dir = "page"                                        # package page: the folder needs index.html

[export]
methods = ["greet"]                                 # methods other packages can call with package.call

[[slash]]
name = "hi"
summary = { en = "Say hi", zh = "打个招呼" }

[[widget]]
id = "status"
place = "status"
```


## The format is frozen

The tables and fields on this page are stable (frozen on 2026-10-23): from now on they are only added to, never renamed, changed or removed, so a manifest you've written keeps working with newer Miyu versions. New fields and tables say which version they arrived in; if you use one, put the lowest version you need in `[package] miyu`, and an older Miyu will say it needs updating. Unknown tables and fields are always errors, so write nothing that isn't on this page.
## `[package]`

| Field | Notes |
|---|---|
| `name` | Required, the name shown to people |
| `protocol` | Required for packages with a program: `[lowest, highest]`; only `1` exists |
| `version` | Version, for people only |
| `summary` | A one-line summary |
| `author` | Author: a person or organization, may include an email, one line, at most 100 characters. Shown on the package info page |
| `upstream` | Upstream: the package's source repository, starting with `https://`. Shown on the package info page |
| `icon` | A Lucide icon name. Not every head knows every Lucide name: the terminal only knows the names in its mapping table and draws the generic tool icon (a wrench) for the rest |
| `logo` | A picture shipped in the package: a relative path inside the package folder, `.png` or `.webp`. At most 256 KiB and 512 pixels a side; `miyu pkg pack` shrinks a bigger one in its own format while packing (the original in the folder stays). A missing, broken or oversized one counts as no logo: the package still installs and `miyu check` warns. Heads draw it first and fall back to `icon` |
| `needs` | What a place must have for it to be useful: only `images` for now (the whole package is off in sessions that can't show images) |
| `miyu` | The oldest Miyu it works with: three numbers, such as `"0.2.0"`. Write it when you use newly added fields or methods; an older Miyu then says it needs updating instead of reporting keys it doesn't know |
| `groups` | Only for packages that ship with Miyu: the groups it belongs to (for example `["basesystem"]`); a preset that names a group switches the packages in it. Other packages get an error. The old `required` (also shipped-only) was dropped on 2026-10-26 |
| `targets` | Which systems it installs on: `linux-x86_64`, `linux-aarch64`, `macos-aarch64`, `windows-x86_64`. Leave it out for any system; when listed, other systems refuse it, and `miyu pkg pack --target` makes one package file per system |

## `[command]`

For extensions and UIs. `name`: when a person types `miyu <name> …` in a terminal, the core hands the rest of the arguments to your program as-is and uses your exit code (`miyu help <name>` becomes `--help`); it starts with a lowercase letter, uses only lowercase letters, digits and `-`, at most 32 characters; built-in subcommands (`ask`, `config`, `pkg`, `web`…) win a clash. `program`: the program name under `bin/`; on Windows the core looks for `<name>.exe`. `about`: a one-line description. `args`: optional, put before what the person typed when the arguments are handed over.

So besides arguments meant for the core, like `serve`, your program should handle what people type.

## `[process]`

Makes it an extension; needs `[command]`.

| Field | Notes |
|---|---|
| `args` | Arguments the core starts it with |
| `start` | `manual`: started when switched on (default); `always`: started with the core |
| `capabilities` | What it needs, see below |

**Capabilities**: only these seventeen. For packages you installed, you approve them when turning it on; after an upgrade, only new ones are asked. **Declare honestly**: calling a method whose capability isn't declared returns `needs_approval`.

| Capability | Allows |
|---|---|
| `tools` | Registering tools in the tool catalog |
| `commands` | Providing slash commands (`[[slash]]`) |
| `search` | Being a search provider (`[[search]]`) |
| `widgets` | Showing its own content in the UI (`[[widget]]`) |
| `context.inject` | Adding a few lines to her context at the start of a turn |
| `context.system` | Putting a section at the end of the system prompt |
| `packages.call` | Calling methods other packages export |
| `act_for_external` | Speaking for people on a chat platform: for platform bridges only |
| `sessions.drive` | Driving sessions: creating, sending, interrupting, reverting, compacting, forking, running slash commands |
| `events.read` | Reading sessions: subscribing to events, reading history and background command output |
| `events.write` | Recording your own events in a session |
| `memory.write` | Writing memories and knowledge bases |
| `fs.read`, `network` | Reading files and fetching links through the core |
| `fs.write` | Writing given paths: granted once extensions run in a sandbox |
| `tool.guard`, `tool.rewrite` | Stopping a tool before it runs, rewriting its result afterwards: not available yet |

## `[settings.<name>]`

One table per setting; the key is `<package id>.<name>`. To group many settings, add one level: `[settings.<group>.<name>]` gives `<package id>.<group>.<name>` (for example `im.qq.listen`); only one extra level, and a setting in a group can't be called `type` or `name`. The settings screen's Packages page has a group for you. **Don't read config files yourself**: the values arrive in the handshake and changes are pushed (see [Writing an extension](extensions.en.md)).

| Field | Notes |
|---|---|
| `type` | Required: `bool`, `int` (`min`, `max`), `option` (`choices`, at least two), `text` (`max`, default 200 characters), `name`, `url`, `secret` (only as a `{ secret = … }` reference), `list` (`element` is the element type) |
| `default` | Default value of that type; not allowed for `secret` |
| `layers` | One or both of `system`, `personal`; both by default |
| `applies` | When it takes effect: `now`, `new_session`, `next_turn`, `program_start` (your program's next start, default) |
| `name`, `description` | Shown to people |
| `hidden` | `true`: not drawn on the settings screen, still writable and readable |

## `[features.<id>]`

One preset switch controls a group of tools. Without any, the whole package is one feature; with several, every tool you register must be listed under some feature or it won't register.

| Field | Notes |
|---|---|
| `name` | Required, the name people see |
| `summary` | One line about it |
| `tools` | The tool names this feature switches |

```toml
[features.greet]
name = { en = "Greeting", zh = "打招呼" }
summary = { en = "Greet people" }                  # optional
tools = ["hello_greet"]
```

Feature ids are global; prefixing them with the package id helps.

## `[check]`

`args`: `miyu check` runs `<program> <args…>`. Report problems on stdout, one JSON per line (`kind`, `file`, `level`, `message` required; optional `line` and `column` as positive integers and `code`, `key`, `rule`, `source` as strings; other fields are dropped); exit 0 with only warnings, 1 with errors. At most 30 seconds. Needs `[command]`.

## `[page]`

`dir`: the package page folder, which needs `index.html`; opened from the web UI's Packages page. The page calls methods you registered through a channel the web app provides (`package.methods`, `method.call`; see [Writing an extension](extensions.en.md)).

## `[[mcp]]`

MCP servers it brings, one per entry; see [MCP servers](mcp.en.md).

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

## `[[slash]]`

Slash commands the extension provides, one per entry; `capabilities` must include `commands`.

| Field | Notes |
|---|---|
| `name` | Command name, written like `[command] name`; a clash with a Miyu built-in command isn't accepted |
| `summary` | The line shown in the command menu |
| `argument` | A hint for what follows the name, e.g. `{ en = "<city>", zh = "<城市>" }` |
| `access` | Who can use it: `admin` (default: the person, the owner and managers in groups), `anyone`, `off` (disabled for now). Your setting in the config overrides it |

When someone types it, the core calls your program with `command.call`; see [Writing an extension](extensions.en.md). For a command that just expands into a message to the AI, use a command template; no program needed (see [Scripts, skills, command templates](scripts-skills.en.md)).

## `[[search]]`

Registers your program as a search provider, one per entry; `capabilities` must include `search`.

| Field | Notes |
|---|---|
| `id` | Id, written like `[command] name`; unique within the package |
| `name` | Shown to people |

The settings screen's "Search provider" dropdown gets one more entry, valued `<package id>/<id>`. It's used only when a person picks it and never joins the "Auto" order, so your program never receives search terms without the person knowing. When the AI searches, the core calls your program with `search.query`; when you can't answer (an error, a rate limit), the shipped providers take over. See [Writing an extension](extensions.en.md).

## `[[side]]`

Registers the model requests your program makes outside the conversation (side requests), one per entry, each declaring which tier of model it wants. No capability is needed.

| Field | Notes |
|---|---|
| `id` | Id, written like `[command] name`; unique within the package |
| `tier` | Which tier: `lite` (cheap and fast, for high-volume judgements), `normal` (has to understand a stretch of conversation), `flagship` (the best) |
| `name` | Shown to people |
| `summary` | One sentence: when it is called and what it does |

```toml
[[side]]
id = "judge"
tier = "lite"
name = { en = "Reply judge", zh = "判断要不要回" }
summary = { en = "Decides whether to reply in a group.", zh = "群里有人说话时判断要不要回" }
```

Call `model.call` with `"side": "judge"` (only an id registered in your own package, without the package name) and without `model` or `use`: the core takes a model from the pool named after the tier (`lite`, `normal`, `flagship`; people put models into them in settings). When that pool is empty, or every model in it is cooling down or failed, it uses the session's model — the session you pass as `session`, or `models.chat` if you pass none. People don't configure a model for your request. An id you didn't register gets `bad_params`. `model.list` lists yours under `side` (as `<package id>/<id>`) with where each one lands right now. See [Writing an extension](extensions.en.md).

## `[[widget]]`

A block of your own content in the terminal and web UIs; `capabilities` must include `widgets`. At most 16 per package.

| Field | Notes |
|---|---|
| `id` | Unique in the package: starts with a lowercase letter; lowercase letters, digits, `-`; at most 32 characters |
| `place` | `home` home screen, `sidebar`, `status` a small slot in the status bar, `page` a full page |
| `title` | Required for sidebar and page |
| `scope` | `global` (default); `session` follows the session, one per session; sidebar only |
| `command` | Pages only: typing this slash command opens the page |
| `state` | `true`: this widget's content comes from your package's state in the session (a tool result's `state` or `session.set_state`, see [Extensions](../reference/extensions.en.md)), and stays there even when your program is not running; only for `scope = "session"` widgets |

Your program pushes the content; see [Writing an extension](extensions.en.md).

## `[export]`

`methods`: which of your methods other packages can call with `package.call` (they still have to be registered with `package.methods`).

## `[content.<package id>]`

Bring a folder of content for another package (sticker content packs work this way): `dir` is a relative folder in your package. That package's extension reads it with `content.list` and `content.read`. You can write several.

## `[connection]`

`platform`: the chat platform it connects to, written like a package id (e.g. `qq`); a list for several (`["qq", "tg"]`). Extensions only; chat platform bridges write it.

With several platforms, start tool names with the platform: `qq_send` is only given in `qq:` groups and private chats, `tg_send` only in `tg:` ones; local sessions get all of them. Tools without a platform prefix are given everywhere.

## `[depends]`, `[recommends]`

`packages`: ids of packages it depends on or recommends. A missing dependency blocks installing and starting; a missing recommendation doesn't, it just loses some abilities. The older spelling `workers` is still read, into the same list.

## `[mascot]`

The mascot on the terminal's home screen, as a package:

```toml
[package]
name = { zh = "布丁", en = "Pudding" }
icon = "cat"            # optional

[mascot]
model = "mascot.json"   # relative path inside the package folder
```

- `model` is required: a relative path inside the package folder, no `..` and no leading `/`. Nothing else is accepted.
- A mascot-only package has just `[package]` and `[mascot]`, plus optionally `[depends]` and `[recommends]`. It speaks no protocol: `protocol` or any other table is `wrong_kind`. An extension package may also carry a `[mascot]`.
- `miyu check` checks the model file: missing, over 256 KiB, or not a JSON object is an error. What goes inside is defined and checked by the terminal, see [docs/blueprint/tui.md](https://github.com/miyu-agent/miyu-tui/blob/main/docs/blueprint/tui.md) (`mascot.json`) in the terminal repository.
- No switch, settings or program; the terminal picks one by its setting `tui.mascot`.

## `[theme]`

A terminal theme (box lines, symbols, text styles), as a package:

```toml
[package]
name = { zh = "纯字符", en = "ASCII" }

[theme]
file = "theme.json"     # relative path inside the package folder
```

- `file` is required, written like `[mascot] model`.
- A theme package has just `[package]` and `[theme]`, plus optionally `[depends]` and `[recommends]`. `protocol`, `[command]`, `[mascot]` or any other table is `wrong_kind`: a theme package carries no mascot, and no other package can carry a theme.
- `miyu check` checks the theme file the same way as a mascot's. What goes inside is in [docs/blueprint/tui.md](https://github.com/miyu-agent/miyu-tui/blob/main/docs/blueprint/tui.md) ("主题", themes) in the terminal repository.
- No switch, settings or program; the terminal picks one by its setting `tui.look`.
