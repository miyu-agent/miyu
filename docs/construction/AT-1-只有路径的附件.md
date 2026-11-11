## 施工单 AT-1：只有路径的附件

状态：做完，等主会话审、合（2026-11-11 写；2026-11-11 项目主人定：上传上限不动，超限的文件在头的附件栏照常显示，实际只把它在核心那台机器上的路径交给她，不拷内容；协议的形状主会话和头的会话对过，技术细节照施工 3-9 五补的路子）。

### 目的

附件上传上限 20 MiB（`blob.put`、分块上传都是）。超过的文件现在附不上。附件必须先 `blob.put` 拿到 `blob` 才能带进 `session.send`；施工 3-9 五补给有 blob 的附件加了可选的 `path`。这一步加**没有 blob、只有路径**的一项：核心不读它，她看到一句写着名字、路径、大小的说明，要看自己照路径用工具读。

### 协议

`session.send`、`session.redo` 的 `attachments` 里多收一种项：

```json
{"path": "/home/alice/Downloads/disk.iso", "name": "disk.iso", "size": 31457280}
```

1. 没写 `blob`（写 `null` 等于没写）、写了 `path` 和 `name` 的就是。`path` 照 `SourcePath`（绝对路径、没有控制字符、最多 4096 字节），`name` 照文件名的写法（`FileName`：1 到 255 字节，没有控制字符、`/`、`\`），`size` 可以不写，写了的是不带小数的非负整数。写错的 `bad_params`，不带 `data`（照附件写错的先例）。
2. 核心不读内容、不 stat、不查在不在；不落成会话附件目录里的文件。
3. 有 `blob` 的项一格都不变：缺格的照旧在读参数时就拒，多写的格（包括 `size`）照旧不看。
4. 一句话里几项不另设上限：`session.send` 的附件本来没有个数上限。
5. 场所会话不收：带 `as` 的 `session.send`、场所会话里的 `session.redo`，`bad_params`。`session.answer` 的附件照旧每一项要 `blob`。
6. 视图里照样是一件附件：`kind` 是 `file`，没有 `media_type`、`blob`，有 `name`、`path`、`path_only: true`，头写了大小的有 `size`。

### 蓝图改哪几节

- `protocol.md`：`session.send` 的 `attachments` 那一行、第 4、5 条，`session.redo` 第 3 条，`session.answer` 第 1 条。
- `kernel/blocks.md`：块从五种成六种，`path` 那一行，第 14 条说明为什么另成一种块。
- `drivers/openai-chat.md` 第 9 条、给模型看的几句那张表；`anthropic.md`、`openai-responses.md` 各一句。
- `policy.md`：`drivers` 那一行多 `path_only`。
- `view.md`：`user` 的 `attachments`、导出成 Markdown 第 4 条。
- `compaction.md` 第二条第 2 条：这一块怎么估。
- `26-提示词.md` 登记簿两行；`prompts.md` 照 `cargo xtask prompts` 生成。
- 参考：`sessions`、`view`、`events` 中英。

### 怎么改的

1. **日志：新加一种块 `path`**（`{"type":"path","name","path","size"?}`，内核的 `Block::Path(PathOnly)`），不借文件块。依据：文件块的 `blob`、`media_type` 必有，驱动（取字节、认文本、写大小）、执行器（要哪些 blob）、附件落成文件、跨会话带附件、视图、Markdown 导出都照它取；把两格改成可以没有，这十来处都得加「没有的话」，而且旧版本读到没有 `blob` 的文件块整条事件读不进（缺格）。新块只加不改：旧版本照「不认识的块」原样留着、投影跳过，估算照它的 JSON 字节数。
2. **给模型的那一句：新加两份模板**（没写大小的、写了的各一份，模板没有能不填的字段）：
   - `core/drivers/file-path-only.txt`：`A file was attached here ({name}, at {path}). Its content is not included. Tools can read it.`
   - `core/drivers/file-path-only-sized.txt`：`A file was attached here ({name}, {size} bytes, at {path}). Its content is not included. Tools can read it.`
   - 没复用 `file-omitted-path.txt`：它要 `media_type` 和 `size`（这种附件没有类型，大小可以不写），说的是「this model cannot read it」，这里和模型读不读得了无关。
   - 照 J13 只写事实，不写「去读它」这样的引导。
   - token（本机 o200k，名字 `报告.pdf`、路径 `~/Documents/报告.pdf`、大小 `48213`）：27、32。真端点的数待主会话补。
   - 登记在 `26-提示词.md` 第十节「文件的占位」那一组，排在 `file-omitted-path.txt` 后面。
   - 随策略快照（`core.drivers.path_only`）；以前造的快照里没有，这种附件只写路径本身（照模板的规矩转义）：等于人在话里写了一个路径，不多出没登记的字。
   - 三个驱动一样，和模型能收什么无关；不要 blob。
3. **端点**：`attach/path_only.rs`。`session.send`、`session.redo` 的每一项读成 `Sent`（有 `blob` 的是原来的 `Attachment`，没有的是只有路径的），读参数时就分开；`attach::sent` 照写的先后造块。`session.answer` 照旧用 `Attachment`。
4. **用量估算**：这一块照它规范的 JSON 的字节数算进字节里（内核看不到模板）；名字、路径、大小都在里面，和那一句差不多长。
5. **视图**：`Attachment` 的 `media_type`、`blob` 改成可以没有，多 `size`、`path_only`；有 blob 的附件写出来一字不变。

### 过了哪些地方

| 地方 | 代码 | 怎么办的 |
|---|---|---|
| 收参数 | `miyu-endpoint` `attach/path_only.rs`、`attach.rs` 的 `sent`、`methods/params.rs` | 新加；有 `blob` 的走原来那一路 |
| 场所会话 | `methods/session.rs` 的 `send`、`redo`（`attach::local_only`） | 拒 `bad_params`；`venue.*` 几个方法没有附件参数，桥发话走的就是带 `as` 的 `session.send` |
| 远程的连接 | `connection/reach.rs` 的 `local_machine_only` | 没动：带 `path` 的附件照旧整条拒 `local_machine_only`（见「要主会话定的」） |
| 回答提问、拒绝时附的 | `methods/params.rs` 的 `AnswerParams` | 没动：照旧每一项要 `blob` |
| 落成会话里的文件 | `attach/keep.rs` | 没动：只认图片块、文件块，这一块原样过 |
| blob 读得到没有 | `attach.rs` 的 `blobs_of`、`attach/reach.rs` | 没动：这一块没有 blob |
| 事件体 | `miyu-kernel` `block.rs` | 新块 `path` |
| 重发 | `miyu-kernel` `session/redo.rs` | 没动：照「文字以外的块」带、换，测试守着 |
| 撤销、恢复 | `miyu-kernel` `history` | 没动：整条事件进出，测试守着 |
| 压缩 | `miyu-kernel` `session/compaction.rs`、`miyu-assemble` | 没动：摘要请求照原样的块发，驱动写成那一句；测试守着 |
| 分叉 | `miyu-endpoint` `forking.rs`、`miyu-store` | 没动：照日志的行拷，不解块 |
| 用量估算 | `miyu-kernel` `estimate.rs` | 加一支：照 JSON 字节数 |
| 起标题、回顾、摘要里的字 | `miyu-kernel` `session/title.rs`、`miyu-assemble` `recap.rs`、`summary.rs`、`group.rs` | 没动：只取文字块，附件都不看 |
| 替它看图 | `miyu-kernel` `session/sight.rs` | 没动：只认图片块 |
| 回合索引、会话列表的第一行 | `miyu-store` `index/row.rs`、`miyu-kernel` `block::words` | 没动：只取文字块 |
| 记忆抽取 | `miyu-session` `memory/extract` | 没动：只取文字块、工具调用 |
| 请求组装 | `miyu-assemble` `render.rs` 的 `known` | 没动：认识的块原样进请求 |
| 驱动 | `miyu-drivers` `media.rs`、`texts/path_only.rs`、三家的编码 | user 消息、工具结果里写成那一句；assistant 里的不写；要哪些 blob 不算它 |
| 扩展写的驱动 | `miyu-session` `route/extension` | 没动：请求照规范字节交过去，这一块和文件块一样原样在里面 |
| 快照 | `miyu-policy` `path_only.rs`、`drivers.rs`，`miyu-store` `resources.rs` | 多一组可以没有的字 |
| `history` 工具 | `miyu-basesystem` `history/entry.rs` | 照文件的占位写（`[file <名字>]`），不多一句字 |
| 跨会话带附件、子代理 | `miyu-session` `attached.rs` | 没动：她带的附件是她读得到的文件，照旧存成 blob；这一块造不出来 |
| 视图 | `miyu-view` `entry.rs`、`projector/users.rs` | 照样一件附件，`path_only` |
| 导出 Markdown | `miyu-view` `markdown.rs`、`markdown/files.rs` | 写 `[文件：<路径>]`；`assets: files` 不拷、不算取不到 |
| 导出、导入原始包 | `miyu-store` `bundle.rs` | 没动：照行搬日志，照哈希的写法找 blob，这一块没有哈希；测试守着 |

### 要主会话定的

1. **远程的连接**：`connection/reach.rs`（施工 Y-1 下）把带 `path` 的附件整条拒 `local_machine_only`，`tests/local_machine.rs` 守着（正好就是 `{"path": …}` 这一种写法）。这一步没动它：远程的头发只有路径的附件照旧被拒。要放开的话改那一处判断和那条测试，另起一步。
2. **名字的上限**：照现有的 `FileName`（255 字节），没有另设 200 个字符。
3. **`blob: null`**：照「写 `null` 等于没写」，`{"blob": null, "path", "name"}` 当只有路径的收；原来这样写是 `bad_params`。

### 不做什么

- 不改上传上限；不读、不 stat 那个文件。
- 场所会话、`session.answer` 不收。
- 不给这种附件落成文件、不存 blob。
- 驱动的金样（`docs/designs/samples/`）不加新样本：三家都是一句字，测试直接对原文。

### 验收

1. 测试先写，改之前红：内核 `block::tests::a_path_only_attachment_is_known_and_round_trips`（认成了不认识的块）；端点 `tests/attach_path_only.rs` 先写的六条里四条红（`session.send` 回 `bad_params`），写法不对的、场所会话的两条改前改后都过（守着不多收）；压缩那一条是改完以后补的。
2. 新加的测试：
   - 内核：块的读写、坏的报错；场景 `scenario/path_only.rs`（重做照带、照换，撤销恢复，压缩，估算）。
   - 驱动：`tests/path_only.rs`（三家同一句、不要 blob、转义、工具结果里、旧快照只写路径）。
   - 端点：`tests/attach_path_only.rs`（日志里的块、和有 blob 的混着先后不乱、请求里的块、视图的格子、写法不对的十九种、重做、撤销恢复、压缩、导出再导入、场所会话和回答提问不收）。
   - 视图：`tests/markdown_attached.rs`（导出 Markdown 写路径）。
   - 资源、快照：出厂的两句读得进、旧快照读成没有。
3. 本机跑过、全过（2026-11-11）：`cargo test -p miyu-kernel -p miyu-assemble -p miyu-session -p miyu-endpoint -p miyu-drivers -p miyu-policy -p miyu-store -p miyu-view -p miyu-basesystem -p miyu-http -p miyu-models --no-fail-fast`（现有的测试一条断言没改）、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo fmt --check`、`RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`、`cargo xtask prompts`（`prompts.md` 只多这两份）、内核的 random 长跑（`cargo test --release -p miyu-kernel --lib random:: -- --include-ignored`）。`cargo xtask check`、三台 CI 由主会话跑。

### 风险

- 新加了核心的字：这一步以前造的会话「核心的字变了」，照施工 P-1 三补，等到人格、预设、工具面有真的改动时一起换上；在那以前这种附件在老会话里只写路径本身。
- 头依赖 `miyu-view` 的类型的：`Attachment` 的 `media_type`、`blob` 成了 `Option`，换钉时跟着改。
- 路径照模板的规矩转义：Windows 的 `C:\a\b` 在那一句里写成 `C:\\a\\b`（施工 3-9 五补的占位也是这样）。
