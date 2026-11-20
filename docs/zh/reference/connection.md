# 连接：怎么连、握手、订阅

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

| 方法 | 做什么 |
|---|---|
| [`hello`](#hello) | 握手：说明你是谁，并出示凭据 |
| [`subscribe`、`unsubscribe`](#subscribeunsubscribe) | 订阅一条推送流，或者取消订阅 |
| [`account.setup_code`、`account.setup`、`account.logout`](#accountsetup_codeaccountsetupaccountlogout) | 访问密码：网页第一次设置访问密码时用的一次性码；设置和修改访问密码；作废设备 |
| [`account.set_avatar`、`account.avatar`](#你的头像) | 你的头像 |
| [`notices.claim`](#noticesclaim) | 认领后台通知：后台通知由这个连接发出 |
| [`core.stop`](#corestop) | 让核心停止运行 |

## 怎么连

- **本机的头**：核心在本机的套接字上等待头（连接核心的界面程序，例如终端界面、网页界面）连接，只有你本人能连上。套接字的位置写在数据根的 `run/socket` 里（一行，路径原样写出），本机令牌在 `run/token` 里（64 位小写十六进制）。Linux、macOS 上是 Unix 域套接字，Windows 上是命名管道 `\\.\pipe\miyu-<指纹>`。如果核心没有在运行，头会自己启动 `miyu core`。
- **扩展**：扩展是核心按照软件包清单启动的程序，它通过标准输入、标准输出使用同一套协议，不需要凭据（`hello` 里写了凭据也会被忽略）。
- **网页**：浏览器连接的是网页软件，网页软件再按上面本机的头的方式连接核心。对于替远程的人打开的连接，网页软件在握手之前先写一行通知 `{"jsonrpc":"2.0","method":"connection.remote","params":{"from":<来自哪个地址>}}`，然后原样转发浏览器的字节，这样这个连接就是远程连接。这个标记只能往下标：标记以后就不能再变回本机连接，握手以后再发这条通知会被忽略。
- **本机才能用的**：读取核心所在机器上的文件、交出本机路径、选择本机目录这一类操作，只给本机的连接使用；远程连接调用时返回 `local_machine_only`，并且不做任何事。这些操作包括：`fs.*`、`session.terminal`、`session.set_terminal`、`account.setup_code`、`core.stop`；带 `path` 的 `blob.put`、`package.install`，带 `folder` 的 `kb.mount`，带 `command` 的 `mcp.set`，带 `file`、`cwd` 的 `check`，带 `cwd` 的 `config.trust`、`config.get`；会话的 `cwd`、`dirs`，附件的 `path`，`command.run` 的 `cwd` 和 `/workspace <路径>`，项目的 `workspace`。没有填写 `cwd` 的会话使用它自己的默认工作区。远程连接的握手回应里没有 `host`。

## 一行一条

- 每条消息是一行 JSON，使用 UTF-8 编码，以 `\n` 结尾（读取时也接受 `\r\n`）。一行最长 1 MiB，超过的行返回 `line_too_long`（`data.limit` 是上限；`id` 是从这一行开头能识别出的那一格，识别不出时是 `null`），核心只拒绝这一行，连接仍然保持。发送之前要按这个上限检查一遍，大的内容通过附件（`blob.put`）传送。
- 核心写出的 JSON 是紧凑格式，对象的字段按名字的字母顺序排列。

**请求**使用 JSON-RPC 2.0：`jsonrpc` 写 `"2.0"`，`method` 是方法名，`params` 是对象（不写时当作 `{}`，写成数组时返回 `bad_params`），`id` 是 1 到 128 字节、不含控制字符的字符串。没有 `id` 的消息是通知，核心会忽略它，只接受握手之前的 `connection.remote`（见上面的「网页」）。参数里不认识的字段会被忽略；写 `null` 等于没有写。

**只有一个人**：Miyu 只给安装它的那个人（主人）使用，没有账号、成员、管理员模式。旧版的头在请求外壳里带的 `"scope"` 仍然接受，但是会被忽略。

**回应**：成功时结果在 `result` 里；被拒绝时在 `error` 里：`code` 是错误码，`message` 是给人看的一句话（使用这个连接的语言），`data.reason` 是给程序看的原因码，有的错误还另外带几个字段。

```json
{"id":"c7","jsonrpc":"2.0","result":{"events":[41]}}
{"error":{"code":-32010,"data":{"reason":"session_not_found"},"message":"会话不存在。"},"id":"c7","jsonrpc":"2.0"}
```

**推送**是核心发来的通知：`{"jsonrpc":"2.0","method":<名字>,"params":{…}}`，没有 `id`。每一条流推送什么，详细的见 [推送](pushes.md)。

**先后**：

1. 一个连接上的请求按顺序逐条处理：上一条返回以后才读取下一条。少数需要等很久的请求在后台回答，不会挡住后面的请求，回应按 `id` 对应：`model.call`、`memory.dream`、`link.preview`、`job.stop`、带 `wait` 的 `fs.find`、`job.output`，以及 `command.run` 里需要等待的几条（例如 `/dream`）。
2. 先看到结果，后收到回应：一个命令引起的推送，一定比它的回应先到达。
3. 同一个 `id` 再发送一次（在同一个会话最近的 1024 个之内）时，核心按上一次的回应回答，不会再做一遍。被拒绝的请求不记录，再发送时重新判断。这是给断线重发用的，**不分连接**：如果新连接用了以前用过的 `id`，参数也一样，核心就当成重发，返回上一次的回应，什么都不做；参数不一样时拒绝，返回 `id_reused`（核心重启以后不再比较参数，此时又会返回上一次的回应、什么都不做，例如 `session.send` 有了回应，会话里却没有这句话）。所以每个连接的 `id` 都要和以前的不同，最简单的方法是每次连接时取一个随机前缀，后面接递增的数字。`session.create` 也按 `id` 识别重发：用同一个 `id` 再创建一次，会返回上一次创建的那个会话，核心重启以后也能识别；如果修改了 `cwd`、`dirs` 以外的参数，返回 `id_reused`。

## `hello`

握手。连接以后发送的第一条消息必须是 `hello`；在它之前调用其他方法都返回 `hello_first`。连接 10 秒以后还没有握手成功的连接会被断开。

**谁能调**：人和扩展都能调用。

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `protocol` | `[最低, 最高]`，必写 | 你支持的协议主版本范围；核心只有 `1` |
| `head` | `{"kind", "version"}`，必写 | 你是什么程序、哪一个版本，会记进运行日志。`kind` 还会记在人在这个连接上说的话、输入的命令上（事件的 `by.head`，见[事件](events.md)）：AI 根据它知道这一轮的话是从哪里来的。终端界面写 `tui`，网页写 `web`，命令行（`miyu ask`）写 `cli`，终端无缝集成写 `shellhook`：这几种 AI 看到的是各自对应的一句话，其他种类 AI 看到的是「在一个 Miyu 程序里」。`kind` 以小写字母开头，由小写字母、数字、`-`、`_` 组成，最多 32 个字符；写法不对时仍然能连上，只是不会被记录 |
| `locale` | 字符串 | 你所在系统的语言（`zh-CN`、`en_US.UTF-8` 两种写法都可以）。配置项 `ui.language` 是 `auto` 时，核心按它决定这个连接的语言 |
| `caps` | 对象 | `input`（布尔）：你能不能让人输入。`prefix_ask`（布尔）：文件改了、前缀要变化时，你会先询问人（见 [`session.send`](sessions.md#sessionsend) 的「前缀要变」） |
| `token` | 字符串 | 本机令牌 |
| `code` | 字符串 | 一次性码（网页第一次设置访问密码时使用） |
| `login` | 字符串 | 设备令牌（网页记住的登录） |
| `password` | 字符串 | 访问密码：从局域网连接时使用 |
| `ticket` | 字符串 | 交接票（由 `login.ticket` 获取）：由其他页面带过来，用来换成一个设备令牌并连上核心 |

凭据必须正好填写一种：`token`、`code`、`login`、`ticket`、`password`。扩展不填写凭据。网页软件转来的远程连接（`connection.remote`）如果没有填写任何凭据，并且还没有设置访问密码，返回 `password_not_set`。

**回应**

| 名字 | 说明 |
|---|---|
| `protocol` | 选定的主版本：`1` |
| `core` | `{"version"}`：核心的版本 |
| `avatar` | 你的头像的版本，只有设置了头像时才带：头根据它更新缓存，图片用 `account.avatar` 获取 |
| `host` | `{"home", "platform", "workspace", "dirs"}`：系统的家目录（读取不到时是 `null`）、平台（`linux`/`macos`/`windows`）、工作区的根目录（数据根下的 `root/workspace/`，每个会话的默认工作区是它下面的 `<会话编号>/`）、按 XDG 规范的几个目录 `{"pictures", "videos", "documents", "downloads"}`（目录存在时给出真实位置，还没有创建时按原样给出；Windows 上不带 `\\?\` 前缀）。远程连接的回应里整个没有这一格；扩展的回应里没有 `dirs` |
| `language` | `zh`、`en`、`ja`：之后给人看的文字使用哪种语言。配置里修改了语言以后，下一句话就使用新的语言，不需要重新握手 |
| `view` | 视图投影的版本，现在是 `1`（见 [会话](sessions.md) 的 `view.page`） |
| `sandbox` | `{"usable": true}`，或者 `{"usable": false, "reason"}`，其中 `reason` 是 `helper_missing`、`helper_failed`、`no_mechanism` 之一 |
| `config_errors` | 配置里现在有几处错误；没有错误时不写这一格 |
| `config` | 只有扩展的回应里有：这个软件包自己的配置项的最终值（密钥是真实的值）。之后值变化时推送 `extension.config` |
| `setup` | `true` 表示这个连接是用一次性码连上的，只能设置访问密码 |
| `password_set` | 是否设置过访问密码（布尔）：头根据它决定「允许局域网访问」能不能打开 |
| `login` | `{"expires", "token"}`：只有用访问密码、交接票连上时才有，网页记住它，下次使用。用交接票连上时，这张票当场作废 |

```json
→ {"jsonrpc":"2.0","id":"h1","method":"hello","params":{"protocol":[1,1],"head":{"kind":"tui","version":"0.1.0"},"locale":"zh-CN","token":"3f9a…"}}
← {"id":"h1","jsonrpc":"2.0","result":{"core":{"version":"0.0.1"},"host":{"home":"/home/me","platform":"linux","workspace":"/home/me/.miyu/root/workspace"},"language":"zh","protocol":1,"sandbox":{"usable":true},"view":1}}
```

握手以后可以再发送一次 `hello`：如果通过了，就使用新的语言和能力；如果被拒绝、连接又没有断开，仍然使用上一次的语言和能力。

**出错**（标了「断开」的错误，核心返回以后就会断开连接）

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 缺少必须填写的参数，或者类型不对；凭据写了不止一种（断开） |
| `protocol_mismatch` | `1` 不在你给出的范围里（断开） |
| `bad_token` | 没有写凭据，或者本机令牌不对（断开） |
| `bad_code`、`bad_login`、`bad_password` | 一次性码、设备令牌或者访问密码不对（断开） |
| `password_not_set` | 还没有设置访问密码：填写了 `password`，或者远程连接什么凭据都没有填写（断开）。要先在这台电脑上设置访问密码 |
| `bad_ticket` | 交接票不存在、已经过期，或者已经用过（断开） |
| `login_throttled` | 同一个来源在 60 秒里输错密码 5 次，或者所有来源合起来在 60 秒里尝试了 30 次，需要先等一会儿（断开）；`data.retry_after_ms` 是还要等多久。来源按地址区分，不看端口，IPv6 地址在同一个 /64 网段里的算作一个来源 |
| `hello_first` | 握手之前调用了其他方法（不断开） |
| `setup_first` | 用一次性码连上的连接，调用了 `human.get`、`account.setup` 以外的方法 |

## `subscribe`、`unsubscribe`

订阅一条推送流：从订阅时起，这条流的推送会发给这个连接。

**谁能调**：人和扩展都能调用。扩展订阅 `events`、`view`、`sessions` 需要 `events.read` 能力；`projects`、`schedules`、`notices`、`package`、`prefs` 只给人订阅（扩展调用时返回 `local_only`）。

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `stream` | 字符串，必写 | 订阅哪一条流，见下表 |
| `session` | 字符串 | `events`、`view` 必须填写；`config`、`sessions` 不能填写；`memory` 可以填写（表示这个会话的那一间记忆） |
| `persona` | 字符串 | 只用于 `memory`：表示这个人格的那一间记忆 |
| `package` | 字符串 | `package`、`prefs` 必须填写：表示哪个软件包 |
| `after` | 非负整数 | 只用于 `events`、`memory`：先补发序号比它大的全部内容，`0` 表示从头补发 |

| `stream` | 推什么 | `subscribe` 的回应 |
|---|---|---|
| `events` | 一个会话的事件，原样推送 | 会话这时的状态：`model`、`limits`、`permission`、`usage`、`jobs`、`workspace`、`default_workspace`（会话的默认工作区）、`compactable`（这时有没有可以压缩的内容，也就是 `session.compact` 会不会返回 `nothing_to_compact`；变化时推送 `compactable.changed`），有的话还有 `compaction_line`（会话钉住的压缩触发线）、`persona`、`preset`、`todos`、`goal`、`vision`（会话钉住的多模态模型）、`nested`（正在运行的 `miyu call`：`[{id, name, args, at}]`，`at` 是各自开始的时刻）、`title`（会话的标题，起过标题的会话才有）；填写了 `after` 时还有 `upto` |
| `view` | 一个会话的视图投影（处理好、可以直接显示的条目） | 和上面相同，另外加上最新一页条目和完整的会话 `status` |
| `config` | 配置的变化 | `{}` |
| `sessions` | 会话列表里有一项变化了 | `{"sessions": [<项>…]}`，包含全部会话 |
| `extensions` | 扩展的状态变化了 | `{"extensions": […]}` |
| `packages` | 软件包列表里有一项变化了 | `{"packages": […]}` |
| `projects` | 有一个项目变化了 | `{"projects": […]}`，包含全部项目 |
| `bus` | 总线上的名字，或者名字是否可用变化了（[`bus.list`](bus.md#buscallbuslist)） | `{"names": […]}`，包含全部名字 |
| `schedules` | 定时任务变化了，或者运行了 | `{"schedules": […]}`，包含全部定时任务 |
| `notices` | 给人的通知：新开会话的定时任务那一次运行完了，或者定时任务表损坏、被换成了空表。头根据它弹出系统通知 | `{}`；这一刻有连接认领了后台通知（[`notices.claim`](#noticesclaim)）时是 `{"claimed": true}`。以前的通知不补发 |
| `package` | 带 `package`：那个软件包推送给人的内容，软件包的页面根据它更新，不需要轮询。软件包没有安装时返回 `unknown_package` | `{}`；以前的推送不补发 |
| `prefs` | 带 `package`：那个界面包自己的设置被其他连接修改了，推送 `prefs.changed`（见[头自己的设置](prefs.md)）。软件包没有安装时返回 `unknown_package`，不是界面包时返回 `not_a_ui_package` | `{"values": {…}}`，是这一刻的完整设置 |
| `kb` | 知识库建立索引的进度：`kb.indexing`、`kb.indexed` | `{}`：这时的状态从 `kb.list` 获取 |
| `memory` | 一间记忆的日志 | `{"upto": <这时最后一条的序号>}` |
| `widgets` | 界面块 | 见[软件包和命令](packages.md) |

`events` 回应里的 `model` 表示会话下一次请求哪个模型：`ref`（会话的模型或 `@池`）、`endpoint` 和 `model`（下一次请求发给谁；轮换的池两样都没有），还有 `effort` `{"level", "from"}`：那个模型使用的思考深度，`from` 是 `session`（由 `session.configure` 设置）或 `system`（来自配置）。轮换的池，以及请求不带思考深度的情况，没有 `effort`。

`events` 回应顶层的 `compaction_line` 是会话钉住的压缩触发线，是窗口的百分之几（由 `session.configure` 设置）；`vision` 是会话钉住的多模态模型（模型或 `@池`）。它们和 `view.status` 里的同名两格相同：没有钉住（跟随配置）时不写，之后根据推送过来的 `session.policy_changed` 更新。`limits.compaction_line` 名字一样，但是意思不同：它是正在生效的压缩线，单位是 token 数，钉住新的值以后要到下一个回合开始时才更新。

每一条流推送什么，详细的见[推送](pushes.md)。

1. 一个连接对一个会话只有一个订阅（`events`、`view` 算同一个），再订阅一次会替换原来的订阅。`sessions`、`config`、`extensions`、`packages`、`projects`、`bus`、`schedules`、`notices`、`kb`、`widgets` 每个连接各有一个；`package`、`prefs` 每个软件包各有一个。
2. **补**：填写了 `after` 时，订阅那一刻磁盘上序号比它大的事件会先逐条推送过来，然后是回应（带 `upto`），之后从 `upto + 1` 开始照常推送，不丢失也不重复。如果这一刻有正在流式返回的模型请求，补完以后会把到这时为止的增量（`model.delta`）再推送一遍，以便和后面实时的增量接上。没有序号的临时事件不补发。
3. **掉队**：如果读取得太慢，会话给每个订阅最多积攒 1024 条推送（配置 16 条，记忆 64 条），超过以后推送一条 `resync`，这个订阅随之停止。会话停止时也推送 `resync`。此时重新订阅，`after` 填写你看到的最后一个序号，就能补上。
4. 订阅着一个会话算作有人在看：在没有人看着的会话里，子代理的回报只记录下来，不会为 AI 开启一轮。
5. `unsubscribe`：`stream` 和 `session`、`persona`、`package` 按订阅时的写法填写；核心停止转发，返回 `{}`，没有订阅过时也返回 `{}`。

```json
→ {"jsonrpc":"2.0","id":"s1","method":"subscribe","params":{"session":"01a14aca-…","stream":"events","after":30}}
← （先推 31 到 42 号事件）
← {"id":"s1","jsonrpc":"2.0","result":{"compaction_line":50,"limits":{"compaction_line":850000,"window":1000000},"model":{"endpoint":"deepseek","model":"deepseek-v4","ref":"deepseek/deepseek-v4"},"upto":42,"vision":"c/w",…}}
```

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | `stream` 不认识；必须填写的 `session`、`package` 没有填写，或者填写了不该填写的；`after` 不是非负整数，或者这条流不接受 `after` |
| `local_only` | 扩展订阅了只给人的流 |
| `needs_approval` | 扩展订阅 `events`、`view`、`sessions`，但是清单没有声明 `events.read`；`data.capabilities` 是缺少的那一项 |
| `unknown_package` | `package`、`prefs` 流：那个软件包没有安装 |
| `not_a_ui_package` | `prefs` 流：那个软件包已经安装，但不是界面包 |
| `session_not_found`、`session_broken`、`session_unavailable`、`session_read_only`、`session_stopped` | 分别是：会话不存在；日志无法解析；这一刻无法读取（过一会儿再试）；日志中间损坏了一行（损坏的那一行以前的内容可以用 `view.page` 只读地查看，见 [`errors`](errors.md)）；会话已经停止（再订阅一次会重新载入） |
| `memory_not_installed`、`memory_unavailable` | `memory` 流：没有安装记忆，或者那个位置没有记忆 |

## `account.setup_code`、`account.setup`、`account.logout`

访问密码只在需要从局域网（手机、其他电脑的浏览器）连接时使用；本机的头使用本机令牌，不需要密码。从局域网用访问密码登录一次以后，浏览器会记住设备令牌 30 天。

**谁能调**：人。`account.setup_code` 只给出示本机令牌连上的连接调用；`account.setup`、`account.logout` 任何连接都能调用（用设备令牌连上的连接修改访问密码时，要带上现在的访问密码，见下面）。

| 方法 | 参数 | 回应 |
|---|---|---|
| `account.setup_code` | 没有 | `{"code", "expires", "first"}`：一次性码、作废的时间，以及是不是还没有设置过访问密码 |
| `account.setup` | `password`、`old`? | 用本机令牌连上的连接返回 `{}`；其他连接返回 `{"login": {"expires", "token"}}`：新的设备令牌，这个连接从此按它认证 |
| `account.logout` | `all`（布尔，默认 `false`） | `{"revoked": <作废了几个>}`：作废这一个设备令牌；填写 `all` 时作废全部设备令牌 |

一次性码用过一次就作废。设置或者修改访问密码以后，以前的设备全部作废，正在连接的设备会被断开（进行修改的这个连接除外）。密码最多 1024 个字节，不能是空的，也不能全是空白，长度没有下限。局域网里其他设备经网页软件连进来的连接（`connection.remote`），在已经设置过访问密码的情况下，要带上正确的现在的密码 `old`：这里按来源限流，和用密码登录相同；还没有设置访问密码时不需要带。从本机来的连接都不需要带，包括本机令牌、一次性码，以及网页从 `miyu web` 在这台机器上打开的设备令牌。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 参数不对；密码不符合要求；局域网来的设备令牌连接修改访问密码时没有带 `old` |
| `bad_password`、`login_throttled` | 用设备令牌连上的连接修改访问密码时 `old` 不对；或者这个来源 60 秒内错满 5 次，或者所有来源合起来满 30 次（`data.retry_after_ms`） |
| `local_only` | 扩展调用了这些方法；或者不是用本机令牌连上的连接调用了 `account.setup_code` |

## `notices.claim`

认领发出后台通知的职责：托盘这类不打开会话、浏览器关掉以后也要弹出系统通知的头，在连接时调用一次，表示「没有头打开着的会话（会话列表里那一项没有 `viewed`）回答完毕、在提问、需要确认时的通知，以及 `notices` 流的通知，都由我来弹出」。核心按连接记录认领，连接断开时就释放；同一个连接再调用一次不会有变化。

**谁能调**：人；扩展调用时返回 `local_only`。

| 方法 | 参数 | 回应 |
|---|---|---|
| `notices.claim` | 没有 | `{}` |

其他头从 `notices` 流知道有没有人认领：订阅的回应里，这一刻有人认领时写 `{"claimed": true}`，没有时仍然是 `{}`；「有没有人认领」变化时推送 `notices.claimed {"claimed": <布尔>}`（两个连接都认领了、其中一个离开时不推送）。有人认领时，其他头不再为没有人打开着的会话和 `notice` 弹出系统通知，打开着某个会话的头仍然为它自己打开着的那一个会话弹出通知；认领的连接离开、推送了 `claimed: false` 以后，其他头恢复原来的弹出方式。

**出错**：`bad_params`：调用时带了参数。

## `core.stop`

让核心停止运行：桌面端托盘里的「退出」在关闭时连同核心一起关闭，用的就是这个方法。

**谁能调**：人，并且只给出示本机令牌连上的连接调用；远程连接调用时返回 `local_machine_only`。

| 方法 | 参数 | 回应 |
|---|---|---|
| `core.stop` | 没有 | `{}`：回应写出去以后，核心按收到停止信号时的方式停止：执行到一半的回合记录成「重启了」，下次载入时接着执行；正在运行的后台命令先记录 `restarted` 再被终止；扩展一起退出；然后核心释放套接字。运行日志里记录 `stopped reason=request` |

核心停止以后，所有连接都会断开。返回回应以前，核心会在数据根的 `run/` 里留下一个记号 `stopped`：这表示核心是人停止的，头断线以后自己重连时不会再把它启动起来。

**头怎么拉起核心**（Rust 写的头通过 `miyu-client` 启动）：

| 什么时候 | 用哪个 | 人停的核心 |
|---|---|---|
| 人要使用核心：输入命令、头刚打开、点击了「启动」 | `connect::connect_or_start`、`connect_or_start_bare`，`open::Core::connect` | 照常启动 |
| 头断线以后自己重连 | `connect::reconnect_or_start`、`reconnect_or_start_bare`，`open::Core::reconnect` | 不启动，返回 `StartError::StoppedByUser`（`Core::reconnect` 返回的 `Refused` 的 `reason` 是 `stopped_by_user`） |

- `StoppedByUser` 对应的文字是 `core/start/stopped_by_user`（「核心已停止」），头根据它显示「核心已停止」的卡片，卡片上带一个「启动」按钮，点击以后按「人要使用核心」那一行的方式启动。
- 核心崩溃、空闲退出、收到停止信号而停止时，不会留下记号，头自己重连时仍然会启动核心。
- 不管核心是怎么启动的，拿到数据根的锁以后就会删掉这个记号。
- 不是用 Rust 写的头：自己重连以前，要先检查 `run/stopped` 是否存在，存在就不要启动核心。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `local_only` | 扩展调用了这个方法；或者不是用本机令牌连上的连接（设备令牌、访问密码、交接票）调用了这个方法 |
| `local_machine_only` | 远程连接调用了这个方法 |

## 你的头像

头像是一张 PNG、JPEG 或 WebP 图片，最大 1 MiB，宽和高都不超过 1024（和人格头像的要求相同）。

| 方法 | 参数 | 回应 |
|---|---|---|
| `account.set_avatar` | `blob`（先用 `blob.put` 上传）或 `unset: true`，可以带 `expect`（上次读到的版本，`null` 表示还没有头像） | `{"avatar": <版本或 null>}`；`expect` 对不上时返回 `avatar_conflict`（`data` 里带现在的版本）；不是图片时返回 `avatar_not_image`，图片太大时返回 `avatar_too_big` |
| `account.avatar` | 没有 | `{"avatar", "media_type", "data"}`（版本、媒体类型、base64）；没有头像时是 `null` |

## `login.ticket`、`login.redeem`

交接票让已经登录的人从一个页面跳到另一个程序的页面（从网页跳到接入通讯平台的页面，或者 `miyu web` 打开网页）时，不需要再登录一次。跳转之前先要一张票，带在网址里，接收的一方拿它连接核心，有两种方式：

- 握手时提交 `ticket`（上面「握手」里的凭据）：一步就能连上，回应里带有换到的设备令牌。在浏览器里直接连接核心的页面使用这种方式。
- 用 `login.redeem` 换一个设备令牌，再用这个设备令牌连接核心（`miyu-client` 的 `connect_login`，通过本机套接字，握手时带 `login`；页面自己的登录框使用 `connect_password`，返回换到的登录令牌）。这是旧的方式，仍然可以使用。

**谁能调**：`login.ticket` 给人调用：用本机令牌（终端）、设备令牌、密码、交接票连上的连接都能要票。核心启动的扩展只能要 `via` 是 `"page"` 的票，并且需要批准过能力 `login.page`（见 [扩展](extensions.md) 的「替本机页面换登录票」），没有批准时返回 `needs_approval`；不写 `via` 或者写了其他值时返回 `local_only`。`login.redeem` 谁拿着票谁就能调用。

| 方法 | 参数 | 回应 |
|---|---|---|
| `login.ticket` | 可以填写 `via`：`"page"`（网页服务、扩展替本机打开的页面要票时使用）、`"open"`（`miyu web` 要票时使用） | `{"ticket", "expires"}`：64 位十六进制的票，60 秒后作废。`via` 会记进核心的运行日志（连同这个连接是怎么认证的）；`"page"` 的票不管是人要的还是扩展要的，都只能在本机换（见下面），`"open"` 和不写 `via` 的票在哪里都能换。填写其他值或者其他字段时返回 `bad_params` |
| `login.redeem` | `ticket` | `{"login": {"expires", "token"}}`：换出的设备令牌（30 天） |

一张票只能用一次（握手时提交、用 `login.redeem` 兑换都算一次）；同时最多存在 16 张，超过时作废最早的那张。票只存放在核心的内存里，核心重启以后全部作废。

`via` 是 `"page"` 的票只能在本机换（包括扩展要的票，以及人要的票里由网页软件替本机打开的页面要的那种）：网页软件转来的远程连接（握手之前发送过 `connection.remote` 的连接，不管它写的是什么地址，同一台机器上的反向代理转来的回环地址也一样）在握手时提交这种票，或者用 `login.redeem` 兑换它，都返回 `bad_ticket`，并且这张票当场作废。人用 `"open"` 要的票、不写 `via` 的票仍然在哪里都能换。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | `login.redeem` 没有填写 `ticket` |
| `local_only` | 扩展要票时没有写 `via`，或者写的不是 `"page"` |
| `needs_approval` | 扩展要 `page` 的票，但是没有批准 `login.page`（`data.capabilities` 是 `["login.page"]`） |
| `bad_ticket` | 票不存在、已经过期，或者已经用过；或者 `page` 的票被拿到远程连接上兑换 |

