# 记忆与知识库：`memory.*`、`kb.*`

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

| 方法 | 做什么 |
|---|---|
| [`memory.list`、`memory.search`](#memorylistmemorysearch) | 列出记忆、搜索记忆 |
| [`memory.turns`](#memoryturns) | 搜索以前的对话 |
| [`memory.remember`、`memory.update`、`memory.forget`、`memory.restore`](#memoryremembermemoryupdatememoryforgetmemoryrestore) | 记住、修改、作废和清空、恢复记忆 |
| [`memory.import`](#memoryimport) | 把其他地方的记忆成批导入（例如从旧版迁移） |
| [`memory.cards`、`memory.card_update`、`memory.card_forget`](#人卡memorycardsmemorycard_updatememorycard_forget) | AI 认识的人的卡片：列出、修改、作废 |
| [`memory.dream`](#memorydream) | 立即整理记忆 |
| [`memory.dream_stop`](#memorydream_stop) | 叫停正在进行的整理 |
| [`kb.list`、`kb.search`、`kb.read`、`kb.documents`](#kblistkbsearchkbreadkbdocuments) | 列出知识库、搜索、读取一页、列出一个库里的文档 |
| [`kb.create`、`kb.update`、`kb.mount`、`kb.delete`、`kb.add`、`kb.remove`、`kb.reindex`](#kb-的管理) | 管理知识库 |

**谁能调用**：人和扩展都能调用。扩展调用写入类的方法（记住、导入、修改、忘记、恢复、整理，修改和作废人卡；知识库的添加、修改、挂载、删除、重建索引）需要能力 `memory.write`。人通过协议调用时代表主人这一边：所有内容都能列出、搜索和修改。

## 记忆的「一间」

记忆按「间」分开存放：每个人格有一间（这个人格的所有会话共用），不使用人格的会话另有一间。每个 `memory.*` 方法都接受下面两个参数之一，用来指定是哪一间（填写会话时，指的是那个会话的人格的那一间）：

| 名字 | 说明 |
|---|---|
| `persona` | 这个人格的那一间；填写 `_none` 时指不使用人格的会话的那一间（人格列表里没有这一项，界面需要自己列出一项） |
| `session` | 这个会话使用的那一间 |

两个参数都不填写时，使用默认人格的那一间。想要一直跟着一间记忆的变化，可以订阅 `memory` 流（见 [连接](connection.md)）。

**共同的出错**

| `data.reason` | 什么时候 |
|---|---|
| `memory_not_installed` | 没有安装记忆这个软件包（安装以后就能使用，以前记下的内容都还在） |
| `memory_unavailable` | 这里没有记忆：会话的记忆范围是 `off`，或者这是一个场所会话 |
| `unknown_persona`、`session_not_found` | 找不到指定的人格或会话 |
| `bad_params`（`-32602`） | 两个参数都填写了 |

## 一条记忆

一条记忆是 `{"id", "class", "text", "by", "at", "sources", "retired", "used", "used_at", "venue", "keywords", "who"}`，各字段如下：

| 名字 | 说明 |
|---|---|
| `id` | 写法是 `m<序号>` |
| `class` | `user`（关于主人的）、`feedback`（人对 AI 的要求）、`episode`（发生过的事）、`reference`（资料在哪里）、`person`（AI 认识的其他人，界面上叫「认识的人」） |
| `text` | 这一条记忆的正文，最多 120 个字 |
| `by` | `person`（人记下的）或 `tool`（AI 记下的） |
| `at` | 记下这一条的时刻 |
| `sources` | 这一条的出处，写法是 `[{"session", "turn"}]` |
| `retired` | 作废的原因；没有作废时是 `null` |
| `used`、`used_at` | 用到过几次、最近一次用到的时刻 |
| `venue` | 这一条是在哪个场所说的；本机会话里说的是 `null` |
| `keywords` | 只用来检索的关键词（一行，用逗号隔开，最多 200 字）；没有关键词时是 `null`。搜索和回想时都能通过关键词找到这一条，但是 AI 看不到这一行 |
| `who` | `person` 这一类记忆说的是谁：平台编号（例如 `qq:10086`）或者名字；其他类是 `null` |
| `people` | `episode` 这一类记忆涉及哪些人：平台编号或名字的列表（不包含 AI 自己和主人）；没有涉及其他人时，以及其他类，都是空列表 |
| `place` | 记下时在哪个群：记下那一刻群的名字（桥传来的群名）；私聊、本机会话、人通过协议记下的、以前记下的记忆是 `null`。群后来改了名字时，以前记下的记忆仍然是当时的名字 |

## `memory.list`、`memory.search`

| 方法 | 参数 | 回应 |
|---|---|---|
| `memory.list` | `class`、`from`（会话编号：只列出出处在这个会话里的记忆）、`forgotten`（布尔，是否连作废的一起列出）、`limit`（1 到 500，默认 50） | `{"memories": [...]}`，新的排在前面 |
| `memory.search` | `query`（必须填写）、`forgotten`、`limit`（1 到 500，默认 10） | `{"memories": [...]}`，最相关的排在前面 |

## `memory.turns`

这个方法搜索这一间记忆以前的对话：人开始的每一轮是一段，按照关键词和意思查找。

**参数**：`query`（必须填写）、`limit`（1 到 500，默认 10）。**回应**：`{"turns": [{"session", "turn", "at", "text"}…]}`：`text` 是人说的话、一个空行，再加上 AI 最后的回答。已经撤销的轮和已经删除的会话不在结果里。

## `memory.remember`、`memory.update`、`memory.forget`、`memory.restore`

| 方法 | 参数 | 回应 |
|---|---|---|
| `memory.remember` | `class`、`text`（都必须填写）；`keywords` 可以不填写；`who`：`class` 是 `person` 时必须填写，其他类填写了也不接受 | `{"id"}` |
| `memory.update` | `id`，以及 `text`、`keywords`、`who` 中至少一个 | `{"id"}`：填写了 `text` 时，返回的是改完以后的新一条（旧的一条作废，没有填写的 `keywords`、`who` 沿用旧的）；只填写 `keywords` 时还是同一条，编号不变；只填写 `who` 时用原来的正文新记一条；`keywords` 填写空值表示去掉关键词 |
| `memory.forget` | `id`（可以带 `why`）；或者 `clear`：`session`（清掉这个会话记下的记忆，此时必须填写 `session`）、`me`（清空整间） | 作废时返回 `{}`，清空时返回 `{"cleared": <几条>}` |
| `memory.restore` | `id` | `{}`：作废了的那一条重新生效 |

内容写入磁盘以后才返回回应。同一个 `id`（请求编号）再发送一次只生效一次，核心重启以后也是这样。清掉的记忆无法找回，也不会再出现在任何地方。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 正文是空的；`class` 不对；`person` 类没有填写 `who`；`id` 的写法不对；`id` 和 `clear` 不是正好填写一个；`memory.update` 的 `text`、`keywords`、`who` 都没有填写 |
| `memory_too_long` | 正文超过 120 个字，或者关键词超过 200 个字；另带 `data.chars`、`data.limit`，`data.field` 是 `text` 或 `keywords` |
| `unknown_memory` | 没有这一条记忆 |
| `memory_not_current` | 这一条已经被修改、作废或者清掉了 |

## `memory.import`

这个方法把其他地方的记忆成批导入，并且保留原来记下的时刻（例如从旧版 Miyu 迁移）。导入的记忆和人记下的一样：`by` 是 `person`，出处是空的。

**参数**：`items`，1 到 500 条，每一条的字段如下：

| 名字 | 说明 |
|---|---|
| `at` | 原来记下的时刻，使用 RFC 3339 格式，必须带时区（例如 `2026-10-21T10:53:22+08:00`、`…Z`），不能晚于现在。必须填写 |
| `class` | 和 `memory.remember` 相同。必须填写 |
| `text` | 正文，不超过 120 个字。必须填写 |
| `origin` | 这一条在原来那边是哪一条，1 到 200 个字，同一批里不能重复（例如 `legacy:default:facts:57`）。必须填写 |
| `who` | `class` 是 `person` 时必须填写，其他类不接受 |
| `people` | 涉及哪些人（字符串的列表），只有 `class` 是 `episode` 时才能填写 |
| `keywords` | 只用来检索的关键词，不超过 200 个字，可以不填写 |

**回应**：`{"ids": [...], "imported": <几条>, "skipped": <几条>}`。`ids` 和 `items` 一一对应：记下的那一条是它的编号，跳过的那一条是 `null`。

- 先检查全部再写入：只要有一条不对，整批都不写入，并返回 `bad_params`；`data.index` 是第几条（从 0 开始数，整批的问题没有这一项），`data.field` 是哪一项（`items`、`item`、`at`、`class`、`text`、`origin`、`who`、`people`、`keywords`）。
- 去重：这一间以前导入过同一个 `origin` 的条目会被跳过，已经作废、修改、清空的也算导入过，所以重新运行迁移不会把删掉的记忆再导回来。同一个 `id` 再发送一次也按照这一条处理：全部跳过。
- 列出、搜索到的记忆里，`at` 是原来的时刻；编号按照导入的先后排列，算作这一次新记下的。
- 没有带 `keywords` 的条目，会在返回回应以后在后台另外写关键词；导入了一条以上时，按照「整理」的间隔决定要不要整理（这一间从来没有整理过的会马上整理）。想要马上去重，就接着调用 `memory.dream`。

## 人卡：`memory.cards`、`memory.card_update`、`memory.card_forget`

AI 为认识的每个人各写一张卡：内容是 AI 对这个人的印象（这个人是谁、是什么样的人、对 AI 怎么样，兴趣和近况概括着写），再加上生日、联系方式，最多 400 个字。整理记忆时，会根据这一批里关于这个人的记忆（`class` 是 `person` 的）合成这张卡；人也可以修改人卡。一张卡的字段如下：

| 名字 | 说明 |
|---|---|
| `who` | 这张卡是谁的：平台编号（例如 `qq:10086`）或者名字，和记忆的 `who` 相同 |
| `text` | 卡的正文，最多 400 个字 |
| `names` | 这个人的几种叫法（群名片、外号），最多 8 个；可以用来搜索，但是不用来识别是谁 |
| `at` | 写下这张卡的时刻 |
| `by` | `module`（整理时写的）或 `person`（人修改的） |
| `retired` | 作废的原因；没有作废时是 `null` |

| 方法 | 参数 | 回应 |
|---|---|---|
| `memory.cards` | `forgotten`（布尔，是否连作废的一起列出） | `{"cards": [...]}`，按照 `who` 排列，每人一张（最新的那一张） |
| `memory.card_update` | `who`、`text`（都必须填写），`names` 可以不填写 | `{}`：写入一张新卡，这个人以新卡为准；如果这个人的卡已经作废，也会重新生效 |
| `memory.card_forget` | `who`，可以带 `why` | `{}`：这张卡作废；已经作废的卡不会再写入任何内容 |

清空整间（`memory.forget` 的 `clear: me`）以后，人卡也会一起清掉；只清一个会话时不影响人卡。内容写入磁盘以后才返回回应，同一个请求编号再发送一次只生效一次。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | `who` 或 `text` 是空的；叫法里有空的、有超过 40 个字的，或者叫法超过 8 个 |
| `memory_too_long` | 正文超过 400 个字；另带 `data.chars`、`data.limit`，`data.field` 是 `text` |
| `unknown_memory` | `memory.card_forget` 指定的这个人没有卡 |

## `memory.dream`

这个方法立即整理记忆：带了 `session` 时，先把这个会话里还没有抽取的内容抽取出来，再把新记下的记忆和以前的合并一遍（合并重复的，作废过时的，写一段对人的了解）。它在后台回答，需要几秒到十几秒。

**参数**：只有 `persona` 或 `session`。**回应**：`{"given", "retired", "revised", "summary"}`：交给模型几条、作废几条、修改几条，以及对人的了解有没有更新（布尔）。如果一条记忆都没有，四项都是零，也不请求模型；没有新记下的记忆时，仍然会重看轮到的旧记忆。如果整理被叫停（[`memory.dream_stop`](#memorydream_stop)；从会话里发起的整理，还可以用那个会话的 `/dream stop`、`/stop` 叫停），仍然会返回回应，各项数字是叫停以前已经合并完的部分，另外多一项 `"stopped": true`：正在合并的那一批放弃，下次再合并，已经合并完的保留。

**出错**：`memory_busy`（这一间正在整理）；`dream_failed`（合并没有成功，另带 `data.message`）。

**进度和做了什么**：可以从记忆日志里查看（订阅 `memory` 流）。整理是一批一批合并的，人发起的整理和后台自动的整理都会写进日志：

| `kind` | `body` | 是什么 |
|---|---|---|
| `ext.memory.merging` | `upto`、`fresh`、`left`、`manual`、`given` | 开始合并一批：这一批有几条新记下的，包括这一批在内还剩几条，是不是人发起的 |
| `ext.memory.saved` | 写了 `replaces` 的 | 修改了一条：内容是新的正文，旧的是 `replaces` 指的那一条 |
| `ext.memory.retired` | `id`、`why`、`kind`、`kept` | 删掉了一条：`kind` 是 `outdated`（被后来记下的取代）、`repeat`（重复）、`worthless`（用不着），`kept` 是保留下来的那一条（`worthless` 没有这一项）。人删掉的没有 `kind`、`kept` |
| `ext.memory.summary` | `text` | 新写的对人的了解 |
| `ext.memory.card` | `who`、`text`、`names`? | 这一批里涉及认识的人时，合并出来的这个人的卡（和现在那一张相同时不写）；人修改人卡时也记这一条，`by` 是那个人 |
| `ext.memory.card_retired` | `who`、`why` | 人作废了一张卡 |
| `ext.memory.tagged` | `id`、`keywords` | 补上了关键词：排在这一批的 `merged` 后面 |
| `ext.memory.merged` | `upto`、`given`、`revised`、`retired`、`failed`? | 这一批合并完了（带 `failed` 表示连续三次没有成功，已经跳过） |
| `ext.memory.merge_failed` | `upto`、`why`、`tries`? | 这一批这一次没有合并成功；`why` 是 `interrupted` 表示核心中途停止了，是 `stopped` 表示人叫停了，这两种情况不写 `tries` |
| `ext.memory.swept` | `upto`、`count` | 顺带重看了几条旧记忆、轮到了第几条：排在这一批的 `merged` 后面 |

- `merging` 和收尾它的 `merged` 或 `merge_failed` 带同一个 `upto`；在这两条之间，`by` 是记忆模块的修改、删除、摘要就是这一批做的事（同一时刻其他会话可能在记、人可能在修改，用 `by` 可以区分开）。还没有收尾的一批就是正在合并的。
- 进度：第一批的 `left` 是总数，已经收尾的各批的 `fresh` 加起来就是合并了多少。一批之内不再细分进度。
- 人发起的整理如果要显示「AI 正在写什么」：`merging` 的 `given` 是这一批交给模型的是哪几条，等待时可以让它们浮过去；另外还有不写入磁盘的推送 `memory.dreaming`：AI 每写完一条修改（`kind: revised`，`id`、`text`）或者一条作废（`kind: retired`，`id`、`retire`、`kept`、`why`）就推送一条，摘要每多 20 个字推送一条、写完时再推送一条（`kind: summary`，`chars`、`done`），都带这一批的 `upto`。推送的是 AI 这一刻写的内容，写入磁盘时会全部核对，可能少几条，所以以日志为准；推送丢了不会补发。
- 每一趟整理中，第一个放得下的批会顺带重看最多 20 条轮到的旧记忆（按照编号轮流，轮完一圈从头开始），对它们的修改、删除同样算作这一批做的。人发起的整理即使没有新记下的记忆，也仍然会重看：那一批的 `fresh`、`left` 是 0。
- 从会话里发起的整理，先抽取出来的那几条（`ext.memory.saved`，`by` 是记忆模块，带出处）排在第一条 `merging` 前面，后面跟着 `ext.memory.extracted`。

## `memory.dream_stop`

这个方法叫停这一间此刻由人发起的整理，无论是从哪一处发起的（`memory.dream`、会话里的 `/dream`）；后台自动的整理不会停止。已经合并完的批保留，正在合并的那一批放弃，下次整理时再合并；记忆日志会记一条 `ext.memory.merge_failed`（`why: stopped`）。被叫停的那次 `memory.dream` 仍然会返回回应，带 `"stopped": true`。由于 `memory.dream` 在后台回答，在同一条连接上接着发送这个方法就可以了。

**参数**：只有 `persona` 或 `session`，和 `memory.dream` 相同。**回应**：`{"stopped": true}`。

**出错**：`not_dreaming`（这一间此刻没有人发起的整理）。

## `kb.list`、`kb.search`、`kb.read`、`kb.documents`

知识库放在数据根的 `root/kb/` 下，一个库就是一个目录里的文档（Markdown、PDF、网页……），核心为它们建立索引，用来搜索。这几个方法不检查知识库软件包有没有安装，因为文件是人自己的。

| 方法 | 参数 | 回应 |
|---|---|---|
| `kb.list` | 没有 | `{"bases": [{"id", "name", "description", "documents", "chunks", "auto"}…]}`，按照编号排列；`auto` 表示自动想起有没有打开；读取不了的库另带 `error`，软件包带来的另带 `package`，挂载进来的另带 `mount`（填写的那个目录）；`indexing` 表示此刻是不是正在建立索引（其他地方正在建立的不等待，按照索引现有的样子列出），建立好过索引的另带 `indexed_at`（上一次建好的时刻）；建立索引的进度可以订阅 `kb` 流查看 |
| `kb.search` | `query`（必须填写）、`kb`（只搜索这一个库）、`limit`（1 到 50，默认 10） | `{"hits": [{"kb", "path", "chunk", "heading", "text", "at"}…]}`，最相关的排在前面；PDF 的结果另外多一项 `page` |
| `kb.read` | `kb`、`path`（必须填写）、`page`（从 1 开始，默认 1） | `{"text", "page", "pages"}`：一页是 32 KiB；PDF 按照它自己的页来分 |
| `kb.documents` | `kb`（必须填写） | `{"documents": [{"path", "kind", "size", "at", "chunks"}…], "folders": [{"path", "reason", "why"}…]}`，按照路径排列：列出库里的每一篇文档；这一次读取不了的另带 `reason`、`why` |

`heading` 是标题路径（例如 `语法 › て形`），`at` 是文档修改的时刻。

`kb.documents` 里，`kind` 是 `markdown`、`text`、`pdf`、`html` 之一；`size` 是字节数；`chunks` 是这篇文档在索引里有几块（读取不了、但以前进过索引的文档保留旧的块）。`reason` 是 `too_large`、`not_utf8`、`no_text`（PDF 里没有文字：扫描的，或者只有图片）、`no_extractor`（PDF、网页需要 `miyu-extract`）、`extract_failed`、`unreadable` 之一，给人看的文字由头（连接核心的界面程序，例如终端界面、网页界面）自己写，`why` 是写给日志的英文。`folders` 是读取不了的子目录（挂载进来的库里经常有）。不是文档的文件不会列出。如果挂载的目录已经不在了，返回 `folder_unavailable`。

**出错**：`bad_params`（`page` 是 0 或者超过 `pages`）；`unknown_kb`；`unknown_document`；`internal_error`（PDF 损坏，或者文件超过 64 MiB）。

## `kb` 的管理

| 方法 | 参数 | 回应 |
|---|---|---|
| `kb.create` | `kb`（必须填写）、`name`、`description`、`auto` | 返回一个库 |
| `kb.update` | `kb`；`name`、`description`、`auto` 至少填写一个（填写空字符串、`false` 表示去掉） | 返回一个库 |
| `kb.mount` | `kb`、`folder`（绝对路径，或者以 `~/` 开头）；`name`、`description`、`auto` | 返回一个库：把一个现成的目录挂载进来 |
| `kb.delete` | `kb` | `{"trash"}`：移进系统回收站以后的位置；挂载进来的库只去掉登记，目录不会改动 |
| `kb.add` | `kb`、`path`、`blob`（先用 `blob.put` 上传）、`replace`（布尔） | `{}`：添加一篇文档。`path` 的扩展名只接受 `.md`、`.markdown`、`.txt`、`.pdf`、`.html`、`.htm`（不区分大小写），其他扩展名返回 `bad_params` |
| `kb.remove` | `kb`，以及 `path`（一项）或者 `paths`（几项）：一篇文档，或者一个文件夹（库里的相对路径，整个文件夹都删除） | 填写 `path` 时返回 `{"trash"}`，填写 `paths` 时返回 `{"trash": [...]}`，按照先后排列；只要有一项不存在，就一项都不删除；两个都填写或者都没有填写、有重复、一项在另一项里面时，返回 `bad_params` |
| `kb.reindex` | `kb` | 返回一个库：删掉索引，按照文件重新建立 |

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `kb_exists`、`document_exists` | 已经存在（添加文档时没有带 `replace`） |
| `kb_read_only` | 往挂载进来的库里添加、删除文档 |
| `kb_packaged` | 修改软件包带来的库（只能修改 `auto`） |
| `folder_unavailable` | 要挂载的目录无法使用，或者 `kb.documents` 查看的库挂载的目录已经不在了；另带 `data.message` |
| `trash_failed` | 无法移进回收站；另带 `data.message` |
| `unknown_attachment` | `blob` 不存在 |
| `unknown_kb`、`unknown_document`、`bad_params` | 和上面相同 |
