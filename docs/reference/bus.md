# 总线：`bus.*`

用户态总线：包在清单 `[export]` 里声明 `provides`（一个名字一家）、`plugs`（可以多家），头和别的包照名字找、照名字调，不写死包名。核心自己的方法（会话、工具、驱动这些）不上总线。查谁提供了什么、可不可用：`miyu bus list`。英文版：[bus.en.md](bus.en.md)。

| 方法 | 做什么 |
|---|---|
| `bus.call` | 照名字调一个方法 |
| `bus.list` | 总线上的名字，谁提供、可不可用 |

## `bus.call`、`bus.list`

**`bus.call`**：参数 `name`（名字）、`method`（短名，一段、不带点；核心拼成 `<名字>.<短名>`）、`params`（可不写）、`package`（`plugs` 的名字要写挑哪家；`provides` 的写了要正是提供它的那家）。照 `package.call` 转过去，回什么交回什么；在后台答。参数里的 blob 交给被调的一方（扩展只交得出它自己拿得到的），回应里的交给调的一方。扩展调要声明 `packages.call`；扩展调别的包只有这一条路，`package.call` 照包编号它只调得到自己。

**`bus.list`**：参数 `name`（可不写）。回应 `{"names": [{"name", "kind", "package", "ready", "methods"}…]}`，照名字、包排：`kind` 是 `provides` 或 `plugs`，`ready` 是程序连着、这个名字底下导出的方法都登记了（头照它画入口），`methods` 是这一家在这个名字底下导出了、这时也登记上了的方法的短名（`bus.call` 的 `method` 写的那一段），照字母排，程序没连着的是空的：接进同一个名字的几家各实现了哪几处照它认。写了 `name` 的只列这一个。要跟着变，订阅 `bus` 流（[连接](connection.md)），推 `bus.changed`（[推送](pushes.md)）。

**出错**：`no_provider`（没有包提供这个名字，`data.name`）、`package_required`（`plugs` 的名字没写挑哪家，`data.name`）、`bad_params`（短名带点、是空的）、`not_exported`（拼出来的方法没写进那家的 `[export]`），别的同 `package.call`。

## 公认的名字

下面这些名字的形状是定好的：提供它们的包照这里答，调的一方照这里调。参数、回应都是 JSON 对象，`?` 是可以不写的；blob 写编号（`blob.put` 回的 `blob`）。出错照 `method.call` 的错误回，`code` 是下面列的字符串，核心转成 `method_failed`，`data.code` 带着它。不认识的字段报 `bad_params`。名字没有登记处，谁先装谁占着；没列在这里的名字，形状看提供方的文档。

| 名字 | 种类 | 方法 | 官方的提供方 |
|---|---|---|---|
| `stt` | provides | `transcribe`、`record`、`stop` | `voice-input` |
| `tts` | provides | `synthesize`、`play`、`stop` | `say` |
| `stt.engine` | plugs | `transcribe` | 社区的识别引擎 |
| `tts.engine` | plugs | `synthesize`、`voices`（可选） | 社区的合成引擎 |

`ready`（`bus.list`）照登记算：`voice-input` 模型下好、校验过才登记 `stt` 的三个方法；`say` 选的引擎配好了才登记 `tts` 的三个方法。头查到 `ready` 再画麦克风、朗读按钮。整句进、整句出，不做流式：要首音快，调的一方自己按句切开、一句一句调。

### `stt.transcribe`

一段音频转成字。时限 120 秒。

```json
→ {"audio": "<blob>", "language"?: "zh"}
← {"text": "今天下午三点开会", "language": "zh", "duration_ms": 2380, "clear": true}
```

