# Packages and commands: `package.*`, `extension.*`, `mcp.*`, `skill.list`, `skill.remove`, `script.list`, `tool.set_loading`, `feature.set`, `command.*`, `check`

Manage packages, extension processes and the MCP servers in personal settings; list skills and scripts; change how tools load and the global switches of skills, scripts and MCP servers; run slash commands; check files for mistakes. Methods extensions call themselves, and calls the core makes to extensions, are in [Extensions](extensions.en.md). Chinese version: [packages.md](packages.md).

| Method | What it does |
|---|---|
| [`package.list`](#packagelist) | List packages |
| [`package.info`, `package.files`, `package.owns`, `package.check`](#packageinfo-packagefiles-packageowns-packagecheck) | A package's info and files; which package owns a path; whether installed files changed |
| [`package.install`, `package.remove`](#packageinstall-packageremove) | Install, remove |
| [`package.enable`, `package.disable`, `package.restart`](#packageenable-packagedisable-packagerestart) | Turn on, turn off, restart |
| [`package.sync`](#packagesync) | Fetch the index from the package source |
| [`package.search`](#packagesearch) | Search the package source |
| [`package.clean`](#packageclean) | Delete downloaded package files |
| [`package.upgrades`, `package.upgrade`](#packageupgrades-packageupgrade) | What can be upgraded; upgrade |
| [`package.file`, `package.call`](#packagefile-packagecall) | Read a file of a package's page; call a method its program registered |
| [`extension.status`, `extension.enable`, `extension.disable`, `extension.restart`](#extensionstatus-extensionenable-extensiondisable-extensionrestart) | Extension process status and switches |
| [`mcp.list`, `mcp.restart`, `mcp.set`, `mcp.remove`](#mcplist-mcprestart-mcpset-mcpremove) | MCP servers in personal settings: list, restart, add or replace, remove |
| [`mcp.login`](#mcplogin) | A URL MCP server that needs a browser login: get the authorization page |
| [`skill.list`](#skilllist) | List the skills that can be used |
| [`skill.remove`](#skillremove) | Delete skills the admin placed |
| [`script.list`](#scriptlist) | List the registered scripts |
| [`tool.set_loading`, `feature.set`](#toolset_loading-featureset) | Change how a tool or an MCP server loads; change the global switch of a skill, script or MCP server |
| [`command.run`](#commandrun) | Run a slash command |
| [`command.catalog`](#commandcatalog) | List the slash commands that can be used |
| [`check`](#check) | Check config, personas, presets and packages for mistakes |

**Who can call**: `package.install`, `package.remove`, `package.enable`, `package.disable`, `package.restart`, `package.sync`, `package.search`, `package.clean`, `package.upgrades`, `package.upgrade`, `package.file`, `extension.*`, `mcp.*`, `tool.set_loading` and `feature.set` are for people only; extensions get `local_only`. Everything else is open to people and extensions; extensions calling `package.call` have extra rules (below).

**Two layers**: built-in (installed with Miyu) and installed in the admin's home (`home/admin/packages/`). Installing the same id in the home layer upgrades and replaces it; removing a built-in package records that in the home layer, and it can be installed back.

## `package.list`

**Params**: none. **Response**: `{"packages": [...]}`, by id. An item:

| Name | Notes |
|---|---|
| `package`, `layer` | Id; `shipped` or `home` |
| `kind` | `builtin` (compiled into the core), `process` (an extension), `worker` (a helper started on demand), `ui` (a user interface), `mascot`, `theme` (a terminal theme, with `theme: {"file"}`; `package.file` gives only that file), `bundle` (no program; only MCP servers, skills, scripts and the like) |
| `name`, `summary`, `icon` | Name, one-line summary, Lucide icon name |
| `author`, `upstream` | Author and upstream source repository (`https://`); absent when the manifest doesn't say |
| `from` | Where it came from: `shipped`, `official` (installed from the official software source), `local` (a local folder or package file); `community` will join once community sources exist |
| `version`, `protocol`, `state` | Version, protocol it speaks, state |
| `required` | Required; can't be removed |
| `features` | Its features (presets switch these) |
| `status`, `enabled` | With a process: running or not, on or not |
| `page` | Has a package page |
| `mcp` | Its MCP servers: `name`, `command` (`url` for a URL server), `venues`, `state`, `failures`…; `need_load`, `on` and `skipped` as in `mcp.list` |
| `widgets` | Its declared widgets: `id`, `place`, `title`, `scope`, `command` |
| `command`, `commands` | Subcommands it adds to `miyu`: `command` is the first `{"name", "program", "about"}`, `commands` all of them in the order written (a package can add several) |
| `contents` | Which packages it carries content for (the ids under `[content.<package>]` in its manifest); absent when none |
| `removed` | A removed built-in package |

Broken, conflicting or unreadable ones have only `package`, `layer`, `code`, `problem` (in the connection's language) and `line`. To keep up to date, subscribe to the `packages` stream.

## `package.info`, `package.files`, `package.owns`, `package.check`

| Method | Params | Response |
|---|---|---|
| `package.info` | `package` | `{"package", "layer", "from", "files", "size", "version"?, "author"?, "upstream"?, "installed"?, "source"?}`: where it came from (as in `package.list`), file count, bytes, author, upstream, install time, the local path it was installed from |
| `package.files` | `package` | `{"dir", "files": [{"path", "sha256", "size"}…]}`; `path` is relative to `dir` |
| `package.owns` | `path` (absolute) | `{"package", "layer", "path", "recorded"}`; `{"package": null}` when no package owns it |
| `package.check` | `package` (optional) | `{"packages": [{"package", "modified", "missing", "extra"}…]}`: for packages in the home layer, files changed, missing or added since installation |

**Errors**: `unknown_package`; `bad_params` (a relative path for `owns`).

## `package.install`, `package.remove`

| Method | Params | Response |
|---|---|---|
| `package.install` | `path`: absolute path to a local package folder (the folder name is the id), to its `package.toml`, or to a `.miyu` package file | An item as in `package.list` |
| `package.install` | `package`: a built-in id installs the removed package back; any other id is the same as `source`, from the package source | The same |
| `package.install` | `source`: install this id from the package source (missing dependencies come along, installed first); pushes `package.progress {"package", "done", "total"}` to this connection while downloading | The same, plus `with` listing the dependencies installed along |
| `package.remove` | `package` | `{"package", "removed": true}` |

Both take `preview: true`: look without changing anything, refused the same way the real thing would be. The install preview returns `package`, `version`, `replaces: {"version"}` when replacing, `restores: true` for a built-in coming back, and what it carries (`program`, `mcp`, `command` (the first subcommand name), `commands` (every subcommand name), `page`, `mascot`, `theme`, `connection`, `system_account`, `settings`, `capabilities`, `files`, `size`); the remove preview returns `package`, `layer`, `version`, `files`, `size`, `settings` (config keys removed along with it) and `state` (whether it has a state directory).

Install and remove take effect at once: processes start or stop per their switches, their tools enter or leave the tool catalog, and open sessions pick that up at the next turn.

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | Both given; a path that isn't absolute |
| `path_unreadable` | The folder can't be read |
| `package_invalid` | The manifest is wrong, `data.problem`, `data.line`; for a package that needs a newer Miyu, `data.problem` is `needs_newer_miyu` |
| `package_conflict` | A name it brings clashes with an installed package: subcommand, feature, tool, skill, MCP server, knowledge base, slash command |
| `package_exists` | Same id as a built-in package |
| `package_file_bad` | The package file can't be unpacked |
| `download_failed` | Installing from the package source, the file couldn't be downloaded anywhere (a size or hash mismatch counts); `data.why` lists each place's reason |
| `wrong_target` | The manifest lists `targets` and this system isn't one of them |
| `package_required` | Removing a required package |
| `depends_missing` | A dependency isn't installed, `data.missing` |
| `package_depended` | Another installed package depends on it, `data.by` |
| `unknown_package` | Not installed, or never removed |

## `package.sync`

Fetch the index from the package source. Param `force` (optional): `true` fetches even if the local copy is recent; otherwise a local copy younger than 24 hours is used as is. Addresses are tried in the order of the system config's `packages.mirrors`, then `packages.source`; the index is verified against the official public keys built into Miyu, and one older than the local copy is not taken.

Response `{"generated", "fetched", "packages", "stale"?}`: when the index was generated and fetched (RFC 3339), and how many packages it lists; `stale` (the last place's reason, in English) means nothing could be fetched this time and the local copy is used.

**Errors**

| `data.reason` | When |
|---|---|
| `source_unreachable` | Nothing could be fetched and there is no local index; `data.why` is the last place's reason |

## `package.search`

Search the package source. Param `query` (optional): words separated by spaces, each of which must appear in the id, name or summary (any language), case-insensitive; empty or absent lists everything. Fetches the index as `package.sync` does first (a local copy younger than 24 hours is used as is).

Response `{"packages": [...], "generated", "stale"?}`, each item `{"package", "version", "name", "summary", "official", "installed", "blocked", "contents", "provides", "depends", "kind"?}`: name and summary in the connection's language (falling back to English, Chinese, Japanese), `null` when missing; `installed` is the installed version (empty text when installed without one), `null` when not installed; `official` says whether it comes from the owner's own repositories; `blocked` is the reason it was blocked, `null` when it wasn't; `contents` lists which packages it carries content for (as in `package.list`), an empty array when none; `provides` is what it carries, kind to names, the kinds being `command` (subcommands), `features`, `mcp` (MCP servers), `slash` (slash commands), `content` (packages it carries content for), `skills`, `scripts`, `personas`, `presets`, with empty kinds left out and `{}` when it carries none of them; `depends` lists the ids of the packages it depends on, an empty array when none; `kind` is the package kind as written in the manifest's `[package] kind` (`process`, `ui`, `theme`, `mascot`, `bundle`, …, new ones passed through as they are), missing from indexes generated before it was added. Errors as in `package.sync`.

## `package.clean`

Delete downloaded package files that aren't the installed version. No params; response `{"removed", "freed"}`: how many files and bytes were deleted.

## `package.upgrades`, `package.upgrade`

Upgrade from the package source. Only packages installed from the source are touched; ones installed from a folder or a package file are left alone. Versions aren't compared by size: if the index's version differs from the installed one, it's an upgrade.

| Method | Params | Response |
|---|---|---|
| `package.upgrades` | None | `{"upgrades": [{"package", "from", "to"}], "moved": [{"package", "was", "now"}], "gone", "blocked": [{"package", "reason"}], "held", "generated", "stale"?}`: what can be upgraded; what now comes from another repository (not upgraded); what's gone from the source; what's blocked; what the system config's `packages.hold` skips |
| `package.upgrade` | `packages` (optional, only these), `preview` | Preview `{"upgrades", "download"}`; done `{"upgraded": [{"package", "from", "to"}]}`, pushing progress while downloading as `package.install {"source"}` does |

## `package.enable`, `package.disable`, `package.restart`

**Params**: `package`; `enable` also takes `approve`: approve the capabilities it declares (see "Capabilities" in [Extensions](extensions.en.md)). **Response**: an item as in `package.list`.

- Extensions: on starts the process; off asks it to exit and its tools leave the catalog.
- `bundle`: on starts its MCP servers; off stops them.
- Built-in packages that aren't required: off removes it, on installs it back.
- `restart`: restarts the extension process and its MCP servers; the failure count starts over.

**Errors**

| `data.reason` | When |
|---|---|
| `unknown_package` | No such package |
| `package_required` | Turning off a required one |
| `not_switchable` | UI, worker, mascot and theme packages have no switch |
| `program_missing` | The package's program wasn't found (neither in the package folder's `bin/` nor next to `miyu`) |
| `needs_approval` | Capabilities still need approval, `data.capabilities` |
| `package_depended` | Turning off one another package depends on |
| `package_off`, `not_restartable` | `restart` on one that's off; nothing to restart |

## `package.file`, `package.call`

**`package.file`**: params `package`, `path` (relative, inside its page folder), `offset`. Response `{"data", "size", "eof"}`, at most 512 KiB at a time. The web UI's package pages, mascot model files and theme files are read this way.

**`package.call`**: params `package`, `method`, `params` (optional). The core forwards it to that package's program (`method.call`) and returns whatever it answers. Answered in the background. Widget buttons and saving page forms go through it.

Extensions can call `package.call` too: they must declare the `packages.call` capability and can only call methods listed in the other package's `[export]`; the forwarded call carries `from` (the caller's package id). Blobs the answering package stored that appear in its response become readable by the caller (see [Attachments](blob.en.md#blobget)).

**Errors**

| `data.reason` | When |
|---|---|
| `no_page`, `not_found` | `package.file`: no package page; no such file, outside the folder, not a regular file |
| `program_not_running` | The program isn't connected |
| `unregistered` | The program didn't register that method, `data.method` |
| `method_failed` | The program answered with an error, `data.message`, `data.code` |
| `method_timeout` | No answer in time |
| `needs_approval`, `not_exported` | Called by an extension: `packages.call` not declared; the method isn't exported (`data.method`) |

## `extension.status`, `extension.enable`, `extension.disable`, `extension.restart`

**Params**: none for `status`; `package` for the others, and `enable` also takes `approve`. **Response**: `status` gives `{"extensions": [...]}`, the others the one extension. One:

| Name | Notes |
|---|---|
| `package`, `name` | Id, name |
| `start` | When the manifest says to start |
| `on` | The switch |
| `state` | `off`, `starting`, `running`, `waiting` (failed, waiting to restart), `stopped` |
| `failures` | Consecutive failures |
| `pid` | Process id while running |
| `retry_in` | While waiting: milliseconds until the next start |
| `reason`, `stderr` | When stopped: why (`failed_repeatedly`, `config_error`, `needs_approval`, `needs_login` for a URL server that needs a browser login…), the last lines of stderr |
| `capabilities`, `unapproved` | Declared capabilities `[{id, name, summary}]`, those not yet approved |

To keep up to date, subscribe to the `extensions` stream.

**Errors**: `unknown_package`; `not_an_extension` (a UI package); `extension_off` (restarting one that's off); `needs_approval`.

## `mcp.list`, `mcp.restart`, `mcp.set`, `mcp.remove`

MCP servers in the admin's personal settings (`[mcp.<name>]` in `home/admin/settings.toml`). Editing personal settings by hand or through `mcp.set` and `mcp.remove` takes effect at once: new servers start, changed ones restart, removed ones stop.

**`mcp.list`**: no params. Response `{"servers": [...]}`: `name`, `command` (a URL server has `url` instead), `venues`, `state`, `failures`, `pid` while running (none for a URL server), `retry_in` while waiting, `reason` and `stderr` when stopped, and `covers` (packages whose same-named server it covers). When its tools are known it also has `tools`: the server's tool names in the tool catalog right now (`mcp__<server>__<tool>`), whether listed by the running server or registered from the catalog cache; a server that declares resources also has `mcp__<server>__list_resources` and `mcp__<server>__read_resource`; left out while unknown (never started, no cache). Tools the server listed this time but that were not registered come as `skipped: [{"tool", "why"}]` (`{"prompt", "why"}` for a prompt), sorted by name: `tool` is the server's own name for it (empty when it had none), `why` is `name` (no name), `parameters` (the parameters schema is not an object) or `name_taken` (it clashed after shortening to 64 characters, or another tool already has the name); left out when everything was registered or when the tools only came from the catalog cache and have not been listed yet. `miyu check` reports them as `mcp_tool_skipped`. `need_load` is `{"value", "by"}`: whether the server's tools load on demand; `by` is `source` (as the server table says; unset loads on demand) or `setting` (overridden by `loading.mcp.<server>` in personal settings, see `tool.set_loading`). `on` is the server's global switch (`features.mcp-<server>`, see `feature.set`), `true` when unset. For editing, what was written comes back too: `env` (each value text, or `{"secret": …}`, `{"env": …}`; left out when none) and `timeout_ms` (only when not the default), plus `bearer` and `headers` for a URL server (written like `env`). These come back as written (they are plain text in personal settings anyway): heads mask them on display and show them in full only in the edit box; values worth hiding should be secret references; `command` is the program and its arguments, split into `command` and `args` when sent back to `mcp.set`; change `need_load` with `tool.set_loading`.

**`mcp.restart`**: param `name`. Response: that server. **Errors**: `unknown_server`.

**`mcp.set`**: add a server to personal settings, or replace the one with the same name as a whole.

| Name | Type | Notes |
|---|---|---|
| `name` | string, required | Server name: starts with a lowercase letter; lowercase letters, digits and `-` only; at most 32 |
| `command` | string | The program; write either this or `url` |
| `args` | array of strings | Arguments after the program; needs `command` |
| `env` | object | Environment variables: each value is text, or `{"secret": …}`, `{"env": …}`; goes with `command` |
| `url` | string | A server at a URL: starts with `https://`, or `http://` for this machine; write either this or `command` |
| `bearer` | text or object | A token, sent as `Authorization: Bearer <value>`; written like an `env` value; goes with `url` |
| `headers` | object | Other request headers, values written like `env` values; `Authorization`, `Mcp-Session-Id` and `MCP-Protocol-Version` are the core's; goes with `url` |
| `venues` | array of strings | Which sessions get it: `local`, `private`, `group`; local sessions only when left out |
| `timeout_ms` | integer | How long one call may take, 1000 to 3600000; 600000 when left out |
| `need_load` | boolean | The server's tools must be loaded first; when left out, as if not written in personal settings |
| `clicall` | boolean | Whether the AI can call this server's tools with `miyu call` inside a command; when left out, what was written stays, and unwritten means it can |

Fields left out are not written: replacing drops what was there before and is not sent again. It is checked by the same rules as reading personal settings and written the way `config.set` writes (with a version, never over a concurrent hand edit). Response `{"name"}`; the server then follows what was written, and state changes push `mcp.changed`. **Errors**: `mcp_invalid` (badly written; `data.problem` says what is wrong, including neither or both of `command` and `url`); `bad_params` (`args` without `command`); `server_from_package` (personal settings have no server by that name and a package brings one: servers from packages are not changed here); `config_file_broken` (personal settings can't be read, or `mcp` is written in a shape that can't hold a server); `config_conflict`.

**`mcp.remove`**: param `name`. Removes the server (with its sub-tables) from personal settings; response `{}`. A package server it covered starts again according to the package's switch. **Errors**: `unknown_server`; `config_file_broken`; `config_conflict`.

## `mcp.login`

For a URL MCP server that answered `401` and needs a browser login (`reason` is `needs_login`, and its table has no token of its own): get the authorization page. The person agrees in the browser, the core exchanges the code, stores the token in the secrets file (`mcp-oauth-<server>`, never handed out over the protocol), and the server connects by itself (`mcp.changed` and the package list pushes as usual). Local people only.

**Params**: `service` (the server name, from personal settings or carried by a package).

**Response**: `{"url"}`: the authorization page for the head to open. The core waits for the callback on `127.0.0.1`, up to 10 minutes; logging in again to the same server stops the earlier wait.

**Errors**: `unknown_server`: no such server; `bad_params`: not a URL server, or its table has its own token (`bearer`, or `Authorization` in `headers`); `oauth_unregistered`: no client, and the authorization server doesn't take dynamic registration, so register a client with the service and put its `client_id` in the table; `oauth_failed`: the authorization server can't be found or used, or it refused; `data.why` says why (the server's own words, shortened).

## `skill.list`

**Params**: none. **Response**: `{"skills": [...], "dir"}` (`dir` is the folder where the admin places skills, given even before it exists, written `~/…` under the home directory), the skills the core can use right now, sorted by `name`; when several places have the same name, only the one the core actually uses is listed (your own wins; among packages, built-in before ones the admin installed, then by id). An item:

| Name | Notes |
|---|---|
| `name` | Skill name |
| `display_name`, `summary` | Name and one-line summary for people, picked by the connection's language (falling back to English, Chinese, Japanese in that order); left out when none is written |
| `from` | Where it comes from: `home` (placed by the admin, `home/admin/skills/`), `shipped` (from a built-in package), `package` (from a package the admin installed) |
| `package` | For skills from a package: the package id |
| `path` | Where the `SKILL.md` is; paths under the home directory are written `~/…` |
| `on` | Global switch (`features.skill-<name>`, see `feature.set`); `true` when unset |

## `skill.remove`

**Params**: `names`, an array of skill names, at least one, no repeats. **Response**: `{"trash": [...]}`, where each skill's folder now is in the system trash, in the order of `names`. Only skills the admin placed (`from` is `home`; the whole `home/admin/skills/<folder>/` goes to the trash); when one covered a package skill of the same name, that one is listed again. People only: an extension process gets `local_only`.

**Errors**: no names or repeated names: `bad_params`. No such skill: `unknown_skill`; one that comes with a package: `skill_from_package` (both with `data.name`; if any name fails, nothing is deleted). Could not move to the trash: `trash_failed`.

## `script.list`

**Params**: none. **Response**: `{"scripts": [...], "dir"}` (`dir` is the folder where the admin places scripts, written as in `skill.list`), the scripts registered as tools right now, sorted by `tool`. An item:

| Name | Notes |
|---|---|
| `tool` | Tool name |
| `display_name`, `description` | Name and one-line description for people (the script head's `Display name`, `显示名称`, `描述`), picked by the connection's language as in `skill.list`; left out when none is written |
| `from`, `package` | As in `skill.list`; `home` is placed by the admin (`home/admin/scripts/`) |
| `path` | Where the script file is, written as in `skill.list` |
| `timeout` | Seconds one call waits |
| `need_load` | `{"value", "by"}`: whether it loads on demand; `by` is `source` (as the script head says; unset loads on demand) or `setting` (overridden by `loading.tools.<tool>` in personal settings) |
| `on` | Global switch (`features.script-<tool>`); `true` when unset |

## `tool.set_loading`, `feature.set`

The two things the skill, script and MCP pages change. Both write the admin's personal settings the way `config.set` changes one item (with the version, never over a hand edit made at the same time, pushing `config.changed` once written). They take effect from the next turn. Response `{}`.

**`tool.set_loading`**: whether one tool (`loading.tools.<tool>`) or one MCP server (`loading.mcp.<server>`) loads on demand. A tool's own setting comes first, an MCP tool then follows its server's, and with neither it follows where it came from.

| Name | Type | Notes |
|---|---|---|
| `tool` | string | Tool name: 1 to 128 characters without `.` or control characters |
| `server` | string | MCP server name |
| `need_load` | boolean or `null`, required | `null` removes the setting and goes back to the source |

Give exactly one of `tool` and `server`.

**`feature.set`**: the global switch of one skill, script or MCP server (`features.<feature id>`). Presets whose `unlisted` is `on` follow it, except where the preset turns that feature on or off itself (`by` of each feature in `preset.get`). Presets whose `unlisted` is `off` ignore it: what they leave out stays off.

| Name | Type | Notes |
|---|---|---|
| `feature` | string, required | Feature id: `skill-<name>`, `script-<tool>`, `mcp-<server>` |
| `on` | boolean or `null`, required | `null` removes the switch and goes back to following the presets |

**Errors**: `bad_params` (not exactly one of `tool` and `server`, a name that does not fit the key, a feature that is none of the three kinds, `need_load` or `on` missing); `config_file_broken`; `config_conflict`.

## `command.run`

Run a slash command: the head passes along exactly what the person typed; the core recognizes it, checks who may use it, and runs it.

**Params**

| Name | Type | Notes |
|---|---|---|
| `session` | string, required | Which session |
| `text` | string, required | What the person typed, e.g. `/stop` |
| `cwd` | string | The head's directory; only used to resolve a relative path after `/workspace` |
| `as` | object | Venue sessions: speaking for a person on the platform, as in `session.send` |

**Response**: `{"command", "events", "said"}`: the canonical name (aliases resolved), the events appended, and the reply line (in the connection's language, shown to the person as-is). A command that opens a page returns `{"command", "opens": {"package", "widget"}, "said": ""}` and the head opens that page.

**Commands the core knows**:

| Command | What it does |
|---|---|
| `/clear` (`/reset`) | Clear the context and the to-do list, like `session.clear` |
| `/stop` | Interrupt this turn and stop the background commands and subagents this session spawned, and its organizing |
| `/workspace <path>` | Change the working directory; without a path, say where it is |
| `/todo`, `/todo clear` | Show the to-do list; clear it |
| `/remember <words>` | Remember something |
| `/dream`, `/dream stop` | Organize memory now (answered in the background); stop the organizing in progress, keeping what's merged |
| `/clear-memory` (`/reset-memory`) | Clear what this session remembered |
| `/goal` family | Show, set, pause, resume, edit, clear the standing goal |

Anything else is looked up, in order, among your own command templates (`home/admin/commands/<name>.md`), commands extensions declare (the core forwards them, `command.call`), and templates in packages. A template expands into a message sent as if the person said it: the response's `events` is that message and `said` is empty.

A name of the form `<server>:<prompt>` (exactly one `:`) is an MCP server's prompt: the core fetches it with `prompts/get`, splitting the words that follow on whitespace (a pair of English double quotes makes one word, without the quotes) and matching them to the prompt's declared arguments in order, the last argument taking the rest. The text and images that come back are sent as one message from this person, as with a template. Only in local sessions; it is switched with the server's feature `mcp-<server>`.

**Who can use them**: anyone in local sessions; in venue sessions, the owner and managers. The system setting `commands.<name>.access` can change that to `anyone` or `off` (disabled). `/workspace` is for the owner only.

The same `id` sent again counts once.

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | Doesn't start with `/`; `cwd` not absolute; wrong words after the command |
| `unknown_command` | No such command |
| `command_not_allowed` | This person can't use it, or it's disabled or off in the preset; `data.why` is a sentence for people |
| `owner_only` | Only the owner can use it |
| `command_unavailable`, `command_failed` | An extension's command: the extension isn't connected or didn't answer within a minute; it answered with an error (`data.why` is its own words) or without `said` (`data.why` in the connection's language). An MCP prompt: the server isn't running, didn't list that prompt, the session isn't local, or it didn't answer in time (the server's `timeout_ms`, at most a minute); a required argument is missing (`data.why` names them in the connection's language), or the server answered with an error (`data.why` is its own words) |
| `memory_unavailable`, `goal_unavailable` | No memory or no standing goal in this session, `data.why` |
| the command's own | `/clear`'s `turn_running`, `nothing_to_clear`; `/workspace`'s `path_unreadable`…, as for the matching method |

## `command.catalog`

**Params**: `session` (optional): only list what this connection can use in this session without being refused.

**Response**: `{"commands": [{"name", "aliases", "summary", "argument"?, "usage"?, "opens"?}…]}`, by name. Prompts of running MCP servers are listed too: named `<server>:<prompt>`, `summary` is the server's `title` (or its `description`; not per language), and `argument` is built from the declared arguments (`<name>` for required, `[name]` for optional). `argument` hints what follows the name, `usage` lists the forms of commands with several (`{"form", "summary"}`), `opens` marks commands that open a page. `summary` and `argument` are in the connection's language.

Heads add their own commands (like `/help`) to the menu themselves.

## `check`

**Params**: `cwd` (look for a project config from here), `file` (check only this one).

**Response**: `{"problems": [...]}`, one per issue: `kind` (`config`, `secrets`, `persona`, `preset`, `package`, `script`, `skill`, `slash`, `mcp`; a package's own check passes on whatever it reported, e.g. `venue`), `file`, `code`, `level`, `message`, `line` and `column` when known, for config `key`, `got`, `suggest`, `using`, and from a package's own check optionally `key`, `rule`, `source`.

Without `file` it checks everything: system config, personal settings, the project config of `cwd`, the secrets file, personas and presets in every layer, package manifests, and packages' own checks; scripts, skills and command templates that can't be read, are covered, or are links into a part of the data root that isn't recognized, scripts not registered because of a name clash, and command templates named like a built-in command; tools and prompts an MCP server listed that weren't registered; `sandbox.tools` grants whose tool is gone, comes from somewhere else or is a link, and a grant for `shell`; installed files that changed or went missing. It also returns `embedding`, the state of the semantic model, which is not a problem: `model` (`null` with `why` when it can't be used), `filling` while it backfills, `memory` and `turns` with one `{persona, have, total}` per persona, and `kb` with one `{id, name, have, total}` per knowledge base.

**Errors**: `unknown_file`: `file` isn't one of the kinds Miyu reads.
