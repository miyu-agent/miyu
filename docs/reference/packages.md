# 软件包与命令：`package.*`、`extension.*`、`mcp.*`、`skill.list`、`skill.remove`、`script.list`、`tool.set_loading`、`feature.set`、`command.*`、`check`

管理软件包、扩展进程、个人设置里的 MCP 服务，列出技能、脚本，改工具的加载方式和技能、脚本、MCP 服务的全局开关，执行斜杠命令，查文件写没写错。扩展自己调的方法、核心发给扩展的调用见 [扩展](extensions.md)。英文版：[packages.en.md](packages.en.md)。

| 方法 | 做什么 |
|---|---|
| [`package.list`](#packagelist) | 列出软件包 |
| [`package.info`、`package.files`、`package.owns`、`package.check`](#packageinfopackagefilespackageownspackagecheck) | 一个包的信息、文件；一个路径归哪个包；装好的文件改没改 |
| [`package.install`、`package.remove`](#packageinstallpackageremove) | 装、卸 |
| [`package.enable`、`package.disable`、`package.restart`](#packageenablepackagedisablepackagerestart) | 开、关、重启 |
| [`package.sync`](#packagesync) | 从软件源拿索引 |
| [`package.search`](#packagesearch) | 在软件源里搜 |
| [`package.clean`](#packageclean) | 清下载过的包文件 |
| [`package.upgrades`、`package.upgrade`](#packageupgradespackageupgrade) | 盘点能升级的、升级 |
| [`package.file`、`package.call`](#packagefilepackagecall) | 读后台页的文件、调包的程序登记的方法 |
| [`extension.status`、`extension.enable`、`extension.disable`、`extension.restart`](#extensionstatusextensionenableextensiondisableextensionrestart) | 扩展进程的状态和开关 |
| [`mcp.list`、`mcp.restart`、`mcp.set`、`mcp.remove`](#mcplistmcprestartmcpsetmcpremove) | 个人设置里的 MCP 服务：列、重启、加或换、删 |
| [`skill.list`](#skilllist) | 列出用得上的技能 |
| [`skill.remove`](#skillremove) | 删管理员自己放的技能 |
| [`script.list`](#scriptlist) | 列出登记上的脚本 |
| [`tool.set_loading`、`feature.set`](#toolset_loadingfeatureset) | 改一件工具、一个 MCP 服务的加载方式；改技能、脚本、MCP 服务的全局开关 |
| [`command.run`](#commandrun) | 执行一条斜杠命令 |
| [`command.catalog`](#commandcatalog) | 列能用的斜杠命令 |
| [`check`](#check) | 查配置、人格、预设、软件包写没写错 |

**谁能调**：`package.install`、`package.remove`、`package.enable`、`package.disable`、`package.restart`、`package.sync`、`package.search`、`package.clean`、`package.upgrades`、`package.upgrade`、`package.file`、`extension.*`、`mcp.*`、`tool.set_loading`、`feature.set` 只给人，扩展调回 `local_only`。别的人、扩展都能调；扩展调 `package.call` 另有规矩（见下）。

**两层**：出厂的（随 Miyu 装的）、管理员家目录里装的（`home/admin/packages/`）。家目录里装了同编号的就是升级、换掉原来的；出厂的卸掉是在家目录记一笔，能装回来。

## `package.list`

**参数**：没有。**回应**：`{"packages": [...]}`，照编号排。一项：

| 名字 | 说明 |
|---|---|
| `package`、`layer` | 编号，`shipped` 或 `home` |
| `kind` | `builtin`（编进核心的）、`process`（扩展）、`worker`（按需拉起的小程序）、`ui`（界面）、`mascot`（吉祥物）、`bundle`（只带 MCP 服务、技能、脚本这类，不带程序） |
| `name`、`summary`、`icon` | 名字、一句说明、Lucide 图标名 |
| `author`、`upstream` | 作者、上游源码仓库（`https://` 开头）；清单没写的没有 |
| `from` | 从哪来：`shipped`（出厂）、`official`（从官方软件源装的）、`local`（本机文件夹、包文件装的）；以后有了社区软件源再多 `community` |
| `version`、`protocol`、`state` | 版本、说的协议版本、状态 |
| `required` | 必需的，卸不了 |
| `features` | 里面的功能（预设照它开关） |
| `status`、`enabled` | 有进程的：在跑没有、开着没有 |
| `page` | 有后台页的 |
| `mcp` | 带的 MCP 服务：`name`、`command`（网址的服务是 `url`）、`venues`、`state`、`failures`……；`need_load`、`on` 同 `mcp.list` 的一项 |
| `widgets` | 声明的界面块：`id`、`place`、`title`、`scope`、`command` |
| `command`、`commands` | 给 `miyu` 加的子命令：`command` 是第一个 `{"name", "program", "about"}`，`commands` 是全部，照写的先后（一个包可以加几个） |
| `removed` | 卸掉了的出厂包 |

写错的、撞了的、读不了的只有 `package`、`layer`、`code`、`problem`（照连接的语言说）和 `line`。要一直跟着最新的，订阅 `packages` 流。

## `package.info`、`package.files`、`package.owns`、`package.check`

| 方法 | 参数 | 回应 |
|---|---|---|
| `package.info` | `package` | `{"package", "layer", "from", "files", "size", "version"?, "author"?, "upstream"?, "installed"?, "source"?}`：从哪来（同 `package.list`）、几个文件、多少字节、作者、上游、装的时刻、装的时候的本机路径 |
| `package.files` | `package` | `{"dir", "files": [{"path", "sha256", "size"}…]}`：`path` 相对 `dir` |
| `package.owns` | `path`（绝对路径） | `{"package", "layer", "path", "recorded"}`；哪个包都不在的 `{"package": null}` |
| `package.check` | `package`（可不写） | `{"packages": [{"package", "modified", "missing", "extra"}…]}`：家目录里装的包，装好以后改了、少了、多了的文件 |

**出错**：`unknown_package`；`bad_params`（`owns` 的路径是相对的）。

## `package.install`、`package.remove`

| 方法 | 参数 | 回应 |
|---|---|---|
| `package.install` | `path`：本机一个包文件夹的绝对路径（文件夹名就是编号），或者那份 `package.toml` 的路径，或者一个包文件 `.miyu` 的路径 | 同 `package.list` 的一项 |
| `package.install` | `package`：出厂包的编号是把卸掉的装回来；别的编号同 `source`，从软件源装 | 同上 |
| `package.install` | `source`：从软件源装这个编号（缺的依赖一起装，先装依赖）；下载时推 `package.progress {"package", "done", "total"}` 给这个连接 | 同上，一起装了依赖的多 `with` |
| `package.remove` | `package` | `{"package", "removed": true}` |

都可以带 `preview: true`：只看一眼、什么都不动，拒绝照真做的那一样。`install` 的预览回 `package`、`version`、要换掉的 `replaces: {"version"}`、装回出厂的 `restores: true`，和带了什么（`program`、`mcp`、`command`（第一个子命令名）、`commands`（全部子命令名）、`page`、`mascot`、`connection`、`system_account`、`settings`、`capabilities`、`files`、`size`）；`remove` 的预览回 `package`、`layer`、`version`、`files`、`size`、`settings`（要一并删掉的配置键）、`state`（有没有状态目录）。

装、卸当场生效：带进程的照开关拉起、停下，它的工具进出工具目录，开着的会话下一轮换上。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 两个都写；路径不是绝对的 |
| `path_unreadable` | 读不了那个文件夹 |
| `package_invalid` | 清单写错了，`data.problem`、`data.line`；要更新的 Miyu 才能用的，`data.problem` 是 `needs_newer_miyu` |
| `package_conflict` | 带的名字和装着的别的包撞了：子命令、功能、工具、技能、MCP 服务、知识库、斜杠命令 |
| `package_exists` | 和出厂的同编号 |
| `package_file_bad` | 包文件解不开 |
| `download_failed` | 从软件源装时包文件哪一处都没下成（大小、哈希对不上也算），`data.why` 是每一处的原因 |
| `wrong_target` | 清单写了 `targets`，里面没有本机这一种系统 |
| `package_required` | 卸必需的 |
| `depends_missing` | 依赖的包没装，`data.missing` |
| `package_depended` | 别的装着的包依赖它，`data.by` |
| `unknown_package` | 没装的、没卸过的 |

## `package.sync`

从软件源拿索引（施工 G-3 上）。参数 `force`（可以不写）：`true` 不管本机那份新不新都去拿；不写的本机那份不到 24 小时就直接用。地址照系统配置的 `packages.mirrors`、`packages.source` 的先后试，索引照编进来的官方公钥验签，比本机那份旧的不收。

回应 `{"generated", "fetched", "packages", "stale"?}`：索引生成的时刻、拿到的时刻（都是 RFC 3339）、几个包；这一次一处都没拿成、用的是本机那份的带 `stale`（最后一处的原因，英文）。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `source_unreachable` | 一处都没拿成，本机也没有索引；`data.why` 是最后一处的原因 |

## `package.search`

在软件源里搜（施工 G-3 下）。参数 `query`（可以不写）：几个词，空白分开，都得出现在编号、名字、说明（哪种语言都算）里，不分大小写；不写、空的列出全部。先照 `package.sync` 拿索引（本机那份不到 24 小时的直接用）。

回应 `{"packages": [...], "generated", "stale"?}`，每一项 `{"package", "version", "name", "summary", "official", "installed", "blocked"}`：名字、说明照连接的语言挑（没有的照英文、中文、日文），没有的 `null`；`installed` 是装着的版本（装着、没写版本的是空的字），没装的 `null`；`official` 是不是项目主人自己的仓库；`blocked` 是被拉黑的理由，没有的 `null`。出错同 `package.sync`。

## `package.clean`

清掉下载过、不是装着那一版的包文件（施工 G-4 下）。没有参数，回应 `{"removed", "freed"}`：删了几个文件、几个字节。

## `package.upgrades`、`package.upgrade`

照软件源升级（施工 G-5）。只碰从软件源装的；从文件夹、包文件装的不碰。版本不比大小，索引的和装着的不一样就升。

| 方法 | 参数 | 回应 |
|---|---|---|
| `package.upgrades` | 无 | `{"upgrades": [{"package", "from", "to"}], "moved": [{"package", "was", "now"}], "gone", "blocked": [{"package", "reason"}], "held", "generated", "stale"?}`：能升的；来源仓库变了、不升的；软件源里没有了的；被拉黑的；系统配置 `packages.hold` 跳过的 |
| `package.upgrade` | `packages`（可以不写，只升这几个）、`preview` | 看一眼 `{"upgrades", "download"}`；真升 `{"upgraded": [{"package", "from", "to"}]}`，下载时同 `package.install {"source"}` 推进度 |

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

## `mcp.list`、`mcp.restart`、`mcp.set`、`mcp.remove`

管理员个人设置（`home/admin/settings.toml` 的 `[mcp.<名字>]`）里的 MCP 服务。手改个人设置、用 `mcp.set`、`mcp.remove` 改，都当场生效：新写的起，改了的停下再起，删掉的停下。

**`mcp.list`**：没有参数。回应 `{"servers": [...]}`：`name`、`command`（网址的服务没有这一格，是 `url`）、`venues`、`state`、`failures`，在跑的 `pid`（网址的服务没有），在等的 `retry_in`，停下的 `reason`、`stderr`，盖住了哪些包里同名服务的 `covers`。知道有哪些工具的另带 `tools`：这个服务这时在工具目录里的工具名（`mcp__<服务名>__<工具名>`），起好列完的、照目录缓存先登记的都算；还不知道的（没起过、没缓存）没有这一格。`need_load` 是 `{"value", "by"}`：这个服务的工具要不要先加载，`by` 是 `source`（服务表写的，没写的先加载）或 `setting`（个人设置的 `loading.mcp.<服务名>` 盖过的，见 `tool.set_loading`）。`on` 是这个服务的全局开关（`features.mcp-<服务名>`，见 `feature.set`），没写的是 `true`。编辑时要先填上原来写的：`env`（值是字，或者 `{"secret": …}`、`{"env": …}`；没写的没有这一格）、`timeout_ms`（不是默认的才带），网址的服务另有 `bearer`、`headers`（写法同 `env`）；`command` 是程序加参数，交回 `mcp.set` 时拆成 `command`、`args`，`need_load` 改用 `tool.set_loading`。

**`mcp.restart`**：参数 `name`。回应是那一项。**出错**：`unknown_server`。

**`mcp.set`**：在个人设置里加上一个服务，或者整条换掉同名的那一个。

| 名字 | 类型 | 说明 |
|---|---|---|
| `name` | 字符串，必写 | 服务名：小写字母开头，只有小写字母、数字、`-`，最多 32 个 |
| `command` | 字符串，必写 | 程序 |
| `args` | 字符串数组 | 程序后面的参数 |
| `env` | 对象 | 环境变量：值是字，或者 `{"secret": …}`、`{"env": …}` |
| `venues` | 字符串数组 | 给哪种会话：`local`、`private`、`group`；不写是只给本机的会话 |
| `timeout_ms` | 整数 | 一次调用等多久，1000 到 3600000；不写是 600000 |
| `need_load` | 布尔 | 这个服务的工具要先加载；不写的照个人设置里没写的算 |
| `clicall` | 布尔 | 这个服务的工具 AI 在命令里用 `miyu call` 能不能调；不写的照原来写的留着，原来没写的能 |

没交的格子不写：整条换掉时原来写着、这次没交的也没了。写之前照读服务表的规矩查，写法同 `config.set`（带版本，不覆盖同时的手改）。回应 `{"name"}`；服务随后照新写的换，状态变了推 `mcp.changed`。**出错**：`mcp_invalid`（写错了，`data.problem` 说哪里不对）；`server_from_package`（个人设置里没写这个名字、有包带着同名服务：随包来的不经它改）；`config_file_broken`（个人设置读不进来，或者 `mcp` 写成了放不下的样子）；`config_conflict`。

**`mcp.remove`**：参数 `name`。从个人设置里删掉这个服务（连同它的子表），回应 `{}`。包里被它盖住的同名服务照包的开关起回来。**出错**：`unknown_server`；`config_file_broken`；`config_conflict`。

## `skill.list`

**参数**：没有。**回应**：`{"skills": [...], "dir"}`（`dir` 是管理员自己放技能的文件夹，还没建的也照写，家目录下的写成 `~/…`），核心这一刻用得上的技能，照 `name` 排；同名的几处都有的，只列核心实际用的那一份（自己放的赢；包之间出厂的先于管理员装的，同一层照编号）。一项：

| 名字 | 说明 |
|---|---|
| `name` | 技能名 |
| `display_name`、`summary` | 给人看的名字、一句说明，照连接的语言挑（这种语言没写的照英文、中文、日文的先后）；都没写的没有这一格 |
| `from` | 从哪来：`home`（管理员自己放的，`home/admin/skills/`）、`shipped`（出厂的包带的）、`package`（管理员装的包带的） |
| `package` | 随包来的：包的编号 |
| `path` | `SKILL.md` 在哪：家目录下的写成 `~/…` |
| `on` | 全局开关（`features.skill-<名字>`，见 `feature.set`）：没写的是 `true` |

## `skill.remove`

**参数**：`names`，技能名的数组，至少一个、不能重复。**回应**：`{"trash": [...]}`，每个技能的文件夹在系统回收站里的位置，照 `names` 的先后。只删管理员自己放的（`from` 是 `home`，`home/admin/skills/<文件夹>/` 整个挪进回收站）；它盖住了包里同名的，删了包里的那一份重新列出来。只给人用，扩展进程调了回 `local_only`。

**出错**：没写名字、名字重复：`bad_params`。没有这个技能：`unknown_skill`；随包来的：`skill_from_package`（两种都带 `data.name`，几个名字里有一个不行就一个都不删）。挪不进回收站：`trash_failed`。

## `script.list`

**参数**：没有。**回应**：`{"scripts": [...], "dir"}`（`dir` 是管理员自己放脚本的文件夹，写法同 `skill.list`），这一刻登记成工具的脚本，照 `tool` 排。一项：

| 名字 | 说明 |
|---|---|
| `tool` | 工具名 |
| `display_name`、`description` | 给人看的名字、一句说明（脚本开头的 `Display name`、`显示名称`、`描述`），照连接的语言挑，挑法同 `skill.list`；都没写的没有这一格 |
| `from`、`package` | 同 `skill.list`：`home` 是管理员自己放的（`home/admin/scripts/`） |
| `path` | 脚本文件在哪，写法同 `skill.list` |
| `timeout` | 一次等多少秒 |
| `need_load` | `{"value", "by"}`：要不要先加载；`by` 是 `source`（脚本开头写的，没写的先加载）或 `setting`（个人设置的 `loading.tools.<工具名>` 盖过的） |
| `on` | 全局开关（`features.script-<工具名>`）：没写的是 `true` |

## `tool.set_loading`、`feature.set`

技能、脚本、MCP 服务那几页改的两样，都写管理员的个人设置，写法同 `config.set` 改一项（带版本，不覆盖同时的手改，写成了推 `config.changed`）。从下一个回合开始生效。回应 `{}`。

**`tool.set_loading`**：改一件工具（`loading.tools.<工具名>`）或一个 MCP 服务（`loading.mcp.<服务名>`）要不要先加载。工具自己那一项先，MCP 的工具再照服务那一项，都没写的照来处。

| 名字 | 类型 | 说明 |
|---|---|---|
| `tool` | 字符串 | 工具名：1 到 128 个字符，没有 `.` 和控制字符 |
| `server` | 字符串 | MCP 服务名 |
| `need_load` | 布尔或 `null`，必写 | `null` 删掉这一项，回到来处写的 |

`tool`、`server` 正好写一个。

**`feature.set`**：改一个技能、脚本、MCP 服务的全局开关（`features.<功能编号>`）。`unlisted` 是 `on` 的预设照它；预设里单独写了开或关的照预设（`preset.get` 每个功能的 `by`）。`unlisted` 是 `off` 的预设不照它：没写的照旧关着。

| 名字 | 类型 | 说明 |
|---|---|---|
| `feature` | 字符串，必写 | 功能编号：`skill-<名字>`、`script-<工具名>`、`mcp-<服务名>` |
| `on` | 布尔或 `null`，必写 | `null` 删掉这一项，回到跟着预设 |

**出错**：`bad_params`（`tool`、`server` 不是正好一个，名字写不进键，不是这三类功能，少了 `need_load`、`on`）；`config_file_broken`；`config_conflict`。

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
| `/stop` | 打断这一轮，停掉这个会话派出去的后台命令和子代理，和正在跑的整理 |
| `/workspace <路径>` | 换工作目录；不带路径的说现在在哪 |
| `/todo`、`/todo clear` | 看待办、清空待办 |
| `/remember <话>` | 记一条记忆 |
| `/dream`、`/dream stop` | 现在就整理记忆（在后台答）；停下正在跑的整理，合完的留着 |
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
