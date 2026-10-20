# The manifest `package.toml`

Every table and field of a package manifest. Only these are accepted; unknown tables and keys are errors (`miyu pkg install` and `miyu check` say which line is wrong). Chinese version: [manifest.md](manifest.md).

**Text for people** (`name`, `summary`, `about`, a setting's `name` and `description`…) is written as "language to sentence": `{ en = "Hello", zh = "打招呼" }`. Only `zh`, `en`, `ja` are recognized; the UI picks the person's language and falls back to `en`, `zh`, `ja`.

**The kind of package** isn't written; it follows from the tables present: `[process]` makes an extension; no program but `[[mcp]]` makes an MCP server package; only `[package]` makes a package of skills, scripts and command templates.

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
system_account = false                              # its own system account (only chat platform bridges need one), default no

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

## `[package]`

| Field | Notes |
|---|---|
| `name` | Required, the name shown to people |
| `protocol` | Required for packages with a program: `[lowest, highest]`; only `1` exists |
| `version` | Version, for people only |
| `summary` | A one-line summary |
| `author` | Author: a person or organization, may include an email, one line, at most 100 characters. Shown on the package info page |
| `upstream` | Upstream: the package's source repository, starting with `https://`. Shown on the package info page |
| `icon` | A Lucide icon name |
| `needs` | What a place must have for it to be useful: only `images` for now (the whole package is off in sessions that can't show images) |
| `targets` | Which systems it installs on: `linux-x86_64`, `linux-aarch64`, `macos-aarch64`, `windows-x86_64`. Leave it out for any system; when listed, other systems refuse it, and `miyu pkg pack --target` makes one package file per system |

## `[command]`

For extensions and UIs. `name`: when a person types `miyu <name> …` in a terminal, the core hands the rest of the arguments to your program as-is and uses your exit code (`miyu help <name>` becomes `--help`); it starts with a lowercase letter, uses only lowercase letters, digits and `-`, at most 32 characters; built-in subcommands (`ask`, `config`, `pkg`, `web`…) win a clash. `program`: the program name under `bin/`; on Windows the core looks for `<name>.exe`. `about`: a one-line description.

So besides arguments meant for the core, like `serve`, your program should handle what people type.

## `[process]`

Makes it an extension; needs `[command]`.

| Field | Notes |
|---|---|
| `args` | Arguments the core starts it with |
| `start` | `manual`: started when switched on (default); `always`: started with the core |
| `capabilities` | What it needs, see below |
| `system_account` | `true`: it has its own system account, named after the package. Only chat platform bridges need it: group sessions belong to this account |

**Capabilities**: only these seventeen. For packages the admin installed, a person approves them when turning it on; after an upgrade, only new ones are asked. **Declare honestly**: calling a method whose capability isn't declared returns `needs_approval`.

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

```toml
[features.greet]
name = { en = "Greeting", zh = "打招呼" }
summary = { en = "Greet people" }                  # optional
tools = ["hello_greet"]
```

Feature ids are global; prefixing them with the package id helps.

## `[check]`

`miyu check` runs `<program> <args…>`. Report problems on stdout, one JSON per line (`kind`, `file`, `level`, `message` required; `line`, `column`, `code`, `key` optional); exit 0 with only warnings, 1 with errors. At most 30 seconds. Needs `[command]`.

## `[page]`

`dir`: the package page folder, which needs `index.html`; opened from the web UI's Packages page. The page calls methods you registered through a channel the web app provides (`package.methods`, `method.call`; see [Writing an extension](extensions.en.md)).

## `[[mcp]]`

MCP servers it brings, one per entry; see [MCP servers](mcp.en.md).

## `[[slash]]`

Slash commands the extension provides, one per entry; `capabilities` must include `commands`.

| Field | Notes |
|---|---|
| `name` | Command name, written like `[command] name`; a clash with a Miyu built-in command isn't accepted |
| `summary` | The line shown in the command menu |
| `argument` | A hint for what follows the name, e.g. `{ en = "<city>", zh = "<城市>" }` |
| `access` | Who can use it: `admin` (default: the person, the owner and managers in groups), `anyone`, `off` (disabled for now). A person's setting in the system config overrides it |

When someone types it, the core calls your program with `command.call`; see [Writing an extension](extensions.en.md). For a command that just expands into a message to the AI, use a command template; no program needed (see [Scripts, skills, command templates](scripts-skills.en.md)).

## `[[search]]`

Registers your program as a search provider, one per entry; `capabilities` must include `search`.

| Field | Notes |
|---|---|
| `id` | Id, written like `[command] name`; unique within the package |
| `name` | Shown to people |

The settings screen's "Search provider" dropdown gets one more entry, valued `<package id>/<id>`. It's used only when a person picks it and never joins the "Auto" order, so your program never receives search terms without the person knowing. When the AI searches, the core calls your program with `search.query`; when you can't answer (an error, a rate limit), the shipped providers take over. See [Writing an extension](extensions.en.md).

## `[[widget]]`

A block of your own content in the terminal and web UIs; `capabilities` must include `widgets`. At most 16 per package.

| Field | Notes |
|---|---|
| `id` | Unique in the package: starts with a lowercase letter; lowercase letters, digits, `-`; at most 32 characters |
| `place` | `home` home screen, `sidebar`, `status` a small slot in the status bar, `page` a full page |
| `title` | Required for sidebar and page |
| `scope` | `global` (default); `session` follows the session, one per session; sidebar only |
| `command` | Pages only: typing this slash command opens the page |

Your program pushes the content; see [Writing an extension](extensions.en.md).

## `[export]`

`methods`: which of your methods other packages can call with `package.call` (they still have to be registered with `package.methods`).

## `[content.<package id>]`

Bring a folder of content for another package (sticker content packs work this way): `dir` is a relative folder in your package. That package's extension reads it with `content.list` and `content.read`. You can write several.

## `[connection]`

`platform`: the chat platform it connects to, written like a package id (e.g. `qq`). Extensions only; chat platform bridges write it.

## `[depends]`, `[recommends]`

`packages`: ids of packages it depends on or recommends. A missing dependency blocks installing and starting; a missing recommendation doesn't, it just loses some abilities.
