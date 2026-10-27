# 扩展：核心拉起的程序怎么说

扩展是软件包带的程序，核心照清单拉起它，经标准输入、标准输出说同一套协议。这一页列扩展特有的：拉起时给它什么、它能调哪些只给扩展的方法、核心会主动调它什么。从头写一个扩展的教程见 `docs/authors/`。英文版：[extensions.en.md](extensions.en.md)。

| 方法 | 方向 | 做什么 |
|---|---|---|
| [`provide`](#provide) | 扩展 → 核心 | 登记它提供的工具 |
| [`tool.call`、`tool.cancel`](#toolcalltoolcancel) | 核心 → 扩展 | 调它的工具；放下一次调用 |
| [`package.methods`、`method.call`](#packagemethodsmethodcall) | 两个方向 | 登记后台页、界面块要调的方法；核心转来的调用 |
| [`package.emit`](#packageemit) | 扩展 → 核心 | 推给开着它页面的人 |
| [`process.retire`、`process.keep`](#processretireprocesskeep) | 扩展 → 核心 | 问能不能自己退；要不要常驻（施工 IM-1 下） |
| [`feature.offer`](#featureoffer) | 扩展 → 核心 | 替这个人交预设菜单里的功能（施工 IM-6） |
| [`port.claim`、`port.release`、`port.list`](#portclaimportreleaseportlist) | 扩展 → 核心（`port.list` 给管理员） | 登记、放开要听的端口；看整张表（施工 IM-5） |
| [`command.call`](#commandcall) | 核心 → 扩展 | 人打了它声明的斜杠命令 |
| [`user.append`](#userappend) | 核心 → 扩展 | 回合开始时问它要不要往上下文里放几句 |
| [`system.append`](#systemappend) | 扩展 → 核心 | 往系统提示词末尾放一段 |
| [`widget.set`、`widget.clear`、`widget.watch`、`widget.unwatch`](#widgetsetwidgetclearwidgetwatchwidgetunwatch) | 两个方向 | 界面块的内容 |
| [`extension.config`](#extensionconfig) | 核心 → 扩展 | 它自己的配置变了 |
| [`schedule.fired`](schedules.md#schedulefired) | 核心 → 扩展 | 它建的定时到点了（施工 J-4） |
| [`content.list`、`content.read`、`content.changed`](#contentlistcontentreadcontentchanged) | 两个方向 | 读别的包给它带的内容 |
| [`events.append`](#eventsappend) | 扩展 → 核心 | 往会话里记一条自己的事件 |

别的方法照一张表挡：只给人的（替人回答确认、改权限级别和工作目录、删会话、改配置、装卸软件包……）回 `local_only`；要某项能力的，清单没声明回 `needs_approval`（见下面「能力」）；别的照常。每个方法归哪一种写在各页的「谁能调」里，表里没有的方法当只给人。

## 拉起

- **程序**：先找包文件夹的 `bin/`，再找 `miyu` 旁边，不走 `PATH`；参数是清单的 `[process] args`。
- **环境**：照核心的，另设 `MIYU_HOME`（数据根）、`MIYU_RESOURCES`（资源目录）、`MIYU_PACKAGE_DIR`（这个包的文件夹）、`MIYU_PACKAGE_STATE`（这个包的状态目录，已经建好）。
- **工作目录**：这个包放状态的目录 `state/packages/<编号>/`。
- **标准错误**：追加到 `state/logs/<编号>.stderr`，超过 1 MiB 的先挪成 `.old`。
- **握手**：照常发 `hello`，不出示凭据。回应的 `account` 是它的账号（声明了系统账号的是那个账号，别的是 `admin`），`config` 是它自己的配置项的最终值（密钥是真值）。
- **退出**：核心关它的标准输入就是「请退出」，5 秒没退的杀掉。
- **崩了**：退出码 1 以外的退出（问过 `process.retire`、核心答应了的退出码 0 除外，见下）、被杀、握手不成都算一次失败，退避 1、2、4、8、16 秒再拉起，连续 5 次停下（`failed_repeatedly`）。**退出码 1** 表示配置错、端口被占这类重启也没用的：不再拉起，直接停下（`config_error`）。连着跑满 60 秒的，失败次数从零数。

## `process.retire`、`process.keep`

每人一份的扩展（`[process] scope = "account"`）用到时才起，空闲了可以自己退：

- **`process.retire {}`**：问能不能退。核心发给你、你还没回的请求一条都没有的，回 `{"ok": true}`，之后不再给你发任何东西，你以退出码 0 退出；有的回 `{"ok": false}`，过一会儿再问。这样退的状态是 `stopped`、`reason` 是 `retired`，不重启、不算失败；这个人再用到时核心再起一份，等着的调用交给新的那一份。没问就退的照「崩了」算。
- **`process.keep {"on": true|false}`**：这个人的这一份要不要常驻，回 `{"on": <现在的值>}`。核心记下，重启也在；核心起来时只替说了要常驻的人拉起，别的等用到。进程退出不清，只由你自己改。比如配了 QQ 连接的 im 要常驻（NapCat 反连它的端口，它退了就没人听）。

只有一份的包也能 `retire`，但不会因为用到再起，要等重启扩展、核心再起来；`keep` 回 `{"on": true}`（本来就常驻），不记。不是核心拉起的连接调这两个回 `not_an_extension`。

## `feature.offer`

替这个人往预设菜单里交几个功能，比如「查询 QQ 群聊记录」，带上它的工具：

```json
{"features": [{"id": "qq-history", "name": {"zh": "查询 QQ 群聊记录", "en": "QQ history"}, "summary": {"zh": "…"}, "tools": ["qq_history"]}]}
```

- 回 `{}`。每次交整份，换掉你上一次替这个人交的；空的是全撤。
- 只记在核心内存里：每次连上、你自己的开关变了都交一遍，核心重启以后再交。进程退了不清。
- `id` 写法同包的编号，不能和这个人装了的功能、别的包交的撞；`name`、`summary` 是一句字或者语言到字的表（同清单 `[features]`）；`tools` 必须是你 `provide` 过的工具。不对的回 `bad_params`（`field` 是 `features`），什么都不换。
- 交了的出现在这个人的预设菜单里，归你的包；列出来的工具归它，不归你包自己的功能。开不开照预设：出厂「全部功能」开，别的出厂预设关，人在预设里打开。开着的会话下一个回合照新的算。
- 你接的平台的群、私聊里你整包不过预设（`venues.md`），这里交的只管本机会话、别的平台。

核心另有给人改全局开关的 `feature.set`，名字不同、别弄混。

## `port.claim`、`port.release`、`port.list`

要自己开端口听的扩展（比如 NapCat 反连 im 的那个）先登记：撞了的当场知道，不用等系统说绑不上。核心只记一张表，不替你听，真正抢端口的还是操作系统。

- **`port.claim {"port": 1–65535}`**：没人登记的记成你的，回 `{}`；你这个连接再登记一次照回 `{}`；别的扩展登记着回 `port_taken`，不说是谁。`0` 回 `bad_params`。
- **`port.release {"port"}`**：是你登记的就拿掉，不是的不动，都回 `{}`。
- **退出就放开**：进程退出、连接断了（包括 `process.retire` 以后退的）登记的都算放开，不用自己放。核心重启表就空了，起来再登记。
- **`port.list {}`**：只给管理员（别人 `not_permitted`）：`{"ports": [{"port", "package", "account"?}]}`，照端口排，`account` 是每人一份的那个人。

不是核心拉起的连接调 `port.claim`、`port.release` 回 `not_an_extension`。

## 能力

清单 `[process] capabilities` 声明要什么。管理员自己装的包要人批（`extension.enable` 带 `approve`）；出厂的算批过。还有没批的不拉起。批过的记在组表 `system/groups.json` 的 `ext.<能力>` 组里（包的账号是成员），卸包时移出；换核心以后以前批过的要重新批一次。

| 能力 | 能做什么 |
|---|---|
| `tools` | 向工具目录提供工具（`provide`） |
| `commands` | 提供斜杠命令（清单 `[[slash]]`） |
| `widgets` | 在界面上显示自己的内容（界面块） |
| `context.inject` | 回合开始时往上下文里放几句（`user.append`） |
| `context.system` | 往系统提示词末尾放一段（`system.append`） |
| `packages.call` | 调别的包导出的方法（`package.call`） |
| `search` | 当搜索服务供应商（清单 `[[search]]`，`search.query`） |
| `act_for_external` | 代表平台上的人说话（带 `as` 的 `session.send`、`command.run`）、`session.note`、`venue.*`、记 `venue.*` 事件：只给通讯平台的桥 |
| `sessions.drive` | 驱动会话：`session.create`、`send`、`interrupt`、`revert`、`unrevert`、`redo`、`compact`、`clear`、`configure`、`fork`、`set_meta`、`respond`，`command.run`、`job.stop` |
| `events.read` | 读会话：订阅 `events`、`view`、`sessions` 流，`session.list`、`session.recap`、`view.page`、`view.detail`、`job.output` |
| `events.write` | 往会话里记自己的事件（`events.append`） |
| `memory.write` | 写记忆、知识库：`memory.remember`、`update`、`forget`、`restore`、`dream`、`dream_stop`，`kb.add`、`create`、`update`、`mount`、`remove`、`delete`、`reindex` |
| `fs.read` | 经核心读文件（`fs.*`） |
| `network` | 经核心抓链接（`link.preview`） |
| `fs.write` | 写指定的路径：扩展进沙盒以后照它放行 |
| `tool.guard`、`tool.rewrite` | 工具执行前拦下、执行后改写结果：还没有 |

没声明的，调对应的方法回 `needs_approval`，`data.capabilities` 是缺的那一项。`network`、`fs.*` 管的是经核心做的；你的程序自己联网、读写文件，等扩展进沙盒以后才照它们挡。

## `provide`

登记这个包现在提供的全部工具：换掉它上一次登记的那几件。只收核心拉起的扩展。

**参数** `{"tools": [...]}`。一件：

| 名字 | 说明 |
|---|---|
| `name` | 英文字母、数字、`_`、`-`，1 到 64 个；不能和核心自带的、别的包的撞 |
| `description` | 给模型看的，英文短句 |
| `input_schema` | `{"type": "object", …}` |
| `access` | 会碰什么：`read`（只看、不出 Miyu）、`write`、`execute`、`network`、`outbound`（往外发东西）、`venue`（在聊天平台上做动作：撤回、禁言）、`install` |
| `venues` | 给哪种会话：`local`、`private`、`group` 里的一个或几个 |
| `plugin` | 场所插件的名字，写法同包的编号：群、私聊里只有那个场所开着它（[`venue.plugins`](venues.md#venueplugins)）才给，本机会话照旧。不写是一直给 |
| `timeout_ms` | 1000 到 600000，默认 60000 |
| `need_load` | 退役（施工 FL-1，工具一律全量）：写了的照旧登记得上，不看 |
| `clicall` | `true`：AI 在命令里用 `miyu call` 也能调它（只读、联网、执行类的才调得成），不写是不能。有结构化结果的写进回应的 `data`，脚本拿到一行 JSON |
| `result` | 有结构化结果的写它的样子，一行字，例如 `{"memes":[{"id","tags"}]}`：告诉 AI 脚本里能直接取哪几格（`miyu call --schema` 也印）。不写是没有 |
| `display_name` | 给人看的名字：`{"zh": "…", "en": "…"}` |
| `subject` | 给人看的名字后面跟哪个参数的值，要是 `input_schema.properties` 里的一个 |

**回应**：`{"tools": <件数>}`。连接断了工具照旧留在目录里，被调到时回「暂时不可用」。

**出错**：`not_a_provider`（不是核心拉起的扩展）；`bad_tool`：`data.tool` 是哪一件，`data.problem` 是 `duplicate`、`name`、`parameters`、`access`、`venues`、`timeout`、`clicall`、`result`、`display_name`、`subject`、`feature`、`plugin`。

## `tool.call`、`tool.cancel`

**`tool.call`**（核心 → 扩展，带 `id`，要回应）：

| 名字 | 说明 |
|---|---|
| `session` | 哪个会话 |
| `call_id` | 这次调用的编号 |
| `tool` | 工具名 |
| `args` | 参数 |
| `cwd` | 这一轮的工作目录 |
| `pictures` | 会话属主放图的目录（家目录里照 XDG 的 `pictures/`，施工 Y-11 下）：画出来、生成的图放这里，人在终端、网页里找得到。真实位置，可能还没建，写之前自己建好（缺的上级一起建）。只给本机的会话：会话外面的调用、场所会话（群、私聊，关在自己的工作区里，施工 Y-11 下修）没有这一格，照旧放工作区 |
| `by` | 是谁要她做的：触发这一轮的那条消息的 `by`；没有触发的回合不写 |
| `owner` | 核心认不认 `by` 是主人本人 |

**回应** `{"result": {"blocks": [...], "error"?, "end_turn"?, "sent"?, "detail"?, "data"?}}`：

| 名字 | 说明 |
|---|---|
| `blocks` | 给模型看的内容块：`{"type": "text", "text"}`、`{"type": "image", "blob", …}`。图先 `blob.put` |
| `error` | 这次调用算出错 |
| `end_turn` | 这一步的结果都到了以后，不再请求模型，这一轮结束 |
| `sent` | 她「发出去」的图，至多 4 张：`[{"blob", "media_type", "width", "height", "name"?, "alt"?, "sticker"?}]`。不给模型看；头画在正文里，聊天平台照它发 |
| `detail` | 给头看的结构化结果，照下面三种通用形状写一种；不给模型看。形状不对的、`error` 是真的，核心丢掉这一格、结果照常 |
| `data` | 给脚本的结构化结果，任意 JSON：AI 在命令里用 `miyu call` 调到的（`clicall`），印成一行 JSON 交给脚本；不给模型看，不记进 `tool.result`。`error` 是真的不交 |

`detail` 的三种形状（整份写成 JSON 至多 64 KiB）：

| `kind` | 其余几格 | 上限 |
|---|---|---|
| `links` | `links: [{"title", "url", "snippet"?, "site"?, "date"?}]`，`url` 只收 `http`、`https` | 50 条 |
| `table` | `columns: ["列名", …]`，`rows: [[格, …], …]`；一行的格数和列数一样，格是字、数、真假或者 `null` | 20 列、200 行 |
| `images` | `images: [{"blob", "media_type", "caption"?}]`，`media_type` 是 `image/` 开头的；图先 `blob.put` | 20 张 |

多一格、少一格都不收。头认得的画成卡片，结果记进 `tool.result` 的 `detail`（见 [事件](events.md)）。

回了 JSON-RPC 错误、写法不对的，算这次调用出错。到了 `timeout_ms` 没回，这次调用交回「没在 N 秒内答完」。

**`tool.cancel`**（核心 → 扩展，通知）：`{"session", "call_id"}`。超时、被打断、核心要关时发：收到了就放下那次调用。

谁能叫、能动谁是提供工具的一方自己的事：照 `by`、`owner` 挡，挡下的照结果回一句、`error` 写真。

## `package.methods`、`method.call`

**`package.methods`**（扩展 → 核心）：`{"methods": [{"name", "timeout_ms"?}…]}`，登记它的后台页、界面块要调的方法，换掉上一次的。回应 `{"methods": <个数>}`。别的连接回 `not_an_extension`。

**`method.call`**（核心 → 扩展）：`{"method", "params", "from"?}`：人的头、别的包经 `package.call` 调到它；`from` 是调的包的编号（人调的没有）。回什么，`package.call` 就交回什么；回 JSON-RPC 错误的，对方收到 `method_failed`。

想让别的扩展调到：在清单 `[export] methods` 里列出来。

每人一份的包（`[process] scope = "account"`）：各人那一份各登记各的，调用转给调的那个人那一份（施工 IM-1 中下）。

## `package.emit`

扩展 → 核心（施工 IM-1 中下二）：`{"event", "data"?}`，推给它替的那个账号、订了 `subscribe {"stream": "package", "package": <它的编号>}` 的连接，对方收到 `package.event {"package", "event", "data"}`。页面照它更新，不用轮询。回应 `{}`；没人订着也是。`event` 写法同方法名（小写字母开头，小写字母、数字、`_`、`.`、`-`，最多 64 个），`data` 写成 JSON 最多 256 KiB，不对的 `bad_params`；别的连接 `not_an_extension`。不存盘、不补。

## `search.query`

人在「搜索服务供应商」里选了这个包清单 `[[search]]` 声明的来源，AI 搜网页时（核心 → 扩展，带 `id`，要回应）：

`{"source", "query", "want", "allowed_domains", "blocked_domains"}`：来源（`<包>/<编号>`）、搜什么、至多要几条、只要这几个域名的、不要这几个域名的（核心拿到以后还会照这两个筛一遍）。

**回应** `{"hits": [{"title", "url", "snippet"?, "date"?}]}`：没有标题、网址的条不收，摘录截短、多的不要，和出厂的几家一样。限额了回 JSON-RPC 错误、`data.reason` 写 `limited`：核心照台阶冷却它（30 分钟起），冷却着不问；别的错、20 秒没答的，这一次照出厂的几家兜底。扩展要开着、登记过连接（`provide`，工具可以是空的）才问得到。

## `command.call`

人打了这个包清单 `[[slash]]` 声明的斜杠命令（核心 → 扩展）：

`{"command", "argument", "text", "session", "by", "owner", "language"}`：命令名、名字后面跟的字、人打的整行、会话、谁打的、是不是主人、连接的语言。

**回应** `{"said": "<回执，可以多行>"}`，核心记一条 `command.ran`、原样交给打命令的人。一分钟没答的、答了错的、没有 `said` 的，人那边收到 `command_unavailable` 或 `command_failed`。

## `user.append`

回合开始时核心问（核心 → 扩展）。只问声明了 `context.inject`、在跑、这个会话的工具面上有它的工具的扩展；几个并行问，**限时 300 毫秒**。

`{"session", "venue", "text"?}`：`venue` 是 `local`、`private`、`group`；`text` 是触发这一轮的人话（不是人开的一轮不带）。

**回应** `{"content": [{"kind", "text"}…]}`，至多 3 块：`kind` 小写字母、数字、`-`、`_`；`text` 1 到 4000 个字。交回的记进会话，排在触发的那句话后面，以后原样回放。空的、超时的、写法不对的当这一轮没交。

## `system.append`

往系统提示词末尾放一段（扩展 → 核心）。要声明 `context.system`。

**参数**：`content`（1 到 4000 个字，`null` 是去掉）、`session`（只管这一个会话；不写管全部）。**回应**：`{}`。

只进工具面上有这个包的工具的会话；改了的下一轮换上。只记在核心内存里：核心重启以后再设一次。

**出错**：`not_a_provider`；`needs_approval`（`data.capabilities` 是 `["context.system"]`）；`bad_params`。

## `widget.set`、`widget.clear`、`widget.watch`、`widget.unwatch`

界面块是清单 `[[widget]]` 声明的、在头上显示的一块内容：首页（`home`）、侧边栏（`sidebar`，可以跟着会话）、底栏（`status`）、整页（`page`，带斜杠命令打开）。要声明 `widgets`。

**扩展 → 核心**

| 方法 | 参数 | 回应 |
|---|---|---|
| `widget.set` | `widget`、`session`（跟着会话的块必写）、`title`（换掉清单的标题）、`content` | `{"kept": true}`；跟着会话、这个会话没人在看的 `{"kept": false}` |
| `widget.clear` | `widget`、`session` | `{}` |

**核心 → 扩展**（通知）：`widget.watch {"widget", "session"?}`：这一块开始有人看了，取数据、推内容；`widget.unwatch`：最后一个人也不看了。握手以后核心把这一刻有人在看的各发一条 `widget.watch`。

**内容**是一个数组，一项一段：

| 段 | 格 |
|---|---|
| `heading` | `text` |
| `text` | `text`（能换行，最多 2000 个字）、`tone` |
| `pairs` | `items`：`{"label", "value", "tone"?}`，至多 50 对 |
| `progress` | `value`（0 到 1）、`text`、`mark`（0 到 1，标在条上的那一处） |
| `list` | `items`：`{"title", "detail"?, "right"?, "state"?, "action"?}`，至多 200 条 |
| `buttons` | `items`：`{"label", "action"}`，至多 8 个 |
| `sections` | 只给整页：`items`：`{"id", "title", "subtitle"?, "badge"?, "state"?, "blocks"}`，`add`：`{"label", "call", "options"?}` |
| `form` | 只给整页：`id`、`fields`（`{"key", "label", "hint"?, "control", "value", …}`）、`submit`（`{"label", "call", "params"?}`） |

- 字是字符串，或者语言到字的表（`{"zh": "…", "en": "…"}`），核心照连接的语言挑。不认 Markdown、HTML。
- `tone`：`normal`、`dim`、`accent`、`good`、`warn`、`bad`。列表的 `state`：`todo`、`doing`、`done`、`failed`、`off`；分区的 `state`：`good`、`warn`、`bad`、`off`。
- `action`：`{"call": <方法>, "params"?}` 调它自己登记的方法；`{"open": <块>}` 打开它的一个整页；`{"copy": <字>}` 复制到剪贴板。
- 表单的控件：`toggle`、`select`（带 `options: [{"value", "label"}]`）、`text`、`textarea`、`list`、`number`（`min`、`max`、`integer`）；`value` 是 `null` 表示照默认；`default` 是 `{"text", "value"?}`：给人看的那一行，和默认到底是什么（头照它画 `null` 的那一格）。保存时头调 `submit.call`，参数是 `submit.params` 加上 `{"values": {只放改过的格}}`；扩展回 `{"saved": true}`，或 `{"saved": false, "problems": [{"key", "message"}]}`，或 `{"saved": false, "message"}`。`add` 的调用参数是 `{"value": <选的那一项>}`，可以回 `{"id": <新的那一项>}`。
- 整块最多 64 段、64 KiB；`status` 只能是一段一行的 `text`、宽最多 40 列；`home` 最多 8 段。同一块最快 250 毫秒推一次给头，中间的只留最新的。

**出错**：`unknown_widget`（`data.widget`）；`bad_widget_content`：`data.path` 错在哪（例如 `content[4].items[2].title`），`data.problem` 是 `type`、`missing`、`too_long`、`too_many`、`too_big`、`control_char`、`tone`、`state`、`value`、`action`、`place`、`unknown`、`duplicate` 之一；`bad_params`（跟着会话的没写 `session`，全局的写了）。

## `extension.config`

它自己的配置项（「包的编号加点」开头的）变了（核心 → 扩展，通知）：`{"keys": {<键>: <新值> 或 null}}`，只放变了的键；密钥是真值；变成没设、取不到的是 `null`。不用订阅。

## `content.list`、`content.read`、`content.changed`

别的包能在清单里写 `[content.<它的编号>] dir = "…"`，给它带一个目录的内容（表情包的内容包就是这样）。

| 方法 | 参数 | 回应 |
|---|---|---|
| `content.list` | 没有 | `{"packages": [{"package", "version"?}…]}`：给它带了内容的包，照编号排 |
| `content.read` | `package`、`path`、`offset` | `{"data", "size", "eof"}`，一次最多 512 KiB |
| `content.changed`（核心 → 扩展，通知） | `{}` | 装、卸、升级了给它带内容的包：再列一遍 |

**出错**：`not_a_provider`；`unknown_package`（没有这个包、没给它带）；`bad_params`；`not_found`。

## `events.append`

往会话里记一条不带回合编号的事件：不开回合、不打断、不给模型看。

**参数**：`session`、`kind`、`body`（JSON 对象，最多 16 KiB）。扩展只能记 `ext.<自己的包编号>.<名字>`（每段小写字母开头，只有小写字母、数字、`_`、`-`，整个最多 128 个字）。通讯平台的桥另能记 `venue.recalled`、`venue.delivered`（见 [场所](venues.md)）。

**回应**：`{"seq": <序号>}`。

**出错**：`bad_params`；`session_not_found`。
