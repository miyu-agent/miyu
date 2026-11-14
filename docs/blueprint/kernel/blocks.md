## 内容块、原样的 JSON、认不出的取值

### 是什么

消息和工具结果的内容由内容块组成：文字、思考、图片、文件、工具调用，加上只有路径的附件（施工 AT-1）、供应商侧工具（施工 HT-1），一共七种。两样零件跟着它们，也用在事件的别处：原样的 JSON，一个字节都不改的一段 JSON；用字符串写的取值，认识的读成对应的一种，不认识的原样留着。新版本加了种类、加了取值，旧版本照样读得进来，写出去也不抹掉。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-kernel/src/block.rs` | 七种块 `Block`、供应商侧工具的结果 `Outcome`、驱动私有数据 `Private`、不认识的块 |
| `crates/miyu-kernel/src/raw.rs` | 原样的 JSON `RawJson`；照 `type` 或 `kind` 分派的读法 `read_tagged`，块、`by`、效果都用它 |
| `crates/miyu-kernel/src/text_enum.rs` | 生成字符串取值的宏 `text_enum!`，内核里的十一种取值都由它生成 |

### 对外的样子

**内容块** `Block`：JSON 里用 `type` 分开。

| `type` | 格 | 例子 |
|---|---|---|
| `text` | `text`：文字 | `{"type":"text","text":"我先看一下目录。"}` |
| `reasoning` | `text`：思考的文字；`private`：驱动私有数据，可以没有 | `{"type":"reasoning","text":"先看目录","private":{"driver":"anthropic","data":{"signature":"sig"}}}` |
| `image` | `blob`：图片存成的 blob 的内容哈希；`name`：文件名，只是名字，不带路径，可以没有（施工 3-9 四补）；`media_type`：媒体类型；`width`、`height`：宽、高，像素；`path`：原来在本机的哪儿，可以没有（施工 3-9 五补，`SourcePath`）；剪贴板贴的、远程传的，核心落成会话附件目录里的文件后记那个路径（施工 3-9 六补） | `{"type":"image","blob":"sha256:…","name":"晚霞.png","media_type":"image/png","width":800,"height":600}` |
| `file` | `blob`：文件存成的 blob 的内容哈希；`name`：文件名，只是名字，不带路径；`media_type`：媒体类型；`path`：原来在本机的哪儿，可以没有（施工 3-9 五补）；同图片，贴的落成文件后记那个路径（施工 3-9 六补） | `{"type":"file","blob":"sha256:…","name":"报告.pdf","media_type":"application/pdf"}` |
| `path` | 只有路径的附件（施工 AT-1）。`name`：文件名，只是名字，不带路径；`path`：它在核心那台机器上的哪儿（`SourcePath`）；`size`：头写的字节数，可以没有。没有 blob：内容没存进来 | `{"type":"path","name":"disk.iso","path":"/home/alice/Downloads/disk.iso","size":31457280}` |
| `tool_call` | `call_id`：内核分的调用编号；`name`：模型说要调用的工具名；`args`：模型给的参数原文，一个字符串；`private`：驱动私有数据，可以没有 | `{"type":"tool_call","call_id":"call_44_1","name":"read","args":"{\"path\":\"src\"}"}` |
| `hosted_tool` | 供应商自己跑了的一次工具（施工 HT-1，设计 71 第三节第 1 条），例如供应商那边的搜索。`call_id`：内核分的调用编号，写法同 `tool_call`，编号见第 13 条；`name`：供应商自己的工具名，原样；`args`：参数原文；`outcome`：结果，可以没有（没收到的，例如被打断），`{"text","failed","cut"}`：`text` 是结果的字，`failed` 是供应商说跑失败了，`cut` 是字超过 64 KiB 截掉了后面，后两格是假的不写；`private`：驱动私有数据，可以没有 | `{"type":"hosted_tool","call_id":"call_44_2","name":"web_search","args":"{\"query\":\"rust\"}","outcome":{"text":"1. Rust"}}` |

- 除了标着「可以没有」的，每一格都必有。
- 写法照 `kernel/ids.md`：`blob` 是内容哈希，`media_type` 是媒体类型，`name`（`image`、`file`、`path` 的）是文件名，`path` 是附件的路径，`size` 是 0 到 18446744073709551615 的整数（`u64`），`call_id` 是调用编号。`width`、`height` 是 0 到 4294967295 的整数（`u32`）。`text`、`args`、`tool_call` 和 `hosted_tool` 的 `name`、`outcome` 的 `text` 是任意字符串；`failed`、`cut` 是 `true`、`false`。

**驱动私有数据** `Private`：供应商要原样传回的东西，例如思考块的签名、供应商自己的调用编号。

| 格 | 是什么 |
|---|---|
| `driver` | 驱动家族（写法见 `kernel/ids.md`）：哪一类驱动的数据 |
| `data` | 数据本身，任意 JSON，原样的 |

两格都必有。内核不解读；哪一类驱动认哪一份，照 `driver` 分（`drivers/openai-chat.md`）。

**原样的 JSON** `RawJson`：

- 只能从 serde_json 读出来，没有别的造法：它记下的就是读到的原文，所以能一字不差。
- `get()` 交回原文。
- 两份原文一字不差才算相等：`{"a":1}` 和 `{ "a": 1 }` 不相等。
- 写出去就是原文：空格、数字的写法（`1.50`）、字段的先后都不变。

用在这些地方：

| 哪里 | 放的是什么 | 见 |
|---|---|---|
| `Private` 的 `data` | 驱动私有数据 | 本页 |
| 不认识的块、不认识的 `by`、不认识的效果 | 整块 | 本页、`kernel/ids.md`、`kernel/events-bodies.md` |
| 不认识的事件种类 | 整个 `body` | `kernel/events.md` |
| `tool.approval_requested` 的 `rule`、`detail` | 提问的模块定的写法 | `kernel/events-bodies.md` |
| 工具的参数格式 `ToolRule.parameters`、`ToolSpec.parameters` | JSON Schema，原样进 tools 数组 | `kernel/tools.md`、`kernel/request.md` |

**字符串取值**（宏 `text_enum!`）：每一种列出认识的几个值，再多一种 `Other(String)`，放不认识的。`as_str()` 是它在 JSON 里的写法。

| 取值 | 在哪（`crates/miyu-kernel/src/` 下） | 认识的值 | 见 |
|---|---|---|---|
| `Access` | `tool.rs` | `read`、`write`、`execute`、`network`、`outbound`、`venue`（施工 O-31 前）、`install`（施工 F-10 下） | `kernel/tools.md` |
| `Level` | `event/session.rs` | `full`（A16 以前的 `workspace` 施工 AUD-4 起不认，同别的不认识的取值） | `kernel/events-bodies.md` |
| `EndReason` | `event/turn.rs` | `completed`、`interrupted`、`error`、`step_limit`、`aborted`、`restarted` | 同上 |
| `ToolStatus` | `event/tool.rs` | `ok`、`error`、`cancelled`、`denied`、`skipped` | 同上 |
| `Decision` | `event/tool.rs` | `once`、`session`、`workspace`、`deny` | 同上 |
| `Part` | `event/model.rs` | `tools`、`system`、`message` | 同上 |
| `MessageRole` | `event/model.rs` | `user`、`assistant`、`tool` | 同上 |
| `CallResult` | `event/model.rs` | `ok`、`error`、`interrupted`、`superseded`（施工 4-15） | 同上 |
| `ErrorClass` | `event/model.rs` | `retryable`、`rate_limited`、`context_too_long`、`auth`、`content_policy`、`other`、`bad_stream`、`empty_reply`，后来加的见 `kernel/events-bodies.md`「出错的分类」（`output_limit` 施工 3-10） | 同上 |

`ErrorClass` 里写成 `other` 的那一个认识的值，代码里叫 `Unclassified`：`Other` 这个名字留给读到的不认识的分类。

### 怎么走

**读一块**（`read_tagged`，`by` 照 `kind`、效果照 `kind` 也是这样读）：

1. 先把整块原样读下来。开头第一个字段就是 `type`（`kind`）、值是不带转义的字的（核心自己写的都是这样），直接认出种类，不把整块解成一张表（施工 V-10：一万条事件的会话读日志从约 30 ms 降到约 18 ms，本机）；别的写法照整表认，结果一样。
2. 不是 JSON 对象的，报 serde_json 的原话（`expected a map`）。
3. 没有 `type` 的，报「missing field `type`」。
4. `type` 不是字符串的，报 serde_json 的原话（`invalid type`）。
5. 认识的七种，照那一种读整块。缺了格、某一格不合写法的，报那一格的错，例如「missing field `width`」「bad content hash: …」「bad call id: …」「bad driver family: …」。
6. 认识的种类多出来的格不管：读进内存时丢掉，写出去不再有。
7. 不认识的种类：整块原样留着（`Block::Unknown`），写出去一字不差，空格、数字的写法都不变。

**写一块**：

8. `type` 在最前，其余几格照上表的先后。
9. `private`、图片块的 `name`、图片块和文件块的 `path`（施工 3-9 五补，排在最后）、`path` 块的 `size`、`hosted_tool` 的 `outcome` 没有就不写这一格；`outcome` 里的 `failed`、`cut` 是假的不写，读时缺了当假。
10. 不认识的块照原文写。

**块里的规矩**：

11. 工具名和参数不检查：模型说了什么就记什么，名字不对、参数坏了，是执行时报给模型的错，日志照样读得进来（`kernel/tools.md`）。
12. 参数存模型给出的原文，不解析以后重新写：重新写会改变字节，前缀缓存随之失效。执行用的是修正过的另一份，日志里的不动（`kernel/tools.md`）。
13. 调用编号由内核分，照这条回复的序号写成 `call_<序号>_<第几个>`（`kernel/request.md` 的流式累积器）；供应商自己的编号放在 `private` 里。中途换模型、换供应商，编号照样一致。供应商侧工具（施工 HT-1）不和工具调用混着数：工具调用照先后排 `_1` 到 `_k`（k 是这条回复里的工具调用数），供应商侧工具照先后接着排 `_k+1` 起（`kernel/ids.md`）。
    供应商侧工具（施工 HT-1，设计 71 第三节第 1 条）：核心不派、不过权限、不等结果，照实记下（`kernel/session.md`「收回复」）。不是正文：`words` 不算它。另成一种块、不借 `tool_call`：工具调用到处照它派、等结果；旧版本读到 `hosted_tool` 照第 7 条当不认识的块原样留着，编号照上一段排，旧版本只数工具调用也对得上，日志读得进。
14. 图片、文件本身不进事件：块里只放 blob 的内容哈希，内容存成 blob（`store.md`）。图片块的宽、高必有：量得出尺寸才当图片。造它们的：`read` 读图片造图片块（施工 4-13，`tools/read.md`），人附的附件造图片块、文件块（施工 3-9 三补，`protocol.md` 的 `blob.put`、`session.send`），都是进来时量好。图片块的名字（施工 3-9 四补）：人附的图片带，是 `blob.put` 回应里的那个名字，她分得清一句话里的几张图哪张是哪个文件；`read` 读出来的不带，那一次调用本来写着路径。以前的日志里图片块没有这一格，照读，写出去也没有。
    只有路径的附件（施工 AT-1，2026-11-11 项目主人定：上传上限不动，超限的文件只把路径交给她）另成一种块 `path`，不借文件块：文件块的 `blob`、`media_type` 必有，执行器、驱动、导出、视图到处照它取内容、取大小、认类型；把那两格改成可以没有，旧版本读到没有 `blob` 的文件块整条事件读不进（缺格），新加一种块旧版本照第 7 条原样留着、投影跳过。造它的只有 `session.send`、`session.redo`（`protocol.md`）：核心不读那个文件，`name`、`path`、`size` 照头写的记。请求里它写成一句（`drivers/openai-chat.md` 第 9 条）；本地估算照这一块规范的 JSON 的字节数（`compaction.md` 第二条第 2 条）。
15. 格式上哪一种块放在哪里都读得进来。谁放什么，见各种事件的 `blocks`（`kernel/events-bodies.md`）。
16. 投影跳过不认识的块：给模型看的只有认识的七种（`crates/miyu-assemble/src/render.rs` 的 `known`，`kernel/request.md`）。供应商侧工具（施工 HT-2）照原来的位置进 assistant 消息，内置驱动写成一段字（`drivers/openai-chat.md` 第 4 条）；交给扩展驱动的，自己家的原样、别家的换成同一段字（`extensions.md`「扩展提供的模型驱动」）。HT-1 时它和不认识的块一起跳过。

**字符串取值**：

17. 读：必须是 JSON 字符串，数字、`null` 读不进来。认识的值一定读成对应的一种；别的读成 `Other(原文)`，不报错。
18. 写：认识的写它的写法，`Other` 写原文。所以读进来再写出去一字不差。
19. 不认识的值当什么，由用到它的地方决定：

| 取值 | 读到不认识的 | 在哪 |
|---|---|---|
| `Access` | 算写入：只读时当场拦下；不和别的调用一起跑 | `crates/miyu-kernel/src/tool.rs` 的 `writes`，`crates/miyu-kernel/src/session/step.rs` 的 `ready`（`kernel/tools.md`） |
| `Level` | 按最严的算：实际生效的是只读，告诉她的也是 `read_only`，执行前的链也按只读判。切级别的命令带着不认识的级别，拒绝，原因码 `unknown_level` | `crates/miyu-kernel/src/session/permission.rs` 的 `rank`、`set_permission`，`crates/miyu-kernel/src/facts.rs` 的 `effective_level`，`crates/miyu-session/src/guard.rs` 的 `effective` |
| `EndReason` | 投影不写回合没走完的那一句（`resources/core/turn-ended/` 下的几句一句都不用），和 `completed` 一样 | `crates/miyu-assemble/src/texts.rs` 的 `for_reason` |
| `ToolStatus` | 投影里算没成（tool 消息的 `error` 是真） | `crates/miyu-assemble/src/render.rs` |
| `Decision` | 回答确认的命令带着不认识的决定，拒绝，原因码 `unknown_decision` | `crates/miyu-kernel/src/session/approval.rs` 的 `answer` |
| `ErrorClass` | 不重试 | `crates/miyu-kernel/src/session/retry.rs` 的 `retryable` |
| `Part`、`MessageRole`、`CallResult` | 内核只写不读 | — |

### 出错

读不进来的，交回 serde_json 的错误，原话见上面「读一块」第 2 到 5 条；在一行事件里的，还要套上外壳的报法（`kernel/events.md`「出错」）。这些报错只有中文和 serde_json 的英文原话，给查问题的人看。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-kernel/src/block/tests.rs` | 供应商侧工具带不带 `outcome`、`private`、`failed`、`cut` 都读写一字不差、认得出种类，缺了的 `failed`、`cut` 读成假，不算正文，缺格的报错（`a_hosted_tool_round_trips_with_or_without_outcome`，施工 HT-1）；只有路径的附件读写一字不差、认得出种类、没写大小的不写、坏的报错（`a_path_only_attachment_is_known_and_round_trips`，施工 AT-1）；五种读写一字不差、认得出种类，图片块带名字、不带名字的都一字不差（`every_block_from_the_drawing_round_trips`）；驱动私有数据一字不差（`private_data_is_kept_byte_for_byte`）；第 11、12 条工具名和参数照原文（`tool_call_keeps_name_and_args_as_the_model_wrote_them`）；第 7 条（`unknown_block_is_kept_byte_for_byte`）；第 3、5 条的坏写法，图片块的名字带路径的也算（`broken_blocks_are_errors`） |
| `crates/miyu-kernel/src/text_enum/tests.rs` | 第 17、18 条：认识的读成对应的一种、不认识的原样留着、只收字符串 |
| `crates/miyu-kernel/src/event/*/tests.rs`、`crates/miyu-kernel/src/tool/tests.rs` | `Level`、`EndReason`、`ToolStatus`、`Decision`、`ErrorClass`、`Access` 每个认识的值读成自己那一种，不认识的原样留着（`each_…_reads_into_its_own_variant`、`an_unknown_…_is_kept_as_it_is`、`access_is_written_as_text_and_unknown_kinds_are_kept`）；`Part`、`MessageRole`、`CallResult` 只查了几个值的写法，没有不认识的值的测试 |
| `crates/miyu-kernel/src/tool/tests.rs` 的 `writing_files_and_unknown_kinds_count_as_writing`；`crates/miyu-kernel/src/session/tests/permission.rs` 的 `an_unknown_level_is_rejected`；`crates/miyu-kernel/src/facts/tests.rs` 的 `the_permission_block_names_the_level_in_effect`；`crates/miyu-session/src/guard/tests.rs`；`crates/miyu-kernel/src/session/tests/approval.rs` 的 `answers_that_do_not_fit_are_rejected` | 第 19 条表里的 `Access`、`Level`、`Decision` 几行 |

### 出处

- `03-事件模型.md` 第四节：内容块，每种块的写法，工具调用存原文，调用编号由内核分，不认识的原样留着。
- `03-事件模型.md` 第八节：格式演进，可以加字段、加种类；第九节：供应商的原样数据。
- `03-事件模型.md` 第三节「会话与回合的事件怎么写」：取值读到不认识的，原样留着，当什么由用到它的地方决定。
- `03-事件模型.md` E1（工具调用是内容块）、E2（驱动私有附件）、E4（大内容存成 blob）。

### 还没有的

- `read` 读 PDF 造文件块（`26-提示词.md` 第十节登记簿 `read.json` 那一行）。
- 超长的工具输出存成 blob、事件里只放引用（`03-事件模型.md` 第三节「大输出的全文」，`08-上下文投影.md` C9）。
- 中途换供应商时，思考按策略降级成普通文本（`03-事件模型.md` 第九节）。
