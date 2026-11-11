# The bus: `bus.*`

The user-space bus (design `67-总线.md`): packages declare `provides` (one provider per name) and `plugs` (several allowed) under `[export]` in their manifest, and heads and other packages find and call them by name instead of by package id. The core's own methods (sessions, tools, drivers and so on) are not on the bus. To see who provides what and whether it's ready: `miyu bus list`. Chinese version: [bus.md](bus.md).

| Method | What it does |
|---|---|
| `bus.call` | Call a method by name |
| `bus.list` | The names on the bus: who provides them and whether they're ready |

## `bus.call`, `bus.list`

**`bus.call`**: params `name`, `method` (the short name: one part, no dots; the core joins it as `<name>.<method>`), `params` (optional) and `package` (required for a `plugs` name to pick one; for a `provides` name it must be the provider if given). Forwarded like `package.call`, answered with whatever it returns; answered in the background. Blobs in `params` are passed to the called package (an extension can only pass ones it can read itself), blobs in the answer to the caller. Extensions must declare `packages.call`; this is the only way an extension reaches another package, since `package.call` by package id reaches only itself.

**`bus.list`**: param `name` (optional). Response `{"names": [{"name", "kind", "package", "ready", "methods"}…]}`, by name then package: `kind` is `provides` or `plugs`; `ready` means the program is connected and has registered every exported method under that name (heads draw entries from it); `methods` lists the short names (what `bus.call` takes as `method`) of the methods this package exports under the name and has registered right now, sorted, empty while its program is not connected: when several packages plug into one name, callers read from it which parts each one implements. With `name`, only that one. To follow changes, subscribe to the `bus` stream ([Connection](connection.en.md)), which pushes `bus.changed` ([Pushes](pushes.en.md)).

**Errors**: `no_provider` (no package provides the name, `data.name`), `package_required` (a `plugs` name without `package`, `data.name`), `bad_params` (an empty short name or one with dots), `not_exported` (the joined method isn't in that package's `[export]`); the rest as `package.call`.

## Well-known names

The shapes of these names are fixed: packages that provide them answer this way, and callers call this way. Params and answers are JSON objects; `?` marks optional fields; blobs are written as ids (the `blob` that `blob.put` returns). Errors come back as `method.call` errors with `code` set to one of the strings listed; the core turns them into `method_failed` and carries the string in `data.code`. Unknown fields get `bad_params`. There is no registry: whoever installs first holds a name. For names not listed here, see the provider's docs.

| Name | Kind | Methods | Official provider |
|---|---|---|---|
| `stt` | provides | `transcribe`, `record`, `stop` | `miyu-voice-input` |
| `tts` | provides | `synthesize`, `play`, `stop` | `miyu-say` |
| `stt.engine` | plugs | `transcribe` | Community speech-recognition engines |
| `tts.engine` | plugs | `synthesize`, `voices` (optional) | Community speech-synthesis engines |

`ready` (`bus.list`) follows registration: `miyu-voice-input` registers the three `stt` methods only once its model is downloaded and verified; `miyu-say` registers the three `tts` methods only once the chosen engine is configured. Heads draw the microphone and read-aloud buttons only when the name is `ready`. One whole sentence in, one whole sentence out, no streaming: for a fast first sound, the caller splits text into sentences and calls once per sentence.

### `stt.transcribe`

Turns a piece of audio into text. Timeout 120 seconds.

```json
→ {"audio": "<blob>", "language"?: "zh"}
← {"text": "Meeting at three this afternoon", "language": "en", "duration_ms": 2380, "clear": true}
```

- `audio`: 16 kHz mono 16-bit PCM wav is always accepted; wav at other rates or channel counts is accepted too; ogg/opus may be refused. At most 10 minutes.
- `language`: `zh`, `yue`, `en`, `ja`, `ko` or `auto`; omitted means the package's setting.
- `clear`: `false` for audio shorter than 0.5 seconds or with fewer than 2 meaningful characters; `text` is then empty, and it isn't an error.
- Errors: `bad_audio`, `too_long`, `engine_failed` (the chosen community engine failed; `message` is its own).

### `stt.record`

Records from this computer's microphone and transcribes it. Timeout 310 seconds.

```json
→ {"language"?: "zh", "max_ms"?: 60000, "silence_ms"?: 1200}
← {"text": "…", "language": "en", "duration_ms": 4100, "clear": true, "ended": "silence"}
```

- Stops after `silence_ms` of silence (default 1200, 300 to 5000), after `max_ms` (default 60000, at most 300000), or on `stt.stop`. `ended` is `silence`, `stop` or `max`.
- The machine has one microphone lock: if another head or the command line is already recording, the answer is `mic_busy`.
- Errors: `mic_busy`, `no_microphone`, `engine_failed`.

### `stt.stop`

Stops the recording in progress, whoever started it.

```json
→ {}
← {"stopped": true}
```

If nothing is recording, the answer is `{"stopped": false}`, not an error.

### `tts.synthesize`

Turns Markdown into audio. Timeout 60 seconds.

```json
→ {"text": "**Sure**, see you at three.", "persona"?: "<persona id>", "format"?: "mp3"}
← {"audio": "<blob>", "media_type": "audio/mpeg", "duration_ms": 1650}
```

- `text`: Markdown; the provider first turns it into speakable text. At most 5000 characters.
- `persona`: picks the voice; omitted or unconfigured means the default voice.
- `format`: `mp3` (default) or `wav`.
- Errors: `empty_text` (nothing speakable left), `too_long`, `engine_failed`, `not_configured`.

### `tts.play`

Plays through this computer's speakers. Timeout 600 seconds.

```json
→ {"audio": "<blob>"}   or   {"text": "…", "persona"?: "<persona id>"}
← {"finished": true}
```

- Either `audio` or `text`; `text` is synthesized first as with `synthesize`.
- Several at once are queued in order and don't interrupt each other; ones stopped by `tts.stop` answer `{"finished": false}`.
- Errors: `bad_audio`, `no_speaker`, and those of `synthesize`.

### `tts.stop`

Stops everything playing or queued.

```json
→ {}
← {"stopped": 2}
```

`stopped` is how many were stopped; 0 if nothing was playing.

### `stt.engine.transcribe`

Implemented by community speech-recognition engines; the `stt` provider calls the one chosen in its settings.

```json
→ {"audio": "<blob>", "language": "zh"}
← {"text": "…", "language": "zh"}
```

The `audio` passed is always 16 kHz mono 16-bit PCM wav, and `language` is always set. Filtering, `clear` and segmenting are the `stt` provider's job; the engine only transcribes. Engine errors are wrapped by the provider as `engine_failed`.

### `tts.engine.synthesize`

Implemented by community speech-synthesis engines.

```json
→ {"text": "Sure, see you at three.", "voice": "<voice id>", "speed"?: 1.0, "format": "mp3"}
← {"audio": "<blob>", "media_type": "audio/mpeg", "duration_ms"?: 1650}
```

- `text` is already plain speakable text, one sentence of at most 500 characters; `voice` is the engine's own voice id; `speed` is 0.5 to 2.0.
- An engine that can't produce `format` returns what it can, with the true `media_type`.

### `tts.engine.voices`

Optional: the engine's voices. Without it, the settings page lets the person type a voice id.

```json
→ {}
← {"voices": [{"id": "female-tianmei", "name": "Sweet female voice", "language"?: "zh"}]}
```
