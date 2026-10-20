# 连接：怎么连、握手、订阅

英文版：[connection.en.md](connection.en.md)。

| 方法 | 做什么 |
|---|---|
| [`hello`](#hello) | 握手：说明你是谁、出示凭据 |
| [`subscribe`、`unsubscribe`](#subscribeunsubscribe) | 订阅、取消订阅一条推送流 |
| [`account.setup_code`、`account.setup`、`account.logout`](#accountsetup_codeaccountsetupaccountlogout) | 网页登录用的一次性码、设用户名和密码、作废登录 |

## 怎么连

- **本机的头**：核心在本机的套接字上等连接，只有本人连得上。套接字在哪写在数据根的 `run/socket` 里（一行，路径原样），本机令牌在 `run/token` 里（64 位小写十六进制）。Linux、macOS 是 Unix 域套接字，Windows 是命名管道 `\\.\pipe\miyu-<指纹>`。核心没在跑的，头自己拉起 `miyu core`。
- **扩展**：核心照软件包清单拉起的程序，经标准输入、标准输出说同一套协议，不用凭据（`hello` 里写了也不看）。
- **网页**：浏览器连的是网页软件，网页软件再按上面本机的头的办法连核心。

## 一行一条

- 每条消息一行 JSON，UTF-8，`\n` 结尾（读的时候 `\r\n` 也认）。一行最长 1 MiB，太长的回 `parse_error` 并断开。
- 核心写出的 JSON 是紧凑的，对象的格照名字的字母先后排。

**请求**是 JSON-RPC 2.0：`jsonrpc` 写 `"2.0"`，`method` 是方法名，`params` 是对象（不写当 `{}`，写成数组回 `bad_params`），`id` 是 1 到 128 字节、不含控制字符的字符串。没有 `id` 的是通知，核心不理。参数里不认识的格不理；写 `null` 等于没写。

**回应**：成了的在 `result`；拒了的在 `error`：`code` 是错误码，`message` 是给人看的一句（照这个连接的语言），`data.reason` 是给程序看的原因码，有的另带几格。

```json
{"id":"c7","jsonrpc":"2.0","result":{"events":[41]}}
{"error":{"code":-32010,"data":{"reason":"session_not_found"},"message":"会话不存在。"},"id":"c7","jsonrpc":"2.0"}
```

**推送**是核心发来的通知：`{"jsonrpc":"2.0","method":<名字>,"params":{…}}`，没有 `id`。哪些流推什么见 [推送](pushes.md)。

**先后**：

1. 一个连接上的请求一条条办：上一条回了才读下一条。少数要等很久的在后台答，不挡后面的请求，回应照 `id` 对上：`model.call`、`memory.dream`、`link.preview`、带 `wait` 的 `fs.find`、`job.output`，和 `command.run` 里要等的几条（例如 `/dream`）。
2. 先见结果，后见回应：一个命令引起的推送，一定比它的回应先到。
3. 同一个 `id` 再发一次（同一个会话最近 1024 个之内）：照上一次的回应答，不再做一遍。被拒的不记，再发重新判。

## `hello`

握手。连上以后第一条必须是它；之前别的方法都回 `hello_first`。连上 10 秒还没握手成的断开。

**谁能调**：人、扩展。

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `protocol` | `[最低, 最高]`，必写 | 你支持的协议主版本范围；核心只有 `1` |
| `head` | `{"kind", "version"}`，必写 | 你是什么、哪一版，只记进运行日志 |
| `locale` | 字符串 | 你所在系统的语言（`zh-CN`、`en_US.UTF-8` 都行）。配置 `ui.language` 是 `auto` 时照它定这个连接的语言 |
| `caps` | 对象 | 现在只看 `input`（布尔）：你能不能让人输入 |
| `token` | 字符串 | 本机令牌 |
| `code` | 字符串 | 一次性码（网页第一次设密码、重设密码） |
| `login` | 字符串 | 登录令牌（网页记住的登录） |
| `user`、`password` | 字符串 | 网页登录的用户名、密码，一起写 |

凭据正好写一种：`token`、`code`、`login`，或者 `user` 加 `password`。扩展不写。

**回应**

| 名字 | 说明 |
|---|---|
| `protocol` | 选定的主版本：`1` |
| `core` | `{"version"}`：核心的版本 |
| `account` | 你是谁：本机的人是 `admin`；声明了系统账号的包的扩展是那个系统账号（账号名同包的编号），别的扩展也是 `admin` |
| `host` | `{"home", "platform", "workspace"}`：系统的家目录（读不出的是 `null`）、`linux`/`macos`/`windows`、这个账号的默认工作区 |
| `language` | `zh`、`en`、`ja`：之后给人看的字用哪种。配置改了语言，下一句就照新的，不用再握手 |
| `view` | 视图投影的版本，现在是 `1`（见 [会话](sessions.md) 的 `view.page`） |
| `sandbox` | `{"usable": true}`，或者 `{"usable": false, "reason"}`：`helper_missing`、`helper_failed`、`no_mechanism` |
| `config_errors` | 配置里现在有几处错误，没有的不写 |
| `config` | 只有扩展有：这个包自己的配置项的最终值（密钥是真值）。之后变了推 `extension.config` |
| `setup` | `true`：用一次性码连上的，只能设用户名和密码 |
| `login` | `{"expires", "token"}`：用用户名、密码连上的才有，网页记住它下次用 |

```json
→ {"jsonrpc":"2.0","id":"h1","method":"hello","params":{"protocol":[1,1],"head":{"kind":"tui","version":"0.1.0"},"locale":"zh-CN","token":"3f9a…"}}
← {"id":"h1","jsonrpc":"2.0","result":{"account":"admin","core":{"version":"0.0.1"},"host":{"home":"/home/me","platform":"linux","workspace":"/home/me/.miyu/home/admin/workspace"},"language":"zh","protocol":1,"sandbox":{"usable":true},"view":1}}
```

握手以后再发一次 `hello` 也行：过了的照新的语言和能力；被拒的，没断开的还照上一次的。

**出错**（标了「断开」的回完就断开连接）

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 缺了必写的、类型不对；凭据写了不止一种、`user`/`password` 只写一个（断开） |
| `protocol_mismatch` | `1` 不在你给的范围里（断开） |
| `bad_token` | 没写凭据，或者本机令牌不对（断开） |
| `bad_code`、`bad_login`、`bad_password` | 一次性码、登录令牌、用户名密码不对（断开） |
| `login_throttled` | 密码错太多次，先等一会儿（断开） |
| `hello_first` | 握手以前调了别的方法（不断开） |
| `setup_first` | 用一次性码连上的，调了 `human.get`、`account.setup` 以外的方法 |

## `subscribe`、`unsubscribe`

订阅一条推送流：从这一刻起，这条流的推送都转给这个连接。

**谁能调**：人、扩展。

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `stream` | 字符串，必写 | 哪条流，见下表 |
| `session` | 字符串 | `events`、`view` 必写；`config`、`sessions` 不能写；`memory` 可写（那个会话用的那一间） |
| `persona` | 字符串 | 只有 `memory` 认：这个人格那一间 |
| `after` | 非负整数 | 只有 `events`、`memory` 认：先补发序号大于它的，`0` 是从头 |

| `stream` | 推什么 | `subscribe` 的回应 |
|---|---|---|
| `events` | 一个会话的事件，原样 | 会话这一刻的样子：`model`、`limits`、`permission`、`usage`、`jobs`、`workspace`、`default_workspace`（这个会话的默认工作区）、`compactable`（这一刻有没有能压的，`session.compact` 会不会回 `nothing_to_compact`；变了推 `compactable.changed`），有的另带 `persona`、`preset`、`todos`、`goal`、`nested`（在跑的 `miyu call`：`[{id, name}]`）；写了 `after` 的多 `upto` |
| `view` | 一个会话的视图投影（给界面直接画的条目） | 上一格那些，加最新一页的条目、整份会话状态 `status` |
| `config` | 配置变了 | `{}` |
| `sessions` | 会话列表的一项变了 | `{"sessions": [<一项>…]}`，全部 |
| `extensions` | 扩展的状态变了 | `{"extensions": […]}` |
| `packages` | 软件包列表的一项变了 | `{"packages": […]}` |
| `projects` | 这个账号的一个项目变了（施工 H-1） | `{"projects": […]}`，全部 |
| `kb` | 这个账号的知识库建索引的进度（施工 W-15）：`kb.indexing`、`kb.indexed` | `{}`：这一刻的样子照 `kb.list` |
| `memory` | 一间记忆的日志 | `{"upto": <那一刻最后一条的序号>}` |
| `widgets` | 界面块 | 见 [软件包与扩展](packages.md) |

各条流的推送长什么样见 [推送](pushes.md)。

1. 一个连接对一个会话只有一个订阅（`events` 和 `view` 也算同一个）；再订阅换一个新的。`sessions`、`config`、`extensions`、`packages`、`projects`、`kb`、`widgets` 一个连接各一个。
2. **补发**：写了 `after` 的，先把日志里序号大于它、订阅那一刻已经落了盘的事件一条条推过来，再回应（带 `upto`），之后从 `upto + 1` 接着推，不丢不重。会话正有一次模型请求在流的，补完以后另推一遍这次请求到目前为止的增量（`model.delta`），接得上后面实时推的。不带序号的瞬时事件不补。
3. **掉队**：你读得太慢，会话给一个订阅最多攒 1024 份（配置 16 份、记忆 64 条），再多就推一条 `resync`，这个订阅停了。会话停了也推 `resync`。重新订阅，带上最后看到的序号（`after`）就能补回来。
4. 订阅着一个会话，就算你在看着它：没人看着的会话，子代理报回来只记下、不叫醒她。
5. `unsubscribe`：同样的 `stream`、`session`/`persona`，停掉转发，回 `{}`；没订阅过的也回 `{}`。

```json
→ {"jsonrpc":"2.0","id":"s1","method":"subscribe","params":{"session":"01a14aca-…","stream":"events","after":30}}
← （先推第 31 到 42 条 event）
← {"id":"s1","jsonrpc":"2.0","result":{"limits":{"compaction_line":850000,"window":1000000},"model":{"endpoint":"deepseek","model":"deepseek-v4","ref":"deepseek/deepseek-v4"},"upto":42,…}}
```

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | `stream` 不认识；`session` 该写没写、不该写写了；`after` 不是非负整数，或者这条流不认 `after` |
| `session_not_found`、`session_broken`、`session_stopped` | 会话没有、读不了、停了（再订阅一次会重新载入） |
| `memory_not_installed`、`memory_unavailable` | `memory` 流：没装记忆，或者这一间没有记忆 |

## `account.setup_code`、`account.setup`、`account.logout`

网页登录。只有一个账号 `admin`，网页第一次进来给它设登录用的用户名和密码；以后用它们登录，浏览器记住 30 天。

**谁能调**：人。`account.setup_code` 只给出示本机令牌连上的；`account.setup` 给用一次性码连上的；`account.logout` 都能调。

| 方法 | 参数 | 回应 |
|---|---|---|
| `account.setup_code` | 没有 | `{"code", "expires", "first"}`：一次性码、到什么时候作废、是不是还没设过密码 |
| `account.setup` | `username`、`password` | `{"username", "login": {"expires", "token"}}`：设好了，换一个登录令牌 |
| `account.logout` | `all`（布尔，默认 `false`） | `{"revoked": <作废了几个>}`：作废这一个登录令牌，`all` 的作废全部 |

一次性码用过就作废。设好密码以后，以前的浏览器登录全部作废。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 参数不对；用户名、密码不合要求 |
| `local_only` | 扩展调了，或者不是本机令牌连上的调了 `account.setup_code` |
## `login.ticket`、`login.redeem`

交接票：已经登录的人从一个页面跳到另一个程序的页面（网页跳到接入通讯平台的页面），不用再登录一次。跳过去之前要一张票带在网址里，接的一方拿它换一个登录令牌，再照这个登录令牌连核心（`miyu-client` 的 `connect_login`，经本机套接字，握手带 `login`；页面自己的登录框用 `connect_password`，交回换到的登录令牌）。

**谁能调**：`login.ticket` 只给人（本机令牌、登录令牌、密码连上的），核心拉起的扩展回 `local_only`；`login.redeem` 谁拿着票谁能调。

| 方法 | 参数 | 回应 |
|---|---|---|
| `login.ticket` | 没有 | `{"ticket", "expires"}`：64 位十六进制的票，60 秒后作废，绑这个连接的账号 |
| `login.redeem` | `ticket` | `{"account", "login": {"expires", "token"}}`：票绑的账号、换出的登录令牌（30 天） |

一张票只换一次；同时最多 16 张，多了作废最早的。票只在核心的内存里，核心重启全部作废。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | `login.redeem` 没写 `ticket` |
| `local_only` | 扩展要票 |
| `bad_ticket` | 票没有、过期了、用过了 |

