# Personas and presets: `persona.*`, `preset.*`

A persona is "who she is": the character prompt, example dialogue, roleplay reminders, avatar, theme color. A preset is "what she can use": which features are on, which tools are off. Both are files in three stacked layers: built-in, system, and the admin's home; changes are written only to the home layer, and a same-named file there covers the ones below. Chinese version: [personas.md](personas.md).

| Method | What it does |
|---|---|
| [`persona.list`, `persona.get`](#personalist-personaget) | List personas; read one as stacked |
| [`persona.read`](#personaread) | Read one prompt's text and version |
| [`persona.set`](#personaset) | Create or change a persona |
| [`persona.delete`](#personadelete) | Delete your layer |
| [`persona.avatar`, `persona.background`](#personaavatar-personabackground) | Read the avatar, the background image |
| [`preset.list`, `preset.get`](#presetlist-presetget) | List presets; read one as stacked |
| [`preset.set`](#presetset) | Create or change a preset |
| [`preset.delete`](#presetdelete) | Delete your layer |

**Who can call**: the read methods are open to people and extensions; `persona.set`, `persona.delete`, `preset.set`, `preset.delete` are for people only, and extensions get `local_only`.

**Ids**: start with a lowercase letter; only lowercase letters, digits, `-`, `_`; at most 64 characters. Others get `bad_params`.

**Names and summaries** (`name`, `summary`) are picked in the connection's language.

## `persona.list`, `persona.get`

**`persona.list`**: no params. Response `{"personas": [...]}`, by id. An item:

| Name | Notes |
|---|---|
| `persona` | Id |
| `name`, `summary` | Name, one-line summary |
| `avatar`, `background` | Versions of the avatar and background image; `null` when none |
| `seed` | Theme color, lowercase `#rrggbb`; `null` when none |
| `seeds` | Candidate colors picked from the avatar, at most 4; `null` without an avatar |
| `background_opacity`, `background_focus` | How much the background shows (0 to 100), its focus `[x, y]`; `null` when none |
| `background_blur` | How much the background is blurred (1 to 100); `null` means no blur |

A persona whose files are wrong has only `persona` and `problem` (what's wrong, in the connection's language), plus `line` when known.

**`persona.get`**: param `persona`. Response: the above, plus:

| Name | Notes |
|---|---|
| `prompts` | `{"persona", "reminders"}`: whether the character prompt and roleplay reminders have text (booleans); read the text with `persona.read` |
| `examples` | How many example exchanges |
| `subagent` | Whether it can be picked when spawning a subagent |
| `remove` | What deleting does: `restore` (back to the built-in version), `delete` (gone), `null` (you have no layer to delete) |

**Errors**: `unknown_persona`; `persona_invalid` (its files are wrong: `data.problem`, `data.message`, `data.line`).

## `persona.read`

**Params**: `persona`; `prompt`: `persona` (character prompt), `examples` (example dialogue), `reminders` (roleplay reminders).

**Response**: `{"text", "version"}`: the stacked text, and the version of this file in your home layer (both `null` when none). Example dialogue also has `pairs: [{"user", "assistant"}…]`.

Editors edit the text and send `version` back as `expect` in `persona.set`.

## `persona.set`

**Params** (at least one besides `persona`)

| Name | Type | Notes |
|---|---|---|
| `persona` | string | Id; omitted creates a new one, and the core picks `persona-<n>` |
| `changes` | array | Changes to `persona.toml`, written like `config.set`'s `changes`; keys are `persona.name`, `persona.summary`, `persona.subagent`, `memory.scope`, `appearance.seed` |
| `prompts` | object | Prompt name (`persona`, `examples`, `reminders`) → `{"text": <whole>}` or `{"unset": true}`; examples may also be `{"pairs": [{"user", "assistant"}…]}`; each may carry `"expect": <version>` or `null` (doesn't exist yet) |
| `avatar`, `background` | object | `{"blob": <hash>}` uses the uploaded image, `{"unset": true}` removes yours; may carry `expect` |
| `seed` | string or `{"unset": true}` | Theme color `#rrggbb` |
| `background_opacity` | 0 to 100 or `{"unset": true}` | How much the background shows |
| `background_focus` | `[x, y]` or `{"unset": true}` | The background's focus, 0 to 100 each |
| `background_blur` | 1 to 100 or `{"unset": true}` | How much the background is blurred; unset means no blur |

**Response**: as `persona.get`, the stacked result. Everything is checked together and nothing is written on error. Parts equal to your layer aren't written.

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | Malformed id; nothing given; unknown prompt name; not exactly one of `text`, `unset`, `pairs`; an empty line in the example dialogue |
| `persona_invalid` | The changed files would be wrong: `data.problem`, `data.message`, `data.line` |
| `persona_conflict` | `expect` doesn't match (`data.current` is the current value or version); someone edited by hand midway (no `data.current`; re-read and retry) |
| `avatar_not_image`, `background_not_image` | Not PNG, JPEG or WebP |
| `avatar_too_big`, `background_too_big` | Avatar over 1 MiB or 1024 pixels, background over 5 MiB or 4096 pixels; `data` has the measured size and the limit |
| `unknown_attachment` | The `blob` doesn't exist |
| `local_only` | Called by an extension |

## `persona.delete`

**Params**: `persona`. **Response**: `{"remains"}`: `true` when built-in or system layers remain and it goes back to them (shown as "restore defaults"); `false` when the persona is gone. Your home copy moves to the trash for 7 days.

**Errors**: `nothing_to_delete` (you have no layer); `local_only`.

## `persona.avatar`, `persona.background`

**Params**: `persona`. **Response**: `{"avatar" or "background", "media_type", "data"}`: the version, media type, and the image as base64; all `null` when none.

## `preset.list`, `preset.get`

**`preset.list`**: no params. Response `{"presets": [...]}`, by id: `preset`, `icon` (a Lucide icon name, `null` when none), `name`, `summary`. Broken ones have only `preset`, `problem`, `line`. The shipped "Nothing" preset (`none`, step P-17) is an ordinary preset with every switch off, and it can be edited. The old fixed last item, no tools (`preset: null`), is no longer listed; `preset: null` when opening or switching still works as the old way to say it.

**`preset.get`**: param `preset`. Response:

| Name | Notes |
|---|---|
| `preset`, `name`, `summary`, `icon` | As above |
| `unlisted` | `on` or `off` after stacking: whether features it doesn't list are on |
| `features` | Each feature: `{"id", "name", "summary", "package", "on", "by", "installed", "tools"}`; `package` is the package it belongs to, for grouping; look its name up in [`package.list`](packages.en.md) (your own scripts, MCP servers and command templates say `~home`; skills outside packages and uninstalled ones have none); `by` says who decided `on`: `preset` (the preset says so), `global` (the preset says nothing and the global switch of a skill, script or MCP server does, see [`feature.set`](packages.en.md#toolset_loading-featureset); only in presets whose `unlisted` is `on`), `default` (follows `unlisted`: everything a preset with `unlisted` `off` leaves out, even when its global switch is on); `tools` are its tools, `{"name", "label", "on"}`; `label` is the name for people (an MCP server's `title` when it gives one, otherwise the tool name without `mcp__<server>__`). Ones it names that aren't installed come last with `installed: false` |
| `remove` | As in `persona.get` |

`features[].id` is for writing `features.<id>` in `preset.set`, `tools[].name` for `tools.<name>`; don't show them to people.

**Errors**: `unknown_preset`; `preset_invalid` (`data.message`, `data.line`).

## `preset.set`

**Params**

| Name | Type | Notes |
|---|---|---|
| `preset` | string | Id; omitted creates a new one, and the core picks `preset-<n>` |
| `changes` | array, required, not empty | Each `{"key", "value" | "unset": true, "expect"?}`; keys are `preset.name`, `preset.summary`, `preset.unlisted`, `features.<feature>`, `tools.<tool>` |

**Response**: as `preset.get`. An empty string as the summary means "no summary" (covering the built-in one); `unset` goes back to the built-in one.

**Errors**: `bad_params`; `preset_invalid`; `preset_conflict` (`expect` doesn't match, `data.current`); `local_only`.

## `preset.delete`

**Params**: `preset`. **Response**: `{"remains"}`, as in `persona.delete`. Open sessions keep the copy they started with.

**Errors**: `nothing_to_delete`; `local_only`.
