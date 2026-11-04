# SE-1 中下下：导出 Markdown 的子会话、附件文件夹和命令行

设计 `58-会话导出和导入.md` 第一节；调研 `docs/reviews/2026-09-30-会话导出调研.md` 第五节第 2、3、5、6、7 条；蓝图 `view.md`「导出成 Markdown」、`protocol.md`「`session.export`」、`cli/session.md`。SE-1 中下派的七件事里的最后一截：`agent` 的旁白一律英文、`assets: files`、`children: append`、命令行 `--md`。

## 做什么

- **`agent` 的旁白、占位一律英文**（core1 2026-11-04 定）：压缩、清空、换模型、后台命令、监视的行、回顾、子代理没做完、一轮的收尾这些旁白，`[Image: …]`、`[File: …]` 占位，思考用时，都不跟 `locale`。写成渲染器里的常量（`miyu-view/src/markdown/agent.rs`，同 `AGENT_NOTE`），编号同给人看的字（`export/compacted`、`view/summary/thought-for`），不进 `resources/core/human`。人说的、她回的照原文；`read`、`chat` 照旧跟 `locale`。
- **`assets: files`**：Markdown 写到 `<目录>/<名字>.md` 时，人附的图片和文件、她发的图、工具结果里的图拷进 `<目录>/<名字>_files/`，Markdown 里写相对链接（图 `![说明](talk_files/shot.png)`，别的 `[说明](…)`）。渲染器的 `Files` 起文件名、写链接，端点 `export/assets.rs` 查 blob、建文件夹、拷文件。Markdown 或文件夹任一已经有了 `export_exists`；配 `inline` 的 `bad_params`；取不到的 blob 照占位写，回应 `assets.missing` 列出来。
- **`children: append`**：渲染器拆成 `draft`（一个会话一份草稿，交回写进去的那几步派的子代理 `Draft::children`）和 `finish`（接起来的几份一起换密钥、家目录、token 估算）。端点照任务编号一个个取子会话（`BTreeMap::pop_first`，子会话又派的 `j2.1` 正好排在 `j2` 后面），整份喂投影、照同一套开关渲染，开头换成 `---`、`# Subagent <任务编号>: <标题>`。回应带 `children`（`[{job, session, title}]`），`redacted` 是合计。
- **命令行**：`miyu session export [<会话>] --md [--for read|agent|chat] [--last N] [-o <文件>|-]`。不写 `-o` 的是 `miyu-session-<编号前 8 位>.md`；`-o -` 走 `inline`，原样写到标准输出；超了 512 KiB 照核心的话报错，另一行提示「用 -o <文件> 存成文件」。`--for`、`--last` 要配 `--md`（clap 的 `requires`）。不做剪贴板。
- 下一步才有的都做了：`Refusal::later` 和 `bad_params` 的 `data.later` 去掉。

## 我定的（技术题）

