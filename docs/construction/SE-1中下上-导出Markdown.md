# SE-1 中下上：导出 Markdown

设计 `58-会话导出和导入.md` 第一节、第六节第 2 条；`07-存储.md` 第三节「导出一个会话」；调研 `docs/reviews/2026-09-30-会话导出调研.md` 第四节第 3 条、第五节第 2、3、7 条。SE-1 中下派的七件事里的第一截：`read` 预设、整份喂投影、协议、去密钥。

## 做什么

- 端点 `view/whole.rs`：整个会话喂投影。照 `read_range` 一块块读、喂完就放掉，不整份读成事件、不载入会话，在跑的会话照这一刻已经落盘的取；喂的是 `view.page` 那台投影、那套字，条目和 `view: true` 从头翻到尾的一样。每一条另交给调的一方看一眼。
- 渲染器 `miyu_view::markdown`（`markdown.rs` 和 `markdown/` 六个文件）：纯函数，吃条目、开关（这一步只有 `read`）、给人看的字、时区、配了的密钥，不碰磁盘和时钟。条目里没有的原文（切过的工具结果全文、压缩的摘要）由 `Sources::see` 照事件记。`read` 预设的样子写进蓝图 `view.md`「导出成 Markdown」。
- 给人看的字：`resources/core/human/{en,zh,ja}.json` 的 `said` 加 `export/…` 三十几句（开头、思考、输出、交代、状态、占位、旁白、收尾）。给模型看的字一个不加。
- 协议 `session.export {format: "markdown"}`：`for`（只收 `read`）、`range`（只收 `all`）、`locale`、`to: {"path"} | "inline"`；回应 `{path | text, bytes, tokens, upto, redacted}`；`inline` 超过 512 KiB 拒 `export_too_big`；`for: agent | chat`、`range: context | {"turns": N}`、写了 `thinking`、`tools`、`children`、`assets` 的 `bad_params` 带 `data.later`。`format: "bundle"` 的行为不变（多拒了 `inline` 和只对 Markdown 的参数）。
- 去密钥：`secrets.toml` 里配的值（核心配置里读好的那一份）在 Markdown 里精确匹配，换成 `[redacted: <名字>]`，回应报每个名字几处。原始包不改。
- 存储 `bundle::write_file`：写好的字节照包的规矩写文件（同目录临时文件、同步、硬链接挂上去、不覆盖），和 `bundle::write` 共用开临时文件那一段。

## 我定的（技术题，照推荐）

1. **太短的密钥不换**：值不到 8 个字（照 Unicode 的字数）的跳过、不报。短值在正文里撞上的多半不是它（`abc`、`1234`），换了反倒把正文弄坏；真的 key 都远长于 8 个字。
2. **下一步才有的写法**：`bad_params` 的 `data` 多一格 `later`，写参数名（`for`、`range`、`thinking`……）；写错的（`for: "nobody"`、`range: {"turns": 0}`）照旧 `bad_params`、不带 `later`，头分得清「还没做」和「写错了」。
3. **`redacted` 的形状**：`{"<名字>": <几处>}`，一处都没有的是 `{}`。
4. **撤销说明也不进**：撤掉的轮不进，「撤销了几轮」那条旁白跟着不写，撤了就当没发生过；恢复的轮照原样回来。
5. **小标题的写法**：消息 `## User`（别处来的 `User (<平台身份>)`、`Session (<短编号>)`、`Module (…)`、`Harness (…)`、`Schedule`、`Kernel`），她 `## Assistant`（接着说的不再起，旁白、回报以后再开口重起），一步工具 `### Tool: <显示名> \`对象\``，子代理的回报 `## Subagent: <标题>`。
6. **长的折起来**：输出超过 20 行或 2000 字节的折进 `<details>`，那一行写几行；思考、交代、压缩的摘要一律折起来。
7. **开头**：标题没起的照第一句（同会话列表的 `preview`），都没有的写「未命名会话」；时间段是造的时刻到最近一次动静，照核心这台机器的时区；模型照没撤掉的每一轮收尾记的端点/模型去重；token 合计同会话列表的 `tokens`。
8. **`inline` 的上限照最后交出去的那一份算**：换完密钥、家目录以后的字节数。
9. **`locale`**：`zh`、`ja` 开头的照它，别的一律英文（同 `ui.language` 的 `auto`）。
10. **没合上的代码块替它合上**：消息、回复、思考、回报、摘要原样写，只在末尾补一行围栏；放进折叠块的，代码块外面的 `</details` 写成 `&lt;/details`，免得把折叠块提前合上。
11. **家目录**：照核心这台机器的家目录，整份 Markdown 里只换整段路径（`/home/alice` 换，`/home/alicex`、`/x/home/alice` 不换），先换密钥再换家目录（密钥的值里有家目录的也换得掉）。

