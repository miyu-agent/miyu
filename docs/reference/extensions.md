# 扩展：核心拉起的程序怎么说

扩展是软件包带的程序，核心照清单拉起它，经标准输入、标准输出说同一套协议。这一页列扩展特有的：拉起时给它什么、它能调哪些只给扩展的方法、核心会主动调它什么。从头写一个扩展的教程见 `docs/authors/`。英文版：[extensions.en.md](extensions.en.md)。

| 方法 | 方向 | 做什么 |
|---|---|---|
| [`provide`](#provide) | 扩展 → 核心 | 登记它提供的工具 |
| [`tool.call`、`tool.cancel`](#toolcalltoolcancel) | 核心 → 扩展 | 调它的工具；放下一次调用 |
| [`package.methods`、`method.call`](#packagemethodsmethodcall) | 两个方向 | 登记后台页、界面块要调的方法；核心转来的调用 |
| [`package.emit`](#packageemit) | 扩展 → 核心 | 推给开着它页面的人 |
| [`feature.offer`](#featureoffer) | 扩展 → 核心 | 替这个人交预设菜单里的功能（施工 IM-6） |
| [`port.claim`、`port.release`、`port.list`](#portclaimportreleaseportlist) | 扩展 → 核心（`port.list` 给主人） | 登记、放开要听的端口；看整张表（施工 IM-5） |
| [`command.call`](#commandcall) | 核心 → 扩展 | 人打了它声明的斜杠命令 |
| [`user.append`](#userappend) | 核心 → 扩展 | 回合开始时问它要不要往上下文里放几句 |
| [`tool.guard`、`guard.cancel`](#toolguardguardcancel) | 核心 → 扩展 | 工具调用执行前问它放行、拒绝还是问人；不等了 |
| [`system.append`](#systemappend) | 扩展 → 核心 | 往系统提示词末尾放一段 |
| [`widget.set`、`widget.clear`、`widget.watch`、`widget.unwatch`](#widgetsetwidgetclearwidgetwatchwidgetunwatch) | 两个方向 | 界面块的内容 |
| [`extension.config`](#extensionconfig) | 核心 → 扩展 | 它自己的配置变了 |
| [`schedule.fired`](schedules.md#schedulefired) | 核心 → 扩展 | 它建的定时到点了（施工 J-4） |
| [`content.list`、`content.read`、`content.changed`](#contentlistcontentreadcontentchanged) | 两个方向 | 读别的包给它带的内容 |
| [`events.append`](#eventsappend) | 扩展 → 核心 | 往会话里记一条自己的事件 |

别的方法照一张表挡：只给人的（替人回答确认、改权限级别和工作目录、删会话、改配置、装卸软件包……）回 `local_only`；要某项能力的，清单没声明回 `needs_approval`（见下面「能力」）；别的照常。每个方法归哪一种写在各页的「谁能调」里，表里没有的方法当只给人。

## 拉起

- **程序**：先找包文件夹的 `bin/`，再找 `miyu` 旁边，不走 `PATH`；参数是清单的 `[process] args`。
- **环境**：照核心的，另设 `MIYU_HOME`（数据根）、`MIYU_RESOURCES`（资源目录）、`MIYU_PROGRAM`（主程序 `miyu` 的真实位置，施工 G-15），和这个包在各层的那一格（设计 55 第四节，都已经建好、交真实位置）：`MIYU_APP_DIR`（包的文件夹）、`MIYU_CONFIG_DIR`、`MIYU_STATE_DIR`（也是工作目录）、`MIYU_CACHE_DIR`、`MIYU_LOG_DIR`、`MIYU_TMP_DIR`（拉起前清空）、`MIYU_RUNTIME_DIR`，`MIYU_PICTURES_DIR` 这四个。旧名 `MIYU_PACKAGE_DIR`、`MIYU_PACKAGE_STATE` 过渡期间一起给。
- **工作目录**：这个包放状态的目录 `var/lib/<编号>/`。
- **标准错误**：追加到 `var/log/<编号>/stderr`，超过 1 MiB 的先挪成 `.old`。
- **握手**：照常发 `hello`，不出示凭据。回应的 `config` 是它自己的配置项的最终值（密钥是真值）。
- **退出**：核心关它的标准输入就是「请退出」，5 秒没退的杀掉。
- **崩了**：退出码 1 以外的退出、被杀、握手不成都算一次失败，退避 1、2、4、8、16 秒再拉起，连续 5 次停下（`failed_repeatedly`）。**退出码 1** 表示配置错、端口被占这类重启也没用的：不再拉起，直接停下（`config_error`）。连着跑满 60 秒的，失败次数从零数。

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
- **退出就放开**：进程退出、连接断了登记的都算放开，不用自己放。核心重启表就空了，起来再登记。
- **`port.list {}`**：只给主人（别人 `not_permitted`）：`{"ports": [{"port", "package"}]}`，照端口排。

不是核心拉起的连接调 `port.claim`、`port.release` 回 `not_an_extension`。

## 能力

清单 `[process] capabilities` 声明要什么。自己装的包要人批：装的时候批（`package.install`、`package.upgrade` 带 `approve`，施工 PK-3 上，见 [软件包](packages.md)），没批全的打开时再批（`extension.enable` 带 `approve`）；出厂的算批过，`login.page` 除外（出厂的也要人批，施工 LP-1）。还有没批的不拉起。批过的记在开关文件 `extensions.json` 的 `approved` 里（包的编号到能力），只留这一份清单声明了的；换了来路（本机文件盖过去、换了源、换了来源仓库）装的不接着用以前批的；卸包时删掉。

| 能力 | 能做什么 |
|---|---|
| `tools` | 向工具目录提供工具（`provide`） |
| `tools.commands` | 提供的工具会跑命令（`provide` 规格写 `runs_commands`）：这一次调用期间，你起的命令里能用 `miyu call` 以这个会话的身份调 AI 的工具（施工 XG-1 下） |
| `commands` | 提供斜杠命令（清单 `[[slash]]`） |
| `widgets` | 在界面上显示自己的内容（界面块） |
| `context.inject` | 回合开始时往上下文里放几句（`user.append`） |
| `context.system` | 往系统提示词末尾放一段（`system.append`） |
| `packages.call` | 调别的包导出的方法（`package.call`） |
| `search` | 当搜索服务供应商（清单 `[[search]]`，`search.query`） |
| `act_for_external` | 代表平台上的人说话（带 `as` 的 `session.send`、`command.run`）、`session.note`、不写 `to` 的 `session.respond`、`venue.*`、记 `venue.*` 事件：只给通讯平台的桥 |
| `login.page` | 替本机打开的页面换登录票（`login.ticket {"via": "page"}`，下面「替本机页面换登录票」）：出厂的也要人批 |
| `sessions.drive` | 驱动会话：`session.create`、`send`、`interrupt`、`revert`、`unrevert`、`redo`、`compact`、`clear`、`configure`、`fork`、`set_meta`、`respond`，`command.run`、`job.stop` |
| `events.read` | 读会话：订阅 `events`、`view`、`sessions` 流，`session.list`、`session.recap`、`view.page`、`view.detail`、`job.output` |
| `events.write` | 往会话里记自己的事件（`events.append`） |
| `memory.write` | 写记忆、知识库：`memory.remember`、`import`、`update`、`forget`、`restore`、`dream`、`dream_stop`，`kb.add`、`create`、`update`、`mount`、`remove`、`delete`、`reindex` |
| `fs.read` | 经核心读文件（`fs.*`） |
| `network` | 经核心抓链接（`link.preview`） |
| `fs.write` | 写指定的路径：扩展进沙盒以后照它放行 |
| `tool.guard` | 工具调用执行前放行、拒绝或者要问人（清单 `[guard]`，`tool.guard`） |
| `tool.rewrite` | 工具执行后改写结果：还没有 |

没声明的，调对应的方法回 `needs_approval`，`data.capabilities` 是缺的那一项。`network`、`fs.*` 管的是经核心做的；你的程序自己联网、读写文件，等扩展进沙盒以后才照它们挡。

## 替本机页面换登录票：`login.ticket`

你的包带一个网页（例如平台桥的管理页），想让主人在这台电脑上打开它时不用输访问密码（施工 LP-1，项目主人 2026-11-06 定：只给本机免密）。声明 `login.page`、人批过以后，在扩展连接上调 `login.ticket {"via": "page"}`，回 `{"ticket", "expires"}`：60 秒、一次性。页面拿它照交接票登录：握手凭据 `ticket`（直连核心的页面），或者你在扩展连接上替它 `login.redeem {"ticket"}` 换一个设备令牌（见 [连接](connection.md)「`login.ticket`、`login.redeem`」）。

- 只认 `via` 是 `"page"`：不写、写别的回 `local_only`；没批 `login.page` 的回 `needs_approval`，`data.capabilities` 是 `["login.page"]`。出厂的包声明了它也要人批，不算批过。
- 票只在本机换：网页软件转来的远程连接（握手之前说过 `connection.remote` 的，不管说的是什么地址）拿它握手、`login.redeem` 都回 `bad_ticket`，票当场作废。你自己在扩展连接上 `login.redeem` 算本机：核心看不到你的页面是谁、从哪儿打开的，下面几条只有你守得住。
- 每发一张，核心的运行日志记一行 `ticket issued purpose=page by=extension package=<你的编号>`，不记票。

**你的义务**：

1. 只给这样的请求要票、换票：
   - 对端地址是回环：IPv4 的 `127.0.0.0/8`、IPv6 的 `::1`；IPv4 映射进 IPv6 的（`::ffff:127.0.0.1`）照 IPv4 算。
   - 不带反向代理加的头（`Forwarded`、`X-Forwarded-*`、`X-Real-IP`、`Via`、`CF-Connecting-IP`、`True-Client-IP`、`Tailscale-*` 这类）：同机的反向代理转进来的，对端也是回环，只能靠这些头认出来，认出来的当远处来的。
   - `Host` 是本机的写法：`127.0.0.1`、`localhost`、`[::1]`，加你在听的那个端口；你听局域网时也只认这几种（挡 DNS 重绑定）。
   - `Origin` 和 `http://` 加 `Host` 逐字一样（同源），或者没带。
2. 远处来的页面（局域网、反向代理转来的）一律走核心的访问密码登录（握手凭据 `password`，`miyu-client` 的 `connect_password`），不能拿这张票顶替，也不能自己另设密码、核过了再替它换票。
3. 票不进你的日志；交给页面放在网址的片段（`#`）里，页面拿到就换掉、从地址栏去掉。
4. 建议照网页软件的 `/ticket` 再加一道：要票的请求带一个自定义头（例如 `X-Miyu-Local: 1`），正文不是表单的三种类型；别的网站的页面发不出带自定义头的请求。

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
| `role` | 职能：核心照它认这件是干什么的，不看名字。现在只有 `read_file`（读文本文件：工具面上有这样一件，AI 才看得到技能列表）。不写是没有 |
| `scope` | 给哪种会话，只能在 `venues` 之内再收窄：`any`（不写就是它）、`local`（本机的会话）、`local_main`（本机的主会话，不给子代理）、`local_main_attended`（本机的主会话，而且有人看着）、`spawn`（本机、子代理的层数没到上限） |
| `timeout_ms` | 1000 到 600000，默认 60000 |
| `need_load` | 退役（施工 FL-1，工具一律全量）：写了的照旧登记得上，不看 |
| `clicall` | `true`：AI 在命令里用 `miyu call` 也能调它（只读、联网、执行类的才调得成），不写是不能。有结构化结果的写进回应的 `data`，脚本拿到一行 JSON |
| `result` | 有结构化结果的写它的样子，一行字，例如 `{"memes":[{"id","tags"}]}`：告诉 AI 脚本里能直接取哪几格（`miyu call --schema` 也印）。不写是没有 |
| `display_name` | 给人看的名字：`{"zh": "…", "en": "…"}` |
| `subject` | 给人看的名字后面跟哪个参数的值，要是 `input_schema.properties` 里的一个 |
| `paths` | 哪几个参数是路径：`[{"parameter": "file_path", "write": true}]`。参数要是 `input_schema.properties` 顶层的一个，类型是字或字的数组；`write` 不写是读，写了真的 `access` 要是 `write`。核心照它判这次调用要碰的路径（工作区外的写问人，和内置的 `edit` 一样），`tool.call` 带 `checked`，回应里的文件效果只收这几条。不写的（写 `null` 同不写）不报路径，文件效果一条都不收 |
| `background_parameters` | 只在会话有后台时才给的参数名：`["run_in_background"]`。要是 `input_schema.properties` 顶层的、不在 `required` 里的。后台运行关着的会话里，AI 看到的参数格式没有它们。写 `null` 同不写 |
| `runs_commands` | `true`：这件工具会跑命令（施工 XG-1 下）。包要批过能力 `tools.commands`。AI 看到的说明后面接上 `miyu call` 怎么用那一段（和内置 `shell` 一样）；拿到了通道的调用，`tool.call` 带 `channel`（见下面「跑命令」）。不写、写 `null`、`false` 的不是 |

**回应**：`{"tools": <件数>}`。连接断了工具照旧留在目录里，被调到时回「暂时不可用」。

**出错**：`not_a_provider`（不是核心拉起的扩展）；`bad_tool`：`data.tool` 是哪一件，`data.problem` 是 `duplicate`、`name`、`parameters`、`access`、`venues`、`timeout`、`clicall`、`result`、`display_name`、`subject`、`feature`、`plugin`、`role`、`scope`、`paths`、`background_parameters`、`runs_commands`（写了真假、`null` 以外的）；`needs_approval`：写了 `runs_commands` 的、包没批 `tools.commands`，`data.capabilities` 是 `["tools.commands"]`，整个不收。

## `tool.call`、`tool.cancel`

**`tool.call`**（核心 → 扩展，带 `id`，要回应）：

| 名字 | 说明 |
|---|---|
| `session` | 哪个会话 |
| `call_id` | 这次调用的编号 |
| `tool` | 工具名 |
| `args` | 参数 |
| `cwd` | 这一轮的工作目录 |
| `pictures` | 放图的目录（数据根里照 XDG 的 `pictures/`，施工 Y-11 下）：画出来、生成的图放这里，人在终端、网页里找得到。真实位置，可能还没建，写之前自己建好（缺的上级一起建）。只给本机的会话：会话外面的调用、场所会话（群、私聊，关在自己的工作区里，施工 Y-11 下修）没有这一格，照旧放工作区 |
| `by` | 是谁要她做的：触发这一轮的那条消息的 `by`；没有触发的回合不写 |
| `owner` | 核心认不认 `by` 是主人本人 |
| `checked` | 只给声明了 `paths` 的：权限检查判过的真实位置，照参数里路径的先后（数组每项一条）。写文件就写这几个：你自己把参数里的路径换成真实位置以后，不在这里面的别碰（检查以后路径上的链接可能被换过） |
| `channel` | 只给声明了 `runs_commands` 的、这次调用开了通道的：`{"sock", "token", "bin"}`，见下面「跑命令」 |

**回应** `{"result": {"blocks": [...], "error"?, "end_turn"?, "sent"?, "detail"?, "data"?, "state"?, "effects"?}}`：

| 名字 | 说明 |
|---|---|
| `blocks` | 给模型看的内容块：`{"type": "text", "text"}`、`{"type": "image", "blob", …}`。图先 `blob.put` |
| `error` | 这次调用算出错 |
| `end_turn` | 这一步的结果都到了以后，不再请求模型，这一轮结束 |
| `sent` | 她「发出去」的图，至多 4 张：`[{"blob", "media_type", "width", "height", "name"?, "alt"?, "sticker"?}]`。不给模型看；头画在正文里，聊天平台照它发 |
| `detail` | 给头看的结构化结果，照下面三种通用形状写一种；不给模型看。形状不对的、`error` 是真的，核心丢掉这一格、结果照常 |
| `data` | 给脚本的结构化结果，任意 JSON：AI 在命令里用 `miyu call` 调到的（`clicall`），印成一行 JSON 交给脚本；不给模型看，不记进 `tool.result`。`error` 是真的不交 |
| `state` | 你这个包在这个会话里的一份状态，写法见下面「`session.set_state`、`session.state`」：核心照会话记最新一份，撤销、清空上下文跟着走。写坏了整份丢掉、结果照常；`null` 当没写，清掉写 `{}`；`error` 是真的不收 |
| `effects` | 这次调用读了、改了、删了哪些文件，换上的待办，照先后：写法见下面「效果」。核心查过才收，收下的和内置工具报的一样：时间线上有增删行数、能看差异，AI 读过的文件被你改了不用重读 |

**效果**（`effects` 的每一项，`kind` 分开）：

| `kind` | 其余几格 | 核心怎么查 |
|---|---|---|
| `file.changed` | `path`；可以带 `before`、`after`（内容哈希 `sha256:…`）、`before_text`、`after_text` | `path` 要是 `checked` 里判了写的。改前是发 `tool.call` 之前核心读的，改后是回应到了核心现读的；前后一样的、带了的哈希原文对不上的、超过 20 MiB 的不收 |
| `file.read` | `path`；`lines`：`[从, 到]`，从 1 数起，从不大于到（写错的整条丢） | `path` 要是 `checked` 里的（读写都行）；哈希由核心照现在的整份算 |
| `file.trashed` | `path`、`trash`（回收站里的位置，只给人看，至多 4096 个字） | `path` 要是 `checked` 里判了写的，现在要不在了 |
| `todo.written` | `todos: [{"content", "status"}]`、`done`?（同内置的 `todowrite`；写 `null` 同不写） | 只查写法；`error` 是真的不收 |

- 相对的 `path` 照这次调用的 `cwd` 算，`~` 开头的照家目录。只报「哪个文件」：内容核心自己读，你报的内容骗不了看差异的人。
- 别的种类、写法不对的、没判过的路径、同一种同一个文件报第二遍的，这一条丢掉，结果别的照常，不算出错；运行日志记一行 `tool effect dropped`（`why` 是 `not_checked`、`unchanged`、`mismatch`、`too_big`、`kind`、`shape` 之一）。
- `error` 是真的照样收文件效果：改了一半出错，改了的是真的。
- 撤销不改回文件（和内置工具一样），只退回 AI 看过的。

**跑命令**（声明了 `runs_commands` 的，施工 XG-1 下）：会话里有 AI 能在脚本里调的工具时，这次调用开一个只给它的端点，`tool.call` 带 `channel`：

| 名字 | 说明 |
|---|---|
| `sock` | 端点：Unix 上是套接字的路径，Windows 上是命名管道的名字（`\\.\pipe\miyu-call-…`）。设成环境变量 `MIYU_CALL_SOCK` |
| `token` | 令牌，32 个十六进制字。设成环境变量 `MIYU_CALL_TOKEN` |
| `bin` | 只放着 `miyu` 的目录，放到 `PATH` 最前面：命令里调到的 `miyu` 和在跑的核心是同一个版本。建不成的不写 |

- 把这三样设进你为这次调用起的命令。命令里的 `miyu call <工具>` 照会话的权限、预设调 AI 的工具，用法、退出码和内置 `shell` 里跑的一样（退出码 1 工具出错或被拒，2 用法不对或认不出你，3 这次调用已经完了，4 要人确认）。
- 只认你这个扩展进程的子孙：连上来的进程照父进程一层层往上找，要找得到你的进程；你自己、`setsid` 或两次 `fork` 甩出去的、别的进程拿着令牌也连不上（退出码 2）。这是防误用（令牌漏给了别的进程、命令甩出去以后还想调），不是挡你的扩展：你起一个子进程就能调。真正的关口是包的能力 `tools.commands` 要人批。
- 这次调用答了、超时、被掐，端点就关，还在等的、之后连的 `miyu call` 退出码 3：要命令里调工具，等命令跑完再回应。
- 人在会话里打的命令（`!`）不交给你，只交给内置 `shell`。

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

## `session.set_state`、`session.state`

包在一个会话里的一份状态（设计 46）：你画在界面块上的、压缩时给模型留的、你自己的数据。要 `sessions.drive` 能力；只读写你这个包的、你替的那个人的会话。

```json
{"widgets": {"todos": {"content": [<界面块的内容>], "title"?: <标题>}}, "note": "Todo list:\n- [ ] ...", "data": {...}}
```

| 格 | 说明 |
|---|---|
| `widgets` | 块的编号到 `{"content", "title"?}`：块要是你清单里写了 `state = true` 的（只给 `scope = "session"` 的块），内容写法同 `widget.set`、照那一块的位置查。头照它画，你的程序没在跑也一样 |
| `note` | 压缩时原样写进检查点、给模型看的一段英文（最多 2000 个字，CJK 不过半，除了换行没有控制字符）。平常的回合不发给模型 |
| `data` | 你自己的数据，任意 JSON，写出来最多 16 KiB：核心只存，`session.state` 原样还给你 |

- 三格都可以不写；`{}`、`null` 是清掉。写坏了的整份不收：`widgets` 里的内容不对回 `bad_widget_content`（`path` 从 `widgets.<块>` 写起），别的回 `bad_params`，`field` 是哪一格（例如 `state.widgets.other`：块不是你声明成状态块的）。
- **`session.set_state {"session", "state"}`**：换上一份，回合进行中、空闲时都收，不开回合，落了盘回 `{}`。回合进行中写的跟着那一轮：撤掉那一轮就退回更早的一份。
- **`session.state {"session"}`**：回 `{"state": {...} | null}`，你这个包当前的那一份（含 `note`、`data`）。
- 清空上下文（`/clear`）清掉所有包的；平常的压缩不动。

## `package.methods`、`method.call`

**`package.methods`**（扩展 → 核心）：`{"methods": [{"name", "timeout_ms"?}…]}`，登记它的后台页、界面块要调的方法，换掉上一次的。回应 `{"methods": <个数>}`。别的连接回 `not_an_extension`。

**`method.call`**（核心 → 扩展）：`{"method", "params", "from"?}`：人的头、别的包经 `package.call` 调到它；`from` 是调的包的编号（人调的没有）。回什么，`package.call` 就交回什么；回 JSON-RPC 错误的，对方收到 `method_failed`。

想让别的扩展调到：在清单 `[export] methods` 里列出来。

## `package.emit`

扩展 → 核心（施工 IM-1 中下二）：`{"event", "data"?}`，推给订了 `subscribe {"stream": "package", "package": <它的编号>}` 的连接，对方收到 `package.event {"package", "event", "data"}`。页面照它更新，不用轮询。回应 `{}`；没人订着也是。`event` 写法同方法名（小写字母开头，小写字母、数字、`_`、`.`、`-`，最多 64 个），`data` 写成 JSON 最多 256 KiB，不对的 `bad_params`；别的连接 `not_an_extension`。不存盘、不补。

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

## `tool.guard`、`guard.cancel`

AI 的一次工具调用执行之前问你：放行、拒绝，还是要问人（核心 → 扩展，带 `id`，要回应）。公司策略（某些目录不许写、某些命令要问人）、审计这类用法。

**谁被问**：清单 `[process] capabilities` 声明了 `tool.guard`、写了 `[guard]`、主人批了、开着的扩展。`[guard]` 写一张静态的表，核心起来时就照它建好：

```toml
[guard]
tools = ["shell", "write", "edit"]   # 工具名，可以不写
access = ["execute", "write"]        # 访问类别，可以不写
timeout_ms = 1000                    # 等你多久，可以不写
```

- 一次调用的工具名在 `tools` 里，或者它的访问类别在 `access` 里，才问你；两样都没写的清单读不进（`bad_guard`，`miyu check` 报），不许「什么都问」。访问类别是 `read`、`write`、`execute`、`network`、`outbound`、`venue`、`install`。
- 自带的、别的包的、MCP 服务的工具都照名字、访问类别认；你自己 `provide` 的工具不问你。不用 `provide` 也能当守卫。
- `timeout_ms` 不写是 1000，只能写 100 到 5000。
- 权限策略先判：它拒绝的不问你；放行、要问人的再问你。几个守卫并行问，最严者胜：有一个拒绝就拒绝，没有拒绝、有一个要问人的问人；策略要问人的，你说放行也照旧问人。你只能收紧，不能放开，也不改沙盒。
- 脚本里调的工具（`miyu call`）同样问你，`call_id` 是嵌套调用的编号（`call_12_1#3` 这样）。

**参数**：`{"session", "call_id", "tool", "args", "access", "cwd", "owner", "venue"}`：会话、这次调用的编号、工具名、修正过的参数（执行时就是这一份）、访问类别、这一轮的工作目录、开口的是不是主人（同 `tool.call` 的 `owner`）、在哪说的（`local`、`private`、`group`）。

**回应** `{"verdict": "allow" | "deny" | "ask", "reason"?}`：

- `allow`：放行，不进上下文，AI 只看到工具的结果。
- `deny`：拒绝，工具不跑。AI 看到的结果是 `Blocked by <你的包编号>: <reason>`：`reason` 去掉头尾空白、截到 300 个字，换行、引号、尖括号转义成 `\n`、`\u0022` 这样，整句一行；没写的是 `no reason given`。时间线上的结果 `by` 是你的包，给人看的照你写的原文。
- `ask`：要问人，走平常的确认（`tool.approval_requested`，`by` 是你的包，`detail` 是 `{"guard", "tool", "reason"?}`），只能允许这一次或者拒绝；没人能确认的会话照拒。
- 多了格、`verdict` 不认识、`reason` 不是字的，照拒绝算，理由 `the guard answered in a wrong format`。

**没答的算拒绝**（守卫失效往严里走）：

| 什么时候 | AI 看到的理由 |
|---|---|
| 过了 `timeout_ms` 没答 | `the guard did not answer in time` |
| 你开着却没在跑（正在起、刚崩、起不来），问着问着连接断了 | `the guard is not running` |
| 回了 JSON-RPC 错误 | `the guard returned an error` |

主人关掉的扩展不问。p95 最好在 20 毫秒以内：人一直等着这一步。

**`guard.cancel`**（核心 → 扩展，通知）：`{"session", "call_id"}`。人打断、回合被取消、你超时了，核心不等了：这次调用照打断收尾（或者已经照超时拒了），你的回答到了也不理。

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

**参数**：`session`、`kind`、`body`（JSON 对象，最多 16 KiB）。扩展只能记 `ext.<自己的前缀>.<名字>`（前缀是清单的 `prefix`，没写的是包编号）（每段小写字母开头，只有小写字母、数字、`_`、`-`，整个最多 128 个字）。通讯平台的桥另能记 `venue.recalled`、`venue.delivered`（见 [场所](venues.md)）。

**回应**：`{"seq": <序号>}`。

**出错**：`bad_params`；`session_not_found`。