1. **`agent` 的常量表只放 `agent` 走得到的那几句**：`read` 才用得到的（开头、确认、访问种类、脚本里调了几个……）不抄；表里没有的照英文那一份给人看的字，所以不论表漏没漏，`agent` 都不会出中文。字段一遍过地换（值里写着 `{别的字段}` 的不再换），控制字符换成 `�`（同 `human::clean`）。
2. **带哪些子会话照渲染进去的条目定**：工具那一步的 `job` 对上 `job.started` 记的子会话（`Sources::jobs`）。撤掉的轮、`range` 切掉的那几步派的不带；同一个子会话只带一次；日志已经没了的（删过的）跳过、不报错，读不下去的整个拒 `session_broken`（不吞错误）。
3. **子会话的开头**：`---` 分隔线、`# Subagent <任务编号>: <派它时的 description>`（同角色小标题一律英文），下面的列表 `read` 时间段、模型、token 合计，`chat` 时间段，`agent` 工作目录、时间段、模型；`AGENT_NOTE`、`chat` 的 token 估算只在最前面写一次（估算照接起来的整份算）。
4. **附件文件夹的名字**照 Markdown 文件名去掉扩展名接 `_files`（同浏览器「网页，全部」的惯例）；名字不是 UTF-8 的照替换过的那一份建，链接和真的文件夹对得上。
5. **文件名**：附件的名字，没有的照路径的最后一截，都没有的 `image-<哈希前 8 位>.<类型的子类型>`；Windows 不收的字换成 `_`、头尾空格和点去掉、最多 80 个字（扩展名留着）、`CON` 这类垫 `_`；同一个 blob 只拷一份；撞名不分大小写（macOS、Windows 上是同一个文件），后来的接 `-2`。
6. **链接里写 `%XX` 的字**：空格、括号、方括号、尖括号、`#`、`?`、`%`、反斜杠、反引号、引号、控制字符；中文照原样（GitHub、VS Code、Typora 都认）。不用 `<…>` 包路径：有的看器不认。
7. **写的先后**：先查 Markdown 和文件夹都不在；渲染（每个会话渲染前照 `Blobs::check` 查它写到的 blob，核对哈希）；一个附件都没有的不建文件夹；`create_dir` 建（别人抢先建了照样 `export_exists`），文件一个个 `create_new`、同步；Markdown 最后写，看到它就是写完了的；拷到一半、Markdown 写不成的，把建了的文件夹整个删掉。
8. **回应的字段**：`children` 总有（没写 `append` 的是 `[]`），一项 `{job, session, title}`；`assets` 只有 `assets: files` 才有，`{dir?, files, missing}`，`dir` 只在建了文件夹时写，`missing` 是哈希（头照它对得上是哪个附件）。
9. **命令行的那句建议照命令行现在的写法放进 `language.rs`**（中英两种，同别的报错、建议）：命令行还没接给人看的字的资源、也没有日文那一份。帮助页照旧中英两份。

## 要拍板的（报给 core1）

1. **附在后面的子会话的范围一律 `all`**：主会话写了 `range: context`（`agent` 的默认）、`{"turns": N}` 也不切子会话，子会话整份带。推荐照现在的：子会话本来就是「要的话附上全文」，再切一刀读的人对不上派它那一步；另一种是照主会话的 `range` 切。
2. **命令行的字没进 `resources/core/human` 三份**：派工单写的是进 human 资源三份，可命令行现在所有报错、建议都在 `crates/miyu-cli/src/language.rs` 里、只有中英（`Language` 只有这两种），没接资源。推荐这一步照命令行现在的规矩，等命令行整体接资源时一起搬；另一种是这一步先给命令行接上资源（要动 `Language`、加日文，范围大）。
3. **命令行不收 `--append`、`--assets files`**：派工单只列了 `--for`、`--last`、`-o`。推荐先不加，要的话下一步加两个开关。
4. **`read` 预设里交代写两遍**：主会话那一步折着交代，附在后面的子会话第一条（`## Session (…)`）又是同一段交代的原文。推荐照现在的（子会话原样，不特意删）。

## 还没有的

- 给 agent 的 `tools: full`：前台工具结果的全文进 blob（`08-上下文投影.md` C9）还没做，`full` 现在取的是后台命令的输出和脚本里调的工具交回的结果，日志里她看到的就是前台工具的全部。C9 做了以后要接上。
- 终端界面的 `/export`、网页的「导出」菜单（调研第五节第 5 条），各自的头做。
- 分叉出来的会话的底子那一段不进 Markdown（SE-1 中下上要拍板的第 1 条）。

## 测试

- 渲染器：
  - `crates/miyu-view/src/markdown/agent.rs` 单测一条：表里都是 ASCII，字段照换、控制字符换掉、值里的花括号不再换、少了字段的没有。
  - `crates/miyu-view/src/markdown/files/tests.rs` 三条：名字在三个平台都收（路径只留最后一截、怪字、头尾的点、保留名、80 个字留扩展名）、扩展名；同一个 blob 一份、不分大小写撞名接序号、说明里的方括号垫反斜杠、没名字的照哈希；链接里的怪字写成 `%XX`、取不到的记下、不起名字。
  - `crates/miyu-view/tests/markdown_attached.rs` 三条：`agent` 连接是中文时每一种旁白、占位整段英文逐字对，`read` 照旧中文；`assets` 列出人附的、结果里的、她发的，取得到的写链接、取不到的照占位并记下；`children` 照任务编号、撤掉的那一步派的不带，`chat` 接起来以后密钥合计、token 估算只写一次且照整份，`read`、`agent` 子会话开头的写法、不再写 `AGENT_NOTE`。
  - `markdown_ranges.rs`：`agent` 写 `range: all` 的压缩旁白改成英文（`> Context compacted`、`<summary>Summary</summary>`）。
