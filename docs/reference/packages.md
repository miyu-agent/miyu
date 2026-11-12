# 软件包与命令：`package.*`、`extension.*`、`mcp.*`、`skill.list`、`skill.remove`、`script.list`、`tool.set_loading`、`feature.set`、`command.*`、`check`

管理软件包、扩展进程、系统配置里的 MCP 服务，列出技能、脚本，改工具的加载方式和技能、脚本、MCP 服务的全局开关，执行斜杠命令，查文件写没写错。扩展自己调的方法、核心发给扩展的调用见 [扩展](extensions.md)。英文版：[packages.en.md](packages.en.md)。

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
| [`package.file`、`package.call`、`package.page`](#packagefilepackagecallpackagepage) | 读后台页的文件、调包的程序登记的方法、不靠网页打开后台页 |
| [`extension.status`、`extension.enable`、`extension.disable`、`extension.restart`](#extensionstatusextensionenableextensiondisableextensionrestart) | 扩展进程的状态和开关 |
| [`driver.check`](#drivercheck) | 查扩展提供的模型驱动编码稳不稳（会重启扩展） |
| [`mcp.list`、`mcp.restart`、`mcp.set`、`mcp.remove`](#mcplistmcprestartmcpsetmcpremove) | 系统配置里的 MCP 服务：列、重启、加或换、删 |
| [`mcp.login`](#mcplogin) | 网址的 MCP 服务要浏览器登录的：拿授权页 |
| [`mcp.login_finish`、`mcp.logout`](#mcplogin_finishmcplogout) | 浏览器不在核心那台机器上时把跳到的地址贴回来；登出 |
| [`skill.list`](#skilllist) | 列出用得上的技能 |
| [`skill.remove`](#skillremove) | 删你自己放的技能 |
| [`script.list`](#scriptlist) | 列出登记上的脚本 |
| [`tool.set_loading`、`feature.set`](#toolset_loadingfeatureset) | 退役（照旧收、什么都不改）；改技能、脚本、MCP 服务的全局开关 |
| [`command.run`](#commandrun) | 执行一条斜杠命令 |
| [`command.catalog`](#commandcatalog) | 列能用的斜杠命令 |
| [`check`](#check) | 查配置、人格、预设、软件包写没写错 |

**谁能调**：`package.install`、`package.remove`、`package.enable`、`package.disable`、`package.restart`、`package.sync`、`package.search`、`package.clean`、`package.upgrades`、`package.upgrade`、`package.file`、`package.page`、`extension.*`、`mcp.*`、`tool.set_loading`、`feature.set` 只给人，扩展调回 `local_only`；`package.info` 带 `path` 的也只给人（看的是本机任意一处的文件夹，同 `package.install`，施工 IN-4）。别的人、扩展都能调；扩展调 `package.call` 只调得到自己（见下）。

**三层**：出厂的（随 Miyu 装的，`shipped`）、系统加装的（系统的包管理器另外装的，`distro`：Linux 是 `$XDG_DATA_DIRS` 里每一处的 `miyu/packages/`，没设照 `/usr/local/share:/usr/share`，Linuxbrew 自己的 `share` 写进 `XDG_DATA_DIRS`；macOS 是 Homebrew 前缀的 `share/miyu/packages/`；Windows 没有；`MIYU_DISTRO_PACKAGES` 设了的只看它）、装上的（数据根的 `opt/`，`home`）。同一个编号几层都有的，装上的盖过系统加装的、系统加装的盖过出厂的，被盖住的不加载、不算错。装上那一层装了同编号的就是升级、换掉原来的；出厂的、系统加装的卸掉是在 `etc/removed/` 记一笔（空文件 `<编号>`），文件不动，能装回来。

## `package.list`

**参数**：没有。**回应**：`{"packages": [...], "groups": [...]}`：包照编号排；组是出厂组表里的组，每个 `{"id", "name", "summary"?, "packages"}`，照连接的语言；`packages` 是组里装着的包，照用途的先后（出厂组表写的，没写到的排在后面、照编号），头照它把组里的包收在一起、照这个先后排。一项：

| 名字 | 说明 |
|---|---|
| `package`、`layer` | 编号，`shipped`、`distro` 或 `home` |
| `shadowed` | 被上面一层盖住的才有：`{"by", "version", "over", "newer"}`，盖住它的那一层、这一份的版本、认的那一份的版本、这一份是不是更新。不算错，没有 `code` |
| `kind` | `builtin`（编进核心的）、`process`（扩展）、`worker`（按需拉起的小程序）、`ui`（界面）、`mascot`（吉祥物）、`theme`（外观：终端主题、网页主题，见下面 `theme` 一格）、`bundle`（只带 MCP 服务、技能、脚本这类，不带程序） |
| `name`、`summary`、`icon` | 名字、一句说明、Lucide 图标名 |
| `logo` | 包自带的图标的版本（施工 F-18 上）：清单写了 `[package] logo`、图合规矩的才有，图用 `package.logo` 读 |
| `author`、`upstream` | 作者、上游源码仓库（`https://` 开头）；清单没写的没有 |
| `installed_at` | 装的时刻；出厂的没有。头照它排「别的按安装先后」 |
| `from` | 从哪来：`shipped`（出厂）、`official`（从官方软件源装的）、`local`（本机文件夹、包文件装的）；以后有了社区软件源再多 `community` |
| `source` | 是怎么来的，取值同装之前看一眼的 `source`：`official`（官方源里官方收录的）、`community`（官方源里社区收录的，或配置里自己加的源里的）、`file`（本机文件夹、包文件装的）、`shipped`（出厂）、`distro`（系统的包管理器装的）；早先装的、推不出来的是 `unknown`，不当成官方 |
| `feed` | 从软件源装的才有：哪个源，官方的是 `official`，配置里加的写它的名字 |
| `version`、`protocol`、`state` | 版本、说的协议版本、状态 |
| `groups` | 在哪几个组里（只有出厂的内置包有，`basesystem` 是基础系统）；不在组里的没有 |
| `features` | 里面的功能（预设照它开关） |
| `status`、`enabled` | 有进程的：在跑没有、开着没有 |
| `connection` | 平台接入的包（清单写了 `[connection]`）：`{"platform", "platforms"?, "owner_builtin"?}`。`platform` 是接的平台，接几个的是第一个，另带 `platforms`（全部，照写的先后）。头照 `platform` 在这个包的信息页上画「你在这个平台上的号」，写进主人对应表 `external.bindings`。带 `owner_builtin: true` 的平台（现在只有 `voice`）主人不用在对应表里填号，头不画填号那一块；别的平台不带这一格 |
| `page` | 有后台页的，头照它列「软件后台」 |
| `page_server` | 后台页是它的程序自己起的网页服务（清单 `[page] server = true`）：头问它的 `status` 拿 `web` 端口，带一张 `login.ticket` 跳过去，不放进框里 |
| `tray` | 有后台页、清单写了 `[tray]` 的：`{"name"}`，托盘菜单里那一行的字，照连接的语言；点了打开这个包的后台页。托盘只列开着的 |
| `theme` | 外观包带了什么：`{"tui"?: {"file"}, "web"?: {"dir"}}`，`tui` 是终端主题文件，`web` 是网页主题目录（一个普通的网页包），至少有一样；路径都相对包目录，文件用 `package.file` 读 |
| `mcp` | 带的 MCP 服务：`name`、`command`（网址的服务是 `url`）、`venues`、`state`、`failures`……；`on`、`skipped` 同 `mcp.list` 的一项。不替主人干活的扩展（场所里替外人的平台桥）拿到的每个服务只有 `name`、`state`（停下的带 `reason`），也没有 `state`、`pages_dir`、`command`、`commands`、`process`、`worker`、`check`、`problem`、`line` |
| `widgets` | 声明的界面块：`id`、`place`、`title`、`scope`、`command` |
| `command`、`commands` | 给 `miyu` 加的子命令：`command` 是第一个 `{"name", "program", "about"}`，`commands` 是全部，照写的先后（一个包可以加几个） |
| `contents` | 给哪几个包带了内容（清单里 `[content.<包>]` 的编号）；没带的没有这一格 |
| `provides` | 带了哪几样，写法同 `package.search` 每一行的 `provides`：种类到名字，种类有 `command`（子命令）、`features`（清单写明的功能）、`mcp`（MCP 服务）、`slash`（斜杠命令）、`content`（带了内容的包）、`skills`、`scripts`（脚本的文件名）、`personas`、`presets`，每一种照名字排，没带的种类不写，什么都没带的是 `{}`。出厂的、系统加装的、装上的、卸掉了的出厂包都有，照包文件夹现算；头照它在「已安装」里标「技能」「脚本」 |
| `removed` | 卸掉了的出厂包 |

写错的、撞了的、读不了的只有 `package`、`layer`、`code`、`problem`（照连接的语言说）和 `line`。要一直跟着最新的，订阅 `packages` 流。

## `package.info`、`package.files`、`package.owns`、`package.check`

| 方法 | 参数 | 回应 |
|---|---|---|
| `package.logo` | `package`、`from`? | `{"package", "logo", "media_type", "data"}`：包自带的图标（施工 F-18 上），`logo` 是版本（内容的 SHA-256 前 16 位，同 `package.list` 那一格），`data` 是 base64，`media_type` 是 `image/png` 或 `image/webp`；清单没写、不合规矩（不在、不是 PNG / WebP、超 256 KiB、宽高超 512）的三格都是 `null`。另可写 `from`（软件源的名字，同 `package.search` 每一行的 `source`）：读那个源里的包的图标，没装的也行（施工 F-18 下）；不认识的源 `unknown_source`，索引里没有 `unknown_package`，下不成 `download_failed` |
| `package.info` | `package`、`source`? | `source` 写 `true` 只查软件源，`false` 只查装着的，不写的先查装着的、没装的查软件源。装着的：`{"package", "layer", "from", "files", "size", "version"?, "author"?, "upstream"?, "installed"?, "path"?, "source", "feed"?, "name"?, "summary"?, "depends"?, "recommends"?, "provides"?, "capabilities"?, "required_by"?, "origin"?, "tools"?}`：从哪来（`from`、`source`、`feed` 同 `package.list`）、几个文件、多少字节、作者、上游、装的时刻、装的时候的本机路径（`path`，只给主人；以前这一格叫 `source`）；名字、说明、依赖、推荐、带了什么（同装之前看一眼的「包含」）、要的能力、装着的包里依赖它的、来源仓库；`tools` 是装上以后模型会多的工具 `[{"name", "title"?, "from"}]`：`name` 是她看到的工具名，`title` 是显示名 `{"zh"?, "en"?}`，`from` 是 `script` 或 `extension`（MCP 的、要跑起来才知道的不列）。没装的照软件源：`{"from": "source", "source", "package", "version", "depends", "recommends", "provides", "capabilities", "origin", "official", "name"?, "summary"?, "author"?, "upstream"?, "download"?, "installed_size"?, "tools"?, "stale"?}`：`source` 是源的名字，`provides` 是索引原样，`download` 是本机那份文件多少字节，`installed_size` 是装好以后多大，`tools` 照索引，写法同装着的 |
| `package.info` | `path`（和 `package` 二选一，不收 `source`；施工 IN-4） | 看本机一个包文件夹（或里面的 `package.toml`）、包文件 `.miyu` 的绝对路径，照 pacman 的 `-Qip`：什么都不装，照真装之前查的那几样查一遍。回应照装着的那一份的样子：`{"from": "file", "path", "package", "files", "size", "version"?, "name"?, "summary"?, "author"?, "upstream"?, "depends"?, "recommends"?, "provides"?, "capabilities"?, "skipped"?, "replaces"?}`：`path` 照写的原样（包文件的是那个文件，不是解开的暂存处），`files`、`size` 是包里几个文件、多少字节，`provides`、`capabilities`、`skipped`（读不成的技能、脚本、斜杠命令模板 `[{"file", "why"}]`）、`replaces` 同 `package.install` 看一眼的那几格；没有 `layer`、`installed`、`required_by`、`origin` |
| `package.files` | `package` | `{"dir", "files": [{"path", "sha256", "size"}…]}`：`path` 相对 `dir` |
| `package.owns` | `path`（绝对路径） | `{"package", "layer", "path", "recorded"}`；哪个包都不在的 `{"package": null}` |
| `package.check` | `package`（可不写） | `{"packages": [{"package", "modified", "missing", "extra"}…]}`：装上那一层的包，装好以后改了、少了、多了的文件。系统加装的另有一项，多 `"layer": "distro"`，不比哈希：清单读不成的多 `manifest`，清单写的程序不在 `bin/` 的进 `missing` |

**出错**：`unknown_package`（`package.info` 两处都没有）；`source_unreachable`（`package.info` 只查软件源、软件源拿不到，本机也没有索引）；`bad_params`（`owns` 的路径是相对的；`info` 的 `package`、`path` 都写或都没写，带 `path` 又写了 `source`，路径是相对的）。`info` 带 `path` 的另照 `package.install` 的出错拒：`path_unreadable`、`package_invalid`、`package_conflict`、`wrong_target`、`package_exists`、`package_file_bad`、`depends_missing`，见下一节。

## `package.install`、`package.remove`

| 方法 | 参数 | 回应 |
|---|---|---|
| `package.install` | `path`：本机一个包文件夹的绝对路径（文件夹名就是编号），或者那份 `package.toml` 的路径，或者一个包文件 `.miyu` 的路径 | 同 `package.list` 的一项 |
| `package.install` | `package`：出厂的、系统加装的编号是把卸掉的装回来；别的编号同 `source`，从软件源装 | 同上 |
| `package.install` | `source`：从软件源装这个编号（缺的依赖一起装，先装依赖；依赖写了版本下限的照下限挑，装着的版本不够的一起升，施工 PK-2）；先全下完、再一个一个核对、再一个一个装，每一段推 `package.progress` 给这个连接（见下）。`from`（可以不写）：从哪个软件源装（`official` 或配置里加的源的名字），不写的照先后找（施工 U23 下下） | 同上，一起装了依赖的多 `with` |
| `package.install` | `package`：组名（`package.list` 顶上的 `groups`，例如 `basesystem`）是把组里卸掉的出厂包一起装回来 | `{"group", "packages": [<同 `package.list` 的一项>…]}`，一个都没卸过的是空的 |
| `package.remove` | `package`；`purge`（可以不写，默认 `false`）：`true` 的连配置目录 `etc/<编号>/`、系统配置里它的设置项一起删，不写的留下，重装同编号照旧用（照 pacman）。已经卸掉、还留着这几样的编号也收 `purge: true`：只删留下的；不带的照旧 `unknown_package` | `{"package", "removed": true, "layer"}`（没装的 `purge` 不带 `layer`） |
| `package.remove` | `package`：组名是把组里装着的一起卸掉；组外装着的包依赖组里的整个拒（`package_depended`，`data.by` 是它们）；`purge` 同上，照样传给组里每个包 | `{"group", "removed": [<编号>…]}` |

都可以带 `preview: true`：只看一眼、什么都不动，拒绝照真做的那一样。`install` 的预览回 `package`、`version`、要换掉的 `replaces: {"version"}`、装回出厂的、系统加装的 `restores: true`，和带了什么（`program`、`mcp`、`command`（第一个子命令名）、`commands`（全部子命令名）、`page`、`mascot`、`theme`、`connection`、`settings`、`capabilities`、`files`、`size`）；`remove` 的预览回 `package`、`layer`、`version`、`files`、`size`、`settings`（要一并删掉的配置键：只有 `purge` 的列，不 `purge` 的是 `[]`）、`state`（会一并删掉的状态目录：`{"path", "size"}`，`path` 是 `var/lib/<编号>/` 的位置，`size` 是底下文件一共多少字节，符号链接不跟进去；没有状态目录的不写这一格。以前这一格是 `true`/`false`）、`config`（配置目录和设置项：`{"path"?, "kept", "settings"?}`，`path` 是 `etc/<编号>/` 的位置，不在的、空的不写；`settings` 是系统配置里写了的它的设置项有几项，0 的不写；`kept` 是 `true` 两样都留下、`false` 都删掉（写了 `purge: true`）；`path`、`settings` 都没有的不写这一格）。卸掉的包留在配置里的设置项不生效，`check` 不报不认识，一个包说一句 `settings_left`。

从软件源装的预览另有 `source`（从哪来：`official` 官方源里官方的、`community` 官方源里社区收录的或者配置里自己加的源里的）、`feed`（源的名字，官方的是 `official`；施工 PK-3 上以前这一格叫 `source`）、`official`、`origin`、`download`（要下的字节数，连依赖）、`with`（一起装的依赖 `[{"package", "version", "source", "feed", "capabilities"?}]`）、`recommends`（要装的包推荐的、还没装的，去重：`[{"package", "version", "source", "feed", "capabilities"?, "download", "installed_size"?}]`，没有的是空的）、`installed`（要装的全部包装好以后一共多少字节，不算推荐的；有一个索引没写的不写这一格）；`capabilities` 同从包文件夹装的那一格（`{"id", "name", "summary"}`）。

从哪来、要的能力（施工 PK-3 上）：预览的 `source` 一共五种，从软件源装的 `official`、`community`，从包文件夹、包文件装的 `file`，装回的 `shipped`、`distro`。能力的每一项这一刻已经批过、装完照旧留着的多 `held: true`：只有从软件源装、源和来源仓库都和装着的那一份一样时才有（本机文件装的、换了源的、换了来源仓库的前面批过的不接着用）。社区的、本机文件的预览多 `notice`：要给人看的话，照连接的语言、一句一个字符串（「这个包没经过官方审核。」，带程序、子命令、MCP 服务、脚本的再一句「它带的程序会以你的身份在这台机器上跑，不在沙盒里。」，几个包的写出是哪几个）；推荐的里有的另放 `recommends_notice`。照它原样印，「注意：」这样的前缀是头自己的字。

装的时候批（施工 PK-3 上）：`package.install`（`path`、`source`）、`package.upgrade` 可以带 `approve`：`{"<编号>": ["<能力>", …]}`，这一次装上的每个包各写各的（照预览里列出来的拼：要装的、`with` 的，升级的 `upgrades`、`with` 的）。能力名、编号写法不对的 `bad_params`；装上的那一份没声明的、编号不是这一次装的不理；不写的装上不批，打开时照旧用 `package.enable`、`extension.enable` 的 `approve` 批。批过的记进 `extensions.json` 的 `approved`：照「(源和来源仓库都没变时以前批过的 ∪ 这一次批的) ∩ 这一份声明的」，所以新版不再要的会去掉、以后加回来要重新批。手动开（`start = "manual"`）的装好以后打开不用再批，开着就跑的装上当场跑。

真装时推给这个连接的 `package.progress`：`{"package", "stage", "index", "count", "done"?, "total"?}`。`stage` 是 `download`（带 `done`、`total` 字节）、`verify`（核对之前）、`install`（装之前）；`index`、`count` 是第几个（从 1 数）、一共几个。从包文件夹、包文件装的只推 `verify`、`install` 各一次（`1`、`1`）。老的核心只推 `{"package", "done", "total"}`，没写 `stage` 的当 `download`。

装、卸当场生效：带进程的照开关拉起、停下，它的工具进出工具目录，开着的会话下一轮换上。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 两个都写；路径不是绝对的 |
| `path_unreadable` | 读不了那个文件夹 |
| `package_invalid` | 清单写错了，`data.problem`、`data.line`；要更新的 Miyu 才能用的，`data.problem` 是 `needs_newer_miyu` |
| `package_conflict` | 带的名字和装着的别的包撞了：子命令、功能、工具、技能、MCP 服务、知识库、斜杠命令、人格、预设、前缀、总线名字（`bus`：一个名字只有一家提供，一个名字不能一家提供、一家接进） |
| `not_permitted` | 会话里的 `package` 工具：会话不是主人的（场所里替外人的）装、升级、卸、装回包，启用扩展；列、看、搜照旧给 |
| `package_exists` | 和出厂的同编号 |
| `package_file_bad` | 包文件解不开；从软件源装的，包里的编号和索引那一条对不上 |
| `download_failed` | 从软件源装时包文件哪一处都没下成（大小、哈希对不上也算），`data.why` 是每一处的原因 |
| `wrong_target` | 清单写了 `targets`，里面没有本机这一种系统 |
| `depends_missing` | 依赖的包没装、装着的版本比清单写的下限低（施工 PK-2），`data.missing`：缺的那几个，照清单写的原样（带下限的是 `"onebot>=1.2.0"`） |
| `package_depended` | 别的装着的包依赖它，`data.by` |
| `unknown_package` | 没装的、没卸过的 |
| `unknown_source` | `from` 写的软件源没有（没加、写错了） |

## `package.sync`

从软件源拿索引（施工 G-3 上）。参数 `force`（可以不写）：`true` 不管本机那份新不新都去拿；不写的本机那份不到 24 小时就直接用。地址照系统配置的 `packages.mirrors`、`packages.source` 的先后试，索引照编进来的官方公钥验签，比本机那份旧的不收。

回应 `{"generated", "fetched", "packages", "stale"?}`：索引生成的时刻、拿到的时刻（都是 RFC 3339）、几个包；这一次一处都没拿成、用的是本机那份的带 `stale`（最后一处的原因，英文）。

别的软件源（施工 U23 下下）：系统配置里 `packages.sources.<名字>` 写了 `url`、`key`（Ed25519 公钥的 base64）的。官方的拿完再拿它们，各照各的公钥验；回应多 `sources: [{"source", "generated", "fetched", "packages", "stale"?}]`，拿不成的那一项是 `{"source", "error"}`。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `source_unreachable` | 一处都没拿成，本机也没有索引；`data.why` 是最后一处的原因 |

## `package.search`

在软件源里搜（施工 G-3 下）。参数 `query`（可以不写）：几个词，空白分开，都得出现在编号、名字、说明（哪种语言都算）里，不分大小写；不写、空的列出全部。先照 `package.sync` 拿索引（本机那份不到 24 小时的直接用）。

回应 `{"packages": [...], "generated", "stale"?}`，每一项 `{"package", "version", "name", "summary", "official", "installed", "blocked", "contents", "provides", "depends", "kind"?, "icon"?, "logo"?, "published"?, "downloads"?, "downloads_week"?}`：名字、说明照连接的语言挑（没有的照英文、中文、日文），没有的 `null`；`installed` 是装着的版本（装着、没写版本的是空的字），没装的 `null`；`official` 是不是项目主人自己的仓库；`blocked` 是被拉黑的理由，没有的 `null`；`contents` 是给哪几个包带了内容（同 `package.list`），没带的是空数组；`provides` 是带了哪几样：种类到名字，种类有 `command`（子命令）、`features`（功能）、`mcp`（MCP 服务）、`slash`（斜杠命令）、`content`（带了内容的包）、`skills`、`scripts`、`personas`、`presets`，没带的种类不写，什么都没带的是 `{}`；`depends` 是依赖的包，照索引原样：编号，或带版本下限的 `<编号>>=<版本>`（施工 PK-2），没有的是空数组（施工 G-10：软件市场照种类筛）；`kind` 是包的种类，照清单 `[package] kind` 的原样（`process`、`ui`、`theme`、`mascot`、`bundle`……，以后多了的照样原样），以前生成的索引没有这一格（施工 G-11）；`source` 是哪个软件源的（`official` 或配置里起的名字，施工 U23 下下），别的源里的 `official` 一律是 `false`；`icon` 是 Lucide 图标名，`logo` 是包自带图标的版本（同 `package.list`，图用 `package.logo` 带 `from` 读），索引里有的才有（施工 F-18 下）；`published` 是这一版第一次进这个软件源的时刻（RFC 3339），`downloads` 是各版本、各系统一共被下载了几次，`downloads_week` 是近 7 天下载了几次，索引里有、写法对的才有（施工 DL-1，设计 54：头照它们排最新、下载量、趋势，没有的排在最后）。出错同 `package.sync`。

## `package.clean`

清掉下载过、不是装着那一版的包文件（施工 G-4 下）。没有参数，回应 `{"removed", "freed"}`：删了几个文件、几个字节。

## `package.upgrades`、`package.upgrade`

照软件源升级（施工 G-5）。只碰从软件源装的；从文件夹、包文件装的不碰。版本不比大小，索引的和装着的不一样就升。

| 方法 | 参数 | 回应 |
|---|---|---|
| `package.upgrades` | 无 | `{"upgrades": [{"package", "from", "to"}], "moved": [{"package", "was", "now"}], "gone", "blocked": [{"package", "reason"}], "held", "generated", "stale"?}`：能升的；来源仓库变了、不升的；软件源里没有了的；被拉黑的；系统配置 `packages.hold` 跳过的 |
| `package.upgrade` | `packages`（可以不写，只升这几个）、`preview`、`approve`（同 `package.install`，施工 PK-3 上） | 看一眼 `{"upgrades", "download", "with", "notice"?}`：`upgrades` 每一项多 `source`、`feed`、`capabilities`（新版声明的，带 `held`），`with` 是一起装的新依赖（同 `package.install` 预览的那一格）；真升 `{"upgraded": [{"package", "from", "to"}]}`，下载时同 `package.install {"source"}` 推进度 |

## `package.enable`、`package.disable`、`package.restart`

**参数**：`package`；`enable` 另可带 `approve`：批准它声明的能力（见 [扩展](extensions.md) 的「能力」）。**回应**：同 `package.list` 的一项。

- 扩展：开是拉起进程，关是请它退出、它登记的工具出目录。
- `bundle`：开是起它带的 MCP 服务，关是停下。
- 出厂的内置包：关是卸掉，开是装回来。
- `restart`：重启扩展的进程和它带的 MCP 服务，连续失败从零数。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `unknown_package` | 没有这个包 |
| `not_switchable` | 界面、小程序、吉祥物包、主题包没有开关 |
| `program_missing` | 包的程序没找到（包文件夹的 `bin/`、`miyu` 旁边都没有） |
| `needs_approval` | 要的能力还有没批的，`data.capabilities` |
| `package_depended` | 关一个别的包依赖的 |
| `package_off`、`not_restartable` | `restart` 一个关着的；没有能重启的进程、服务 |

## `package.file`、`package.call`、`package.page`

**`package.file`**：参数 `package`、`path`、`offset`。回应 `{"data", "size", "eof"}`，一次最多 512 KiB。网页的后台页、吉祥物的模型文件、外观包的主题照它读。`path` 对后台页是后台页目录里的相对路径；对吉祥物包只能是 `model` 那一份；对外观包相对包目录写，只能是终端主题的 `file` 那一份，或者网页主题目录下的任何一份普通文件（例如 `web/index.js`）。照真实路径找，链接指出包目录、指出网页目录的，目录本身、别的路径，都回 `not_found`。

**`package.call`**：参数 `package`、`method`、`params`（可不写）。核心把它转给那个包的程序（`method.call`），程序回什么交回什么。在后台答。界面块上的按钮、整页的表单保存都走它。

**`package.page`**：参数 `package`。回应 `{"url"}`：核心按需起的本机后台页服务上这个包的宿主页（`http://127.0.0.1:<端口>/h/<票>/`），交给浏览器打开，不用装网页（`miyu pkg page` 就照它开）。只给后台页在包目录里的（清单写 `[page] dir`）；服务第一次要的时候起，闲了十分钟（没有请求、没有开着的推送流）就关，关了地址作废，再要一次就是。宿主页照网页的那一套把页面放进隔离的框、替它调 `settings.get`、`settings.set`、`call`，深浅照系统。

`package.call` 是给人的连接用的（各个头、命令行、人在终端起的程序）：哪个包登记了的方法都调得到，写没写进 `[export]` 都行，转过去不带 `from`。扩展（核心拉起的包的程序）照包编号只调得到自己：`package` 写了别的包的编号回 `own_package_only`，那个包在不在、方法导没导出都是这一句；调自己要声明能力 `packages.call`，只调得到自己清单 `[export]` 里列的方法，转过去带 `from`（自己的编号）。扩展要调别的包，照名字走 [`bus.call`](bus.md)。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `no_page`、`not_found` | `package.file`：没有后台页；没有这份文件、跑出目录、不是普通文件。`package.page`：没有包目录里的后台页（`no_page`） |
| `program_not_running` | 程序没连着 |
| `unregistered` | 程序没登记这个方法，`data.method` |
| `method_failed` | 程序回了错，`data.message`、`data.code`（扩展回的错带字的码放在 `error.data.code`，核心照它交回；没有的照 `error.code`）。程序回的那一行超过 1 MiB 的核心不读，`data.code` 是 `line_too_long` |
| `method_timeout` | 到时限没回 |
| `own_package_only` | 扩展调的：`package` 不是它自己，`data.package` 是它写的那个编号。跨包用 `bus.call` |
| `needs_approval`、`not_exported` | 扩展调自己的：没声明 `packages.call`；清单 `[export]` 里没有这个方法（`data.method`） |

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
| `reason`、`stderr` | 停下的：为什么（`failed_repeatedly`、`config_error`、`needs_approval`、网址的服务要浏览器登录的 `needs_login`……）、标准错误的最后几行 |
| `capabilities`、`unapproved` | 声明的能力 `[{id, name, summary}]`、还没批的 |

要一直跟着最新的，订阅 `extensions` 流。

**出错**：`unknown_package`；`not_an_extension`（是界面包）；`extension_off`（重启一个关着的）；`needs_approval`。

## `driver.check`

查一个包提供的模型驱动编码稳不稳（作者用 `miyu dev driver-check <包编号>` 调它）。只给人。**会重启那个扩展**。

核心照内置的一小套样本请求（纯文字、工具面、带私有数据的思考和工具调用、看不了图的转述、续写，各一份）调扩展的 `driver.encode`：每一份编两次；重启扩展、等它重新握手，再编一次；三次要逐字节相同。另有只在末尾加一条消息的一对：最后一条稳定区消息里放着记号字 `miyustableend7f3a`，两份编出来的字节都要找得到它，到它为止逐字节相同；找不到的照「没法查稳定区」算没通过。

**参数**：`package`。**回应** `{"package", "drivers": [...]}`，一个驱动一项：

| 名字 | 说明 |
|---|---|
| `driver` | `<包编号>/<驱动名>` |
| `cache` | 清单写的缓存类型 |
| `result` | `passed`、`failed`、`skipped` |
| `samples` | `[{"sample", "ok", "why"?}]`：`plain`、`tools`、`private`、`image`、`continuation`、`pair`；没通过的 `why` 是英文的一句，写明哪里不对（第几个字节） |
| `why` | 没实现 `driver.encode` 的：`driver.encode is not implemented`。`cache = "contract"` 的算 `failed`，别的算 `skipped` |

**出错**：`unknown_package`；`no_drivers`（清单没有 `[[driver]]`）；`program_not_running`（没连上、重启以后等不到握手）；`driver_busy`（这一刻有会话正经它的驱动调模型，`data.running` 是没答完的几次：等它们说完再查，不重启）；`extension_off`、`needs_approval`（同 `extension.restart`）。

## `mcp.list`、`mcp.restart`、`mcp.set`、`mcp.remove`

系统配置（`config.toml` 的 `[mcp.<名字>]`）里的 MCP 服务。手改系统配置、用 `mcp.set`、`mcp.remove` 改，都当场生效：新写的起，改了的停下再起，删掉的停下。

**`mcp.list`**：没有参数。回应 `{"servers": [...]}`：`name`、`command`（网址的服务没有这一格，是 `url`）、`venues`、`state`、`failures`，在跑的 `pid`（网址的服务没有），在等的 `retry_in`，停下的 `reason`、`stderr`，盖住了哪些包里同名服务的 `covers`。知道有哪些工具的另带 `tools`：这个服务这时在工具目录里的工具名（`mcp__<服务名>__<工具名>`），起好列完的、照目录缓存先登记的都算；声明了资源的另有 `mcp__<服务名>__list_resources`、`mcp__<服务名>__read_resource` 两件（施工 X-41）；还不知道的（没起过、没缓存）没有这一格。这一次列出来却没登记的带 `skipped: [{"tool", "why"}]`（提示词是 `{"prompt", "why"}`），照原名排：`tool` 是服务那边的原名（没写名字的是空的），`why` 是 `name`（没写名字）、`parameters`（参数格式不是对象）、`name_taken`（截到 64 个字以后撞了，或者别的工具已经叫这个）；都登记上了的、照目录缓存先登记还没列过的没有这一格，`miyu check` 照它报 `mcp_tool_skipped`。`on` 是这个服务的全局开关（`features.mcp-<服务名>`，见 `feature.set`），没写的是 `true`。浏览器登录过的（存着 `mcp.login` 换来的令牌）带 `logged_in: true`，没登录的、`mcp.logout` 以后的没有这一格（施工 X-42 下再补）：头照它把「登录」换成「登出」；只看存没存着，令牌不经协议交出去。编辑时要先填上原来写的：`env`（值是字，或者 `{"secret": …}`、`{"env": …}`；没写的没有这一格）、`timeout_ms`（不是默认的才带），网址的服务另有 `bearer`、`headers`（写法同 `env`）。这几格的字照原样给（本来就明文写在系统配置里），头显示时遮住，编辑框里再给全；要藏的值该写成密钥的引用；`command` 是程序加参数，交回 `mcp.set` 时拆成 `command`、`args`，`need_load` 改用 `tool.set_loading`。

**`mcp.restart`**：参数 `name`。回应是那一项。**出错**：`unknown_server`。

**`mcp.set`**：在系统配置里加上一个服务，或者整条换掉同名的那一个。

| 名字 | 类型 | 说明 |
|---|---|---|
| `name` | 字符串，必写 | 服务名：小写字母开头，只有小写字母、数字、`-`，最多 32 个 |
| `command` | 字符串 | 程序；和 `url` 写一个 |
| `args` | 字符串数组 | 程序后面的参数；要有 `command` |
| `env` | 对象 | 环境变量：值是字，或者 `{"secret": …}`、`{"env": …}`；跟着 `command` |
| `url` | 字符串 | 网址的服务：`https://` 开头，本机的可以是 `http://`；和 `command` 写一个 |
| `bearer` | 字或对象 | 令牌，发成 `Authorization: Bearer <值>`；写法同 `env` 的值；跟着 `url` |
| `headers` | 对象 | 别的请求头，值的写法同 `env` 的；`Authorization`、`Mcp-Session-Id`、`MCP-Protocol-Version`、`Mcp-Method`、`Mcp-Name` 留给核心；跟着 `url` |
| `venues` | 字符串数组 | 给哪种会话：`local`、`private`、`group`；不写是只给本机的会话 |
| `timeout_ms` | 整数 | 一次调用等多久，1000 到 3600000；不写是 600000 |
| `need_load` | 布尔 | 退役（施工 FL-1，工具一律全量）：照旧收，不看、不写进服务表 |
| `clicall` | 布尔 | 这个服务的工具 AI 在命令里用 `miyu call` 能不能调；不写的照原来写的留着，原来没写的能 |

没交的格子不写：整条换掉时原来写着、这次没交的也没了。写之前照读服务表的规矩查，写法同 `config.set`（带版本，不覆盖同时的手改）。回应 `{"name"}`；服务随后照新写的换，状态变了推 `mcp.changed`。**出错**：`mcp_invalid`（写错了，`data.problem` 说哪里不对：`command`、`url` 都没写、两样混写的也是）；`bad_params`（交了 `args` 没交 `command`）；`server_from_package`（系统配置里没写这个名字、有包带着同名服务：随包来的不经它改）；`config_file_broken`（系统配置读不进来，或者 `mcp` 写成了放不下的样子）；`config_conflict`。

**`mcp.remove`**：参数 `name`。从系统配置里删掉这个服务（连同它的子表），回应 `{}`。包里被它盖住的同名服务照包的开关起回来。**出错**：`unknown_server`；`config_file_broken`；`config_conflict`。

## `mcp.login`

网址的 MCP 服务回 `401` 要浏览器登录的（`reason` 是 `needs_login`，这张表自己没写令牌）：拿授权页，人在浏览器里点同意，核心换令牌、存进密钥文件（`mcp-oauth-<服务名>`，值不经协议交出去），服务自己连上（`mcp.changed`、软件包列表的推送照常）。只给人，远程连进来的也行（施工 X-42 下补）：核心只交回网址，头在它自己那边打开；回调听核心那台机器的 `127.0.0.1`，浏览器不在那台机器上的照 `mcp.login_finish` 贴回来。

**参数**：`service`（服务名：系统配置里的，或者包里带的）。

**回应**：`{"url"}`：授权页的地址，头打开它。核心在本机 `127.0.0.1` 上等回调，最多 10 分钟；同一个服务又登录的，上一次在等的停掉。

**出错**：`unknown_server`：没有这个服务；`bad_params`：不是网址的服务，或者这张表自己写了令牌（`bearer`、`headers` 里的 `Authorization`）；`oauth_unregistered`：没有客户端、授权服务器又不认动态注册，要人去服务那边注册、把 `client_id` 写进这张表；`oauth_failed`：找不到、用不了授权服务器，或者它拒了，`data.why` 说为什么（授权服务器的原话截过）。

登录以后（施工 X-42 下）：令牌快过期的、服务回 `401` 的，核心自己拿 `refresh_token` 续，续期写回密钥不重起服务；续不成的、服务说范围不够（`403 insufficient_scope`）的，服务停在 `needs_login`，再 `mcp.login` 一次（要的范围是以前的加上这一次要的）。

## `mcp.login_finish`、`mcp.logout`

**`mcp.login_finish`**（施工 X-42 下）：浏览器不在核心那台机器上的（网页从别的机器连进来），授权完浏览器跳到打不开的 `http://127.0.0.1:<端口>/callback?…`，把地址栏里那一整段交回来。参数 `service`、`url`。和本机收到的回调同一处判 `state`、`iss`。只给人，远程的也行。**回应**：`{}`：令牌换到、存好了，服务自己连上。**出错**：`not_waiting`：这个服务没有在等的登录，先 `mcp.login`；`oauth_failed`：对不上（那一次登录照旧等着，可以再贴），或者换令牌被拒。

**`mcp.logout`**（施工 X-42 下）：删掉存着的那一份（令牌、客户端、回调的端口），在等的那一次登录一起取消；服务重起、停在 `needs_login`。不吊销授权服务器那边的令牌。参数 `service`。只给人，远程的也行。**回应**：`{}`。**出错**：`not_logged_in`：没登录过。

## `skill.list`

**参数**：没有。**回应**：`{"skills": [...], "dir"}`（`dir` 是你自己放技能的文件夹，还没建的也照写，家目录下的写成 `~/…`），核心这一刻用得上的技能，照 `name` 排；同名的几处都有的，只列核心实际用的那一份（自己放的赢；包之间出厂的先于你装的，同一层照编号）。一项：

| 名字 | 说明 |
|---|---|
| `name` | 技能名 |
| `display_name`、`summary` | 给人看的名字、一句说明，照连接的语言挑（这种语言没写的照英文、中文、日文的先后）；都没写的没有这一格 |
| `from` | 从哪来：`home`（你自己放的，`skills/`）、`shipped`（出厂的包带的）、`package`（你装的包带的）、`agents`（配置 `skills.external.enabled` 开着时，`skills.external.dirs` 里列的别的目录里的，出厂是 `~/.agents/skills`，Codex 这些别的客户端也认这个目录；关着的不列） |
| `dir` | `from` 是 `agents` 的：它在哪个目录，照配置里写的原样（`~` 不展开） |
| `package` | 随包来的：包的编号 |
| `path` | `SKILL.md` 在哪：家目录下的写成 `~/…` |
| `on` | 全局开关（`features.skill-<名字>`，见 `feature.set`）：没写的是 `true` |
| `warnings` | 照收了、但不合格式的地方：`[{"code", "message"}]`，`code` 是 `skill_name_mismatch`（名字和文件夹不同名，照写的名字用）、`skill_name_from_folder`（名字没写或写错，照文件夹的名字用）、`skill_yaml_lenient`（开头不合 YAML，照宽松读法读成）、`skill_description_cut`（说明超过 1024 个字被截了）、`skill_compatibility`、`skill_metadata`（这两格写得不合格式）之一，`message` 照连接的语言；`miyu check` 报同样的码。没有的不带这一格 |

## `skill.remove`

**参数**：`names`，技能名的数组，至少一个、不能重复。**回应**：`{"trash": [...]}`，每个技能的文件夹在系统回收站里的位置，照 `names` 的先后。只删你自己放的（`from` 是 `home`，`skills/<文件夹>/` 整个挪进回收站）；它盖住了包里同名的，删了包里的那一份重新列出来。只给人用，扩展进程调了回 `local_only`。

**出错**：没写名字、名字重复：`bad_params`。没有这个技能：`unknown_skill`；随包来的：`skill_from_package`；别的目录里的（`from` 是 `agents`）：`skill_shared`（三种都带 `data.name`，几个名字里有一个不行就一个都不删）。挪不进回收站：`trash_failed`。

## `script.list`

**参数**：没有。**回应**：`{"scripts": [...], "dir"}`（`dir` 是你自己放脚本的文件夹，写法同 `skill.list`），这一刻登记成工具的脚本，照 `tool` 排。一项：

| 名字 | 说明 |
|---|---|
| `tool` | 工具名 |
| `display_name`、`description` | 给人看的名字、一句说明（脚本开头的 `Display name`、`显示名称`、`描述`），照连接的语言挑，挑法同 `skill.list`；都没写的没有这一格 |
| `from`、`package` | 同 `skill.list`：`home` 是你自己放的（`scripts/`） |
| `path` | 脚本文件在哪，写法同 `skill.list` |
| `timeout` | 一次等多少秒 |
| `on` | 全局开关（`features.script-<工具名>`）：没写的是 `true` |

## `tool.set_loading`、`feature.set`

技能、脚本、MCP 服务那几页改的。回应 `{}`。

**`tool.set_loading`**：退役（施工 FL-1，2026-10-27 项目主人定：工具一律全量，没有先加载了）。协议 1 冻结了这个方法：照旧收、照旧查参数，什么都不改，配置一个字不写。以前写进系统配置的 `loading.tools.<工具名>`、`loading.mcp.<服务名>` 照退役的键读，不报错、不起作用。

| 名字 | 类型 | 说明 |
|---|---|---|
| `tool` | 字符串 | 工具名：1 到 128 个字符，没有 `.` 和控制字符 |
| `server` | 字符串 | MCP 服务名 |
| `need_load` | 布尔或 `null`，必写 | 照旧要写，不看 |

`tool`、`server` 正好写一个。

**`feature.set`**：改一个技能、脚本、MCP 服务的全局开关（`features.<功能编号>`），写系统配置，写法同 `config.set` 改一项（带版本，不覆盖同时的手改，写成了推 `config.changed`），从下一个回合开始生效。`unlisted` 是 `on` 的预设照它；预设里单独写了开或关的照预设（`preset.get` 每个功能的 `by`）。`unlisted` 是 `off` 的预设不照它：没写的照旧关着。

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
| `prefix` | `"keep"` 或 `"compact"` | 开新一轮的命令（模板、MCP 提示词、`/goal` 设定和继续）前缀要变时人选的，同 `session.send`「前缀要变」 |

**回应**：`{"command", "events", "said"}`：正名（别名换成正名）、这一次追加的事件、回执那一句（照连接的语言，原样给人看）。打开整页的命令回 `{"command", "opens": {"package", "widget"}, "said": ""}`，头打开那一页。

**核心认的命令**：

| 命令 | 做什么 |
|---|---|
| `/clear`（`/reset`） | 清空上下文和待办，同 `session.clear` |
| `/stop` | 打断这一轮，停掉这个会话派出去的后台命令和子代理，和正在跑的整理 |
| `/workspace <路径>` | 换工作目录；不带路径的说现在在哪 |
| `/context <数值>`、`/context unset`（`off` 也行）、`/context` | 设这个会话的上下文上限（`128000`、`128k`、`1m` 都行），同 `session.configure` 的 `context`；去掉；说这时的上限和压缩线 |
| `/todo`、`/todo clear` | 看待办、清空待办 |
| `/remember <话>` | 记一条记忆 |
| `/dream`、`/dream stop` | 现在就整理记忆（在后台答）；停下正在跑的整理，合完的留着 |
| `/clear-memory`（`/reset-memory`） | 清掉这个会话记下的记忆 |
| `/goal` 系列 | 看、设、停、继续、改、清掉长期目标 |

不是这些的，依次照这几处找：自己放的命令模板（`commands/<名字>.md`）、扩展声明的命令（核心转给扩展，`command.call`）、包带的命令模板。模板展开成一句话，当这个人说的发出去：回应的 `events` 是那一句，`said` 是空的。

名字是 `<服务名>:<提示词名>`（恰好一个 `:`）的是 MCP 服务的提示词：核心照 `prompts/get` 去取，人跟的字照空白切开（成对的英文双引号中间算一个，引号去掉）、照提示词声明的参数先后对上，最后一个参数拿剩下的整段；取回来的字、图并成这个人说的一句发出去，同模板。只在本机的会话里认，跟着服务的功能 `mcp-<服务名>` 开关。

**谁能用**：本机的会话都能用；场所会话里是主人和管理的人。系统配置 `commands.<名字>.access` 能改成 `anyone`（谁都行）或 `off`（停用）。`/workspace` 只给主人本人。

同一个 `id` 再发只算一次：参数一样的拿到头一次的回应（`events`、`said` 都是当时的），命令不再执行；参数换了的拒 `id_reused`，命令不执行。核心记最近 1024 条，重启以后不记得：命令会再执行一遍。被拒的不记，再发重新判。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 不以 `/` 开头；`cwd` 不是绝对的；命令后面跟的字不对；`/context` 设得太小（`data.field` 是 `context`，`data.min` 是最少要多少） |
| `unknown_command` | 没有这个命令 |
| `command_not_allowed` | 这个人不能用，或者停用了、预设没开，`data.why` 是给人看的一句 |
| `owner_only` | 只有主人本人能用 |
| `command_unavailable`、`command_failed` | 扩展的命令：扩展没连上、一分钟没答；扩展答了错（`data.why` 照原话）、回答里没有 `said`（`data.why` 照连接的语言）。MCP 的提示词：服务没在跑、没列出这一个、不是本机的会话、到时没答（照服务的 `timeout_ms`，最多一分钟）；缺必填的参数（`data.why` 照连接的语言说缺哪几个）、服务回错（`data.why` 照原话） |
| `memory_unavailable`、`goal_unavailable` | 这个会话没有记忆、没有长期目标，`data.why` |
| 命令自己的 | `/clear` 的 `turn_running`、`nothing_to_clear`，`/workspace` 的 `path_unreadable`……，同对应的方法 |

## `command.catalog`

**参数**：`session`（可不写）：写了只列这个连接在这个会话里打了不会被拒的；不写的列全部，跟着包的命令（`/loop`、`/goal`、`/todo`）照你的默认预设筛，默认预设没开那个包的不列。

**回应**：`{"commands": [{"name", "aliases", "summary", "argument"?, "usage"?, "opens"?}…], "skills": [{"name", "summary"}…]}`，照名字排。技能另列在 `skills`：在输入框里用 `$技能名` 点名，发 `session.send` 时写进 `skills`（见 [会话](sessions.md)），一句话里能点几个；`user-invocable: false` 的不列，写了会话的只列预设开着的。在跑的 MCP 服务的提示词也在里面：名字 `<服务名>:<提示词名>`，`summary` 是服务写的 `title`（没有的照 `description`，不分语言），`argument` 照声明的参数拼（必填的 `<名字>`、可选的 `[名字]`）。`argument` 是名字后面跟什么的提示，`usage` 是有几种写法的命令一行一种（`{"form", "summary"}`），`opens` 是打开整页的命令。`summary`、`argument` 照连接的语言。

头自己的命令（`/help` 这种）头自己拼进菜单。

## `check`

**参数**：`cwd`（照它找项目配置）、`file`（只查这一份）。

**回应**：`{"problems": [...]}`，一处一项：`kind`（`config`、`secrets`、`persona`、`preset`、`package`、`script`、`skill`、`slash`、`mcp`、`schedule`（唤醒表里读不懂的行，`schedule_unreadable`）；包自己的检查照它报的原样交，例如 `venue`）、`file`、`code`、`level`、`message`，有行列的带 `line`、`column`，配置的另带 `key`、`got`、`suggest`、`using`，包自己的检查可以另带 `key`、`rule`、`source`。

不写 `file` 的全查：系统配置、`cwd` 的项目配置、密钥文件、每一层的人格和预设、软件包清单、包自己的检查；读不成的、被盖住的、链接指到数据根里不认的地方的脚本、技能、命令模板，撞名没登记上的脚本，和自带命令同名的命令模板，MCP 服务列出来却没登记上的工具、提示词；装好的文件改了、少了。另带 `embedding`：语义模型的状态，不是问题（`model`，用不上的 `model` 是 `null`、带 `why`；后台在补的 `filling`；`memory`、`turns` 每个人格一项 `{persona, have, total}`；`kb` 每个库一项 `{id, name, have, total}`）。

**出错**：`unknown_file`：`file` 不是 Miyu 读的那几种。
