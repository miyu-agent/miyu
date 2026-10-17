# 软件包与命令：`package.*`、`extension.*`、`mcp.*`、`command.*`、`check`

管理软件包、扩展进程、个人设置里的 MCP 服务，执行斜杠命令，查文件写没写错。扩展自己调的方法、核心发给扩展的调用见 [扩展](extensions.md)。英文版：[packages.en.md](packages.en.md)。

| 方法 | 做什么 |
|---|---|
| [`package.list`](#packagelist) | 列出软件包 |
| [`package.info`、`package.files`、`package.owns`、`package.check`](#packageinfopackagefilespackageownspackagecheck) | 一个包的信息、文件；一个路径归哪个包；装好的文件改没改 |
| [`package.install`、`package.remove`](#packageinstallpackageremove) | 装、卸 |
| [`package.enable`、`package.disable`、`package.restart`](#packageenablepackagedisablepackagerestart) | 开、关、重启 |
| [`package.file`、`package.call`](#packagefilepackagecall) | 读后台页的文件、调包的程序登记的方法 |
| [`extension.status`、`extension.enable`、`extension.disable`、`extension.restart`](#extensionstatusextensionenableextensiondisableextensionrestart) | 扩展进程的状态和开关 |
| [`mcp.list`、`mcp.restart`](#mcplistmcprestart) | 个人设置里的 MCP 服务 |
| [`command.run`](#commandrun) | 执行一条斜杠命令 |
| [`command.catalog`](#commandcatalog) | 列能用的斜杠命令 |
| [`check`](#check) | 查配置、人格、预设、软件包写没写错 |

**谁能调**：`package.install`、`package.remove`、`package.enable`、`package.disable`、`package.restart`、`package.file`、`extension.*`、`mcp.*` 只给人，扩展调回 `local_only`。别的人、扩展都能调；扩展调 `package.call` 另有规矩（见下）。

**两层**：出厂的（随 Miyu 装的）、管理员家目录里装的（`home/admin/packages/`）。家目录里装了同编号的就是升级、换掉原来的；出厂的卸掉是在家目录记一笔，能装回来。

## `package.list`

**参数**：没有。**回应**：`{"packages": [...]}`，照编号排。一项：

| 名字 | 说明 |
|---|---|
| `package`、`layer` | 编号，`shipped` 或 `home` |
| `kind` | `builtin`（编进核心的）、`process`（扩展）、`worker`（按需拉起的小程序）、`ui`（界面）、`mascot`（吉祥物）、`bundle`（只带 MCP 服务、技能、脚本这类，不带程序） |
| `name`、`summary`、`icon` | 名字、一句说明、Lucide 图标名 |
| `version`、`protocol`、`state` | 版本、说的协议版本、状态 |
| `required` | 必需的，卸不了 |
| `features` | 里面的功能（预设照它开关） |
| `status`、`enabled` | 有进程的：在跑没有、开着没有 |
| `page` | 有后台页的 |
| `mcp` | 带的 MCP 服务：`name`、`command`、`venues`、`state`、`failures`…… |
| `widgets` | 声明的界面块：`id`、`place`、`title`、`scope`、`command` |
| `removed` | 卸掉了的出厂包 |

写错的、撞了的、读不了的只有 `package`、`layer`、`code`、`problem`（照连接的语言说）和 `line`。要一直跟着最新的，订阅 `packages` 流。

## `package.info`、`package.files`、`package.owns`、`package.check`

| 方法 | 参数 | 回应 |
|---|---|---|
| `package.info` | `package` | `{"package", "layer", "files", "size", "version"?, "installed"?, "source"?}`：几个文件、多少字节、装的时刻、从哪装的 |
| `package.files` | `package` | `{"dir", "files": [{"path", "sha256", "size"}…]}`：`path` 相对 `dir` |
| `package.owns` | `path`（绝对路径） | `{"package", "layer", "path", "recorded"}`；哪个包都不在的 `{"package": null}` |
| `package.check` | `package`（可不写） | `{"packages": [{"package", "modified", "missing", "extra"}…]}`：家目录里装的包，装好以后改了、少了、多了的文件 |

**出错**：`unknown_package`；`bad_params`（`owns` 的路径是相对的）。

## `package.install`、`package.remove`

| 方法 | 参数 | 回应 |
|---|---|---|
| `package.install` | `path`：本机一个包文件夹的绝对路径（文件夹名就是编号），或者那份 `package.toml` 的路径 | 同 `package.list` 的一项 |
| `package.install` | `package`：把卸掉的出厂包装回来 | 同上 |
| `package.remove` | `package` | `{"package", "removed": true}` |

都可以带 `preview: true`：只看一眼、什么都不动，拒绝照真做的那一样。`install` 的预览回 `package`、`version`、要换掉的 `replaces: {"version"}`、装回出厂的 `restores: true`，和带了什么（`program`、`mcp`、`command`、`page`、`mascot`、`connection`、`system_account`、`settings`、`capabilities`、`files`、`size`）；`remove` 的预览回 `package`、`layer`、`version`、`files`、`size`、`settings`（要一并删掉的配置键）、`state`（有没有状态目录）。

装、卸当场生效：带进程的照开关拉起、停下，它的工具进出工具目录，开着的会话下一轮换上。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 两个都写；路径不是绝对的 |
| `path_unreadable` | 读不了那个文件夹 |
| `package_invalid` | 清单写错了，`data.problem`、`data.line` |
| `package_conflict` | 带的名字和装着的别的包撞了：子命令、功能、工具、技能、MCP 服务、知识库、斜杠命令 |
| `package_exists` | 和出厂的同编号 |
| `package_required` | 卸必需的 |
| `depends_missing` | 依赖的包没装，`data.missing` |
| `package_depended` | 别的装着的包依赖它，`data.by` |
| `unknown_package` | 没装的、没卸过的 |

## `package.enable`、`package.disable`、`package.restart`

**参数**：`package`；`enable` 另可带 `approve`：批准它声明的能力（见 [扩展](extensions.md) 的「能力」）。**回应**：同 `package.list` 的一项。

- 扩展：开是拉起进程，关是请它退出、它登记的工具出目录。
- `bundle`：开是起它带的 MCP 服务，关是停下。
- 出厂的、不是必需的内置包：关是卸掉，开是装回来。
- `restart`：重启扩展的进程和它带的 MCP 服务，连续失败从零数。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `unknown_package` | 没有这个包 |
| `package_required` | 关必需的 |
| `not_switchable` | 界面、小程序、吉祥物包没有开关 |
| `program_missing` | 包的程序没找到（包文件夹的 `bin/`、`miyu` 旁边都没有） |
| `needs_approval` | 要的能力还有没批的，`data.capabilities` |
| `package_depended` | 关一个别的包依赖的 |
| `package_off`、`not_restartable` | `restart` 一个关着的；没有能重启的进程、服务 |

## `package.file`、`package.call`

**`package.file`**：参数 `package`、`path`（后台页目录里的相对路径）、`offset`。回应 `{"data", "size", "eof"}`，一次最多 512 KiB。网页的后台页、吉祥物的模型文件照它读。

**`package.call`**：参数 `package`、`method`、`params`（可不写）。核心把它转给那个包的程序（`method.call`），程序回什么交回什么。在后台答。界面块上的按钮、整页的表单保存都走它。

扩展也能调 `package.call`：要在清单里声明能力 `packages.call`，只调得到对方清单 `[export]` 里列的方法；转过去带 `from`（调的包的编号）。对方回应里出现的、它自己存的 blob，调用方之后也读得到（见 [附件](blob.md#blobget)）。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `no_page`、`not_found` | `package.file`：没有后台页；没有这份文件、跑出目录、不是普通文件 |
| `program_not_running` | 程序没连着 |
| `unregistered` | 程序没登记这个方法，`data.method` |
| `method_failed` | 程序回了错，`data.message`、`data.code` |
| `method_timeout` | 到时限没回 |
| `needs_approval`、`not_exported` | 扩展调的：没声明 `packages.call`；对方没导出这个方法（`data.method`） |

## `extension.status`、`extension.enable`、`extension.disable`、`extension.restart`

**参数**：`status` 没有；别的 `package`，`enable` 另可带 `approve`。**回应**：`status` 是 `{"extensions": [...]}`，别的是那一个。一个：

| 名字 | 说明 |
|---|---|
| `package`、`name` | 编号、名字 |
| `start` | 清单写的什么时候起 |
| `on` | 开关 |
| `state` | `off`、`starting`、`running`、`waiting`（失败了、等着再拉起）、`stopped` |
| `failures` | 连续失败几次 |
| `pid` | 在跑的进程号 |
| `retry_in` | 在等的：多少毫秒以后再拉起 |
| `reason`、`stderr` | 停下的：为什么（`failed_repeatedly`、`config_error`、`needs_approval`……）、标准错误的最后几行 |
| `capabilities`、`unapproved` | 声明的能力 `[{id, name, summary}]`、还没批的 |

要一直跟着最新的，订阅 `extensions` 流。

**出错**：`unknown_package`；`not_an_extension`（是界面包）；`extension_off`（重启一个关着的）；`needs_approval`。

## `mcp.list`、`mcp.restart`

管理员个人设置（`home/admin/settings.toml` 的 `[mcp.<名字>]`）里的 MCP 服务。加、删、改直接改个人设置，当场生效。

**`mcp.list`**：没有参数。回应 `{"servers": [...]}`：`name`、`command`、`venues`、`state`、`failures`，在跑的 `pid`，在等的 `retry_in`，停下的 `reason`、`stderr`，盖住了哪些包里同名服务的 `covers`。

**`mcp.restart`**：参数 `name`。回应是那一项。**出错**：`unknown_server`。

## `command.run`

执行一条斜杠命令：头把人打的原文交过来，核心认、判谁能用、执行。

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `text` | 字符串，必写 | 人打的原文，例如 `/stop` |
| `cwd` | 字符串 | 头所在的目录，只用来接 `/workspace` 后面相对的路径 |
| `as` | 对象 | 场所会话：代表平台上的人，同 `session.send` |

**回应**：`{"command", "events", "said"}`：正名（别名换成正名）、这一次追加的事件、回执那一句（照连接的语言，原样给人看）。打开整页的命令回 `{"command", "opens": {"package", "widget"}, "said": ""}`，头打开那一页。

**核心认的命令**：

| 命令 | 做什么 |
|---|---|
| `/clear`（`/reset`） | 清空上下文和待办，同 `session.clear` |
| `/stop` | 打断这一轮，停掉这个会话派出去的后台命令和子代理 |
| `/workspace <路径>` | 换工作目录；不带路径的说现在在哪 |
| `/todo`、`/todo clear` | 看待办、清空待办 |
| `/remember <话>` | 记一条记忆 |
| `/dream` | 现在就整理记忆（在后台答） |
| `/clear-memory`（`/reset-memory`） | 清掉这个会话记下的记忆 |
| `/goal` 系列 | 看、设、停、继续、改、清掉长期目标 |

不是这些的，依次照这几处找：自己放的命令模板（`home/admin/commands/<名字>.md`）、扩展声明的命令（核心转给扩展，`command.call`）、包带的命令模板。模板展开成一句话，当这个人说的发出去：回应的 `events` 是那一句，`said` 是空的。

**谁能用**：本机的会话都能用；场所会话里是主人和管理的人。系统配置 `commands.<名字>.access` 能改成 `anyone`（谁都行）或 `off`（停用）。`/workspace` 只给主人本人。

同一个 `id` 再发只算一次。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 不以 `/` 开头；`cwd` 不是绝对的；命令后面跟的字不对 |
| `unknown_command` | 没有这个命令 |
| `command_not_allowed` | 这个人不能用，或者停用了、预设没开，`data.why` 是给人看的一句 |
| `owner_only` | 只有主人本人能用 |
| `command_unavailable`、`command_failed` | 扩展的命令：扩展没连上、一分钟没答；扩展答了错（`data.why`） |
| `memory_unavailable`、`goal_unavailable` | 这个会话没有记忆、没有长期目标，`data.why` |
| 命令自己的 | `/clear` 的 `turn_running`、`nothing_to_clear`，`/workspace` 的 `path_unreadable`……，同对应的方法 |

## `command.catalog`

**参数**：`session`（可不写）：写了只列这个连接在这个会话里打了不会被拒的。

**回应**：`{"commands": [{"name", "aliases", "summary", "argument"?, "usage"?, "opens"?}…]}`，照名字排。`argument` 是名字后面跟什么的提示，`usage` 是有几种写法的命令一行一种（`{"form", "summary"}`），`opens` 是打开整页的命令。`summary`、`argument` 照连接的语言。

头自己的命令（`/help` 这种）头自己拼进菜单。

## `check`

**参数**：`cwd`（照它找项目配置）、`file`（只查这一份）。

**回应**：`{"problems": [...]}`，一处一项：`kind`（`config`、`secrets`、`persona`、`preset`、`package`、`script`、`skill`、`mcp`）、`file`、`code`、`level`、`message`，有行列的带 `line`、`column`，配置的另带 `key`、`got`、`suggest`、`using`。

不写 `file` 的全查：系统配置、个人设置、`cwd` 的项目配置、密钥文件、每一层的人格和预设、软件包清单、包自己的检查。

**出错**：`unknown_file`：`file` 不是 Miyu 读的那几种。
