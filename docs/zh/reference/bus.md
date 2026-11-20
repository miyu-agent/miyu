# 总线：`bus.*`

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

用户态总线让软件包之间按名字互相调用：软件包在清单的 `[export]` 里声明 `provides`（一个名字只由一个软件包提供）或 `plugs`（一个名字可以由多个软件包接入），头（连接核心的界面程序，例如终端界面、网页界面）和其他软件包按名字查找、按名字调用，不需要写死软件包的名字。核心自己的方法（会话、工具、驱动等）不放在总线上。想查看谁提供了什么、是否可用，可以运行 `miyu bus list`。

| 方法 | 做什么 |
|---|---|
| `bus.call` | 按名字调用一个方法 |
| `bus.list` | 列出总线上的名字：由谁提供、是否可用 |

## `bus.call`、`bus.list`

**`bus.call`**：参数是 `name`（名字）、`method`（短名，只有一段，不带点；核心把它拼成 `<名字>.<短名>`）、`params`（可以不填写）、`package`（`plugs` 的名字需要用它选择由哪一个软件包处理；`provides` 的名字如果填写了，它必须正好是提供这个名字的软件包）。核心按 `package.call` 的方式把调用转发过去，对方返回什么，这个方法就返回什么；它在后台应答。参数里的 blob 交给被调用的一方（扩展只能交出它自己拿得到的 blob），回应里的 blob 交给调用的一方。扩展调用这个方法需要声明 `packages.call`；这是扩展调用其他软件包的唯一途径，因为 `package.call` 按软件包编号调用，扩展通过它只能调用到自己。

**`bus.list`**：参数是 `name`（可以不填写）。回应是 `{"names": [{"name", "kind", "package", "ready", "methods"}…]}`，先按名字、再按软件包排序：`kind` 是 `provides` 或 `plugs`；`ready` 表示程序已经连接，并且这个名字下导出的方法都已经登记（头根据它决定是否画出入口）；`methods` 是这个软件包在这个名字下导出、并且此刻已经登记的方法的短名（也就是 `bus.call` 的 `method` 填写的那一段），按字母排序，程序没有连接时为空：几个软件包接入同一个名字时，可以根据它识别每个软件包各自实现了哪几个方法。填写了 `name` 时只列出这一个名字。想要一直跟着变化，可以订阅 `bus` 流（[连接](connection.md)），核心会推送 `bus.changed`（[推送](pushes.md)）。

**出错**：`no_provider`（没有软件包提供这个名字，附带 `data.name`）、`package_required`（`plugs` 的名字没有填写选择哪一个软件包，附带 `data.name`）、`bad_params`（短名带点，或者是空的）、`not_exported`（拼出来的方法没有写进那个软件包的 `[export]`），其他错误和 `package.call` 相同。

## 公认的名字

下面这些名字的形状是固定的：提供它们的软件包按这里写的方式应答，调用的一方按这里写的方式调用。参数和回应都是 JSON 对象，带 `?` 的字段可以不填写；blob 填写编号（也就是 `blob.put` 返回的 `blob`）。出错时按 `method.call` 的错误返回，`code` 是下面列出的字符串，核心把它转成 `method_failed`，并在 `data.code` 里带上这个字符串。遇到不识别的字段时报 `bad_params`。名字没有登记处，谁先安装谁就占用这个名字；没有列在这里的名字，形状要看提供方的文档。

| 名字 | 种类 | 方法 | 官方的提供方 |
|---|---|---|---|
| `stt` | provides | `transcribe`、`record`、`stop` | `voice-input` |
| `tts` | provides | `synthesize`、`play`、`stop` | `say` |
| `stt.engine` | plugs | `transcribe` | 社区的识别引擎 |
| `tts.engine` | plugs | `synthesize`、`voices`（可选） | 社区的合成引擎 |

`ready`（`bus.list`）按登记的情况计算：`voice-input` 在模型下载完成、校验通过以后才登记 `stt` 的三个方法；`say` 在选择的引擎配置好以后才登记 `tts` 的三个方法。头查到 `ready` 以后再画出麦克风和朗读按钮。这些方法都是整句输入、整句输出，不做流式：如果想让第一个音尽快出来，调用的一方需要自己按句子切开，一句一句地调用。

### `stt.transcribe`

把一段音频转成文字。时限是 120 秒。

```json
→ {"audio": "<blob>", "language"?: "zh"}
← {"text": "今天下午三点开会", "language": "zh", "duration_ms": 2380, "clear": true}
```

