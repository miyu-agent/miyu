## `show`

### 是什么

把一个文件给人看（设计 `35-展示.md`，施工 D-5，2026-10-17 项目主人定）：图片、HTML、Markdown、PDF、代码、文本，别的文件也收。怎么显示由头照种类定（设计 35 第三节）；她只收到一句「已展示给用户」，文件的内容不进上下文。核心不拷内容、不限大小（施工 SW-1，2026-11-12 项目主人定）：只记下文件的真实路径、大小、修改时间和认出来的种类，写进这次结果的效果 `file.shown`；本机的头照路径读硬盘，远程的头照 `session.shown` 分段读（`protocol.md`）。同一个路径再展示一次，头换掉那一张。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/show.rs` | 参数、要碰的路径、打开、取大小和修改时间、读开头认种类、结果和效果 |
| `crates/miyu-basesystem/src/show/kind.rs` | 认种类：图片、PDF 照内容，HTML、Markdown、代码照后缀，别的文本、文件 |
| `crates/miyu-tool/src/run.rs`、`crates/miyu-tool/src/stamp.rs` | 工具报的效果 `Effect::Shown`（不带内容）；修改时间换成毫秒的时刻（`modified_at`，`show` 记、`session.shown` 核对共用） |
| `crates/miyu-session/src/effects.rs` | 换成内核的 `file.shown` |
| `crates/miyu-kernel/src/event/effect.rs` | `file.shown` 的格 |
| `crates/miyu-view/src/projector/tools.rs` | 工具卡的 `shown` |
| `crates/miyu-endpoint/src/shown.rs` | `session.shown`：照会话加调用编号找到 `file.shown`、核对、分段读（施工 SW-1） |
| `resources/software/basesystem/tools/show.json` | 说明和参数格式 |
| `resources/software/basesystem/show/*.txt`、`common/*.txt` | 输出里给她看的几句 |
| `resources/software/basesystem/show/system.txt`、`crates/miyu-policy/src/compose.rs` | system 里的那一句（施工 D-5 补二，`Snapshot::with_show`） |
| `crates/miyu-session/src/settings.rs` | 那一句的开关 `show.system_hint`（`ShowSettings`，造会话时在 `open/face.rs` 读） |
| `resources/packages/base-show/package.toml` | 功能「展示」（`show`） |

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
  Files passed to show open in the user's interface. A path in a reply is only text to them.
  ```
- 报给权限策略的路径：一条，`path` 原样，读（同 `read`）：工作区以外的问人、只读的数据根碰不到、沙盒藏起来的看不见（`session/guard.md`）。

### 怎么走

1. 读参数，读不懂的：参数不对。
2. 照 `resolve` 换成真实的位置（链接换成它指向的，权限也照它判，同 `read`），再照 `open_file` 打开（同附件 `attach.rs`）：没有的、是目录的、不是普通文件的、读不了的照共用的几句（`common/*.txt`）。
3. 照打开的那个句柄取大小和修改时间（不另 stat 路径，免得中间被换掉；修改时间换成 UTC 毫秒的时刻，`miyu_tool::modified_at`，换不成的照读不了），再读开头至多 1 MiB（`KIND_HEAD`）认种类。不限大小、不存 blob（施工 SW-1，2026-11-12 项目主人定：以前照附件的 20 MiB 挡，录好的视频展示不了）。
4. 认种类（`show/kind.rs`），只看读到的那一截，照先后：
   1. 图片（`miyu_tool::picture` 认得出、量得出宽高的四种）：媒体类型照认出的，带宽高。只给人看，不照给模型的图的上限（5 MiB、8000 像素）挡。
   2. 开头是 `%PDF-` 的：`application/pdf`。
   3. 音视频（施工 D-5 补，网页报、项目主人 2026-10-23 指出展示的 mp4、mp3 只能下载；`show/kind/media.rs`）：照前 64 字节的文件头认浏览器放得了的七种。`ftyp` 的是 `video/mp4`，牌子是 `M4A `、`M4B ` 的是 `audio/mp4`，QuickTime（`qt  `）不认；EBML 头、文档类型写 `webm` 的是 `video/webm`，Matroska 不认；`ID3` 开头或者 MPEG 第三层帧同步的是 `audio/mpeg`（AAC 的 ADTS 不算）；`fLaC` 是 `audio/flac`；`RIFF`…`WAVE` 是 `audio/wav`；`OggS` 是 `audio/ogg`。
   4. 后缀 `.svg`：`image/svg+xml`，不带宽高。
   5. 后缀 `.html`、`.htm`：`text/html`；`.md`、`.markdown`：`text/markdown`。
   6. 是 UTF-8 的：`text/plain`；后缀认得出语言的（下面的表）带 `language`。文件比读到的那一截长的，截断处半个字不算坏（施工 SW-1）；1 MiB 以内的文件整份都看，结果和以前一样。
   7. 别的二进制照后缀认音视频（施工 D-5 补）：`.mp4` `video/mp4`、`.webm` `video/webm`、`.mp3` `audio/mpeg`、`.flac` `audio/flac`、`.wav` `audio/wav`、`.m4a` `audio/mp4`、`.ogg` `.oga` `.opus` `audio/ogg`；`.mov`、`.mkv` 浏览器多半放不了，不认。
   8. 再别的：`application/octet-stream`。
5. 成了：结果 `Shown to the user.`（`show/shown.txt`），报 `file.shown`（下面）。不读回内容、不给图块。
6. 叫停：读在阻塞线程里做，读之前看一眼旗，举了就不读，交回 `stopped`。

语言（后缀到 `language`，照 Markdown 代码块的写法，头拿去高亮）：`rs` rust、`py` python、`js` `mjs` `cjs` javascript、`ts` `mts` typescript、`tsx` tsx、`jsx` jsx、`go` go、`java` java、`kt` `kts` kotlin、`swift` swift、`c` `h` c、`cc` `cpp` `cxx` `hpp` `hh` cpp、`cs` csharp、`rb` ruby、`php` php、`sh` `bash` `zsh` bash、`fish` fish、`ps1` powershell、`lua` lua、`sql` sql、`css` css、`scss` scss、`json` json、`toml` toml、`yaml` `yml` yaml、`xml` xml、`dart` dart、`scala` scala、`hs` haskell、`ex` `exs` elixir、`r` r、`vim` vim；文件名 `Dockerfile` dockerfile、`Makefile` makefile。认不出的没有这一格。

**效果 `file.shown`**（`kernel/events-bodies.md`，给头看、不给模型）：

| 格 | 是什么 |
|---|---|
| `path` | 换成真实位置以后的绝对路径（Windows 上不写 `\\?\`）：头照它认「同一张」，再展示一次换掉那一张 |
| `name` | 文件名：真实位置的最后一段 |
| `media_type` | 上面认出的 |
| `size` | 展示那一刻的字节数 |
| `modified` | 展示那一刻的修改时间：UTC，精确到毫秒，写法同事件的 `at`（施工 SW-1）。`session.shown` 照它和 `size` 核对文件变没变。以前（施工 SW-1 以前）记的没有这一格、带着 `blob`，照样读得进，`blob` 不认、写出去没有 |
| `title` | 她写的标题，没写的没有这一格 |
| `language` | 认得出语言的代码、文本才有 |
| `width`、`height` | 图片才有，像素 |


**视图**（`view.md`）：这次调用的工具卡多 `shown`，格同上（不带 `path` 以外的会话信息），没展示成的没有。

**头怎么读内容**（施工 SW-1）：握手回应带 `host` 的是本机的连接（同施工 AT-1 的判法，`connection/reach.rs`），照 `path` 直接读硬盘；远程的连接读不到这台电脑的硬盘，照 `session.shown {session, call}` 分段读（`protocol.md`），核心只读这次展示记下的那个路径，大小、修改时间对不上的拒 `file_changed`，没了的 `file_gone`。视频、音频边传边放，很大的文本只取开头预览，由头定，核心不替头截。

**撤销**：效果随结果走，撤掉这一轮，视图里那一张也没了。不碰文件。

### 出错

| 情形 | 给她的 |
|---|---|
| 参数不对 | `common/bad-args.txt` |
| 没有、是目录、不是普通文件、读不了 | `common/*.txt`（同 `read`） |

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-basesystem/src/show/kind/tests.rs` | 每一种照先后认，语言表，svg、空文件、非 UTF-8，截断处半个字 |
| `crates/miyu-basesystem/tests/show.rs` | 结果那一句、效果的每一格、标题去空白、过了 20 MiB 照常展示、目录、没有、链接照它指向的 |
| `crates/miyu-session/src/effects/tests.rs` | 换成 `file.shown`：路径、大小、修改时间，不存 blob |
| `crates/miyu-view/tests/lines.rs` | 视图的 `shown`；以前带 `blob` 的照样读得进 |
| `crates/miyu-endpoint/tests/session_shown.rs` | `session.shown`：远程、本机分段读，读到结尾、只问大小、一段太长；没展示过的、没有的会话 `unknown_shown`；改了 `file_changed`、删了 `file_gone` |