- `audio`：一定收 16 kHz、单声道、16 位 PCM 的 wav；别的采样率、声道的 wav 也收，ogg/opus 可以不认。最长 10 分钟。
- `language`：`zh`、`yue`、`en`、`ja`、`ko`、`auto`；不写照包的配置。
- `clear`：短于 0.5 秒、有效字少于 2 个的是 `false`，`text` 是空的，不算出错。
- 出错：`bad_audio`、`too_long`、`silent_audio`（整段是数字静音：峰值不超过 2/32768，麦克风静音、没插上或系统默认输入没声音；头照它说「麦克风没有声音」）、`engine_failed`（选的社区引擎答错了，`message` 照它的）。

### `stt.record`

录本机的麦克风，录完转成字。时限 310 秒。

```json
→ {"language"?: "zh", "max_ms"?: 60000, "silence_ms"?: 1200}
← {"text": "…", "language": "zh", "duration_ms": 4100, "clear": true, "ended": "silence"}
```

- 人声停了 `silence_ms`（默认 1200，300 到 5000）就停；最长 `max_ms`（默认 60000，最多 300000）；`stt.stop` 也能停。`ended` 是 `silence`、`stop`、`max`。
- 整台机器一把麦克风锁：别的头、命令行已经在录的，回 `mic_busy`。
- 出错：`mic_busy`、`no_microphone`、`silent_audio`（同 `stt.transcribe`）、`engine_failed`。

### `stt.stop`

停正在录的那一个，不分是谁开的。

```json
→ {}
← {"stopped": true}
```

没在录的回 `{"stopped": false}`，不算出错。

### `tts.synthesize`

一段 Markdown 合成一段音频。时限 60 秒。

```json
→ {"text": "**好的**，三点见。", "persona"?: "<人格编号>", "format"?: "mp3"}
← {"audio": "<blob>", "media_type": "audio/mpeg", "duration_ms": 1650}
```

- `text`：Markdown，提供方先转成念得出来的话；最多 5000 字。
- `persona`：照它挑音色；不写或者没给这个人格配，用默认音色。
- `format`：`mp3`（默认）、`wav`。
- 出错：`empty_text`（转完没有能念的）、`too_long`、`engine_failed`、`not_configured`。

### `tts.play`

从本机的音箱放出来。时限 600 秒。

```json
→ {"audio": "<blob>"}   或   {"text": "…", "persona"?: "<人格编号>"}
← {"finished": true}
```

- `audio`、`text` 二选一；给 `text` 的先照 `synthesize` 合成。
- 同时来几个照先后排队放，不互相打断；被 `tts.stop` 停的回 `{"finished": false}`。
- 出错：`bad_audio`、`no_speaker`，和 `synthesize` 的那几个。

### `tts.stop`

正在放的、排着队的全停。

```json
→ {}
← {"stopped": 2}
```

`stopped` 是停掉的条数，没有在放的是 0。

### `stt.engine.transcribe`

社区识别引擎接的，`stt` 的提供方照配置里选的那一家调。

```json
→ {"audio": "<blob>", "language": "zh"}
← {"text": "…", "language": "zh"}
```

交过去的 `audio` 一定是 16 kHz、单声道、16 位 PCM 的 wav，`language` 一定写了。过滤、`clear`、切段都由 `stt` 的提供方做，引擎只管转。出错照引擎自己的，提供方包成 `engine_failed`。

### `tts.engine.synthesize`

社区合成引擎接的。

```json
→ {"text": "好的，三点见。", "voice": "<音色编号>", "speed"?: 1.0, "format": "mp3"}
← {"audio": "<blob>", "media_type": "audio/mpeg", "duration_ms"?: 1650}
```

- `text` 已经是念得出来的纯文字，一次一句、最多 500 字；`voice` 是引擎自己的音色编号；`speed` 0.5 到 2.0。
- 出不了 `format` 那种的，交出得了的，`media_type` 写实。

### `tts.engine.voices`

可选：这个引擎有哪些音色。没登记的，配置页让人手填音色编号。

```json
→ {}
← {"voices": [{"id": "female-tianmei", "name": "甜美女声", "language"?: "zh"}]}
```
