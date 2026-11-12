# A head's own settings: `prefs.*`

Settings of a UI package (one whose manifest has `[ui]`: heads such as the terminal and the web UI): skin, color scheme, theme, mascot and the like. They live in the core's data root and follow the person, not the address or the device: opening the same web UI via `127.0.0.1`, `localhost`, a LAN address or a phone shows the same settings. The core doesn't interpret the keys; the head decides keys and values. Anything that belongs to one device (column widths, a collapsed sidebar, input history) stays with the head. Chinese version: [prefs.md](prefs.md).

| Method | What it does |
|---|---|
| [`prefs.get`](#prefsget) | Read the whole object |
| [`prefs.set`](#prefsset) | Change some keys, get the whole result back |

**Who can call**: people only (head connections); extensions and platform bridges get `local_only`. To follow changes made by other heads, subscribe to the `prefs` stream (with `package`); `prefs.changed` follows ([Pushes](pushes.en.md)).

**Where it is stored**: `prefs.json` in that package's state directory (`var/lib/<package id>/prefs.json`, the directory `MIYU_STATE_DIR` points to). The whole file is one JSON object on one line, readable only by you on Unix. The core owns this name: a head shouldn't use it for its own files in the state directory. Removing the package removes the settings along with the state directory.

**Limit**: the whole object serialized as JSON (without whitespace) is at most 64 KiB (65536 bytes).

## `prefs.get`

**Params**: `{"package"}`, the UI package's id. **Response**: `{"values": {…}}`, the whole object; `{}` if nothing was set yet.

## `prefs.set`

**Params**: `{"package", "values": {key: value or null}}`. Merged by top-level key: a key you write is replaced as a whole by the new value (any JSON value; an object replaces the old one, it is not merged into it), a key written as `null` is deleted, keys you leave out stay. `values: {}` changes nothing.

**Response**: `{"values": {…}}`, the whole merged object. A change is pushed to the other connections subscribed to this package, not to yours: the response already has the whole object.

## Subscribing

`subscribe {"stream": "prefs", "package"}`: the response is `{"values": {…}}`, the whole object at that moment; after that, changes made by other connections push `prefs.changed {"package", "values"}`, where `values` is the whole new object: replace yours with it. One per package per connection; subscribing again replaces it. `unsubscribe {"stream": "prefs", "package"}` stops it. If you read too slowly you get `resync {"stream": "prefs", "package"}`; subscribe again.

## Errors

| Reason | When |
|---|---|
| `bad_params` | No `package`; `values` missing or not an object; subscribing without `package` |
| `unknown_package` | No such package installed |
| `not_a_ui_package` | Installed, but not a UI package |
| `prefs_too_big` | The merged object is over 64 KiB; nothing changed. `data` has `size` (bytes of the merged object) and `limit` (65536) |
| `local_only` | Called by an extension or a platform bridge |

```json
→ {"jsonrpc":"2.0","id":"1","method":"prefs.set","params":{"package":"webui","values":{"skin":"paper","mascot":null}}}
← {"jsonrpc":"2.0","id":"1","result":{"values":{"scheme":"dark","skin":"paper"}}}
```