- 协议：
  - `crates/miyu-endpoint/tests/session_export_attached.rs` 两条：真核心主会话两步各派一个子代理（A 又派 A1），`chat` 的 `children: append` 照 `j1`、`j1.1`、`j2` 接在后面，各段内容对，三处密钥合计、估算只写一次；`agent` 中文连接也只写一次 `AGENT_NOTE`、子会话开头是英文；不写 `append` 的 `children: []`。传一张图、一个文件、删掉文件的 blob，`assets: files` 拷了图（字节一样、空格的名字写 `%20`）、文件照占位、`missing` 列出它；再导、只剩文件夹的都 `export_exists`、什么都不写；配 `inline` 的 `bad_params`；没附件的不建文件夹。
  - 文件夹已经有了、会话却没有附件的也 `export_exists`、什么都不写（变异 14 先活下来，补的这一条）。
  - `session_export_markdown.rs`：「下一步才有」那一段去掉，写错的多一种（`assets: files` 配 `inline`）。`session_export_presets.rs`：`agent` 写 `all` 的压缩旁白是英文、没有「上下文压缩了」「摘要」。
- 命令行（`crates/miyu-cli/tests/session.rs`）两条：`--md` 写到默认的 `.md`、印路径；`-o -` 带 `--for agent --last 1` 打到标准输出、开头是英文、只有最近一轮、不写文件；600 KiB 的会话 `-o -` 退出码 1、标准输出空、标准错误有核心的话和建议。帮助页的测试照旧（选项和 clap 对得上、80 列）。
- 变异十六个，只跑相关的测试，都红、都还原了：
  1. `agent` 照连接的语言说：`markdown_attached` 的 `agent` 逐字对、`markdown_ranges` 的 `agent` 那一条红。
  2. 改坏一句英文常量（`Context compacted`）：`agent` 逐字对那一条红。
  3. 取不到的 blob 也写链接：附件那一条、`files` 单测红。
  4. 撞名分大小写：`files` 单测红。
  5. 链接里的空格不写 `%20`：`files` 单测红。
  6. 子会话不照任务编号排：子会话那一条红。
  7. 撤掉的那一步派的也带（照全部条目渲染）：子会话那一条红。
  8. token 估算只算主会话那一份：子会话那一条红。
  9. 子会话的开头也写 `AGENT_NOTE`：子会话那一条红。
  10. 端点不接子会话又派的：协议的子会话那一条红（少了 `j1.1`）。
  11. 一个附件都没有也建文件夹：协议的附件那一条红。
  12. `assets: files` 配 `inline` 不拒：协议的两条红。
  13. 不查 blob、当都取得到：协议的附件那一条红（拷的那一刻 `session_broken`）。
  14. 写之前不查文件夹在不在：先是绿的（有附件时 `create_dir` 照样拦），补了「没附件、文件夹已经有了」那一句以后红。
  15. 命令行 `-o -` 不走 `inline`：命令行两条红。
  16. 太大的不带建议：命令行太大的那一条红。

## 文档

蓝图 `view.md`（在哪两行加一行，「导出成 Markdown」：开关两种写法、占位、旁白跟不跟 `locale`、第 8 条 `agent` 的常量，新加「子会话附在后面」「附件文件夹」两节）、`protocol.md`（在哪一行、参数表、回应、第 4、6、7 条）、`cli/session.md`；参考 `sessions.md`、`sessions.en.md`（参数、回应、长什么样、出错）、`errors.md`、`errors.en.md`（`bad_params` 去掉 `data.later`）。`methods-v1.txt` 不动。