- `audio`：一定接受 16 kHz、单声道、16 位 PCM 的 wav；其他采样率、声道数的 wav 也接受，ogg/opus 可以不接受。音频最长 10 分钟。
- `language`：`zh`、`yue`、`en`、`ja`、`ko`、`auto`；不填写时使用软件包的配置。
- `clear`：音频短于 0.5 秒，或者有效字少于 2 个时为 `false`，此时 `text` 是空的，不算出错。
- `speech`：整段音频没有听到说话（最响的一小段也只有底噪）时，回应里多出这一个字段，值只会是 `false`：此时音频没有送去识别，`clear` 是 `false`、`text` 是空的，不算出错；听到了说话时不带这个字段。头根据它提示「没听到说话」，而不是「没听清」。
- 出错：`bad_audio`、`too_long`、`silent_audio`（整段是数字静音：峰值不超过 2/32768，也就是麦克风静音、没有插上，或者系统默认的输入设备没有声音；头根据它提示「麦克风没有声音」）、`engine_failed`（选择的社区引擎返回了错误，`message` 使用引擎自己的）。

### `stt.record`

录制本机麦克风的声音，录完以后转成文字。时限是 310 秒。

```json
→ {"language"?: "zh", "max_ms"?: 60000, "silence_ms"?: 1200}
← {"text": "…", "language": "zh", "duration_ms": 4100, "clear": true, "ended": "silence"}
```

- `speech`：和 `stt.transcribe` 相同。`speech` 是 `false` 时回应里还带 `device`：它是打开的那个麦克风给人看的名字（例如 `fifine Microphone Mono`），叫不出名字时不带；头根据它提示「没听到说话，看看麦克风选对没有，现在用的是 …」。
- 人声停止 `silence_ms`（默认 1200，范围是 300 到 5000）以后停止录音；录音最长 `max_ms`（默认 60000，最多 300000）；调用 `stt.stop` 也能停止录音。`ended` 是 `silence`、`stop`、`max` 之一。
- 整台机器共用一把麦克风锁：如果其他头或者命令行已经在录音，返回 `mic_busy`。
- 出错：`mic_busy`、`no_microphone`、`silent_audio`（和 `stt.transcribe` 相同）、`engine_failed`。

### `stt.stop`

停止正在进行的那一次录音，不区分是谁开始的。

```json
→ {}
← {"stopped": true}
```

没有在录音时返回 `{"stopped": false}`，不算出错。

### `tts.synthesize`

把一段 Markdown 合成一段音频。时限是 60 秒。

```json
→ {"text": "**好的**，三点见。", "persona"?: "<人格编号>", "format"?: "mp3"}
← {"audio": "<blob>", "media_type": "audio/mpeg", "duration_ms": 1650}
```

- `text`：Markdown 格式的文字，提供方先把它转成念得出来的话；最多 5000 字。
- `persona`：按这个人格挑选音色；不填写，或者没有给这个人格配置音色时，使用默认音色。
- `format`：`mp3`（默认）、`wav`。
- 出错：`empty_text`（转换以后没有可以念的内容）、`too_long`、`engine_failed`、`not_configured`。

### `tts.play`

通过本机的音箱播放出来。时限是 600 秒。

```json
→ {"audio": "<blob>"}   或   {"text": "…", "persona"?: "<人格编号>"}
← {"finished": true}
```

- `audio`、`text` 二选一；给的是 `text` 时，先按 `synthesize` 的方式合成。
- 同时来了几个请求时，按先后顺序排队播放，不互相打断；被 `tts.stop` 停止的请求返回 `{"finished": false}`。
- 核心自己也会调用这个方法：如果本机会话里没有人开口的一轮（到点的定时任务、循环）产生了一段语音（工具结果 `sent` 里的 `voice`），而这时又没有一个头打开着这个会话，核心就交给它 `{"audio": <blob>}`，不带 `language`。
- 出错：`bad_audio`、`no_speaker`，以及 `synthesize` 的那几个错误。

### `tts.stop`

停止正在播放的和排队等待的全部音频。

```json
→ {}
← {"stopped": 2}
```

`stopped` 是停掉的条数，没有在播放时是 0。

### `stt.engine.transcribe`

这个方法由社区识别引擎实现，`stt` 的提供方按配置里选择的那一家调用它。

```json
→ {"audio": "<blob>", "language": "zh"}
← {"text": "…", "language": "zh"}
```

交给引擎的 `audio` 一定是 16 kHz、单声道、16 位 PCM 的 wav，`language` 也一定已经填写。过滤、`clear`、切段都由 `stt` 的提供方负责，引擎只负责转成文字。出错时引擎返回自己的错误，提供方把它包装成 `engine_failed`。

### `tts.engine.synthesize`

这个方法由社区合成引擎实现。

```json
→ {"text": "好的，三点见。", "voice": "<音色编号>", "speed"?: 1.0, "format": "mp3"}
← {"audio": "<blob>", "media_type": "audio/mpeg", "duration_ms"?: 1650}
```

- `text` 已经是念得出来的纯文字，一次一句，最多 500 字；`voice` 是引擎自己的音色编号；`speed` 的范围是 0.5 到 2.0。
- 如果引擎无法输出 `format` 指定的格式，就交出它能输出的格式，并在 `media_type` 里如实填写。

### `tts.engine.voices`

可选：列出这个引擎有哪些音色。引擎没有登记这个方法时，配置页让人手动填写音色编号。

```json
→ {}
← {"voices": [{"id": "female-tianmei", "name": "甜美女声", "language"?: "zh"}]}
```
