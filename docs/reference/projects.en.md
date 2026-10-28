# Projects: `project.*`

A project groups sessions (like Projects in ChatGPT or the Claude web app): a name, a color, and the defaults for new sessions (working directory, persona, preset), plus extra instructions and a messaging switch. Each account has its own, stored in `projects.json`. It is not the project config of a code repository (`.miyu/config.toml`, `untrusted_project` in replies). Design: `docs/designs/38-项目.md`. Chinese version: [projects.md](projects.md).

| Method | What it does |
|---|---|
| [`project.list`](#projectlist) | List this account's projects |
| [`project.create`](#projectcreate) | Create one |
| [`project.update`](#projectupdate) | Change or clear fields |
| [`project.delete`](#projectdelete) | Delete one |

**Who can call**: people only; extensions get `local_only`. To follow changes, subscribe to the `projects` stream ([Connection](connection.en.md)); `projects.changed` follows ([Pushes](pushes.en.md)).

## An item

| Name | Meaning |
|---|---|
| `project` | Id, a UUIDv7 made by the core |
| `name` | 1 to 40 characters, no leading or trailing spaces |
| `description` | At most 80 characters |
| `color` | `#rrggbb` |
| `workspace` | Working directory for new sessions in the project, as people write it |
| `persona` | Persona id for new sessions; `null` is no persona; absent means the configured default |
| `preset` | Preset id for new sessions; absent means the configured default |
| `prompt` | Extra instructions, at most 8000 characters |
| `open` | `true`: sessions may message sessions outside the project; omitted when `false` |
| `created` | When it was created |

Fields that were never set are omitted.

## `project.list`

**Params**: none. **Result**: `{"projects": [<item>…]}`, oldest first.

## `project.create`

**Params**: the fields above except `project` and `created`; `name` is required; an empty string counts as not given. **Result**: `{"project": <id>}`.

## `project.update`

**Params**: `project` (required), the fields to change, and `unset`: an array of field names to clear (`description`, `color`, `workspace`, `persona`, `preset`, `prompt`). Given values replace, empty strings clear, and fields in `unset` go back to unset. **Result**: `{}`.

## `project.delete`

**Params**: `project`. **Result**: `{"moved": <sessions moved out>}`: each session in it first gets a `session.meta_changed {"project": ""}`; sessions are not deleted.

## Errors

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | `data.field` names the field: the name is empty, too long or padded with spaces; the description or instructions are too long; the color is not `#rrggbb`; the working directory is blank; `unset` lists a field that cannot be cleared or one that also has a value |
| `unknown_project` | No such project |
| `unknown_persona`, `persona_invalid`, `unknown_preset`, `preset_invalid` | The persona or preset does not exist or is broken: checked on create and update |
| `internal_error` (`-32603`) | `projects.json` cannot be read or written |
