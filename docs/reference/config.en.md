# Config, secrets, models: `config.*`, `secret.*`, `provider.*`, `model.*`, `usage.query`

Chinese version: [config.md](config.md).

| Method | What it does |
|---|---|
| [`config.schema`](#configschema) | The settings list: each setting's name, description, type, where it can be written |
| [`config.get`](#configget) | Final values and where they come from; all current problems |
| [`config.set`](#configset) | Change or reset one or more settings, or replace a whole file |
| [`config.check`](#configcheck) | Check some text as a config layer without applying it |
| [`config.trust`](#configtrust) | Trust or reject a project config |
| [`secret.set`, `secret.delete`, `secret.list`](#secretset-secretdelete-secretlist) | Store and delete secrets; list secret names |
| [`model.list`](#modellist) | Configured providers and models, pools, the catalog in use |
| [`provider.detect`](#providerdetect) | Find what's already there: keys in the environment, local model servers |
| [`provider.catalog`](#providercatalog) | Search the provider catalog |
| [`provider.test`](#providertest) | Check whether a provider works |
| [`provider.rename`](#providerrename) | Change a provider's ID |
| [`model.call`](#modelcall) | Call a model once, outside any session |
| [`model.embed`](#modelembed) | Compute embeddings for some texts |
| [`usage.query`](#usagequery) | Query usage and cost |

## The layers

Settings stack from the bottom up: built-in defaults → system config (`system/config.toml`) → personal settings (`home/<account>/settings.toml`) → project config (`.miyu/config.toml` in a repository; only counts once trusted, and can only tighten). Which layers a setting may be written in is in the settings list (`layers`). Secrets live in `system/secrets.toml`; a setting refers to one with `{ secret = "<name>" }`, or to an environment variable with `{ env = "<variable>" }`.

**Problems**: every issue found while reading, checking or changing config is one item: `level` (`error` or `warning`), `code`, `file`, `line`, `column`, `key`, `expected`, `got`, `suggest` (the nearest key for a misspelled one), `using` (what's in use instead), `message` (a full sentence for people, in the connection's language). Fields that don't apply are omitted.

**Who can call**: the read methods (`config.schema`, `config.get`, `config.check`, `secret.list`, `model.list`, `provider.*`, `model.call`, `model.embed`, `usage.query`) are open to people and extensions. The write methods (`config.trust`, `secret.set`, `secret.delete`) are for people only; extensions get `local_only`. `config.set` lets an extension change, one key at a time, only system settings that start with its own package id, aren't secrets and aren't written as references.

## `config.schema`

**Params**: `keys` (array of strings): only these; all when omitted.

**Response**: `items` (each setting, in list order), `pages`, `groups` (pages and groups of the settings screen). An item:

| Name | Notes |
|---|---|
| `key`, `type` | Key and type: `bool`, `option`, `int`, `float`, `text`, `duration`, `path`, `url`, `name`, `reference`, `model`, `model_or`, `english`, `secret`, `list`, `table`. A segment a person names is written as a placeholder `<id>`, `<model>`, `<external>`, `<name>`, e.g. `providers.<id>.base_url` |
| `options` | Options: `value` and a display `name`; `note` (shown dimmed after the name) and `available: false` (shown greyed out) when the core can tell |
| `ticks` | How to tick a list with options: `unlisted` means ticked ones are those not in the value (`sandbox.private.remove`: ticked stays private, unticking adds it to the value; likewise `sandbox.readonly.remove`, where ticked stays protected); without it, ticked ones are those in the value |
| `min`, `max`, `max_chars`, `element` | Range, length, list element type |
| `default` | Default value |
| `layers` | Where it may be written: `system`, `personal`, `project` |
| `applies` | When it takes effect: `now`, `next_turn`, `new_session`, `restart`, `head_start` (the head's next start) |
| `name`, `description` | Display name and description, in the connection's language |
| `page`, `group`, `common`, `control` | Where it goes on the settings screen, whether it's common, which control |
| `suggestions` | `combo` controls: suggested values |
| `hidden` | Not drawn on the settings screen |
| `package` | A package's setting: which package |

**Errors**: `unknown_config_key`: `keys` has keys not in the list; `data.problems` has one item each, with the nearest key as `suggest`.

## `config.get`

**Params**

| Name | Type | Notes |
|---|---|---|
| `keys` | array of strings | Only these; all when omitted |
| `cwd` | string | Look for a project config from this directory and include it; omitted ignores project config |
| `all` | boolean, default `false` | Also list every layer that sets each key |

**Response**

| Name | Notes |
|---|---|
| `items` | key → `{value, origin}`: final value and where it comes from (`origin` has `layer`, `file`, `line`). With `all`, also `layers`: each layer top-down `{origin, value, used}`; layers not in effect carry `problem` |
| `files` | Each layer's file: `system`, `personal`, `project` (`file`, `version`; the project also has `trusted`: `true`, `false` or `null` not asked yet), `secrets` (only `file`) |
| `problems` | All current problems in these files |

```json
{"files":{"personal":{"file":"home/admin/settings.toml","version":"sha256:…"},"secrets":{"file":"system/secrets.toml"},"system":{"file":"system/config.toml","version":null}},"items":{"ui.language":{"origin":{"file":"home/admin/settings.toml","layer":"personal","line":3},"value":"en"}},"problems":[]}
```

**Errors**: `unknown_config_key`.

## `config.set`

**Params**

| Name | Type | Notes |
|---|---|---|
| `layer` | `system` or `personal`, required | Which layer |
| `changes` | array | Change some settings, see below |
| `text` | string | Replace the whole file with this text |
| `version` | string or `null` | Required with `text`: the version of the file you read; `null` means it didn't exist then |

Give exactly one of `changes` and `text`. Each change: `key` (required); one of `value` (a JSON value), `input` (what a person typed, read by the setting's type), `unset: true` (reset to default); `expect` (optional): what this setting currently is in this layer, `{"value": …}` or `{}` (not set); refused if it doesn't match.

**Response**: `{"keys", "version"}`. `keys` has each setting that changed: `value` (now in this layer; omitted when removed), `effective`, `origin` (final value and source counting the other layers), `applies`. If nothing changed, `keys` is empty and nothing is written.

```json
→ {"jsonrpc":"2.0","id":"s1","method":"config.set","params":{"layer":"personal","changes":[{"key":"ui.language","value":"en"}]}}
← {"id":"s1","jsonrpc":"2.0","result":{"keys":{"ui.language":{"applies":"now","effective":"en","origin":{"file":"home/admin/settings.toml","layer":"personal","line":3},"value":"en"}},"version":"sha256:…"}}
```

All changes are checked and written together: one bad item refuses the whole request. The response comes after the file is written and `config.changed` is pushed.

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | Not exactly one of `changes` and `text`; a change without exactly one of `value`, `input`, `unset`; the same key twice; `text` without `version` |
| `unknown_config_key` | A key not in the list, `data.problems` |
| `config_invalid` | A bad value, or not allowed in this layer; `data.problems` lists each. Where `sandbox.private.add` names your home folder or a folder above it, that problem's `code` is `private_too_broad` and `got` is that folder; for `sandbox.readonly.add` it is `readonly_too_broad` |
| `config_conflict` | `expect` doesn't match (`data.current`); `version` doesn't match or someone keeps editing by hand (`data.version`) |
| `config_file_broken` | The file can't be read right now, so single settings can't be changed; `data.problems` |
| `local_only` | An extension changed something it may not |

## `config.check`

**Params**: `layer` (`system`, `personal`, `project`, required), `text` (required). **Response**: `{"problems": [...]}`, empty when fine; problems carry no `file`.

## `config.trust`

**Params**: `cwd` (required; the project config is looked up from it), `version` (required; the version the person looked at: `config.get`'s `files.project.version`), `trust` (boolean, required).

**Response**: `{"file", "trusted"}`. Only the version the person looked at can be trusted.

**Errors**: `no_project_config`; `config_conflict` (it changed since they looked; `data.version` is the current one).

## `secret.set`, `secret.delete`, `secret.list`

| Method | Params | Response |
|---|---|---|
| `secret.set` | `name`, `value` (both required), `layer` | `{"replaced": <whether it existed>}` |
| `secret.delete` | `name`, `layer` | `{}` |
| `secret.list` | none | `{"secrets": [{"name", "set", "layer", "used_by"}…]}` |

The value is stored with surrounding whitespace removed. `secret.list` never returns values: it lists secrets that are set and those referenced by config but not set; `used_by` is the setting keys that refer to each.

There are two shelves. `system` is the shared `system/secrets.toml`, and only admins can write it. `personal` is the person's own `home/<account>/secrets.toml`, used only by their own config. Without `layer`, admins write `system` and members write `personal`. A `{ secret = name }` reference looks in the person's own shelf first, then the system one. `secret.list` lists both shelves the person can use; `layer` is `personal`, `system`, or `null` when not set.

**Errors**: `not_permitted` (a non-admin writing the system shelf); `bad_params` (malformed name; value empty, with control characters, or over 16 KiB); `unknown_secret` (deleting one that doesn't exist); `config_file_broken` (the secrets file can't be read); `local_only` (an extension calling the write ones).

## `model.list`

**Params**: `provider` (only this one), `refresh` (boolean; fetch providers' model lists first, then answer), `models` (array; only these `<provider>/<model>`).

**Response**

| Name | Notes |
|---|---|
| `providers` | Configured providers by id. Each: `id`, `layer` (`personal`: configured in the person's own settings; `system`: only in the system settings, shared), `name` (`{value, from, key}`), `driver`, `base_url`, `key` (`{ref, set, state}`, never the value), `catalog` (which catalog entry it matched), `logo` (`{svg, tint}` or `null`), `models`; unusable ones also have `problem` |
| each of `models` | `model`, `ref`, `listed` (where it was listed: `config`, `provider`, `catalog`), `facts` (window, max output, input kinds, tools, reasoning, price, name… each with `value` and source), `state` (`ok`, `cooling` with `until`/`class`, `no_key`); embedding models have `embedding: true` |
| `pools` | Each pool: `name`, `strategy`, `models`, `usable`, `subagent`, `description`, `fixed` (`true` for the three fixed pools `lite`, `normal` and `flagship`: they always exist and are listed even when the configuration does not mention them, cannot be deleted or renamed, and default to `subagent = true`) |
| `side` | Side requests (model requests outside the conversation), one entry each: `id` (`core/title` for titles, `memory/organize` for memory organizing), `tier` (which tier it asks for: `lite`, `normal` or `flagship`), `name` and `summary` (in the connection's language), and `resolved` (where it would go right now: the pool name when that tier's pool has members, `session` when it is empty and the session's model is used). When every member of the pool is cooling down or has failed, the request also falls back to the session's model |
| `uses` | The references set for `chat` and `vision`, `null` when not set |
| `catalog` | The catalog in use: `source`, `fetched`; `null` when none can be read |

**Errors**: `unknown_provider`; `bad_params` (malformed `models`).

## `provider.detect`

**Params**: none. **Response**:

| Name | Notes |
|---|---|
| `keys` | Keys set in the core's environment: `env`, `provider`, `name`, `driver`, `supported`; `configured` when already configured. Never the value |
| `local` | Model servers running on this machine: `provider`, `name`, `base_url`, `models`; `configured` when already configured |
| `looked_for` | The names of the environment variables looked at |

## `provider.catalog`

**Params**: `query` (id or name contains this), `limit` (default 50), `featured` (boolean; only the common ones, with domestic or international ones picked by the connection's language).

**Response**: `{"providers": [...]}`, each with `id`, `name`, `driver`, `base_url`, `env`, `doc`, `models` (how many), `supported`, `local` (runs on this machine, no key needed), `logo`. Usable ones first.

## `provider.test`

**Params**: `provider` (a configured one) or `candidate` (not yet in the config: `driver`, `base_url`, `key` (`{secret}`, `{env}` or `{value}`; `value` is used once and not stored), `catalog`), exactly one; `model` (which model to try).

Being able to list models counts as working, with no request sent; only when listing fails or the given model isn't listed is one message sent, which costs a little.

**Response**: success `{"ok": true, "checked", "models", "listed", "model"}`, plus `first_token_ms` when a message was sent; failure `{"ok": false, "stage", "error": {"class", "status"?, "message"}}`, `stage` being `list`, `request` or `config`; `need_model: true` when naming a model would allow another try.

```json
{"ok":true,"checked":"list","models":["deepseek-flash","deepseek-v4-pro"],"listed":"provider","model":"deepseek-flash"}
{"ok":false,"stage":"request","error":{"class":"auth","status":401,"message":"Authentication Fails (no such user)"}}
```

**Errors**: `bad_params`; `unknown_provider`.

## `provider.rename`

Changes a provider's ID in one go: in both config layers the `[providers.<from>]` table (with its subtables and key reference) becomes `<to>`, and every `<from>/<model>` in `models.chat`, `models.vision`, `models.embedding` and pool members becomes `<to>/<model>`. Sessions pinned to the old ID use the new one from their next turn. People only.

**Params**: `from`, `to`. `to` follows the ID rule: starts with a lowercase letter; lowercase letters, digits, `-`, `_`; at most 32.

**Response**: `{}`; each changed layer pushes [`config.changed`](pushes.md).

**Errors**: `bad_params` (`to` malformed or equal to `from`); `unknown_provider` (no `from`); `provider_exists` (`to` already exists). If the second layer can't be written, the first stays changed and that layer's error is returned (`config_conflict`, `config_file_broken`); calling again finishes the rest.

## `model.call`

Call a model once and get the whole answer and usage. Not part of any session and nothing is pushed; it costs money and is counted in usage.

**Params**

| Name | Type | Notes |
|---|---|---|
| `purpose` | string, required | What it's for: 1 to 32 lowercase letters, digits, `-`, e.g. `vision` |
| `messages` | array, required | `{"role": "system"|"user"|"assistant", "text", "images"?}`. At most one `system`, first only; the last one is `user`; only `user` may carry `images` (blob hashes: looked up first under this connection's own account, where `blob.put` stores them, then under the account it works for; missing from both gives `unknown_attachment`) |
| `model` | string | `<provider>/<model>` or `@<pool>`; omitted follows `models.chat`. When the same provider is configured in both the system and the personal settings, `<id>/<model>` is the person's own copy and `<id>@system/<model>` is the shared one (with the shared key) |
| `use` | `chat` or `vision` | When you don't know which model the person configured: follows `models.chat` or `models.vision`; at most one of `model` and `use` |
| `max_tokens` | positive integer | Most output tokens |

**Response**: `{"text", "provider", "model", "usage"}`. `usage` has `uncached`, `cache_read`, `cache_write`, `output`, plus `reasoning` when reported; unreported ones are `null`. Answered in the background, so other requests on the connection aren't blocked.

```json
{"text":"A cat on a red sofa.","provider":"deepseek","model":"deepseek-flash","usage":{"uncached":812,"cache_read":0,"cache_write":0,"output":9}}
```

**Errors**

| `data.reason` | When |
|---|---|
| `bad_params` (`-32602`) | Malformed params; an image blob that isn't an image |
| `unknown_attachment` | An image blob this account doesn't have |
| `unknown_model` | `model` can't be resolved |
| `no_model` | No usable model (not configured, key unavailable), `data.message` |
| `cooling` | Every candidate is cooling down, `data.message`, `data.wait_ms` |
| `model_failed` | Sent and failed: `data.class`, `data.status`, `data.message` |

## `model.embed`

**Params**: `texts`: 1 to 64 texts, 1 to 8000 characters each. Uses the account's `models.embedding` (the built-in local model or a remote provider).

**Response**: `{"model", "vectors"}`. `model` is the model id (`local:<id>` or `<provider>/<model>`): vectors from different models can't be compared. `vectors` are in order and normalized.

**Errors**: `bad_params`; `embedding_off` (off, or local requested but no built-in model installed); `embedding_failed` (one couldn't be computed or timed out, `data.why`).

## `usage.query`

**Params**

| Name | Type | Notes |
|---|---|---|
| `from`, `until` | time | Only this range, start inclusive, end exclusive |
| `group` | array | Group by: `account`, `venue`, `model`, `day`, `session`, `purpose`, `use` (what for: `main` conversation, `compaction`, `subagent`, `recap`, `title`, `memory`, `other`); one total row when omitted |
| `session` | string | Only this session; `tree: true` includes the child sessions it spawned |
| `offset` | time zone, e.g. `+09:00` | Which time zone splits days; the core machine's when omitted |

**Response**: `{"rows": [...]}`. A row: the grouped fields, `requests`, `usage` (the four counts added up), `amounts` (per currency, as recorded), `unpriced` (requests with usage but no known price), `converted` (one amount in `usage.currency`: each charge converted at its own day's rate, `{"currency","amount"}`; charges without a rate go in its `rest`, written like `amounts`; left out when nothing could be converted). Everyone's sessions are counted; non-admins only see their own. Amounts are ordered by and converted to `usage.currency` from the caller's own configuration. When rates from open.er-api.com were used, the reply also carries `rates` (source and link), and the UI must credit it.

```json
{"rows":[{"purpose":"vision","model":"deepseek/deepseek-flash","requests":2,"usage":{"uncached":1624,"cache_read":0,"cache_write":0,"output":18},"amounts":[{"currency":"USD","amount":0.0002544}],"unpriced":0}]}
```

**Errors**: `bad_params`.