## 要拍板的（报给 core1）

1. **分叉出来的会话只导它自己的日志**：同 `view.page`，底子那一段（分叉点以前，在父会话的日志里）不进 Markdown，读的人看不到分叉前聊了什么。推荐照现在的（界面上翻得到的才进），`read` 开头写一句「分叉自会话 …」留到下一步一起定。
2. **token 合计算哪些**：现在同会话列表的 `tokens`，是这个会话自己每次模型调用的输入加输出（起标题、压缩都算，撤掉的轮也算），不含子代理。推荐照现在的（同会话列表一个数）；要连子代理的，改用会话树的用量。
3. **没写进 Markdown 的几样**：要人确认的那一次（只在被拒时写「被拒」）、脚本里调的工具（`nested`）、工具上挂的提问（答了的照「回答了提问」那条旁白写）。推荐 `read` 先这样，`agent` 预设时再看要不要。

## 留着的（下一步）

- `agent`、`chat` 预设，`range` 的 `context`、`{"turns": N}`，四个开关，`children: append`，`assets: files`（连图一起导成文件夹）。
- 命令行 `miyu session export --md`、原始包里的 `transcript.md`。

## 测试

- 渲染器：`crates/miyu-view/tests/markdown.rs` 十二条：`read` 一整轮逐字对；撤掉的、撤回的、撤销说明不进，另照真会话撤一轮；真会话压缩一次，旁白照时间线、摘要折在下面、她再开口重起小标题；长输出折起来、切过的照日志取全文、出错写一个字；没合上的代码块、行内代码和代码块的反引号、mermaid；思考里的 `</details>`；子代理的交代和回报；附件占位不内联；家目录只换整段路径；密钥精确匹配、同一处取长的、太短的不换不报；旁白和收尾一行一个。
- 整份喂投影：`crates/miyu-endpoint/src/view/whole/tests.rs` 照翻页对照测试的造法（一段到几段、空段、末尾半行、分叉的从中间起）造四十份会话，边读边喂的和整份读再喂的条目一样、交出的事件一样、最后一条一样；一条都没有的当没有这个会话。
- 协议：`crates/miyu-endpoint/tests/session_export_markdown.rs` 三条：真核心跑三轮（七万字、读文件加思考、手动压缩、再一轮）导到文件和 `inline` 一字不差，字节数、`tokens`、`upto` 对，密钥在人说的和读到的文件里各一处、换掉、报 2，照 `locale` 换成英文；600 KiB 的会话 `inline` 拒 `export_too_big`、写文件照常；下一步才有的八种写法带 `later`，写错的、写文件的几种拒（相对路径、已经有了、数据根里、目录不在）、没有这个会话、原始包写 `inline` 和 Markdown 参数的拒。`session_export.rs` 里原来「Markdown 随 SE-1 中」那一条换成原始包写 `inline` 拒。
- 变异十个，只跑相关的测试，都红、都还原了：
  1. 渲染时不跳过 `hidden`：撤掉的两条红。
  2. 没合上的代码块不补围栏：反引号那一条红。
  3. 代码块围栏固定三个反引号：反引号那一条红。
  4. 太短的密钥也换：密钥那一条红。
  5. 撤回的排队消息照写：撤掉的那一条红。
  6. `Sources` 不记压缩的摘要：真会话压缩那一条红。
  7. 整份喂的每一块跳过第一条：整份对照的单测红，协议的两条红。
  8. 端点不交密钥给渲染器：协议的密钥那一条红。
  9. `inline` 的上限放大四倍：太大的那一条红。
  10. `for: agent` 不带 `later`：下一步才有的那一条红。

## 文档

蓝图 `protocol.md`「`session.export`」、`view.md`（在哪的三行、「导出成 Markdown」一节）；参考 `sessions.md`、`sessions.en.md`（`session.export`）、`errors.md`、`errors.en.md`（`export_too_big`）。`methods-v1.txt` 不动：冻结检查只查方法名，`session.export` 早就在。
