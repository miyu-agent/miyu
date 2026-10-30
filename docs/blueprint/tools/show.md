## `show`

### 是什么

把一个文件给人看（设计 `35-展示.md`，施工 D-5，2026-10-17 项目主人定）：图片、HTML、Markdown、PDF、代码、文本，别的文件也收。怎么显示由头照种类定（设计 35 第三节）；她只收到一句「已展示给用户」，文件的内容不进上下文。核心读一份存成 blob，记进这次结果的效果 `file.shown`，头照它画；同一个路径再展示一次，头换掉那一张。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/show.rs` | 参数、要碰的路径、读、认种类、结果和效果 |
| `crates/miyu-basesystem/src/show/kind.rs` | 认种类：图片、PDF 照内容，HTML、Markdown、代码照后缀，别的文本、文件 |
| `crates/miyu-tool/src/run.rs` | 工具报的效果 `Effect::Shown`（带着内容） |
| `crates/miyu-session/src/effects.rs` | 内容存成 blob，换成内核的 `file.shown` |
| `crates/miyu-kernel/src/event/effect.rs` | `file.shown` 的格 |
| `crates/miyu-view/src/projector/tools.rs` | 工具卡的 `shown` |
| `resources/software/basesystem/tools/show.json` | 说明和参数格式 |
| `resources/software/basesystem/show/*.txt`、`common/*.txt` | 输出里给她看的几句 |
| `resources/software/basesystem/show/system.txt`、`crates/miyu-policy/src/compose.rs` | system 里的那一句（施工 D-5 补二，`Snapshot::with_show`） |
| `crates/miyu-session/src/settings.rs` | 那一句的开关 `show.system_hint`（`ShowSettings`，造会话时在 `open/face.rs` 读） |
| `resources/packages/miyu-base-show/package.toml` | 功能「展示」（`show`） |

### 对外的样子

访问类别 `read`，基础系统的功能 `show`（「展示」，预设能关）。本机的会话、子会话、场所会话都有。说明和参数的原文如下。

样本 `resources/software/basesystem/tools/show.json`：

```json
{
  "description": "Show a file to the user in their interface, such as an image, video, audio, web page, PDF or code. You get no content back. Showing the same path again replaces it.",
  "parameters": {"type":"object","properties":{"path":{"type":"string"},"title":{"type":"string","description":"Default: the file name."}},"required":["path"]}
}
```

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `path` | 是 | 要给人看的文件：绝对路径、相对这一轮工作目录的路径、`~` 开头的路径。写成 `file_path`、`filePath` 的也认；写了两种的，参数不对 |
| `title` | 否 | 标题：去掉前后空白，空的当没写；没写的头照文件名 |

- 别的参数不认，也不报错。
- system 里的一句（施工 D-5 补二，项目主人 2026-10-24 定留）：工具面上有 `show`、开关 `show.system_hint` 开着（默认开）的会话，system 在调工具的程序那一段后面单成一段带 `show/system.txt`（常量字节，不拼会变的东西）；没有的、关着的、以前造的快照不带。开关造会话时读一次钉进快照（`no_show_hint`，关着的才写），换人格、换预设重拼照钉下的，会话中途改了 system 不变（新会话生效）。`session.context` 里是 `system.show`。原文：

  ```text
  When you make or fetch something meant to be looked at — an image, a web page, a document, video or audio — handing it over with show lets the user see it right away, rather than just a path.
  ```
- 报给权限策略的路径：一条，`path` 原样，读（同 `read`）：工作区以外的问人、只读的数据根碰不到、沙盒藏起来的看不见（`session/guard.md`）。

### 怎么走

1. 读参数，读不懂的：参数不对。
2. 照 `resolve` 换成真实的位置（链接换成它指向的，权限也照它判，同 `read`），再照 `open_file` 打开（同附件 `attach.rs`）：没有的、是目录的、不是普通文件的、读不了的照共用的几句（`common/*.txt`）。
3. 最多读到 20 MiB 多一个字节（`miyu_tool::attachment::LIMIT`，同附件）：超了的回 `show/too-big.txt`，什么都不存。
4. 认种类（`show/kind.rs`），照先后：
   1. 图片（`miyu_tool::picture` 认得出、量得出宽高的四种）：媒体类型照认出的，带宽高。只给人看，不照给模型的图的上限（5 MiB、8000 像素）挡。
   2. 开头是 `%PDF-` 的：`application/pdf`。
   3. 音视频（施工 D-5 补，网页报、项目主人 2026-10-23 指出展示的 mp4、mp3 只能下载；`show/kind/media.rs`）：照前 64 字节的文件头认浏览器放得了的七种。`ftyp` 的是 `video/mp4`，牌子是 `M4A `、`M4B ` 的是 `audio/mp4`，QuickTime（`qt  `）不认；EBML 头、文档类型写 `webm` 的是 `video/webm`，Matroska 不认；`ID3` 开头或者 MPEG 第三层帧同步的是 `audio/mpeg`（AAC 的 ADTS 不算）；`fLaC` 是 `audio/flac`；`RIFF`…`WAVE` 是 `audio/wav`；`OggS` 是 `audio/ogg`。
   4. 后缀 `.svg`：`image/svg+xml`，不带宽高。
   5. 后缀 `.html`、`.htm`：`text/html`；`.md`、`.markdown`：`text/markdown`。
   6. 是 UTF-8 的：`text/plain`；后缀认得出语言的（下面的表）带 `language`。
   7. 别的二进制照后缀认音视频（施工 D-5 补）：`.mp4` `video/mp4`、`.webm` `video/webm`、`.mp3` `audio/mpeg`、`.flac` `audio/flac`、`.wav` `audio/wav`、`.m4a` `audio/mp4`、`.ogg` `.oga` `.opus` `audio/ogg`；`.mov`、`.mkv` 浏览器多半放不了，不认。
   8. 再别的：`application/octet-stream`。
5. 成了：结果 `Shown to the user.`（`show/shown.txt`），报 `file.shown`（下面）。不读回内容、不给图块。
6. 叫停：读在阻塞线程里做，读之前看一眼旗，举了就不读，交回 `stopped`。

语言（后缀到 `language`，照 Markdown 代码块的写法，头拿去高亮）：`rs` rust、`py` python、`js` `mjs` `cjs` javascript、`ts` `mts` typescript、`tsx` tsx、`jsx` jsx、`go` go、`java` java、`kt` `kts` kotlin、`swift` swift、`c` `h` c、`cc` `cpp` `cxx` `hpp` `hh` cpp、`cs` csharp、`rb` ruby、`php` php、`sh` `bash` `zsh` bash、`fish` fish、`ps1` powershell、`lua` lua、`sql` sql、`css` css、`scss` scss、`json` json、`toml` toml、`yaml` `yml` yaml、`xml` xml、`dart` dart、`scala` scala、`hs` haskell、`ex` `exs` elixir、`r` r、`vim` vim；文件名 `Dockerfile` dockerfile、`Makefile` makefile。认不出的没有这一格。

**效果 `file.shown`**（`kernel/events-bodies.md`，给头看、不给模型）：

| 格 | 是什么 |
|---|---|
| `path` | 换成真实位置以后的绝对路径（Windows 上不写 `\\?\`）：头照它认「同一张」，再展示一次换掉那一张 |
| `blob` | 内容的哈希，头照 `blob.read` 读 |
| `name` | 文件名：真实位置的最后一段 |
| `media_type` | 上面认出的 |
| `size` | 字节数 |
| `title` | 她写的标题，没写的没有这一格 |
| `language` | 认得出语言的代码、文本才有 |
| `width`、`height` | 图片才有，像素 |


**视图**（`view.md`）：这次调用的工具卡多 `shown`，格同上（不带 `path` 以外的会话信息），没展示成的没有。

**撤销**：效果随结果走，撤掉这一轮，视图里那一张也没了。不碰文件。

### 出错

| 情形 | 给她的 |
|---|---|
| 参数不对 | `common/bad-args.txt` |
| 没有、是目录、不是普通文件、读不了 | `common/*.txt`（同 `read`） |
| 超过 20 MiB | `show/too-big.txt`：`{path} is larger than 20 MiB and cannot be shown.` |

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-basesystem/src/show/kind/tests.rs` | 每一种照先后认，语言表，svg、空文件、非 UTF-8 |
| `crates/miyu-basesystem/tests/show.rs` | 结果那一句、效果的每一格、标题去空白、太大、目录、没有、链接照它指向的 |
| `crates/miyu-session/tests/show.rs` | 内容存成 blob、日志里的 `file.shown`、视图的 `shown`、撤销以后没了 |
