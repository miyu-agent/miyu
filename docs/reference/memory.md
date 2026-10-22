# 记忆与知识库：`memory.*`、`kb.*`

英文版：[memory.en.md](memory.en.md)。

| 方法 | 做什么 |
|---|---|
| [`memory.list`、`memory.search`](#memorylistmemorysearch) | 列、搜记忆 |
| [`memory.turns`](#memoryturns) | 搜以前的对话 |
| [`memory.remember`、`memory.update`、`memory.forget`、`memory.restore`](#memoryremembermemoryupdatememoryforgetmemoryrestore) | 记、改、作废和清空、恢复 |
| [`memory.cards`、`memory.card_update`、`memory.card_forget`](#人卡memorycardsmemorycard_updatememorycard_forget) | 认识的人的卡：列、改、作废 |
| [`memory.dream`](#memorydream) | 现在就整理记忆 |
| [`memory.dream_stop`](#memorydream_stop) | 叫停正在进行的整理 |
| [`kb.list`、`kb.search`、`kb.read`、`kb.documents`](#kblistkbsearchkbreadkbdocuments) | 列知识库、搜、读一页、列一个库里的文档 |
| [`kb.create`、`kb.update`、`kb.mount`、`kb.delete`、`kb.add`、`kb.remove`、`kb.reindex`](#kb-的管理) | 管理知识库 |

**谁能调**：人、扩展；扩展写的（记住、改、忘、恢复、整理、改和作废人卡，知识库加、改、挂、删、重建索引）要能力 `memory.write`。人经协议是主人这一边：全部列得出、搜得到、改得了。

## 记忆的「一间」

记忆一间一间分开放：跟着人格的（这个人格的所有会话共用），只在一个会话里的（会话的记忆范围是 `session`），不用人格的会话另有一间。每个 `memory.*` 方法都收这两格之一来指哪一间：

| 名字 | 说明 |
|---|---|
| `persona` | 这个人格那一间；写 `_none` 是不用人格的会话那一间（人格列表里没有它，界面自己列一项） |
| `session` | 这个会话用的那一间 |

都不写的照默认人格。要一直跟着一间的变化，订阅 `memory` 流（[连接](connection.md)）。

**共同的出错**

| `data.reason` | 什么时候 |
|---|---|
| `memory_not_installed` | 记忆这个软件包没装（装上就能用，以前记的都在） |
| `memory_unavailable` | 这里没有记忆：会话的范围是 `off`、场所会话 |
| `unknown_persona`、`session_not_found` | 找不到那个人格、会话 |
| `bad_params`（`-32602`） | 两格都写了 |

## 一条记忆

`{"id", "class", "text", "by", "at", "sources", "retired", "used", "used_at", "venue", "keywords", "who"}`：

| 名字 | 说明 |
|---|---|
| `id` | `m<序号>` |
| `class` | `user`（关于主人）、`feedback`（人对她的要求）、`episode`（发生过的事）、`reference`（资料在哪）、`person`（她认识的别人，界面上叫「认识的人」） |
| `text` | 那一条，最多 120 个字 |
| `by` | `person`（人记的）或 `tool`（她记的） |
| `at` | 记下的时刻 |
| `sources` | 出处 `[{"session", "turn"}]` |
| `retired` | 作废的原因，没作废的 `null` |
| `used`、`used_at` | 用到几次、最近一次的时刻 |
| `venue` | 在哪个场所说的，本机的 `null` |
| `keywords` | 只用来检索的关键词（一行、逗号隔开，最多 200 字），没有的 `null`：照它也搜得到、想得起，她看不到这一行 |
| `who` | `person` 那一类说的是谁：平台编号（`qq:10086`）或名字；别的类是 `null` |

## `memory.list`、`memory.search`

| 方法 | 参数 | 回应 |
|---|---|---|
| `memory.list` | `class`、`from`（会话编号：出处在它里面的）、`forgotten`（布尔，连作废的一起）、`limit`（1 到 500，默认 50） | `{"memories": [...]}`，新的在前 |
| `memory.search` | `query`（必写）、`forgotten`、`limit`（1 到 500，默认 10） | `{"memories": [...]}`，最相关的在前 |

## `memory.turns`

搜这一间以前的对话：人开的一轮一段，照关键词和意思找。

**参数**：`query`（必写）、`limit`（1 到 500，默认 10）。**回应**：`{"turns": [{"session", "turn", "at", "text"}…]}`：`text` 是人的话、空一行、她最后的回答。撤销了的一轮、删了的会话不在里面。

## `memory.remember`、`memory.update`、`memory.forget`、`memory.restore`

| 方法 | 参数 | 回应 |
|---|---|---|
| `memory.remember` | `class`、`text`（都必写）；`keywords` 可以不写；`who`：`class` 是 `person` 的必写，别的类写了不收 | `{"id"}` |
| `memory.update` | `id`，`text`、`keywords`、`who` 至少写一个 | `{"id"}`：写了 `text` 的是改完的新那一条（旧的作废，没写 `keywords`、`who` 的带过去）；只写 `keywords` 的是同一条，编号不变；只写 `who` 的照旧文新记一条；`keywords` 写空的是去掉 |
| `memory.forget` | `id`（可带 `why`）；或者 `clear`：`session`（清掉这个会话记下的，这时 `session` 必写）、`me`（清空整间） | 作废的 `{}`，清空的 `{"cleared": <几条>}` |
| `memory.restore` | `id` | `{}`：作废了的那一条重新算数 |

落了盘才回应。同一个 `id`（请求编号）再发只算一次，核心重启以后也是。清掉的找不回来、哪里都不再出现。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 字是空的；`class` 不对；`person` 没写 `who`；`id` 写法不对；`id`、`clear` 不是正好一个；`memory.update` 的 `text`、`keywords`、`who` 都没写 |
| `memory_too_long` | 正文超过 120 个字、关键词超过 200 个字，`data.chars`、`data.limit`，`data.field` 是 `text` 或 `keywords` |
| `unknown_memory` | 没有这一条 |
| `memory_not_current` | 那一条已经改掉、作废、清掉了 |

## 人卡：`memory.cards`、`memory.card_update`、`memory.card_forget`

她认识的人一人一张卡：他是谁、怎么认识的、喜好、近况、生日、联系方式，最多 300 个字。整理记忆时照这一批里关于他的记忆（`class` 是 `person` 的）合，人也能改。一张卡：

| 名字 | 说明 |
|---|---|
| `who` | 是谁：平台编号（`qq:10086`）或名字，同一条记忆的 `who` |
| `text` | 卡的正文，最多 300 个字 |
| `names` | 他的几个叫法（群名片、外号），最多 8 个；搜得到，不拿来认人 |
| `at` | 写下的时刻 |
| `by` | `module`（整理写的）或 `person`（人改的） |
| `retired` | 作废的原因，没作废的 `null` |

| 方法 | 参数 | 回应 |
|---|---|---|
| `memory.cards` | `forgotten`（布尔，连作废的一起） | `{"cards": [...]}`，照 `who` 排，一人一张（最新的那一张） |
| `memory.card_update` | `who`、`text`（都必写），`names` 可以不写 | `{}`：写一张新的，这个人以它为准；作废了的也重新算数 |
| `memory.card_forget` | `who`，可带 `why` | `{}`：这张卡作废；已经作废的什么都不写 |

整间清空（`memory.forget` 的 `clear: me`）以后卡也没了；只清一个会话的不动卡。落了盘才回应，同一个请求编号再发只算一次。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | `who`、`text` 是空的；叫法有空的、超过 40 个字的、超过 8 个 |
| `memory_too_long` | 正文超过 300 个字，`data.chars`、`data.limit`，`data.field` 是 `text` |
| `unknown_memory` | `memory.card_forget` 的这个人没有卡 |

## `memory.dream`

现在就整理：带 `session` 的先把这个会话还没抽的抽了，再把新记的和以前的合一遍（重复的合掉，过时的作废，写一段对人的了解）。在后台答，几秒到十几秒。

**参数**：只有 `persona` 或 `session`。**回应**：`{"given", "retired", "revised", "summary"}`：交进去几条、作废几条、改了几条、对人的了解换没换（布尔）。一条记忆都没有的四样都是零，不请求模型；没有新记的照样重看轮到的旧记忆（R-19）。叫停了（[`memory.dream_stop`](#memorydream_stop)，会话里叫的还有那个会话的 `/dream stop`、`/stop`）照样回，几样数是叫停以前合完了的，多 `"stopped": true`（R-22、R-23）：正在合的那一批放弃、下次再合，合完了的留着。

**出错**：`memory_busy`（这一间正在整理）；`dream_failed`（没合成，`data.message`）。

**进度和做了什么**：照记忆日志看（订阅 `memory` 流）。整理一批一批合，人叫的、后台自己合的都写：

| `kind` | `body` | 是什么 |
|---|---|---|
| `ext.memory.merging` | `upto`、`fresh`、`left`、`manual`、`given` | 开始合一批：这一批几条新记的，连它在内还有几条，是不是人叫的 |
| `ext.memory.saved` | 写了 `replaces` 的 | 改了一条：新的正文；旧的是 `replaces` 那一条 |
| `ext.memory.retired` | `id`、`why`、`kind`、`kept` | 删了一条：`kind` 是 `outdated`（被后记的取代）、`repeat`（重复）、`worthless`（用不着），`kept` 是留下的那一条（`worthless` 没有）。人删的没有 `kind`、`kept` |
| `ext.memory.summary` | `text` | 新的对人的了解 |
| `ext.memory.card` | `who`、`text`、`names`? | 这一批里有认识的人的，合出来的他的卡（和现在那一张一样的不写）；人改的也是这一条，`by` 是那个人 |
| `ext.memory.card_retired` | `who`、`why` | 人作废了一张卡 |
| `ext.memory.tagged` | `id`、`keywords` | 补了关键词：在这一批的 `merged` 后面 |
| `ext.memory.merged` | `upto`、`given`、`revised`、`retired`、`failed`? | 这一批合完了（`failed` 是连着三次没成、跳过了） |
| `ext.memory.merge_failed` | `upto`、`why`、`tries`? | 这一批这一次没合成；`why` 是 `interrupted` 的是核心半路停了，`stopped` 的是人叫停了（R-22），这两种不写 `tries` |
| `ext.memory.swept` | `upto`、`count` | 顺带重看了几条旧记忆、轮到第几条（施工 R-19）：在这一批的 `merged` 后面 |

- `merging` 和收尾它的 `merged`、`merge_failed` 带同一个 `upto`；这中间 `by` 是记忆模块的改、删、摘要就是这一批做了什么（同一时刻别的会话可能在记、人可能在改，`by` 分得开）。没收尾的是正在合。
- 进度：头一批的 `left` 是总数，收了尾的各批 `fresh` 加起来是合了多少。一批之内不细分。
- 人叫的整理要画「她正在写什么」（施工 R-21）：`merging` 的 `given` 是这一批交给模型的是哪几条，等的时候可以让它们浮过去；另有不落盘的推送 `memory.dreaming`：她每写完一条改的（`kind: revised`，`id`、`text`）、作废的（`kind: retired`，`id`、`retire`、`kept`、`why`）推一条，摘要每多 20 个字、写完推一条（`kind: summary`，`chars`、`done`），都带这一批的 `upto`。推的是她这一刻写的，落的照全部核对、可能少几条，以日志为准；掉了不补。
- 每一趟头一个放得下的批顺带重看最多 20 条轮到的旧记忆（照编号轮，轮完一圈从头来），对它们的改、删同样算这一批做的。人叫的整理没有新记的也照样重看：那一批的 `fresh`、`left` 是 0。
- 会话里叫的先抽的那几条（`ext.memory.saved`，`by` 是记忆模块，带出处）在第一条 `merging` 前面，后面跟 `ext.memory.extracted`。

## `memory.dream_stop`

叫停这一间这时人叫的整理（R-23），不管是哪一路叫的（`memory.dream`、会话里的 `/dream`）；后台自己合的不停。合完了的批留着，正在合的那一批放弃、下次整理再合；记忆日志记一条 `ext.memory.merge_failed`（`why: stopped`）。被叫停的那条 `memory.dream` 照样回应，带 `"stopped": true`。`memory.dream` 在后台答，在同一条连接上接着发它就行。

**参数**：只有 `persona` 或 `session`，同 `memory.dream`。**回应**：`{"stopped": true}`。

**出错**：`not_dreaming`（这一间这时没有人叫的整理）。

## `kb.list`、`kb.search`、`kb.read`、`kb.documents`

知识库在管理员账号下，一个库是一个目录里的文档（Markdown、PDF、网页……），核心建索引来搜。不看知识库软件包装没装：文件是人的。

| 方法 | 参数 | 回应 |
|---|---|---|
| `kb.list` | 没有 | `{"bases": [{"id", "name", "description", "documents", "chunks", "auto"}…]}`，照编号排；`auto` 是自动想起开着没有；读不了的另带 `error`，软件包带来的另带 `package`，挂进来的另带 `mount`（写的那个目录）；`indexing` 是这时在不在建（别处正在建的不等它，照索引现有的样子列），建好过的另带 `indexed_at`（上次建好的时刻，施工 W-15）；建的进度订阅 `kb` 流 |
| `kb.search` | `query`（必写）、`kb`（只搜这一个）、`limit`（1 到 50，默认 10） | `{"hits": [{"kb", "path", "chunk", "heading", "text", "at"}…]}`，最相关的在前；PDF 的多 `page` |
| `kb.read` | `kb`、`path`（必写）、`page`（从 1 起，默认 1） | `{"text", "page", "pages"}`：一页 32 KiB；PDF 照它自己的页 |
| `kb.documents` | `kb`（必写） | `{"documents": [{"path", "kind", "size", "at", "chunks"}…], "folders": [{"path", "reason", "why"}…]}`，照路径排：库里的每一篇；这一回读不了的另带 `reason`、`why` |

`heading` 是标题路径（`语法 › て形`），`at` 是文档改的时刻。

`kb.documents` 里 `kind` 是 `markdown`、`text`、`pdf`、`html`；`size` 是字节数；`chunks` 是索引里几块（读不了的以前进过索引的留着旧块）。`reason` 是 `too_large`、`not_utf8`、`no_text`（PDF 里没有字：扫描的、只有图）、`no_extractor`（PDF、网页要 `miyu-extract`）、`extract_failed`、`unreadable` 之一，给人看的字头自己写，`why` 是给日志的英文。`folders` 是读不了的子目录（挂进来的常有）。不是文档的文件不列。挂的目录不在了：`folder_unavailable`。

**出错**：`bad_params`（`page` 是 0 或超过 `pages`）；`unknown_kb`；`unknown_document`；`internal_error`（坏的 PDF、文件超过 64 MiB）。

## `kb` 的管理

| 方法 | 参数 | 回应 |
|---|---|---|
| `kb.create` | `kb`（必写）、`name`、`description`、`auto` | 一个库 |
| `kb.update` | `kb`；`name`、`description`、`auto` 至少一个（空字、`false` 是拿掉） | 一个库 |
| `kb.mount` | `kb`、`folder`（绝对路径或 `~/`）；`name`、`description`、`auto` | 一个库：把一个现成的目录挂进来 |
| `kb.delete` | `kb` | `{"trash"}`：挪进系统回收站的位置；挂进来的只拿掉登记，目录不动 |
| `kb.add` | `kb`、`path`、`blob`（先 `blob.put`）、`replace`（布尔） | `{}`：加一篇，什么格式都行 |
| `kb.remove` | `kb`，`path`（一样）或者 `paths`（几样）：一篇文档，或者一个文件夹（库里的相对路径，整个删） | `path` 的回 `{"trash"}`，`paths` 的回 `{"trash": [...]}`，照先后；有一样不在就一样都不删；两样都写或都没写、重了、一样在另一样里面的 `bad_params` |
| `kb.reindex` | `kb` | 一个库：删掉索引照文件重建 |

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `kb_exists`、`document_exists` | 已经有了（加文档的不带 `replace`） |
| `kb_read_only` | 往挂进来的库里加、删 |
| `kb_packaged` | 改软件包带来的库（只能改 `auto`） |
| `folder_unavailable` | 要挂的目录用不了、`kb.documents` 的库挂的目录不在了，`data.message` |
| `trash_failed` | 挪不进回收站，`data.message` |
| `unknown_attachment` | `blob` 不在 |
| `unknown_kb`、`unknown_document`、`bad_params` | 同上 |
