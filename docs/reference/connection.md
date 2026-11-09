# 连接：怎么连、握手、订阅

英文版：[connection.en.md](connection.en.md)。

| 方法 | 做什么 |
|---|---|
| [`hello`](#hello) | 握手：说明你是谁、出示凭据 |
| [`subscribe`、`unsubscribe`](#subscribeunsubscribe) | 订阅、取消订阅一条推送流 |
| [`account.setup_code`、`account.setup`、`account.logout`](#accountsetup_codeaccountsetupaccountlogout) | 访问密码：网页第一次设用的一次性码、设和改、作废设备 |
| [`account.set_avatar`、`account.avatar`](#你的头像) | 你的头像 |
| [`notices.claim`](#noticesclaim) | 认领：后台通知由这个连接发 |
| [`core.stop`](#corestop) | 叫核心停下 |

## 怎么连

- **本机的头**：核心在本机的套接字上等连接，只有本人连得上。套接字在哪写在数据根的 `run/socket` 里（一行，路径原样），本机令牌在 `run/token` 里（64 位小写十六进制）。Linux、macOS 是 Unix 域套接字，Windows 是命名管道 `\\.\pipe\miyu-<指纹>`。核心没在跑的，头自己拉起 `miyu core`。
- **扩展**：核心照软件包清单拉起的程序，经标准输入、标准输出说同一套协议，不用凭据（`hello` 里写了也不看）。
- **网页**：浏览器连的是网页软件，网页软件再按上面本机的头的办法连核心。替远程的人开的连接，网页软件在握手之前先写一行通知 `{"jsonrpc":"2.0","method":"connection.remote","params":{"from":<来自哪个地址>}}`，再原样转浏览器的字节：这个连接就是远程的（施工 Y-1 下）。只能往下标：标了就变不回本机，握手以后说的不理。
- **本机才能用的**（施工 Y-1 下）：读核心那台机器的文件、交本机路径、选本机的目录这一类，只给本机的连接，远程的回 `local_machine_only`，什么都不做：`fs.*`、`session.terminal`、`session.set_terminal`、`account.setup_code`、`core.stop`；带 `path` 的 `blob.put`、`package.install`，带 `folder` 的 `kb.mount`，带 `command` 的 `mcp.set`，带 `file`、`cwd` 的 `check`，带 `cwd` 的 `config.trust`、`config.get`；会话的 `cwd`、`dirs`、附件的 `path`、`command.run` 的 `cwd` 和 `/workspace <路径>`、项目的 `workspace`。会话不写 `cwd` 的用它自己的默认工作区。远程的握手回应不给 `host`。

## 一行一条

- 每条消息一行 JSON，UTF-8，`\n` 结尾（读的时候 `\r\n` 也认）。一行最长 1 MiB，太长的回 `parse_error`（`id` 是 `null`），只拒这一行，连接照旧。
- 核心写出的 JSON 是紧凑的，对象的格照名字的字母先后排。

**请求**是 JSON-RPC 2.0：`jsonrpc` 写 `"2.0"`，`method` 是方法名，`params` 是对象（不写当 `{}`，写成数组回 `bad_params`），`id` 是 1 到 128 字节、不含控制字符的字符串。没有 `id` 的是通知，核心不理，只认握手之前的 `connection.remote`（上面「网页」）。参数里不认识的格不理；写 `null` 等于没写。

**只有一个人**：Miyu 只给装它的那个人（主人）用，没有账号、成员、管理员模式（2026-10-28 起）。老头请求外壳里带的 `"scope"` 照收不理。

**回应**：成了的在 `result`；拒了的在 `error`：`code` 是错误码，`message` 是给人看的一句（照这个连接的语言），`data.reason` 是给程序看的原因码，有的另带几格。

```json
{"id":"c7","jsonrpc":"2.0","result":{"events":[41]}}
{"error":{"code":-32010,"data":{"reason":"session_not_found"},"message":"会话不存在。"},"id":"c7","jsonrpc":"2.0"}
```

**推送**是核心发来的通知：`{"jsonrpc":"2.0","method":<名字>,"params":{…}}`，没有 `id`。哪些流推什么见 [推送](pushes.md)。

**先后**：

1. 一个连接上的请求一条条办：上一条回了才读下一条。少数要等很久的在后台答，不挡后面的请求，回应照 `id` 对上：`model.call`、`memory.dream`、`link.preview`、`job.stop`、带 `wait` 的 `fs.find`、`job.output`，和 `command.run` 里要等的几条（例如 `/dream`）。
2. 先见结果，后见回应：一个命令引起的推送，一定比它的回应先到。
3. 同一个 `id` 再发一次（同一个会话最近 1024 个之内）：照上一次的回应答，不再做一遍。被拒的不记，再发重新判。这是给断线重发用的，**不分连接**：新连上的连接用了以前用过的 `id`，也当成重发，拿到的是上一次的回应、什么都不做（例如 `session.send` 有回应，会话里却没有这句话）。所以每个连接的 `id` 要和以前的都不一样，最简单的是每次连接取一个随机前缀，后面接递增的数字。`session.create` 也照 `id` 认重发：用同一个 `id` 再造，交回上一次造的那个会话，核心重启以后也认得。

## `hello`

握手。连上以后第一条必须是它；之前别的方法都回 `hello_first`。连上 10 秒还没握手成的断开。

**谁能调**：人、扩展。

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `protocol` | `[最低, 最高]`，必写 | 你支持的协议主版本范围；核心只有 `1` |
| `head` | `{"kind", "version"}`，必写 | 你是什么、哪一版，只记进运行日志 |
| `locale` | 字符串 | 你所在系统的语言（`zh-CN`、`en_US.UTF-8` 都行）。配置 `ui.language` 是 `auto` 时照它定这个连接的语言 |
| `caps` | 对象 | `input`（布尔）：你能不能让人输入。`prefix_ask`（布尔）：文件改了、前缀要变时你先问人（[`session.send`](sessions.md#sessionsend)「前缀要变」） |
| `token` | 字符串 | 本机令牌 |
| `code` | 字符串 | 一次性码（网页第一次设访问密码） |
| `login` | 字符串 | 设备令牌（网页记住的登录） |
| `password` | 字符串 | 访问密码：从局域网连时用 |
| `ticket` | 字符串 | 交接票（`login.ticket` 要的）：别的页面带过来的，换成一个设备令牌连上 |

凭据正好写一种：`token`、`code`、`login`、`ticket`、`password`。扩展不写。网页软件转来的远程连接（`connection.remote`）什么凭据都没写、又还没设访问密码的回 `password_not_set`。

**回应**

| 名字 | 说明 |
|---|---|
| `protocol` | 选定的主版本：`1` |
| `core` | `{"version"}`：核心的版本 |
| `avatar` | 你的头像的版本，有头像的才带（施工 AV-1）：头照它换缓存，图用 `account.avatar` 取 |
| `host` | `{"home", "platform", "workspace", "dirs"}`：系统的家目录（读不出的是 `null`）、`linux`/`macos`/`windows`、默认工作区、照 XDG 的几处 `{"pictures", "videos", "documents", "downloads"}`（在的给真实位置、还没建的照原样，施工 Y-11 上；Windows 上不带 `\\?\` 前缀，施工 WP-1）。远程的连接整个没有（施工 Y-1 下）；扩展的没有 `dirs` |
| `language` | `zh`、`en`、`ja`：之后给人看的字用哪种。配置改了语言，下一句就照新的，不用再握手 |
| `view` | 视图投影的版本，现在是 `1`（见 [会话](sessions.md) 的 `view.page`） |
| `sandbox` | `{"usable": true}`，或者 `{"usable": false, "reason"}`：`helper_missing`、`helper_failed`、`no_mechanism` |
| `config_errors` | 配置里现在有几处错误，没有的不写 |
| `config` | 只有扩展有：这个包自己的配置项的最终值（密钥是真值）。之后变了推 `extension.config` |
| `setup` | `true`：用一次性码连上的，只能设访问密码 |
| `password_set` | 设过访问密码没有（布尔）：头照它决定「允许局域网访问」开不开得了 |
| `login` | `{"expires", "token"}`：用访问密码、交接票连上的才有，网页记住它下次用。交接票连上的票当场作废 |

```json
→ {"jsonrpc":"2.0","id":"h1","method":"hello","params":{"protocol":[1,1],"head":{"kind":"tui","version":"0.1.0"},"locale":"zh-CN","token":"3f9a…"}}
← {"id":"h1","jsonrpc":"2.0","result":{"core":{"version":"0.0.1"},"host":{"home":"/home/me","platform":"linux","workspace":"/home/me/.miyu/home/me/workspace"},"language":"zh","protocol":1,"sandbox":{"usable":true},"view":1}}
```

握手以后再发一次 `hello` 也行：过了的照新的语言和能力；被拒的，没断开的还照上一次的。

**出错**（标了「断开」的回完就断开连接）

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 缺了必写的、类型不对；凭据写了不止一种（断开） |
| `protocol_mismatch` | `1` 不在你给的范围里（断开） |
| `bad_token` | 没写凭据，或者本机令牌不对（断开） |
| `bad_code`、`bad_login`、`bad_password` | 一次性码、设备令牌、访问密码不对（断开） |
| `password_not_set` | 还没设访问密码：写了 `password` 的，或者远程来的什么都没写（断开）。先在这台电脑上设 |
| `bad_ticket` | 交接票没有、过期了、用过了（断开） |
| `login_throttled` | 同一个来源 60 秒里密码错了 5 次，或者所有来源合起来 60 秒里试了 30 次，先等一会儿（断开）；`data.retry_after_ms` 是还要等多久。来源是地址，不看端口，IPv6 同一段 /64 算一个 |
| `hello_first` | 握手以前调了别的方法（不断开） |
| `setup_first` | 用一次性码连上的，调了 `human.get`、`account.setup` 以外的方法 |

## `subscribe`、`unsubscribe`

订阅一条推送流：从这时起，它的推送发给这个连接。

**谁能调**：人、扩展。扩展订阅 `events`、`view`、`sessions` 要有 `events.read` 能力；`projects`、`schedules`、`notices`、`package`、`prefs` 只给人（扩展调回 `local_only`）。

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `stream` | 字符串，必写 | 哪一条流，见下表 |
| `session` | 字符串 | `events`、`view` 必写；`config`、`sessions` 不能写；`memory` 可写（这个会话的那一间记忆） |
| `persona` | 字符串 | 只给 `memory`：这个人格的那一间记忆 |
| `package` | 字符串 | `package`、`prefs` 必写：哪个软件包 |
| `after` | 非负整数 | 只给 `events`、`memory`：先补序号比它大的全部，`0` 是从头补 |

| `stream` | 推什么 | `subscribe` 的回应 |
|---|---|---|
| `events` | 一个会话的事件，原样 | 会话这时的状态：`model`、`limits`、`permission`、`usage`、`jobs`、`workspace`、`default_workspace`（会话的默认工作区）、`compactable`（这时有没有可压缩的，也就是 `session.compact` 会不会回 `nothing_to_compact`；变了推 `compactable.changed`），有的话还有 `persona`、`preset`、`todos`、`goal`、`nested`（在跑的 `miyu call`：`[{id, name, args, at}]`，`at` 是各自开始的时刻）；写了 `after` 的还有 `upto` |
| `view` | 一个会话的视图投影（画好就能显示的条目） | 同上，再加最新一页条目和完整的会话 `status` |
| `config` | 配置的变化 | `{}` |
| `sessions` | 会话列表里一项变了 | `{"sessions": [<项>…]}`，全部 |
| `extensions` | 扩展的状态变了 | `{"extensions": […]}` |
| `packages` | 软件包列表里一项变了 | `{"packages": […]}` |
| `projects` | 一个项目变了 | `{"projects": […]}`，全部 |
| `bus` | 总线上的名字、可不可用变了（[`bus.list`](bus.md#buscallbuslist)） | `{"names": […]}`，全部 |
| `schedules` | 定时任务变了、跑了（施工 J-1 下） | `{"schedules": […]}`，全部 |
| `notices` | 给人的通知（施工 J-3 中）：新开会话的定时任务那一次跑完了、定时任务表坏了换成了空的。头照它弹系统通知 | `{}`；这一刻有连接认领了后台通知（[`notices.claim`](#noticesclaim)）的是 `{"claimed": true}`。以前的不补 |
| `package` | 带 `package`：那个软件包推给人的（施工 IM-1 中下 2），它的页面照它更新，不用轮询。没装的回 `unknown_package` | `{}`；以前的不补 |
| `prefs` | 带 `package`：那个界面包自己的设置被别的连接改了，推 `prefs.changed`（施工 HP-1，[头自己的设置](prefs.md)）。没装的回 `unknown_package`，不是界面包的回 `not_a_ui_package` | `{"values": {…}}`，这一刻的整份 |
| `kb` | 知识库建索引的进度：`kb.indexing`、`kb.indexed` | `{}`：这时的样子从 `kb.list` 拿 |
| `memory` | 一间记忆的日志 | `{"upto": <这时最后一条的序号>}` |
| `widgets` | 界面块 | 见[软件包和命令](packages.md) |

`events` 回应里的 `model` 是会话下一次问谁：`ref`（它的模型或 `@池`）、`endpoint` 和 `model`（下一次请求发给谁；轮换的池两样都没有），还有 `effort` `{"level", "from"}`：那个模型用的思考深度，`from` 是 `session`（`session.configure` 设的，施工 SM-1）或 `system`（配置）。轮换的池、请求不带思考深度的，没有 `effort`。

每条流推的是什么见[推送](pushes.md)。

1. 一个连接对一个会话只有一个订阅（`events`、`view` 算同一个），再订阅一次换掉原来的。`sessions`、`config`、`extensions`、`packages`、`projects`、`bus`、`schedules`、`notices`、`kb`、`widgets` 每个连接各一个；`package`、`prefs` 每个软件包各一个。
2. **补**：写了 `after` 的，订阅那一刻磁盘上序号比它大的事件先一条条推过来，然后是回应（带 `upto`），之后从 `upto + 1` 起照常推，不丢不重。这一刻正在流式回的模型请求，补完以后把到这时为止的增量（`model.delta`）再推一遍，接得上后面实时的。没有序号的临时事件不补。
3. **掉队**：读得太慢的，会话给每个订阅最多攒 1024 条（配置 16 条，记忆 64 条），再多就推一条 `resync`，这个订阅停了。会话停了也推 `resync`。重新订阅，`after` 写你看到的最后一个序号，补上。
4. 订阅着一个会话算有人在看：没人看着的会话里，子代理回报只记下、不叫醒她。
5. `unsubscribe`：`stream` 和 `session`、`persona`、`package` 照订阅时写；停止转发，回 `{}`，没订阅过的也是。

```json
→ {"jsonrpc":"2.0","id":"s1","method":"subscribe","params":{"session":"01a14aca-…","stream":"events","after":30}}
← （先推 31 到 42 号事件）
← {"id":"s1","jsonrpc":"2.0","result":{"limits":{"compaction_line":850000,"window":1000000},"model":{"endpoint":"deepseek","model":"deepseek-v4","ref":"deepseek/deepseek-v4"},"upto":42,…}}
```

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 不认得的 `stream`；该写的 `session`、`package` 没写，或者写了不该写的；`after` 不是非负整数，或者这条流不收 |
| `local_only` | 扩展订阅只给人的流 |
| `needs_approval` | 扩展订阅 `events`、`view`、`sessions`，清单没声明 `events.read`；`data.capabilities` 是缺的那一项 |
| `unknown_package` | `package`、`prefs` 流：那个软件包没装 |
| `not_a_ui_package` | `prefs` 流：那个软件包装着，但不是界面包 |
| `session_not_found`、`session_broken`、`session_stopped` | 会话没有、读不了、停了（再订阅一次会重新载入） |
| `memory_not_installed`、`memory_unavailable` | `memory` 流：没装记忆，或者那个地方没有记忆 |

## `account.setup_code`、`account.setup`、`account.logout`

访问密码：只在要从局域网（手机、别的电脑的浏览器）连时需要；本机的头凭本机令牌，不用密码。从局域网用访问密码登录一次，浏览器记住设备令牌 30 天。

**谁能调**：人。`account.setup_code` 只给出示本机令牌连上的；`account.setup`、`account.logout` 都能调（设备令牌连上的改访问密码要带现在的，见下）。

| 方法 | 参数 | 回应 |
|---|---|---|
| `account.setup_code` | 没有 | `{"code", "expires", "first"}`：一次性码、到什么时候作废、是不是还没设过访问密码 |
| `account.setup` | `password`、`old`? | 本机令牌连上的回 `{}`；别的回 `{"login": {"expires", "token"}}`：新的设备令牌，这个连接从此照它算 |
| `account.logout` | `all`（布尔，默认 `false`） | `{"revoked": <作废了几个>}`：作废这一个设备令牌，`all` 的作废全部 |

一次性码用过就作废。设、改了访问密码，以前的设备全部作废，连着的断开（改的这个连接除外）。密码最多 1024 个字节，不能是空的、不能全是空白，多短都行。局域网别的设备经网页软件连进来的（`connection.remote`）、已经设过访问密码的，要带对现在的 `old`：照来源限流，同密码登录；还没设的不用带。本机来的都不用带：本机令牌、一次性码，和网页从 `miyu web` 在这台机器上打开的设备令牌（施工 PW-2，2026-10-31）。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 参数不对；密码不合要求；局域网来的设备令牌改访问密码没带 `old` |
| `bad_password`、`login_throttled` | 设备令牌连上的改访问密码，`old` 不对；这个来源 60 秒内错满 5 次、或者合起来满 30 次（`data.retry_after_ms`） |
| `local_only` | 扩展调了；不是本机令牌连上的调了 `account.setup_code` |

## `notices.claim`

认领发后台通知（施工 TR-1）：托盘这样不开会话、浏览器关了也要弹系统通知的头连上时调一次，说「没有头开着的会话（会话列表那一项没有 `viewed`）答完、在问、要确认，和 `notices` 流的通知，由我来弹」。核心照连接记，连接断了就放；同一个连接再调一遍不变。

**谁能调**：人；扩展调回 `local_only`。

| 方法 | 参数 | 回应 |
|---|---|---|
| `notices.claim` | 没有 | `{}` |

别的头从 `notices` 流知道有没有人认领：订阅的回应这一刻有人认领的写 `{"claimed": true}`，没有的还是 `{}`；「有没有人认领」变了推 `notices.claimed {"claimed": <布尔>}`（两个连接认领、走了一个不推）。有人认领时，别的头不再为没人开着的会话、`notice` 弹系统通知，开着某个会话的头照常弹它自己开着的那一个；认领的走了、推了 `claimed: false`，照原来的弹。

**出错**：`bad_params`：带了参数。

## `core.stop`

叫核心停下（施工 DT-4 下）：桌面端托盘里的「退出」连核心一起关用它。

**谁能调**：人，只给出示本机令牌连上的；远程的回 `local_machine_only`。

| 方法 | 参数 | 回应 |
|---|---|---|
| `core.stop` | 没有 | `{}`：回应写出去以后，核心照收到停的信号那样停下：跑到一半的回合记成「重启了」，下次载入接着干，在跑的后台命令先记 `restarted` 再杀，扩展一起退出，然后放开套接字。运行日志记 `stopped reason=request` |

核心停了，连着的连接都断开。之后谁再连（终端、网页的标签页重连），照旧把核心拉起来。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `local_only` | 扩展调了；不是本机令牌连上的（设备令牌、访问密码、交接票）调了 |
| `local_machine_only` | 远程的连接调了 |

## 你的头像

一张 PNG、JPEG、WebP，最大 1 MiB、宽高不超过 1024（同人格头像）。

| 方法 | 参数 | 回应 |
|---|---|---|
| `account.set_avatar` | `blob`（先 `blob.put` 传上来）或 `unset: true`，可带 `expect`（上次读到的版本，`null` 是还没有） | `{"avatar": <版本或 null>}`；`expect` 对不上的 `avatar_conflict`（`data` 带现在的版本）；不是图 `avatar_not_image`，太大 `avatar_too_big` |
| `account.avatar` | 没有 | `{"avatar", "media_type", "data"}`（版本、媒体类型、base64）；没有的是 `null` |

## `login.ticket`、`login.redeem`

交接票：已经登录的人从一个页面跳到另一个程序的页面（网页跳到接入通讯平台的页面，`miyu web` 打开网页），不用再登录一次。跳过去之前要一张票带在网址里，接的一方拿它连核心，有两种接法：

- 握手时交 `ticket`（上面「握手」的凭据）：一步连上，回应带换到的设备令牌。浏览器里直连核心的页面用它。
- `login.redeem` 换一个设备令牌，再照这个设备令牌连核心（`miyu-client` 的 `connect_login`，经本机套接字，握手带 `login`；页面自己的登录框用 `connect_password`，交回换到的登录令牌）。老的接法，照旧能用。

**谁能调**：`login.ticket` 给人：本机令牌（终端）、设备令牌、密码、交接票连上的都能要。核心拉起的扩展只能要 `via` 是 `"page"` 的，要批过能力 `login.page`（施工 LP-1，见 [扩展](extensions.md)「替本机页面换登录票」），没批的回 `needs_approval`，不写 `via`、写别的回 `local_only`。`login.redeem` 谁拿着票谁能调。

| 方法 | 参数 | 回应 |
|---|---|---|
| `login.ticket` | 可写 `via`：`"page"`（网页服务、扩展替本机打开的页面要的）、`"open"`（`miyu web` 要的） | `{"ticket", "expires"}`：64 位十六进制的票，60 秒后作废。`via` 进核心的运行日志（连同这个连接怎么认出来的）；`"page"` 的票不管是人还是扩展要的都只在本机换（下面），`"open"`、不写的哪儿都能换。别的值、别的格回 `bad_params` |
| `login.redeem` | `ticket` | `{"login": {"expires", "token"}}`：换出的设备令牌（30 天） |

一张票只用一次（握手交了、`login.redeem` 换了都算）；同时最多 16 张，多了作废最早的。票只在核心的内存里，核心重启全部作废。

`via` 是 `"page"` 的票只在本机换（扩展要的施工 LP-1，人要的施工 LP-2：网页软件替本机打开的页面要的那种）：网页软件转来的远程连接（握手之前说过 `connection.remote` 的，不管说的是什么地址，同机反向代理转来的回环地址也一样）握手交它、`login.redeem` 它都回 `bad_ticket`，票当场作废。人要的 `"open"`、不写 `via` 的票照旧哪儿都能换。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | `login.redeem` 没写 `ticket` |
| `local_only` | 扩展要票没写 `via`，或者写的不是 `"page"` |
| `needs_approval` | 扩展要 `page` 的票，没批 `login.page`（`data.capabilities` 是 `["login.page"]`） |
| `bad_ticket` | 票没有、过期了、用过了；`page` 的票拿到远程的连接上换 |

