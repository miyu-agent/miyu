# Venues: sessions on chat platforms

A venue is where a session happens: this machine (`local`), or a private chat or group on a chat platform. A chat platform bridge (such as the QQ bridge) is an extension that declares a system account: group sessions belong to that system account, and the owner's private chats belong to the owner. This page lists the methods bridges use. Chinese version: [venues.md](venues.md).

| Method | What it does |
|---|---|
| [`venue.session`](#venuesession) | Find or create a venue's main session |
| [`venue.sessions`](#venuesessions) | List all of this bridge's venue sessions |
| [`venue.binding`](#venuebinding) | Ask which local account a platform identity belongs to |
| [`venue.plugins`](#venueplugins) | Say which venue plugins a venue has on |
| [`as` and `venue` in `session.send`](#as-and-venue-in-sessionsend) | Speak for a person on the platform |
| [`session.respond`](#sessionrespond) | Start a turn from messages recorded while listening |
| [`session.note`](#sessionnote) | Record a few facts without starting a turn |
| [`venue.records`](#venuerecords) | Group chat records for the judge |
| [`venue.*` in `events.append`](#venue-in-eventsappend) | Record recalls and messages she delivered |

**Who can call**: `venue.session`, `venue.sessions`, `venue.binding` and `venue.plugins` are for system account connections (extensions of packages that declare a system account) and per-account copies of a package (`[process] scope = "account"`); others get `no_system_account`. For a per-account copy, venue sessions always belong to the person it runs for (even a private chat whose peer is in the owner's binding table); the owner's binding table only counts in the owner's own copy, so a member's copy gets `null` from `venue.binding` and `as` in sessions under the member is an external identity without an account; `venue.sessions` lists only that person's sessions on the package's platform. Speaking for people on a platform (`session.send` and `command.run` with `as`), `session.note`, `venue.*` and recording `venue.*` events all require the `act_for_external` capability.

**Venue ids**: short names the bridge chooses (1 to 128 bytes, no control characters), e.g. `qq:private:10001`, `qq:group:123456`. The core doesn't interpret them beyond the platform prefix before the first `:`.

**Owner bindings**: the system setting `[external.bindings]`, platform identity → local account:

```toml
[external.bindings]
"qq:10001" = "admin"
```

In a private chat this identity is that account's person; in a group it's the owner.

## `venue.session`

**Params**

| Name | Type | Notes |
|---|---|---|
| `venue` | string, required | Venue id |
| `kind` | `private` or `group`, required | Private chat or group |
| `peer` | string | The other side of a private chat; required for `private`, not allowed for `group` |
| `persona` | string or `null` | Persona when creating (the bridge works it out from its rules); `null` means none |
| `preset` | string | Preset when creating |
| `profile` | boolean | Whether to include the owner's "about you" when creating: `true` only for the owner's own private chat |
| `images` | boolean, default `true` | Whether this venue can display images |
| `titles` | boolean, default `true` | Whether this session gets automatic titles: with `false` it never does; renaming by hand still works. Ignored when an existing session is found |
| `cwd` | string | Working directory when creating; the owner's default workspace when omitted |
| `recent` | integer, 1 to 1000 | Groups only: at most this many recent group messages (newest first) go in front of a turn; the byte budget still caps it, the smaller wins; omitted means only the byte budget. Fixed when created |

The creation fields are ignored when an existing session is found.

**Response**: `{"session", "created", "account"}`: session id, whether it was just created, the owner. If the private chat's peer is in the bindings, the owner is that local account; otherwise the bridge's system account (an `account` equal to the bridge's own means a stranger's private chat).

Nobody can approve anything in venue sessions: everything needing approval is refused. The same `id` sent again returns the same session.

**Errors**: `no_system_account`; `unknown_persona`, `unknown_preset`, `preset_invalid`; `bad_params`.

## `venue.sessions`

**Params**: none. **Response**: `{"sessions": [{"session", "venue"}…]}`, newest first: the venue sessions owned by this system account, and the owner's private chats with this platform's identities in the bindings (the platform is read from the manifest's `[connection] platform`). Only the newest session per venue. Bridges subscribe to these on start.

## `venue.binding`

**Params**: `id`: a platform identity. **Response**: `{"account": <local account>}`, `null` when not bound. Answered from the config at that moment. Bridges use it when guarding owner-only actions: check the target before acting on it.

## `venue.plugins`

**Params**: `venue`: the venue; `on`: the names of the venue plugins it has on (as in `provide`'s `plugin`); unset or empty means none. **Response**: `{}`.

Replaces the whole list. Tools with a `plugin` are only given in this venue's sessions while it is on, from the next turn; a call this turn to one just turned off gets "not available right now". The core keeps this in memory only, so after a restart no venue has any: send it once per venue after connecting. Only for venues on your own platform (from the manifest's `[connection] platform`): other platforms get `bad_params` (`data.field` is `venue`), badly written names get `bad_params` (`data.field` is `on`).

## `as` and `venue` in `session.send`

Venue sessions only accept messages with `as` (otherwise `venue_session`); `as` in a local session is `bad_params`.

**`as`**: `{"external": <platform identity>, "role": "manager" | "member"}`. `role` defaults to `member`; slash commands check it. The owner isn't recognized by `role` but by the bindings.

**`venue`**: the fields of one platform message, recorded as-is:

| Name | Notes |
|---|---|
| `msg` | The platform message id, required |
| `reply_to` | The id of the message it quotes |
| `name` | What the sender is called here right now (group card, else nickname), at most 64 characters |
| `my_name` | What she is called here right now (group card, else nickname; in a private chat, the nickname), at most 64 characters. Memory treats it like her persona's name: things about this name are kept as her own, not as someone she knows |
| `place` | What the group is called right now (its name), at most 64 characters; not written for private chats. Only memory uses it: memories saved in the group carry it, nothing she reads shows it |
| `mentions` | Who was @-mentioned (platform identities) |
| `mentions_me`, `mentions_all` | Mentioned her; mentioned everyone |
| `media` | What it carries `[{"kind", "id", "name"?, "size"?, "shown"?}]`: `kind` is `image`, `file`, `voice`, `video`, `sticker`; `id` is the platform's id (not downloaded); `shown` means the original was already passed in as an attachment |
| `forwards` | Merged forwards: `[{"nodes": [{"name", "text", "media"?, "forwards"?}], "more"?}]`, nested at most 3 deep; at most 100 nodes and 16 KiB of names and text per message (over that, `bad_params` with `data.limit` of `forward_nodes` or `forward_bytes`). The text must contain a placeholder (e.g. `[forward]`) |
| `ambient` | Listening only: recorded, doesn't start a turn |
| `asleep` | Received while asleep |
| `backfill` | Fetched back after the bridge was disconnected: kept in the chat record only, left out of the recent chat and the judge's records; requires `ambient` |
| `sent` | The time the platform gives, written like an event's `at` (`2026-10-20T04:30:00.000Z`); `history` shows it. The log keeps the order things were recorded in, sequence numbers don't move back |
| `show_ids` | Whether to show the sender's platform identity when rendering it |

**Recorded as**: the owner recognized in a private chat is `{"kind": "person", "account", "via"}`; anyone else is `{"kind": "external", "venue", "id", "role"}`, plus `account` for the owner in a group.

**Command ids**: bridges build `id` from the platform, the logged-in account, the message id and the platform's timestamp, so reconnect resends and platform resends take effect only once.

## `session.respond`

Start a turn from messages recorded while listening (when it's decided she should answer in a group).

**Params**: `session`; `to`: the numbers of those `message.user` events, 1 to 64; `facts`: optional facts `[{"kind", "text"}]` (`text` at most 4 KiB), placed before the trigger.

**Response**: `{"events": [...]}`. If a turn is running, they join it and she hears them at the next step.

**Errors**: `not_ambient` (`to` has messages that weren't recorded while listening, `data.messages`); `already_answered` (already used as a trigger, `data.messages`); `bad_params`.

## `session.note`

Record a few facts without starting a turn, interrupting or waking her (e.g. her message couldn't be delivered and she should know at the next step).

**Params**: `session`; `facts`: 1 to 16 blocks `{"kind", "text"}`. **Response**: `{"events": [...]}`. During a turn they belong to that turn and she sees them at the next request; when idle, at the start of the next turn.

## `venue.records`

Group chat records for the judge (the small request that decides whether to answer a group message), in the same line format she sees.

**Params**: `session`; `msg`: the number of the message being judged (a `message.user` with `venue`); `count`: 1 to 100; `grow`: optional step for aligning the start, at least 1, with `count + grow` at most 100. With `grow`, the start sits on a multiple of it (`(earlier messages − count) / grow × grow`, from the beginning when below 0), so between `count` and `count + grow − 1` messages come back: the start stays put for a while, the judge's request prefix stays stable and the cache keeps hitting.

**Response**: `{"records": "<earlier messages, one per line>", "current": "<that message>", "count": <how many earlier messages>}`. Her own replies are `[you]` lines; recalled ones are marked. When `current` quotes another message, its sender and content follow the number (`reply-to: msg=8805 小林: @Miyu 在吗`); if that message is not in the log, only the number is written.

**Errors**: `bad_params`; `session_not_found`.

## `venue.*` in `events.append`

Besides its own `ext.<package>.*` (see [Extensions](extensions.en.md#eventsappend)), a bridge can record two kinds in venue sessions:

| `kind` | `body` | Meaning |
|---|---|---|
| `venue.recalled` | `{"msg", "by"}` | Someone recalled a message: platform id, who recalled it |
| `venue.delivered` | `{"line", "turn", "to", "msg", "text", "images"?}` | Her message reached the platform: which session, which turn, to whom (platform identities), platform id, text, image hashes |

Only accepted in venue sessions; malformed ones are `bad_params`. In the group chat she sees later, recalled messages are marked and `[you]` lines are what she said.
