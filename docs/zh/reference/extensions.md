# 扩展：核心拉起的程序怎么说

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

扩展是软件包带的程序：核心按照清单拉起它，通过标准输入、标准输出和它使用同一套协议通信。这一页列出扩展特有的内容：拉起时核心给它什么、它能调用哪些只给扩展的方法、核心会主动调用它的哪些方法。从头编写一个扩展的教程，可以看 [写扩展](../dev/extensions.md)。

| 方法 | 方向 | 做什么 |
|---|---|---|
| [`provide`](#provide) | 扩展 → 核心 | 登记它提供的工具 |
| [`tool.call`、`tool.cancel`](#toolcalltoolcancel) | 核心 → 扩展 | 调用它的工具；放弃一次调用 |
| [`package.methods`、`method.call`](#packagemethodsmethodcall) | 两个方向 | 登记后台页、界面块要调用的方法；接收核心转来的调用 |
| [`package.emit`](#packageemit) | 扩展 → 核心 | 推送给打开着它的页面的人 |
| [`feature.offer`](#featureoffer) | 扩展 → 核心 | 替这个人向预设菜单提交功能 |
| [`port.claim`、`port.release`、`port.list`](#portclaimportreleaseportlist) | 扩展 → 核心（`port.list` 给主人） | 登记、释放要监听的端口；查看整张表 |
| [`command.call`](#commandcall) | 核心 → 扩展 | 人输入了它声明的斜杠命令 |
| [`user.append`](#userappend) | 核心 → 扩展 | 回合开始时询问它要不要往上下文里放几句话 |
| [`tool.guard`、`guard.cancel`](#toolguardguardcancel) | 核心 → 扩展 | 工具调用执行之前询问它：放行、拒绝，还是询问人；不再等待它的回答 |
| [`tool.after`](#toolafter) | 核心 → 扩展 | 工具调用的结果写入磁盘以后通知它（只读，不等待回答） |
| [`driver.call`、`driver.delta`、`driver.cancel`](#drivercalldriverdeltadrivercancel) | 两个方向 | 它提供的模型驱动：接收一次模型调用、返回增量；不再等待 |
| [`driver.encode`](#driverencode) | 核心 → 扩展 | 编码一次但不发送：检查编码是否稳定 |
| [`driver.models`](#drivermodels) | 核心 → 扩展 | 列出它能用的模型（可选） |
| [`system.append`](#systemappend) | 扩展 → 核心 | 往系统提示词的末尾放一段内容 |
| [`widget.set`、`widget.clear`、`widget.watch`、`widget.unwatch`](#widgetsetwidgetclearwidgetwatchwidgetunwatch) | 两个方向 | 界面块的内容 |
| [`extension.config`](#extensionconfig) | 核心 → 扩展 | 它自己的配置发生了变化 |
| [`schedule.fired`](schedules.md#schedulefired) | 核心 → 扩展 | 它创建的定时任务到点了 |
| [`content.list`、`content.read`、`content.changed`](#contentlistcontentreadcontentchanged) | 两个方向 | 读取其他软件包给它带来的内容 |
| [`events.append`](#eventsappend) | 扩展 → 核心 | 往会话里记录一条它自己的事件 |

其他方法按照一张表来拦截：只给人调用的方法（替人回答确认、修改权限级别和工作目录、删除会话、修改配置、安装和卸载软件包……）返回 `local_only`；需要某项能力的方法，如果清单没有声明这项能力，就返回 `needs_approval`（见下面「能力」）；其余的方法照常可用。每个方法属于哪一种，写在各页的「谁能调用」里；表里没有的方法当作只给人调用。

## 拉起

- **程序**：核心先在包文件夹的 `bin/` 里找程序，再到 `miyu` 所在的目录里找，不从 `PATH` 里找；启动参数是清单的 `[process] args`。
- **环境**：扩展继承核心的环境变量，核心另外设置 `MIYU_HOME`（数据根）、`MIYU_RESOURCES`（资源目录）、`MIYU_PROGRAM`（主程序 `miyu` 的真实位置），以及这个包在各层里各自的目录（都已经创建好，给出的是真实位置）：`MIYU_APP_DIR`（包的文件夹）、`MIYU_CONFIG_DIR`、`MIYU_STATE_DIR`（也是工作目录）、`MIYU_CACHE_DIR`、`MIYU_LOG_DIR`、`MIYU_TMP_DIR`（拉起之前清空）、`MIYU_RUNTIME_DIR`，还有 `MIYU_PICTURES_DIR`、`MIYU_VIDEOS_DIR`、`MIYU_DOCUMENTS_DIR`、`MIYU_DOWNLOADS_DIR` 四个目录（数据根里的 `root/pictures/`、`root/videos/`、`root/documents/`、`root/downloads/`）。旧的变量名 `MIYU_PACKAGE_DIR`、`MIYU_PACKAGE_STATE` 在过渡期间也会一起设置。
- **工作目录**：工作目录是这个包存放状态的目录 `var/lib/<编号>/`。
- **标准错误**：标准错误追加写入 `var/log/<编号>/stderr`；文件超过 1 MiB 时，核心先把它改名成 `.old`。
- **握手**：扩展照常发送 `hello`，不需要出示凭据。回应里的 `config` 是它自己的配置项的最终值（密钥给出的是真实的值）；批准了 `provider.key` 的扩展，回应里还多一个字段 `provider_key`（见下面「`provider_key`」）。
- **退出**：核心关闭扩展的标准输入，就表示「请退出」；5 秒以后还没有退出的扩展会被杀掉。
- **核心意外退出**：核心崩溃或者被强行结束时，你和你启动的子进程会跟着结束。在 Unix 上，你运行在一个属于你自己的进程组里（你不是组长），组里有一个由核心看守的进程，核心一旦消失，整组进程就会被杀掉；用 `setsid` 脱离出去、自己换了进程组的进程不在此列。在 Windows 上，你和你启动的进程都在核心的作业对象（job object）里。有计划的停止按照上一条处理。因为你不在核心的进程组里，所以在终端里按 Ctrl+C 不会传到你这里。
- **崩溃**：退出码 1 以外的退出、被杀掉、握手失败，都算一次失败；核心依次等待 1、2、4、8、16 秒再重新拉起，连续失败 5 次就停止（`failed_repeatedly`）。**退出码 1** 表示配置错误、端口被占用这类重启也解决不了的问题：核心不再拉起，直接停止（`config_error`）。连续运行满 60 秒以后，失败次数重新从零开始计算。

## `feature.offer`

替这个人向预设菜单提交几个功能，例如 `查询 QQ 群聊记录`，并且带上功能的工具：

```json
{"features": [{"id": "qq-history", "name": {"zh": "查询 QQ 群聊记录", "en": "QQ history"}, "summary": {"zh": "…"}, "tools": ["qq_history"]}]}
```

- 返回 `{}`。每次提交的都是整份，会替换你上一次替这个人提交的功能；提交空的列表表示全部撤回。
- 这些功能只记在核心的内存里：每次连接上、你自己的开关发生变化时都要重新提交一遍，核心重启以后也要再提交一次。你的进程退出时不会清除它们。
- `id` 的写法和包的编号相同，不能和这个人已经安装的功能、其他包提交的功能重复；`name`、`summary` 是一句话，或者语言到文字的表（和清单的 `[features]` 相同）；`tools` 必须是你已经 `provide` 过的工具。写得不对时返回 `bad_params`（`field` 是 `features`），并且什么都不替换。
- 提交的功能出现在这个人的预设菜单里，归在你的包下面；功能列出的工具属于这个功能，不属于你的包自己的功能。功能开不开由预设决定：出厂的「全部功能」打开它们，其他出厂预设关闭它们，人可以在预设里打开。已经打开的会话从下一个回合开始按新的功能计算。
- 在你接入的平台的群聊、私聊里，你的整个包都不经过预设（`venues.md`），所以这里提交的功能只对本机会话和其他平台起作用。

核心另外还有一个给人修改全局开关用的 `feature.set`，两者名字不同，不要混淆。

## `port.claim`、`port.release`、`port.list`

需要自己打开端口监听的扩展（例如让 NapCat 反向连接 im 的那个扩展）要先登记端口：这样端口冲突能当场发现，不用等到操作系统报告绑定失败。核心只记录一张表，不替你监听，真正占用端口的仍然是操作系统。

- **`port.claim {"port": 1–65535}`**：没有人登记的端口记成你的，返回 `{}`；你在同一个连接上再登记一次，仍然返回 `{}`；其他扩展已经登记了的端口返回 `port_taken`，不说明是谁登记的。端口写 `0` 时返回 `bad_params`。
- **`port.release {"port"}`**：如果端口是你登记的，就把它移除；不是你登记的不做任何修改。两种情况都返回 `{}`。
- **退出就释放**：进程退出、连接断开时，登记过的端口都算已经释放，不需要自己释放。核心重启以后表是空的，扩展启动以后要重新登记。
- **`port.list {}`**：只给主人调用（其他人调用返回 `not_permitted`），返回 `{"ports": [{"port", "package"}]}`，按端口排序。

不是由核心拉起的连接调用 `port.claim`、`port.release` 时，返回 `not_an_extension`。

## 能力

清单的 `[process] capabilities` 声明扩展需要哪些能力。自己安装的包需要人批准：可以在安装时批准（`package.install`、`package.upgrade` 带上 `approve`，见 [软件包](packages.md)），如果安装时没有全部批准，可以在打开时再批准（`extension.enable` 带上 `approve`）；出厂的包算作已经批准，但是 `login.page`、`provider.key` 除外（出厂的包也要人批准）。还有能力没有批准的扩展不会被拉起。批准过的能力记在开关文件 `extensions.json` 的 `approved` 里（从包的编号到能力），只保留当前这份清单声明了的能力；如果换了来路安装（用本机文件覆盖、换了软件源、换了来源仓库），不会继续使用以前的批准；卸载包时这些记录会被删除。

| 能力 | 能做什么 |
|---|---|
| `tools` | 向工具目录提供工具（`provide`） |
| `tools.commands` | 提供的工具会运行命令（`provide` 的规格里写了 `runs_commands`）：在这一次调用期间，你启动的命令里可以用 `miyu call` 以这个会话的身份调用 AI 的工具 |
| `commands` | 提供斜杠命令（清单的 `[[slash]]`） |
| `widgets` | 在界面上显示自己的内容（界面块） |
| `context.inject` | 回合开始时往上下文里放几句话（`user.append`） |
| `context.system` | 往系统提示词的末尾放一段内容（`system.append`） |
| `packages.call` | 调用其他包在总线上导出的方法（`bus.call`）；按包的编号调用自己（`package.call`） |
| `search` | 提供联网服务（清单的 `[[search]]`，`search.query`） |
| `model.driver` | 提供模型驱动（清单的 `[[driver]]`）：使用它的模型时，发给模型的全部对话都要经过它。这项能力不附带联网的能力 |
| `provider.key` | 获取主人在模型设置里已经配置好的某一家供应商的 key（清单的 `[provider_key]`，见下面「`provider_key`」）：出厂的包也要人批准 |
| `act_for_external` | 代表平台上的人发言（带 `as` 的 `session.send`、`command.run`）、`session.note`、不写 `to` 的 `session.respond`、`venue.*`、记录 `venue.*` 事件：只给通讯平台的桥 |
| `login.page` | 替本机打开的页面换取登录票（`login.ticket {"via": "page"}`，见下面「替本机页面换登录票」）：出厂的包也要人批准 |
| `sessions.drive` | 驱动会话：`session.create`、`send`、`interrupt`、`revert`、`unrevert`、`redo`、`compact`、`clear`、`configure`、`fork`、`set_meta`、`retitle`、`respond`，以及 `command.run`、`job.stop` |
| `events.read` | 读取会话：订阅 `events`、`view`、`sessions` 流，调用 `session.list`、`session.recap`、`view.page`、`view.detail`、`job.output`；接收工具调用以后的通知（清单的 `[after]`，`tool.after`） |
| `events.write` | 往会话里记录自己的事件（`events.append`） |
| `memory.write` | 写入记忆和知识库：`memory.remember`、`import`、`update`、`forget`、`restore`、`dream`、`dream_stop`，以及 `kb.add`、`create`、`update`、`mount`、`remove`、`delete`、`reindex` |
| `fs.read` | 通过核心读取文件（`fs.*`） |
| `network` | 通过核心抓取链接（`link.preview`） |
| `fs.write` | 写入指定的路径：扩展进入沙盒以后，按照这项能力放行 |
| `audio.record` | 录制本机麦克风的声音：扩展自己打开设备录音，核心不经手；扩展进入沙盒以后，按照这项能力放行声卡的录音那一端 |
| `audio.play` | 从本机的音箱、耳机播放声音：扩展自己打开设备播放；扩展进入沙盒以后，按照这项能力放行声卡的放音那一端 |
| `tool.guard` | 在工具调用执行之前放行、拒绝或者要求询问人（清单的 `[guard]`，`tool.guard`） |
| `tool.rewrite` | 在工具执行以后改写结果：目前还没有这项功能 |

没有声明能力时，调用对应的方法返回 `needs_approval`，`data.capabilities` 是缺少的那一项。`network`、`fs.*` 管的是通过核心做的事；你的程序自己联网、读写文件，要等扩展进入沙盒以后才会按这两项拦截。

## 替本机页面换登录票：`login.ticket`

你的包带有一个网页（例如平台桥的管理页），你想让主人在这台电脑上打开它时不用输入访问密码（只有本机可以免密）。声明 `login.page` 并且经过人批准以后，在扩展连接上调用 `login.ticket {"via": "page"}`，返回 `{"ticket", "expires"}`：票的有效期是 60 秒，只能使用一次。页面用这张票按照交接票的方式登录：握手凭据写 `ticket`（直接连接核心的页面），或者由你在扩展连接上替它调用 `login.redeem {"ticket"}`，换取一个设备令牌（见 [连接](connection.md)「`login.ticket`、`login.redeem`」）。

- 核心只接受 `via` 是 `"page"` 的请求：不写 `via` 或者写了其他值，返回 `local_only`；没有批准 `login.page` 时返回 `needs_approval`，`data.capabilities` 是 `["login.page"]`。出厂的包声明了这项能力也要人批准，不算作已经批准。
- 票只能在本机换取：网页软件转发来的远程连接（在握手之前发送过 `connection.remote` 的连接，不论它报的是什么地址）拿这张票握手或者调用 `login.redeem`，都返回 `bad_ticket`，并且这张票当场作废。你自己在扩展连接上调用 `login.redeem` 算作本机：因为核心看不到是谁、从哪里打开了你的页面，所以下面这几条只有你能守住。
- 每发出一张票，核心的运行日志就记录一行 `ticket issued purpose=page by=extension package=<你的编号>`，不记录票本身。

**你的义务**：

1. 只为满足下面全部条件的请求申请票、换取票：
   - 对端地址是回环地址：IPv4 的 `127.0.0.0/8`、IPv6 的 `::1`；映射到 IPv6 里的 IPv4 地址（`::ffff:127.0.0.1`）按 IPv4 计算。
   - 不带反向代理添加的请求头（`Forwarded`、`X-Forwarded-*`、`X-Real-IP`、`Via`、`CF-Connecting-IP`、`True-Client-IP`、`Tailscale-*` 这类）：因为同一台机器上的反向代理转发进来的请求，对端地址也是回环地址，只能靠这些请求头识别出来，识别出来的请求要当作从远处来的请求。
   - `Host` 是本机的写法：`127.0.0.1`、`localhost`、`[::1]`，加上你正在监听的那个端口；即使你监听的是局域网，也只接受这几种写法（用来防止 DNS 重绑定）。
   - `Origin` 和 `http://` 加上 `Host` 逐字相同（同源），或者请求没有带 `Origin`。
2. 从远处来的页面（来自局域网、由反向代理转发的）一律使用核心的访问密码登录（握手凭据 `password`，`miyu-client` 的 `connect_password`），不能拿这张票来顶替，也不能自己另外设一个密码、核对通过以后再替它换票。
3. 票不能写进你的日志；把票交给页面时，放在网址的片段（`#`）里，页面拿到以后立刻换取，并且把它从地址栏里去掉。
4. 建议按照网页软件的 `/ticket` 再加一道检查：申请票的请求要带一个自定义请求头（例如 `X-Miyu-Local: 1`），请求正文不能是表单的三种类型；因为其他网站的页面发不出带自定义请求头的请求。

## `provide`

登记这个包现在提供的全部工具，替换它上一次登记的那几件工具。只接受由核心拉起的扩展调用。

**参数** `{"tools": [...]}`。每一件工具的字段：

| 名字 | 说明 |
|---|---|
| `name` | 只能用英文字母、数字、`_`、`-`，长度 1 到 64 个字符；不能和核心自带的工具、其他包的工具重名 |
| `description` | 给模型看的说明，用英文短句书写 |
| `input_schema` | `{"type": "object", …}` |
| `access` | 工具会碰到什么：`read`（只查看，不离开 Miyu）、`write`、`execute`、`network`、`outbound`（向外发送东西）、`venue`（在聊天平台上做动作，例如撤回、禁言）、`install` |
| `venues` | 给哪几种会话使用：`local`、`private`、`group` 里的一个或几个 |
| `plugin` | 场所插件的名字，写法和包的编号相同：在群聊、私聊里，只有那个场所打开了这个插件（[`venue.plugins`](venues.md#venueplugins)）时才给这件工具，本机会话不受影响。不写时一直给 |
| `role` | 工具的职能：核心根据它识别这件工具是做什么的，不看工具的名字。目前只有 `read_file`（读取文本文件：工具面上有这样一件工具时，AI 才能看到技能列表）。不写时没有职能 |
| `scope` | 给哪种会话使用，只能在 `venues` 的范围之内再收窄：`any`（不写时就是它）、`local`（本机的会话）、`local_main`（本机的主会话，不给子代理）、`local_main_attended`（本机的主会话，并且有人看着）、`spawn`（本机的会话，并且子代理的层数还没有到上限） |
| `timeout_ms` | 取值 1000 到 600000，默认 60000 |
| `clicall` | 写 `true` 时，AI 在命令里用 `miyu call` 也能调用它（只有只读、联网、执行类的工具才能调用成功）；不写时不能。有结构化结果的工具把结果写进回应的 `data`，脚本会拿到一行 JSON |
| `result` | 有结构化结果的工具在这里写出结果的样子，写成一行字，例如 `{"memes":[{"id","tags"}]}`：它告诉 AI 在脚本里能直接取哪几个字段（`miyu call --schema` 也会输出它）。不写时没有 |
| `display_name` | 给人看的名字：`{"zh": "…", "en": "…"}` |
| `subject` | 给人看的名字后面跟上哪个参数的值，必须是 `input_schema.properties` 里的一个 |
| `paths` | 哪几个参数是路径：`[{"parameter": "file_path", "write": true}]`。参数必须是 `input_schema.properties` 顶层的一个，类型是字符串或者字符串的数组；不写 `write` 时表示读取，`write` 写成真时，`access` 必须是 `write`。核心根据它判断这次调用要碰的路径（在工作区以外写入时询问人，和内置的 `edit` 一样），`tool.call` 会带上 `checked`，回应里的文件效果也只接受这几个路径。不写（写 `null` 和不写相同）时不报告路径，文件效果一条都不接受 |
| `background_parameters` | 只在会话有后台时才给出的参数名，例如 `["run_in_background"]`。这些参数必须是 `input_schema.properties` 顶层的、不在 `required` 里的参数。在关闭了后台运行的会话里，AI 看到的参数格式里没有它们。写 `null` 和不写相同 |
| `runs_commands` | 写 `true` 表示这件工具会运行命令，此时包必须已经批准了能力 `tools.commands`。AI 看到的说明后面会接上讲 `miyu call` 怎样使用的那一段（和内置的 `shell` 一样）；拿到了通道的调用，`tool.call` 会带上 `channel`（见下面「运行命令」）。不写、写 `null` 或者 `false` 时表示不会运行命令 |

**回应**：`{"tools": <件数>}`。连接断开以后，工具仍然留在目录里，被调用时返回「暂时不可用」。

**出错**：`not_a_provider`（调用方不是由核心拉起的扩展）；`bad_tool`：`data.tool` 是出错的那一件工具，`data.problem` 是 `duplicate`、`name`、`parameters`、`access`、`venues`、`timeout`、`clicall`、`result`、`display_name`、`subject`、`feature`、`plugin`、`role`、`scope`、`paths`、`background_parameters`、`runs_commands`（写了布尔值、`null` 以外的值）之一；`needs_approval`：写了 `runs_commands` 的工具所在的包没有批准 `tools.commands`，`data.capabilities` 是 `["tools.commands"]`，此时整份工具都不接受。

## `tool.call`、`tool.cancel`

**`tool.call`**（核心 → 扩展，带 `id`，需要回应）：

| 名字 | 说明 |
|---|---|
| `session` | 哪个会话 |
| `call_id` | 这次调用的编号 |
| `tool` | 工具名 |
| `args` | 参数 |
| `cwd` | 这一轮的工作目录：真实的绝对路径，`~` 已经按照家目录替换好（不知道家目录时，按照会话记录的原样给出） |
| `pictures` | 放图片的目录（数据根里的 `root/pictures/`）：画出来的、生成的图片放在这里，人可以在终端界面、网页界面里找到它们。给出的是真实位置，但是目录可能还没有创建，写入之前要自己创建好（缺少的上级目录一起创建）。只有本机的会话才有这个字段：会话以外的调用、场所会话（群聊、私聊，关在自己的工作区里）没有这个字段，图片仍然放在工作区里 |
| `by` | 是谁要 AI 做这件事：触发这一轮的那条消息的 `by`；没有触发消息的回合不写 |
| `owner` | 核心是否认定 `by` 是主人本人 |
| `permission` | 派发这次调用那一刻实际生效的权限 `{"level", "read_only"}`：`level` 是 `full`，`read_only` 打开时就是只读。根据它决定能碰多大的范围，例如完全放开的会话可以发送任何文件。外人的会话另外被关着（`owner` 是假），你自己按照对外人的规矩处理 |
| `persona` | 会话这一轮使用的人格编号，没有人格时不写。根据它挑选声音、语气 |
| `voice_max_ms` | 只给场所会话，并且只在那个平台的语音有长度上限时给出：一段语音最多多少毫秒（桥创建会话、找回会话时报告的最新值，核心原样转交）。合成的语音比它长时不交付，返回出错，让 AI 重新说 |
| `checked` | 只给声明了 `paths` 的工具：权限检查判定过的真实位置，顺序和参数里路径的先后相同（数组的每一项各一条）。写文件只写这几个位置：你自己把参数里的路径换成真实位置以后，不在这个列表里的路径不要碰（检查以后，路径上的链接可能被替换过） |
| `channel` | 只给声明了 `runs_commands`、并且这次调用打开了通道的工具：`{"sock", "token", "bin"}`，见下面「运行命令」 |

**回应** `{"result": {"blocks": [...], "error"?, "end_turn"?, "sent"?, "detail"?, "data"?, "state"?, "effects"?}}`：

| 名字 | 说明 |
|---|---|
| `blocks` | 给模型看的内容块：`{"type": "text", "text"}`、`{"type": "image", "blob", …}`。图片要先用 `blob.put` 上传 |
| `error` | 表示这次调用算作出错 |
| `end_turn` | 这一步的结果全部到达以后，不再请求模型，这一轮结束 |
| `sent` | AI「发出去」的东西，不给模型看；头（连接核心的界面程序，例如终端界面、网页界面）把它画在正文里，聊天平台按照它发送。图片最多 4 张：`{"blob", "media_type", "width", "height", "name"?, "alt"?, "sticker"?}`；语音最多一段：`{"blob", "media_type", "voice": true, "duration_ms"?, "name"?}`，`media_type` 以 `audio/` 开头（`audio/mpeg`、`audio/wav`、`audio/ogg`），blob 最大 10 MiB。数量超过上限、写法不对时，整份结果算作出错，原话交给 AI；语音太大时不交付，并且回复 AI 一句 `The audio was not delivered: it is larger than 10 MB.`（这里的 10 MB 就是上面的 10 MiB）。头根据这一轮是谁开启的来决定放不放语音（见 [视图流](view.md) 的 `voice`）；在本机会话里，没有人开口的一轮（到点的定时任务、循环）产生了语音，而此时又没有头打开这个会话时，核心通过总线调用 `tts.play {"audio": <blob>}`，从这台电脑的音箱播放；场所会话的语音由桥发送成语音消息 |
| `detail` | 给头看的结构化结果，按照下面三种通用形状中的一种书写；不给模型看。形状不对、或者 `error` 是真时，核心丢掉这个字段，其余的结果照常处理 |
| `data` | 给脚本的结构化结果，可以是任意 JSON：AI 在命令里用 `miyu call` 调用到这件工具时（`clicall`），它会输出成一行 JSON 交给脚本；它不给模型看，也不记进 `tool.result`。`error` 是真时不交付 |
| `state` | 你这个包在这个会话里的一份状态，写法见下面「`session.set_state`、`session.state`」：核心按会话记录最新的一份，撤回、清空上下文时状态会跟着变化。写坏了时整份丢掉，其余的结果照常处理；`null` 当作没写，要清除就写 `{}`；`error` 是真时不接受 |
| `effects` | 这次调用读取、修改、删除了哪些文件，以及换上的待办，按照先后顺序排列：写法见下面「效果」。核心检查通过以后才接受，接受以后的效果和内置工具报告的一样：时间线上显示增删的行数、可以查看差异，AI 读过的文件被你修改以后不需要重新读取 |

**效果**（`effects` 的每一项，用 `kind` 区分）：

| `kind` | 其余几格 | 核心怎么查 |
|---|---|---|
| `file.changed` | `path`；可以带上 `before`、`after`（内容哈希 `sha256:…`）、`before_text`、`after_text` | `path` 必须是 `checked` 里判定为写入的路径。修改前的内容是核心在发送 `tool.call` 之前读取的，修改后的内容是回应到达时核心当场读取的；前后相同的、带上的哈希或原文对不上的、超过 20 MiB 的效果不接受 |
| `file.read` | `path`；`lines`：`[从, 到]`，从 1 开始计数，「从」不大于「到」（写错时整条丢掉） | `path` 必须在 `checked` 里（读取、写入都可以）；哈希由核心按照文件现在的全部内容计算 |
| `file.trashed` | `path`、`trash`（在回收站里的位置，只给人看，最多 4096 个字符） | `path` 必须是 `checked` 里判定为写入的路径，并且现在已经不存在 |
| `todo.written` | `todos: [{"content", "status"}]`、`done`?（和内置的 `todowrite` 相同；写 `null` 和不写相同） | 只检查写法；`error` 是真时不接受 |

- 相对的 `path` 按照这次调用的 `cwd` 计算，以 `~` 开头的按照家目录计算。你只需要报告「是哪个文件」：内容由核心自己读取，所以你报告的内容骗不了查看差异的人。
- 如果一条效果是其他种类、写法不对、路径没有经过判定，或者同一种效果对同一个文件报告了第二遍，就只丢掉这一条，其余的结果照常处理，不算出错；运行日志会记录一行 `tool effect dropped`（`why` 是 `not_checked`、`unchanged`、`mismatch`、`too_big`、`kind`、`shape` 之一）。
- `error` 是真时，文件效果仍然接受：因为修改到一半出错时，已经修改的部分是真实的。
- 撤回不会把文件改回去（和内置工具一样），只退回 AI 看到过的内容。

**运行命令**（声明了 `runs_commands` 的工具）：会话里有 AI 能在脚本里调用的工具时，这次调用会打开一个只给它用的端点，`tool.call` 带上 `channel`：

| 名字 | 说明 |
|---|---|
| `sock` | 端点：在 Unix 上是套接字的路径，在 Windows 上是命名管道的名字（`\\.\pipe\miyu-call-…`）。把它设置成环境变量 `MIYU_CALL_SOCK` |
| `token` | 令牌，由 32 个十六进制字符组成。把它设置成环境变量 `MIYU_CALL_TOKEN` |
| `bin` | 一个只放着 `miyu` 的目录，把它放到 `PATH` 的最前面：这样命令里调用到的 `miyu` 和正在运行的核心是同一个版本。目录创建失败时不写 |

- 把这三样设置进你为这次调用启动的命令。命令里的 `miyu call <工具>` 按照会话的权限、预设调用 AI 的工具，用法、退出码和在内置 `shell` 里运行的一样（退出码 1 表示工具出错或者被拒绝，2 表示用法不对或者认不出你，3 表示这次调用已经结束，4 表示需要人确认）。
- 端点只接受你这个扩展进程的子孙进程：核心沿着连接上来的进程的父进程一层层往上找，必须能找到你的进程；你自己的进程、用 `setsid` 或者两次 `fork` 脱离出去的进程、拿着令牌的其他进程都连接不上（退出码 2）。这是为了防止误用（令牌泄露给了其他进程、命令脱离出去以后还想调用），不是为了拦住你的扩展：你启动一个子进程就能调用。真正的关口是包的能力 `tools.commands` 需要人批准。
- 这次调用已经回答、超时或者被中止时，端点就会关闭，还在等待的、之后才连接的 `miyu call` 以退出码 3 结束：所以要在命令里调用工具，就要等命令运行完再回应。
- 人在会话里输入的命令（`!`）不交给你，只交给内置的 `shell`。

`detail` 的三种形状（整份写成 JSON 后最多 64 KiB）：

| `kind` | 其余几格 | 上限 |
|---|---|---|
| `links` | `links: [{"title", "url", "snippet"?, "site"?, "date"?}]`，`url` 只接受 `http`、`https` | 50 条 |
| `table` | `columns: ["列名", …]`，`rows: [[格, …], …]`；每一行的格数和列数相同，格是字符串、数字、布尔值或者 `null` | 20 列、200 行 |
| `images` | `images: [{"blob", "media_type", "caption"?}]`，`media_type` 以 `image/` 开头；图片要先用 `blob.put` 上传 | 20 张 |

多一个字段、少一个字段都不接受。认得这种形状的头会把它画成卡片，结果记进 `tool.result` 的 `detail`（见 [事件](events.md)）。

返回了 JSON-RPC 错误、或者写法不对的回答，算作这次调用出错。到了 `timeout_ms` 还没有回答时，这次调用返回「没在 N 秒内答完」。

**`tool.cancel`**（核心 → 扩展，通知）：`{"session", "call_id"}`。核心在超时、被打断、自己要关闭时发送：收到以后放弃那次调用即可。

谁能让工具做什么、工具能对谁动手，由提供工具的一方自己负责：根据 `by`、`owner` 拦截，拦下时在结果里回复一句，并且把 `error` 写成真。

## `session.set_state`、`session.state`

包在一个会话里的一份状态，包括你画在界面块上的内容、压缩时给模型留下的内容，以及你自己的数据。这需要 `sessions.drive` 能力；你只能读写你这个包、在你所代表的那个人的会话里的状态。

```json
{"widgets": {"todos": {"content": [<界面块的内容>], "title"?: <标题>}}, "note": "Todo list:\n- [ ] ...", "data": {...}}
```

| 格 | 说明 |
|---|---|
| `widgets` | 从界面块的编号到 `{"content", "title"?}`：界面块必须是你在清单里写了 `state = true` 的（只有 `scope = "session"` 的块可以这样写），内容的写法和 `widget.set` 相同，按照那一块的位置检查。头按照它绘制，你的程序没有在运行时也一样 |
| `note` | 压缩时原样写进检查点、给模型看的一段英文（最多 2000 个字符，CJK 字符不超过一半，除了换行以外没有控制字符）。平常的回合不发给模型 |
| `data` | 你自己的数据，可以是任意 JSON，写出来最多 16 KiB：核心只负责保存，`session.state` 会原样还给你 |

- 三个字段都可以不写；写 `{}` 或者 `null` 表示清除。写坏了时整份不接受：`widgets` 里的内容不对时返回 `bad_widget_content`（`path` 从 `widgets.<块>` 开始写），其他问题返回 `bad_params`，`field` 是出错的那个字段（例如 `state.widgets.other`：这个块不是你声明成状态块的）。
- **`session.set_state {"session", "state"}`**：换上一份新的状态，回合进行中、空闲时都接受，不会开启回合，写入磁盘以后返回 `{}`。回合进行中写入的状态属于那一轮：撤回那一轮时，状态退回更早的那一份。
- **`session.state {"session"}`**：返回 `{"state": {...} | null}`，是你这个包当前的那一份状态（包含 `note`、`data`）。
- 清空上下文（`/clear`）会清除所有包的状态；平常的压缩不会改动它们。

## `package.methods`、`method.call`

**`package.methods`**（扩展 → 核心）：`{"methods": [{"name", "timeout_ms"?}…]}`，登记它的后台页、界面块要调用的方法，替换上一次登记的方法。回应是 `{"methods": <个数>}`。其他连接调用时返回 `not_an_extension`。

**`method.call`**（核心 → 扩展）：`{"method", "params", "from"?, "language"?}`：人的头通过 `package.call`、`bus.call`，或者其他包通过 `bus.call` 调用到它；`from` 是发起调用的包的编号（人调用时没有）；`language` 是发起调用的那个连接此刻的界面语言（`zh`、`en`、`ja`）：头调用时是头的语言，包调用时是那个包的连接的语言；配置 `ui.language` 是 `auto` 时，按照那个连接握手时报告的 `locale` 计算。给人看的文字根据它挑选语言。核心自己发起的调用（例如替没有人打开的会话播放语音）不带这个字段。你回应什么，调用的一方就收到什么；你返回 JSON-RPC 错误时，对方收到 `method_failed`。回应同样是一行最长 1 MiB：太长的回应核心不读取，会返回给你一条 `line_too_long`（`id` 是那条 `method.call` 的），对方当场收到 `method_failed`，`data.code` 是 `line_too_long`；大的内容要存成 blob（`blob.put`），回应里只写哈希。

想让其他扩展调用到你的方法时，在清单的 `[export]` 里声明总线上的名字（`provides` 或 `plugs`），方法按照 `<名字>.<方法>` 列进 `methods`（见 [总线](bus.md)）。其他扩展只能按照名字调用，按照包的编号调用不到你（返回 `own_package_only`）。

## `package.emit`

扩展 → 核心：`{"event", "data"?}`，推送给订阅了 `subscribe {"stream": "package", "package": <它的编号>}` 的连接，对方收到 `package.event {"package", "event", "data"}`。页面根据它更新，不需要轮询。回应是 `{}`；没有人订阅时也是 `{}`。`event` 的写法和方法名相同（以小写字母开头，只有小写字母、数字、`_`、`.`、`-`，最多 64 个字符），`data` 写成 JSON 最多 256 KiB，不符合时返回 `bad_params`；其他连接调用时返回 `not_an_extension`。这个推送不保存到磁盘，也不补发。

## `search.query`

人在「联网服务」里选择了这个包的清单 `[[search]]` 声明的来源以后，AI 搜索网页时，核心调用这个方法（核心 → 扩展，带 `id`，需要回应）：

`{"source", "query", "want", "allowed_domains", "blocked_domains"}`：依次是来源（`<包>/<编号>`）、要搜索的内容、最多要几条结果、只要这几个域名的结果、不要这几个域名的结果（核心拿到结果以后，还会按照这两项再筛选一遍）。

**回应** `{"hits": [{"title", "url", "snippet"?, "date"?}]}`：没有标题或者网址的条目不接受，摘录会被截短，多出来的条目不要，和出厂的几家服务一样。被限额时返回 JSON-RPC 错误，`data.reason` 写 `limited`：核心按照台阶让它冷却（从 30 分钟起），冷却期间不再询问它；遇到其他错误、或者 20 秒内没有回答时，这一次搜索由出厂的几家服务兜底。扩展要处于打开状态、并且登记过连接（`provide`，工具列表可以是空的），才会被询问。

## `command.call`

人输入了这个包的清单 `[[slash]]` 声明的斜杠命令时，核心调用这个方法（核心 → 扩展）：

`{"command", "argument", "text", "session", "by", "owner", "language"}`：依次是命令名、命令名后面跟的文字、人输入的整行、会话、是谁输入的、是不是主人、连接的语言。

**回应** `{"said": "<回执，可以多行>"}`，核心记录一条 `command.ran`，并且把回执原样交给输入命令的人。一分钟内没有回答、回答了错误、回答里没有 `said` 时，人那边会收到 `command_unavailable` 或者 `command_failed`。

## `user.append`

回合开始时，核心询问扩展（核心 → 扩展）。核心只询问声明了 `context.inject`、正在运行、并且这个会话的工具面上有它的工具的扩展；几个扩展并行询问，**限时 300 毫秒**。

`{"session", "venue", "text"?}`：`venue` 是 `local`、`private`、`group` 之一；`text` 是触发这一轮的人说的话（不是由人开启的一轮不带这个字段）。

**回应** `{"content": [{"kind", "text"}…]}`，最多 3 块：`kind` 由小写字母、数字、`-`、`_` 组成；`text` 的长度是 1 到 4000 个字符。返回的内容记进会话，排在触发这一轮的那句话后面，以后原样回放。空的、超时的、写法不对的回答，都当作这一轮没有返回内容。

## `tool.guard`、`guard.cancel`

AI 的一次工具调用执行之前，核心询问你：放行、拒绝，还是询问人（核心 → 扩展，带 `id`，需要回应）。它适合公司策略（某些目录不许写入、某些命令要询问人）、审计这类用法。

**谁被询问**：清单的 `[process] capabilities` 声明了 `tool.guard`、写了 `[guard]`、经过主人批准、并且处于打开状态的扩展。`[guard]` 写的是一张静态的表，核心启动时就按照它建好查找表：

```toml
[guard]
tools = ["shell", "write", "edit"]   # 工具名，可以不写
access = ["execute", "write"]        # 访问类别，可以不写
timeout_ms = 1000                    # 等你多久，可以不写
```

- 只有一次调用的工具名在 `tools` 里，或者它的访问类别在 `access` 里，核心才会询问你；两项都没有写的清单无法读取（`bad_guard`，`miyu check` 会报告），不允许「什么都问」。访问类别有 `read`、`write`、`execute`、`network`、`outbound`、`venue`、`install`。
- 自带的工具、其他包的工具、MCP 服务的工具，都按照名字和访问类别匹配；你自己 `provide` 的工具不会询问你。不使用 `provide` 也能当守卫。
- `timeout_ms` 不写时是 1000，只能写 100 到 5000。
- 权限策略先判定：它拒绝的调用不会询问你；它放行或者要询问人的调用，再询问你。几个守卫并行询问，最严格的结果生效：有一个拒绝就拒绝；没有拒绝、但是有一个要询问人时，就询问人；策略要询问人时，即使你回答放行，也仍然询问人。你只能收紧，不能放宽，也不会改变沙盒。
- 脚本里调用的工具（`miyu call`）同样会询问你，`call_id` 是嵌套调用的编号（例如 `call_12_1#3`）。

**参数**：`{"session", "call_id", "tool", "args", "access", "cwd", "owner", "venue"}`：依次是会话、这次调用的编号、工具名、修正过的参数（执行时用的就是这一份）、访问类别、这一轮的工作目录（和 `tool.call` 的 `cwd` 相同，是真实路径）、开口的人是不是主人（和 `tool.call` 的 `owner` 相同）、在哪里说的（`local`、`private`、`group`）。

**回应** `{"verdict": "allow" | "deny" | "ask", "reason"?}`：

- `allow`：放行。这个回答不进入上下文，AI 只看到工具的结果。
- `deny`：拒绝，工具不会运行。AI 看到的结果是 `Blocked by <你的包编号>: <reason>`：`reason` 会去掉首尾的空白、截到 300 个字符，换行、引号、尖括号会转义成 `\n`、`\u0022` 这样的写法，整句只占一行；没有写 `reason` 时是 `no reason given`。时间线上这个结果的 `by` 是你的包，给人看的是你写的原文。
- `ask`：要询问人，走平常的确认流程（`tool.approval_requested`，`by` 是你的包，`detail` 是 `{"guard", "tool", "reason"?}`），人只能允许这一次或者拒绝；没有人能确认的会话按拒绝处理。
- 多了字段、`verdict` 无法识别、`reason` 不是字符串时，都按拒绝计算，理由是 `the guard answered in a wrong format`。

**没有回答就算拒绝**（守卫失效时往严格的方向处理）：

| 什么时候 | AI 看到的理由 |
|---|---|
| 过了 `timeout_ms` 还没有回答 | `the guard did not answer in time` |
| 你处于打开状态却没有在运行（正在启动、刚刚崩溃、无法启动），或者询问的过程中连接断开了 | `the guard is not running` |
| 返回了 JSON-RPC 错误 | `the guard returned an error` |

主人关掉的扩展不会被询问。p95 最好在 20 毫秒以内，因为人一直在等着这一步。

**`guard.cancel`**（核心 → 扩展，通知）：`{"session", "call_id"}`。人打断了、回合被取消了、或者你超时了，核心不再等待：这次调用按照打断收尾（或者已经按照超时拒绝了），你的回答即使到了也会被忽略。

## `tool.after`

AI 的一次工具调用有了结果、并且记进会话日志以后，核心通知你（核心 → 扩展，通知：不带 `id`，不需要回应）。它适合审计（把每次调用记到外部的日志里）、调用统计这类用法。它是只读的：你收到的就是写入磁盘的那一份，你改不了结果，它也不会进入 AI 的上下文。

**谁收到通知**：清单的 `[process] capabilities` 声明了 `events.read`、写了 `[after]`、经过主人批准、处于打开状态、并且正在运行（已经握过手）的扩展。它不另外增加能力：能查看会话事件流的扩展本来就看得到这些内容，这只是让你不必为了查看工具调用去订阅整条事件流。`[after]` 的写法和 `[guard]` 相同，只是没有时限：

```toml
[after]
tools = ["shell", "write"]   # 工具名，可以不写
access = ["network"]         # 访问类别，可以不写；两样至少一样
```

- 只有一次调用的工具名在 `tools` 里，或者它的访问类别在 `access` 里，核心才会通知你；两项都没有写、访问类别无法识别、工具名写法不对、没有声明 `events.read` 的清单都无法读取（`bad_after`，`miyu check` 会报告）。目录里没有的工具（AI 写错了名字）没有访问类别，只按照 `tools` 匹配。
- 你自己 `provide` 的工具也会通知你。被拒绝的（被权限策略、守卫、内核拒绝）、被跳过的、被取消的、出错的调用都会通知，`status` 如实填写。
- **会话遵守订阅事件流的规则**：你接触不到的会话不会通知你。不是平台桥的扩展只收到本机会话、以及主人本人没有被关起来的私聊的通知；平台桥只收到自己平台（清单的 `[connection] platform`）上的场所会话的通知。
- 脚本里调用的工具（`miyu call`）也会通知，但是形状不同，见下面。

**参数**：

| 格 | 直接调的（`tool.result`） | 脚本里调的（`tool.nested`） |
|---|---|---|
| `session` | 会话编号 | 同左 |
| `call_id` | 调用编号（`call_12_1`） | 嵌套调用的编号（`call_12_1#3`） |
| `tool` | 工具名 | 同左 |
| `access` | 访问类别；目录里没有这件工具时不写 | 同左 |
| `status` | 和 `tool.result` 相同：`ok`、`error`、`denied`、`cancelled`、`skipped`…… | 和 `tool.nested` 相同：`ok`、`error`、`denied`、`needs_approval`、`cancelled` |
| `duration_ms` | 执行用了多少毫秒；没有真正执行过时不写 | 同左 |
| `by` | 那条事件的 `by`，和事件流相同：正常运行完时是那次调用本身，被拒绝时是拒绝它的一方（`{"kind": "module", "id": "<包>"}`） | 同左 |
| `args` | 模型写的参数原文（一段 JSON 字符串，和事件流里 `tool_call` 块的 `args` 相同） | 修正过的参数原文（和 `tool.nested` 的 `args` 相同） |
| `blocks` | 和 `tool.result` 的 `blocks` 相同：给 AI 看的内容，大的块只给出引用 | 不写 |
| `result`、`bytes` | 不写 | 交给脚本的那份结果存成的 blob 的哈希和字节数（和 `tool.nested` 相同），没有存下来时不写；要查看内容就用 `blob.get` |

`args` 是一段字符串，不是对象（`tool.guard` 的 `args` 是解析过的对象）：事件流里怎样记录，这里就怎样给出。要分辨两种形状，可以看有没有 `blocks`，或者 `call_id` 里有没有 `#`。

**不等待、不拦截**：

- 核心发出通知以后就继续往下走，回合不等你；你慢了、崩溃了，都不会拖住会话。
- 每个扩展有一条队列，**最多 256 条**；队列满了就丢掉新来的通知，并且记录一行 `WARN`（一阵连续的丢弃只记录第一条，带上你累计被丢掉了几条；队列有了空位以后再记录一行）。
- 连接断开、或者你崩溃了，队列会整个丢掉，不会补发；重启以后从新的调用开始通知。处于打开状态但是还没有握手的扩展不会收到通知。
- **`tool.after` 不保证送达**：要做完整的审计，就订阅事件流（`subscribe` 的 `events`，需要 `events.read`），事件流会按照日志补发。

## `driver.call`、`driver.delta`、`driver.cancel`

你提供的模型驱动接收一整次模型调用（核心 → 扩展，带 `id`，需要回应）：Miyu 交给你统一的请求，你把它编码成线上的格式、登录、发送出去、解析响应、给出错分类，再以增量的形式返回。重试、冷却、在池里换模型、计价都由 Miyu 负责。

**谁被调用**：清单的 `[process] capabilities` 声明了 `model.driver`、写了 `[[driver]]`、经过主人批准、并且处于打开状态的扩展；只有主人在供应商的配置里写了 `driver = "<你的包编号>/<驱动名>"` 的那一家供应商，才会使用它（见 [清单](../dev/manifest.md) 的 `[[driver]]`）。

**参数** `{"call", "driver", "request", "fingerprint", "params", "key"?, "base_url"?, "blobs"?}`：

| 格 | 说明 |
|---|---|
| `call` | 这一次调用的编号，`driver.delta`、`driver.cancel` 按照它对应到这次调用 |
| `driver` | `<包编号>/<驱动名>`，如果你声明了几个驱动，就按照它区分 |
| `request` | 统一的请求，原样给出：`tools`、`system`、`messages`、`stable`，有内容时才写 `continuation`、`described`（无法看图时对图片的转述）、`output_cap`、`tail`。同样的状态给出的请求一字不差；其他驱动的私有数据已经去掉。`hosted_tool` 块（模型在供应商那边自己运行了的工具，例如搜索）如果带着你这个驱动的私有数据，就原样交给你，由你决定要不要带回给供应商；属于其他驱动的、没有带私有数据的块，会换成一块 `text`，内容是 Miyu 写给模型看的那一段文字（用 `<hosted-tool name="…">` 包着参数和结果：参数和结果原样保留，其中的 `</` 写成 `<\/`；末尾不带换行）。判断一块是不是「你自己的」，只看块上的私有数据 |
| `fingerprint` | `{"tools", "system", "messages": [...]}`：工具面、system、每条消息各有一个哈希（16 位十六进制的字符串），同样的内容得到同样的哈希，可以拿来做你自己的缓存 |
| `params` | `{"model", "max_output"?, "effort"?, "temperature"?}` |
| `key` | 这一家供应商的配置里引用的那一把 key；你声明了 `key = "none"`、或者这一家没有写 key 时，不交给你 |
| `base_url` | 这一家供应商写的地址；没有写时不交给你 |
| `blobs` | 只给声明了 `images = true` 的驱动：请求里的图片，`{"<blob 哈希>": {"media_type", "data"}}`，`data` 是 base64；没有声明时不交给你，请求里按照 `described` 使用转述 |

**增量** `driver.delta`（扩展 → 核心，通知）：`{"call", "deltas": [...]}`，一份通知里可以有几条增量，按照先后顺序排列：

| 一条 | 意思 |
|---|---|
| `{"start": {"index", "kind"}}` | 第 `index` 块开始，`kind` 是 `"text"`、`"reasoning"`、`{"tool_call": {"name"}}` 或者 `{"hosted_tool": {"name"}}`（供应商在它那边自己运行了的一件工具，`name` 是供应商自己的工具名原样；Miyu 不执行它）；块从 0 开始编号，一块接一块地开始 |
| `{"text": {"index", "text"}}` | 这一块的一段文字：正文、思考，或者工具调用、供应商侧工具的参数原文的一段 |
| `{"private": {"index", "data"}}` | 思考、工具调用、供应商侧工具这几种块的私有数据（签名、供应商自己的调用编号、步骤编号），可以是任意 JSON，一块最多一份；以后的请求里会原样还给你，不会交给其他驱动 |
| `{"outcome": {"index", "text", "failed"?}}` | 供应商侧工具的结果：模型和人都看得懂的文字（标题、网址、摘录、读到的正文），核心截到 64 KiB；`failed` 是 `true` 表示供应商报告运行失败，不写时当作 `false`。只给供应商侧工具的块，一块最多一次，并且要在这一块收全以前；被打断、没有结果时不交 |
| `{"end": {"index"}}` | 这一块已经收全了 |
| `"truncated"` | 写到了输出上限，回复没有写完 |

只有在一块开始以后才能有文字，一块收全以后不能再写；增量不对时，这一次调用按照 `unclassified` 结束（`the driver sent a bad delta: …`），之后到达的增量都会丢掉。`call` 无法识别的增量直接丢掉，不报错。

供应商侧工具的名字要写进清单里这个驱动的 `hosted`（[清单](../dev/manifest.md#driver)）：这样主人在安装包时就能根据它知道供应商会自己运行什么。没有写进去的名字仍然接受、照常显示，核心会在运行日志里记录一行警告（同一个驱动的同一个名字，核心每次运行只记录一次；每个驱动最多列出 64 个名字，超出的名字、长度超过 64 个字节的名字，只再记录一行「还有更多」，不带名字）。会修改东西的工具（运行命令、写入文件）不要报告成这一种，要把它关掉，改为使用核心的工具。

**回应** `{"usage"?, "error"?, "sent"?}`，在这一次调用的所有 `driver.delta` 之后发送：

- `usage`：`{"uncached", "cache_read", "cache_write", "output", "reasoning"?}`；没有报告的项记成没有报告，不做估算。
- `error`：`{"class", "message", "retry_after_ms"?}`，`class` 是 `retryable`、`stalled`（卡住了：一直没有动静，内核连续遇到两次就不再重试）、`rate_limited`、`context_too_long`、`auth`、`quota`、`content_policy`、`unclassified` 之一，无法识别的值当作 `unclassified`；`retry_after_ms` 只在 `rate_limited` 时生效；`message` 截到 500 个字符，只给人看。已经发送了一部分增量时，不要自己重试（否则回复会重复），Miyu 会按照分类重试、冷却、在池里换模型。
- `sent`：这一次真正发送出去的字节的 SHA-256，写成 `sha256:<64 位十六进制>`，记进 `model.called`。同样的请求再发送一次（重试）时，如果报告的 `sent` 不一样，就说明编码看了参数以外的东西，缓存前缀无法命中，Miyu 会记录一行 `WARN`（每个驱动每次运行记录一次）。

多了字段、写法不对的回应，当作 `unclassified`（`the driver sent a bad answer: …`）；返回 JSON-RPC 错误也一样（`the driver refused the call: …`）。

**时限**：和内置驱动相同：从发送出去到第一条增量、两条增量之间、从最后一条增量到回应，都不能超过空闲时限（180 秒，按照思考强度放大；清单的 `[[driver]]` 写了更长的 `quiet_secs` 时，按照它计算），整次调用不能超过空闲时限的十倍；超时的调用当作 `retryable`（`the driver went quiet`）。扩展崩溃、连接断开时，正在等待的调用当作 `retryable`（`the driver went away`）。你正在退避、等待重新拉起时，这一次调用当作 `retryable`，等待多久按照 `retry_in` 计算；处于关闭状态、没有批准、已经停止的扩展不会被调用，这一家供应商当作不可用，池里换下一个。

**`driver.cancel`**（核心 → 扩展，通知）：`{"call"}`。人打断了、回合被取消了、超时了、或者增量不对，核心不再等待：不要再为这一次调用发送请求，之后的增量和回应都会被忽略。

## `driver.encode`

编码一次但不发送（核心 → 扩展，带 `id`，需要回应）：参数和 `driver.call` 相同，回应是 `{"body"}`，`body` 是你按照这份参数会发送出去的字节（一段字符串）。`driver.check`（作者使用 `miyu dev driver-check` 运行它）按照它检查你的编码是否稳定：同样的参数必须编码出同样的字节，重启以后也一样；只在末尾增加一条消息时，稳定区结束之前的字节不能改变。

- 编码只能依据参数：不放入时间、随机数，字段的先后顺序固定；`call` 是这一次调用的编号，不要把它编码进去。
- 声明了 `cache = "contract"` 的驱动必须实现这个方法，没有实现时 `driver.check` 算作没有通过；其他驱动没有实现时报告为「跳过」。没有实现这个方法时，返回 JSON-RPC 的「没有这个方法」（`-32601`）。
- 主人的 key 不交给你（`key`、`base_url` 都不写），图片也不交给你。

## `driver.models`

列出这个驱动能用的模型（核心 → 扩展，带 `id`，需要回应）。这个方法是可选的：主人拉取模型列表时（`model.list` 带上 `refresh`、打开配置页、模型请求返回 404 以后），核心调用它。不实现时返回 JSON-RPC 的「没有这个方法」（`-32601`），主人就要手写模型名。

**参数** `{"driver", "key"?, "base_url"?}`：`key`、`base_url` 和 `driver.call` 相同（按照你的 `key` 声明交给你，没有写时不交）。

**回应** `{"models": [...]}`，一个模型一项：

| 格 | 说明 |
|---|---|
| `id` | 模型名，主人在配置里写成 `<供应商>/<id>` |
| `window` | 上下文窗口，单位是 token |
| `max_output` | 最大输出，单位是 token |
| `tools` | 能不能调用工具 |
| `vision` | 能不能看图：`true` 按照「输入有文字和图」计算 |
| `reasoning` | 核心能识别这个字段，但是暂时不使用 |

每一项都按照「手写的 > 你列的 > 目录」的顺序查找，所以主人手写的会压过你列的。整个调用最多等待 30 秒；多了字段、写法不对时，整份列表都不要，继续使用上一份列表。

## `system.append`

往系统提示词的末尾放一段内容（扩展 → 核心）。需要声明 `context.system`。

**参数**：`content`（1 到 4000 个字符，写 `null` 表示去掉）、`session`（只作用于这一个会话；不写时作用于全部会话）。**回应**：`{}`。

这段内容只进入工具面上有这个包的工具的会话；修改以后，下一轮换上新的内容。它只记在核心的内存里：核心重启以后要再设置一次。

**出错**：`not_a_provider`；`needs_approval`（`data.capabilities` 是 `["context.system"]`）；`bad_params`。

## `widget.set`、`widget.clear`、`widget.watch`、`widget.unwatch`

界面块是清单的 `[[widget]]` 声明的、在头上显示的一块内容，位置可以是首页（`home`）、侧边栏（`sidebar`，可以跟随会话）、底栏（`status`）、整页（`page`，通过斜杠命令打开）。需要声明 `widgets`。

**扩展 → 核心**

| 方法 | 参数 | 回应 |
|---|---|---|
| `widget.set` | `widget`、`session`（跟随会话的块必须写）、`title`（替换清单里的标题）、`content` | `{"kept": true}`；跟随会话的块在这个会话里没有人在看时，返回 `{"kept": false}` |
| `widget.clear` | `widget`、`session` | `{}` |

**核心 → 扩展**（通知）：`widget.watch {"widget", "session"?}` 表示这一块开始有人看了，此时你去获取数据、推送内容；`widget.unwatch` 表示最后一个人也不看了。握手以后，核心会为这一刻有人在看的每一块各发送一条 `widget.watch`。

**内容**是一个数组，每一项是一段：

| 段 | 格 |
|---|---|
| `heading` | `text` |
| `text` | `text`（可以换行，最多 2000 个字符）、`tone` |
| `pairs` | `items`：`{"label", "value", "tone"?}`，最多 50 对 |
| `progress` | `value`（0 到 1）、`text`、`mark`（0 到 1，在进度条上标出的那一处） |
| `list` | `items`：`{"title", "detail"?, "right"?, "state"?, "action"?}`，最多 200 条 |
| `buttons` | `items`：`{"label", "action"}`，最多 8 个 |
| `sections` | 只用于整页：`items`：`{"id", "title", "subtitle"?, "badge"?, "state"?, "blocks"}`，`add`：`{"label", "call", "options"?}` |
| `form` | 只用于整页：`id`、`fields`（`{"key", "label", "hint"?, "control", "value", …}`）、`submit`（`{"label", "call", "params"?}`） |

- 文字是字符串，或者语言到文字的表（`{"zh": "…", "en": "…"}`），核心按照连接的语言挑选。不支持 Markdown、HTML。
- `tone` 的取值：`normal`、`dim`、`accent`、`good`、`warn`、`bad`。列表项的 `state`：`todo`、`doing`、`done`、`failed`、`off`；分区的 `state`：`good`、`warn`、`bad`、`off`。
- `action`：`{"call": <方法>, "params"?}` 调用它自己登记的方法；`{"open": <块>}` 打开它的一个整页；`{"copy": <字>}` 把文字复制到剪贴板。
- 表单的控件有：`toggle`、`select`（带 `options: [{"value", "label"}]`）、`text`、`textarea`、`list`、`number`（`min`、`max`、`integer`）；`value` 是 `null` 表示使用默认值；`default` 是 `{"text", "value"?}`，分别是给人看的那一行文字，和默认值到底是什么（头按照它绘制值为 `null` 的那一格）。保存时，头调用 `submit.call`，参数是 `submit.params` 加上 `{"values": {只放改过的格}}`；扩展返回 `{"saved": true}`，或者 `{"saved": false, "problems": [{"key", "message"}]}`，或者 `{"saved": false, "message"}`。`add` 调用的参数是 `{"value": <选的那一项>}`，扩展可以返回 `{"id": <新的那一项>}`。
- 整块最多 64 段、64 KiB；`status` 只能是一段只有一行的 `text`，宽度最多 40 列；`home` 最多 8 段。同一块最快每 250 毫秒推送一次给头，这段时间里只保留最新的内容。

**出错**：`unknown_widget`（`data.widget`）；`bad_widget_content`：`data.path` 指出错在哪里（例如 `content[4].items[2].title`），`data.problem` 是 `type`、`missing`、`too_long`、`too_many`、`too_big`、`control_char`、`tone`、`state`、`value`、`action`、`place`、`unknown`、`duplicate` 之一；`bad_params`（跟随会话的块没有写 `session`，或者全局的块写了 `session`）。

## `extension.config`

它自己的配置项（以「包的编号加点」开头的配置项）发生变化时，核心通知它（核心 → 扩展，通知）：`{"keys": {<键>: <新值> 或 null}}`，只放发生了变化的键；密钥给出的是真实的值；变成没有设置、或者取不到值时是 `null`。这个通知不需要订阅。批准了 `provider.key` 的扩展，`provider_key` 这个字段发生变化时，也按照同样的方式推送。

### `provider_key`

清单写了 `[provider_key] catalog = "<目录里的编号>"`、声明了 `provider.key`、并且经过人批准的扩展，在握手的 `config` 和 `extension.config` 的 `keys` 里会多一个字段 `"provider_key": "<key 的真值>"`（一个字符串）。只有核心拉起的扩展连接上才有这个字段，头的连接上没有。

- **哪一家**：按照主人的供应商配置 `providers.<id>` 识别（`miyu_models::provider_key`）：如果写了 `catalog`，就按照它识别；如果没有写，字面地址里包含那一家的官方地址时算作这一家（目前只识别 DeepSeek 的 `api.deepseek.com`，地址是环境变量引用时不算）；如果两样都没有写，编号就是那个目录编号时算作这一家。有几家都符合时，取编号排在第一的那一家；没有写 key 的不算。
- **不交**：没有批准 `provider.key`（出厂的包也要人批准）、主人没有配置那一家、那一家的 key 取不到时，都没有这个字段。其他供应商的 key 一律不交。
- **变化**：换了 key（`secret.set`、修改了引用）时推送新的值；删除了那一家、去掉了 key 时推送 `null`。
- 这个值不写进运行日志，也不写进事件。`config.get` 仍然只返回引用 `{"secret": "<名字>"}`。

## `content.list`、`content.read`、`content.changed`

其他包可以在清单里写 `[content.<它的编号>] dir = "…"`，给它带来一个目录的内容（表情包的内容包就是这样做的）。

| 方法 | 参数 | 回应 |
|---|---|---|
| `content.list` | 没有 | `{"packages": [{"package", "version"?}…]}`：给它带来了内容的包，按照编号排序 |
| `content.read` | `package`、`path`、`offset` | `{"data", "size", "eof"}`，一次最多读取 512 KiB |
| `content.changed`（核心 → 扩展，通知） | `{}` | 给它带来内容的包被安装、卸载或者升级了：需要重新列一遍 |

**出错**：`not_a_provider`；`unknown_package`（没有这个包，或者这个包没有给它带来内容）；`bad_params`；`not_found`。

## `events.append`

往会话里记录一条不带回合编号的事件：它不开启回合、不打断回合，也不给模型看。

**参数**：`session`、`kind`、`body`（JSON 对象，最多 16 KiB）。扩展只能记录 `ext.<自己的前缀>.<名字>` 形式的事件（前缀是清单的 `prefix`，没有写时是包的编号；每一段以小写字母开头，只有小写字母、数字、`_`、`-`，整个最多 128 个字符）。通讯平台的桥另外还能记录 `venue.recalled`、`venue.delivered`（见 [场所](venues.md)）。

**回应**：`{"seq": <序号>}`。

**出错**：`bad_params`；`session_not_found`。
