# 软件包与命令：`package.*`、`extension.*`、`mcp.*`、`skill.list`、`skill.remove`、`script.list`、`feature.set`、`command.*`、`check`

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

这一页是管理软件包、扩展进程、系统配置里的 MCP 服务的方法，还有列出技能和脚本、修改工具的加载方式和技能、脚本、MCP 服务的全局开关、执行斜杠命令、检查文件有没有写错的方法。扩展自己调用的方法、核心发给扩展的调用，详细的见 [扩展](extensions.md)。

| 方法 | 做什么 |
|---|---|
| [`package.list`](#packagelist) | 列出软件包 |
| [`package.info`、`package.files`、`package.owns`、`package.check`](#packageinfopackagefilespackageownspackagecheck) | 查看一个软件包的信息和文件；查一个路径属于哪个软件包；检查安装好的文件有没有被修改 |
| [`package.install`、`package.remove`](#packageinstallpackageremove) | 安装、卸载软件包 |
| [`package.enable`、`package.disable`、`package.restart`](#packageenablepackagedisablepackagerestart) | 开启、关闭、重启软件包 |
| [`package.sync`](#packagesync) | 从软件源获取索引 |
| [`package.search`](#packagesearch) | 在软件源里搜索 |
| [`package.clean`](#packageclean) | 清除下载过的包文件 |
| [`package.upgrades`、`package.upgrade`](#packageupgradespackageupgrade) | 列出可以升级的软件包；升级 |
| [`package.file`、`package.call`、`package.page`](#packagefilepackagecallpackagepage) | 读取后台页的文件；调用软件包的程序登记的方法；不通过网页界面打开后台页 |
| [`extension.status`、`extension.enable`、`extension.disable`、`extension.restart`](#extensionstatusextensionenableextensiondisableextensionrestart) | 查看扩展进程的状态，开关扩展进程 |
| [`driver.check`](#drivercheck) | 检查扩展提供的模型驱动编码是否稳定（会重启扩展） |
| [`mcp.list`、`mcp.restart`、`mcp.set`、`mcp.remove`](#mcplistmcprestartmcpsetmcpremove) | 管理系统配置里的 MCP 服务：列出、重启、添加或替换、删除 |
| [`mcp.login`](#mcplogin) | 网址形式的 MCP 服务需要浏览器登录时，获取授权页 |
| [`mcp.login_finish`、`mcp.logout`](#mcplogin_finishmcplogout) | 浏览器不在核心所在的机器上时，把浏览器跳转到的地址贴回来；登出 |
| [`skill.list`](#skilllist) | 列出可以使用的技能 |
| [`skill.remove`](#skillremove) | 删除你自己放的技能 |
| [`script.list`](#scriptlist) | 列出已经登记的脚本 |
| [`feature.set`](#featureset) | 修改技能、脚本、MCP 服务的全局开关 |
| [`command.run`](#commandrun) | 执行一条斜杠命令 |
| [`command.catalog`](#commandcatalog) | 列出可以使用的斜杠命令 |
| [`check`](#check) | 检查配置、人格、预设、软件包有没有写错 |

**谁能调用**：`package.install`、`package.remove`、`package.enable`、`package.disable`、`package.restart`、`package.sync`、`package.search`、`package.clean`、`package.upgrades`、`package.upgrade`、`package.file`、`package.page`、`extension.*`、`mcp.*`、`feature.set` 只给人调用，扩展调用时返回 `local_only`；带 `path` 的 `package.info` 也只给人调用（因为它查看的是本机任意一处的文件夹，和 `package.install` 相同）。其他方法人和扩展都能调用；扩展调用 `package.call` 时只能调用到自己（见下文）。

**三层**：软件包分为三层：出厂的（随 Miyu 一起安装，`shipped`）；系统加装的（由系统的包管理器另外安装，`distro`：Linux 上是 `$XDG_DATA_DIRS` 里每一处的 `miyu/packages/`，没有设置这个变量时使用 `/usr/local/share:/usr/share`，使用 Linuxbrew 时要把它自己的 `share` 写进 `XDG_DATA_DIRS`；macOS 上是 Homebrew 前缀下的 `share/miyu/packages/`；Windows 上没有这一层；设置了 `MIYU_DISTRO_PACKAGES` 时只查看它指定的位置）；安装的（数据根的 `opt/`，`home`）。同一个编号在几层里都有时，安装的覆盖系统加装的，系统加装的覆盖出厂的；被覆盖的那一份不加载，也不算错误。在安装的那一层安装一个同编号的软件包就是升级，会替换原来的那一份；卸载出厂的、系统加装的软件包时，核心在 `etc/removed/` 里记一笔（一个名为 `<编号>` 的空文件），不改动软件包的文件，以后还可以装回来。

## `package.list`

**参数**：没有。**回应**：`{"packages": [...], "groups": [...]}`。软件包按编号排列；组是出厂组表里的组，每个组是 `{"id", "name", "summary"?, "packages"}`，使用连接的语言；`packages` 是组里已经安装的软件包，按用途的先后排列（先后由出厂组表决定，组表里没有写到的排在后面、按编号排列），头（连接核心的界面程序，例如终端界面、网页界面）根据它把组里的软件包收在一起，并按这个先后排列。每一项的字段如下：

| 名字 | 说明 |
|---|---|
| `package`、`layer` | 编号；所在的层，是 `shipped`、`distro` 或 `home` |
| `shadowed` | 只有被上面一层覆盖的那一份才有这一格：`{"by", "version", "over", "newer"}`，分别是覆盖它的那一层、这一份的版本、正在使用的那一份的版本、这一份是否更新。这不算错误，所以没有 `code` |
| `kind` | `builtin`（编译进核心的）、`process`（扩展）、`worker`（按需启动的小程序）、`ui`（界面）、`mascot`（吉祥物）、`theme`（外观：终端主题、网页主题，见下面 `theme` 一格）、`bundle`（只带 MCP 服务、技能、脚本这类内容，不带程序） |
| `name`、`summary`、`icon` | 名字、一句说明、Lucide 图标名 |
| `name_en` | 英文名：只有连接的语言不是英文、并且清单写了英文名时才有，头用它一起搜索；`package.info` 也相同 |
| `logo` | 软件包自带的图标的版本：只有清单写了 `[package] logo`、并且图片符合规矩时才有，图片用 `package.logo` 读取 |
| `author`、`upstream` | 作者、上游源码仓库（以 `https://` 开头）；清单没有写时没有这一格 |
| `installed_at` | 安装的时刻；出厂的软件包没有这一格。头根据它排列「别的按安装先后」 |
| `from` | 从哪里来：`shipped`（出厂）、`official`（从官方软件源安装的）、`local`（从本机文件夹、包文件安装的）；以后有了社区软件源，还会多一种 `community` |
| `source` | 是怎么来的，取值和安装之前预览的 `source` 相同：`official`（官方源里官方收录的）、`community`（官方源里社区收录的，或者配置里自己加的源里的）、`file`（从本机文件夹、包文件安装的）、`shipped`（出厂）、`distro`（由系统的包管理器安装的）；早先安装、无法推断来源的是 `unknown`，不当作官方的 |
| `feed` | 只有从软件源安装的才有：来自哪个源，官方的是 `official`，配置里加的源写它的名字 |
| `version`、`protocol`、`state` | 版本、使用的协议版本、状态目录的真实位置 |
| `groups` | 在哪几个组里（只有出厂的内置包有，`basesystem` 是基础系统）；不在任何组里时没有这一格 |
| `features` | 软件包里的功能（预设根据它们开关） |
| `status`、`enabled` | 有进程的软件包才有：是否正在运行、是否开启 |
| `connection` | 平台接入的软件包（清单写了 `[connection]`）才有：`{"platform", "platforms"?, "owner_builtin"?}`。`platform` 是接入的平台，接入几个平台时是第一个，此时另外带 `platforms`（全部平台，按清单写的先后）。头根据 `platform` 在这个软件包的信息页上绘制「你在这个平台上的号」，写进主人对应表 `external.bindings`。带 `owner_builtin: true` 的平台（现在只有 `voice`）不需要主人在对应表里填号，头不绘制填号那一块；其他平台不带这一格 |
| `page` | 有后台页的软件包才有，头根据它列出「软件后台」 |
| `page_server` | 后台页是软件包的程序自己启动的网页服务（清单 `[page] server = true`）：头向它的 `status` 询问 `web` 端口，带一张 `login.ticket` 跳转过去，不把它放进框里 |
| `tray` | 有后台页、并且清单写了 `[tray]` 的软件包才有：`{"name"}`，是托盘菜单里那一行的文字，使用连接的语言；点击它会打开这个软件包的后台页。托盘只列出开启着的软件包 |
| `theme` | 外观包带了什么：`{"tui"?: {"file"}, "web"?: {"dir"}}`，`tui` 是终端主题文件，`web` 是网页主题目录（一个普通的网页包），至少有其中一样；路径都相对于软件包目录，文件用 `package.file` 读取 |
| `mcp` | 软件包带的 MCP 服务：`name`、`command`（网址形式的服务是 `url`）、`venues`、`state`、`failures`……；`on`、`skipped` 和 `mcp.list` 的一项相同。不替主人工作的扩展（在场所里替外人工作的平台桥）拿到的每个服务只有 `name`、`state`（停下的服务带 `reason`）；软件包这一项本身也没有 `state`（状态目录）、`pages_dir`、`command`、`commands`、`process`、`worker`、`check`、`problem`、`line` |
| `widgets` | 声明的界面块：`id`、`place`、`title`、`scope`、`command` |
| `command`、`commands` | 给 `miyu` 添加的子命令：`command` 是第一个 `{"name", "program", "about"}`，`commands` 是全部子命令，按清单写的先后排列（一个软件包可以添加几个子命令） |
| `contents` | 给哪几个软件包带了内容（清单里 `[content.<包>]` 的编号）；没有带内容时没有这一格 |
| `provides` | 带了哪几样内容，写法和 `package.search` 每一行的 `provides` 相同：从种类映射到名字，种类有 `command`（子命令）、`features`（清单写明的功能）、`mcp`（MCP 服务）、`slash`（斜杠命令）、`content`（带了内容的软件包）、`skills`、`scripts`（脚本的文件名）、`personas`、`presets`、`knowledge`（知识库的编号：软件包文件夹 `kb/` 下编号写法正确的目录），每一种按名字排列，没有带的种类不写，什么都没有带时是 `{}`。出厂的、系统加装的、安装的软件包，以及卸载了的出厂包都有这一格，由核心根据软件包文件夹当场计算；头根据它在「已安装」里标出「技能」「脚本」 |
| `removed` | 表示这是一个已经卸载的出厂包 |

写错的、名字冲突的、无法读取的软件包，这一项只有 `package`、`layer`、`code`、`problem`（使用连接的语言）和 `line`。要一直获得最新的列表，就订阅 `packages` 流。

## `package.info`、`package.files`、`package.owns`、`package.check`

| 方法 | 参数 | 回应 |
|---|---|---|
| `package.logo` | `package`、`from`? | `{"package", "logo", "media_type", "data"}`：软件包自带的图标，`logo` 是版本（内容的 SHA-256 的前 16 位，和 `package.list` 的那一格相同），`data` 是 base64，`media_type` 是 `image/png` 或 `image/webp`；清单没有写图标，或者图标不符合规矩（不存在、不是 PNG / WebP、超过 256 KiB、宽或高超过 512）时，这三格都是 `null`。另外可以写 `from`（软件源的名字，和 `package.search` 每一行的 `source` 相同）：读取那个源里的软件包的图标，没有安装的软件包也可以读取；源无法识别时返回 `unknown_source`，索引里没有这个软件包时返回 `unknown_package`，下载失败时返回 `download_failed` |
| `package.info` | `package`、`source`? | `source` 写 `true` 时只查软件源，写 `false` 时只查已安装的；不写时先查已安装的，没有安装再查软件源。已安装的软件包返回 `{"package", "layer", "from", "files", "size", "version"?, "author"?, "upstream"?, "license"?, "installed"?, "path"?, "source", "feed"?, "name"?, "summary"?, "depends"?, "recommends"?, "contains"?, "capabilities"?, "required_by"?, "origin"?, "tools"?}`：其中有从哪里来（`from`、`source`、`feed` 和 `package.list` 相同）、几个文件、多少字节、作者、上游、许可证（清单 `[package] license` 的原样，清单没有写时没有这一格）、安装的时刻、安装时的本机路径（`path`，只给主人）；名字、说明、依赖、推荐、带了什么（`contains`，和安装之前预览的「包含」相同；它和软件源那一份的 `provides` 不是同一种形状）、需要的能力、已安装的软件包里依赖它的、来源仓库；`tools` 是安装以后模型会多出的工具 `[{"name", "title"?, "from"}]`：`name` 是 AI 看到的工具名，`title` 是显示名 `{"zh"?, "en"?}`，`from` 是 `script` 或 `extension`（MCP 的工具、要运行起来才知道的工具不列出）。没有安装的软件包按软件源返回 `{"from": "source", "source", "package", "version", "depends", "recommends", "provides", "capabilities", "origin", "official", "name"?, "summary"?, "author"?, "upstream"?, "license"?, "download"?, "installed_size"?, "tools"?, "hosted"?, "unsupported"?, "stale"?}`：`source` 是源的名字，`unsupported` 为 `true` 时表示不支持本机系统（安装会被拒绝，返回 `wrong_target`），`provides`、`license` 是索引里的原样，`download` 是本机要下载的那份文件有多少字节，`installed_size` 是安装好以后有多大，`tools` 来自索引、写法和已安装的相同，`hosted` 是模型驱动会让供应商自己运行的工具，来自索引，写法和安装之前预览的相同 |
| `package.info` | `path`（和 `package` 二选一，不接受 `source`） | 查看本机一个软件包文件夹（或者其中的 `package.toml`）、或者一个包文件 `.miyu` 的绝对路径，和 pacman 的 `-Qip` 类似：什么都不安装，只把真正安装之前要查的那几样查一遍。回应的形状和已安装的那一份相同：`{"from": "file", "path", "package", "files", "size", "version"?, "name"?, "summary"?, "author"?, "upstream"?, "license"?, "depends"?, "recommends"?, "contains"?, "capabilities"?, "skipped"?, "replaces"?}`：`path` 是写的原样（包文件的情况下是那个文件，不是解开以后的暂存处），`files`、`size` 是包里有几个文件、多少字节，`contains`、`capabilities`、`skipped`（无法读取的技能、脚本、斜杠命令模板 `[{"file", "why"}]`）、`replaces` 和 `package.install` 预览的那几格相同；没有 `layer`、`installed`、`required_by`、`origin` |
| `package.files` | `package` | `{"dir", "files": [{"path", "sha256", "size"}…]}`：`path` 相对于 `dir` |
| `package.owns` | `path`（绝对路径） | `{"package", "layer", "path", "recorded"}`；哪个软件包都不包含这个路径时返回 `{"package": null}` |
| `package.check` | `package`（可以不写） | `{"packages": [{"package", "modified", "missing", "extra"}…]}`：列出安装那一层的软件包里，安装好以后被修改、缺少、多出的文件。系统加装的软件包另外有一项，多一个 `"layer": "distro"`，不比较哈希：清单无法读取时多一个 `manifest`，清单写的程序不在 `bin/` 里时列进 `missing` |

**出错**：`unknown_package`（`package.info` 在两处都没有找到）；`source_unreachable`（`package.info` 只查软件源，但是软件源无法连接，本机也没有索引）；`bad_params`（`owns` 的路径是相对路径；`info` 的 `package`、`path` 都写了或者都没写，带 `path` 时又写了 `source`，或者路径是相对路径）。带 `path` 的 `info` 还会按照 `package.install` 的出错拒绝：`path_unreadable`、`package_invalid`、`package_conflict`、`wrong_target`、`package_exists`、`package_file_bad`、`depends_missing`，详细的见下一节。

## `package.install`、`package.remove`

| 方法 | 参数 | 回应 |
|---|---|---|
| `package.install` | `path`：本机一个软件包文件夹的绝对路径（文件夹名就是编号），或者其中那份 `package.toml` 的路径，或者一个包文件 `.miyu` 的路径 | 和 `package.list` 的一项相同；包里的工具说明有不是各家模型供应商都收的参数写法时，多一个 `warnings`：`[{"code": "tool_not_portable", "level": "warning", "message"}]`，一份工具说明一条，软件包照样安装（见 [写扩展](../dev/extensions.md)） |
| `package.install` | `package`：如果是出厂的、系统加装的编号，就是把卸载的软件包装回来；其他编号和 `source` 相同，从软件源安装 | 同上 |
| `package.install` | `source`：从软件源安装这个编号的软件包（缺少的依赖一起安装，先安装依赖；依赖写了版本下限时按下限挑选版本，已安装的版本不够时一起升级）；先全部下载完，再逐个核对，再逐个安装，每一个阶段都向这个连接推送 `package.progress`（见下文）。`from`（可以不写）：从哪个软件源安装（`official` 或者配置里加的源的名字），不写时按先后查找 | 同上；一起安装了依赖时多一个 `with` |
| `package.install` | `package`：如果是组名（`package.list` 顶层的 `groups`，例如 `basesystem`），就是把组里卸载了的出厂包一起装回来 | `{"group", "packages": [<同 `package.list` 的一项>…]}`，一个都没有卸载过时是空的 |
| `package.remove` | `package`；`purge`（可以不写，默认 `false`）：为 `true` 时连配置目录 `etc/<编号>/`、系统配置里它的设置项一起删除；不写时保留它们，重新安装同编号的软件包时仍然使用（和 pacman 相同）。已经卸载、但还留着这几样的编号也接受 `purge: true`：此时只删除留下的内容；不带 `purge` 时仍然返回 `unknown_package` | `{"package", "removed": true, "layer"}`（对没有安装的编号使用 `purge` 时不带 `layer`） |
| `package.remove` | `package`：如果是组名，就是把组里已安装的软件包一起卸载；组外已安装的软件包依赖组里的软件包时，整个请求被拒绝（`package_depended`，`data.by` 是这些软件包）；`purge` 同上，会同样传给组里的每个软件包 | `{"group", "removed": [<编号>…]}` |

这两个方法都可以带 `preview: true`：只预览，什么都不修改，拒绝的情形和真正执行时相同。`install` 的预览返回 `package`、`version`、要替换时的 `replaces: {"version"}`、装回出厂的或系统加装的软件包时的 `restores: true`，以及带了什么（`program`、`mcp`、`drivers`（提供的模型驱动名）、`hosted`（驱动会让供应商自己运行的工具：`{"<驱动名>": ["<工具名>", …]}`，来自清单 `[[driver]]` 的 `hosted`，没有写的驱动不列出，一个驱动都没有写时不带这一格）、`command`（第一个子命令名）、`commands`（全部子命令名）、`page`、`mascot`、`theme`、`connection`、`settings`、`skills`、`scripts`、`slash`、`personas`、`presets`、`knowledge`（知识库的编号）、`capabilities`、`files`、`size`）；`remove` 的预览返回 `package`、`layer`、`version`、`files`、`size`、`settings`（要一并删除的配置键：只有带 `purge` 时才列出，不带 `purge` 时是 `[]`）、`state`（会一并删除的状态目录：`{"path", "size"}`，`path` 是 `var/lib/<编号>/` 的位置，`size` 是其中的文件一共有多少字节，不跟进符号链接；没有状态目录时不写这一格）、`config`（配置目录和设置项：`{"path"?, "kept", "settings"?}`，`path` 是 `etc/<编号>/` 的位置，目录不存在或者为空时不写；`settings` 是系统配置里写了几项它的设置，为 0 时不写；`kept` 为 `true` 表示两样都保留，为 `false` 表示都删除（写了 `purge: true`）；`path`、`settings` 都没有时不写这一格）。卸载了的软件包留在配置里的设置项不生效，`check` 不把它们报成无法识别的键，而是每个软件包报一条 `settings_left`。

从软件源安装的预览另外有 `source`（从哪里来：`official` 是官方源里官方收录的，`community` 是官方源里社区收录的、或者配置里自己加的源里的）、`feed`（源的名字，官方的是 `official`）、`official`、`origin`、`download`（要下载的字节数，包括依赖）、`with`（一起安装的依赖 `[{"package", "version", "source", "feed", "capabilities"?}]`）、`recommends`（要安装的软件包推荐的、还没有安装的软件包，已经去重：`[{"package", "version", "source", "feed", "capabilities"?, "download", "installed_size"?}]`，没有时是空的）、`installed`（要安装的全部软件包安装好以后一共占多少字节，不计算推荐的软件包；有一个软件包的大小索引里没有写时，不写这一格）；`capabilities` 和从软件包文件夹安装时的那一格相同（`{"id", "name", "summary"}`）；`hosted` 来自索引，写法和从软件包文件夹安装时的那一格相同，索引里没有写时没有这一格。

关于来源和需要的能力：预览的 `source` 一共有五种：从软件源安装的是 `official`、`community`，从软件包文件夹、包文件安装的是 `file`，装回的是 `shipped`、`distro`。能力的每一项如果此刻已经批准过、安装完以后仍然保留，就多一个 `held: true`：只有从软件源安装、并且源和来源仓库都和已安装的那一份相同时才会这样（从本机文件安装、换了源、换了来源仓库时，以前的批准不继续使用）。社区的、本机文件的预览多一个 `notice`：这是要给人看的话，使用连接的语言，一句一个字符串（社区的是 `这是社区软件包，未经官方审核，可能存在风险。`，本机文件的是 `这个软件包来自本机文件，未经官方审核，可能存在风险。`；涉及几个软件包时会写出是哪几个，两种情况都有时各一句）；推荐的软件包里有这种情况时，另外放在 `recommends_notice` 里。头要把它原样输出，「注意：」这样的前缀是头自己的文字。

安装时批准：`package.install`（`path`、`source`）、`package.upgrade` 可以带 `approve`：`{"<编号>": ["<能力>", …]}`，这一次安装的每个软件包各写各的（按预览里列出的内容拼：要安装的软件包和 `with` 里的，升级时是 `upgrades` 和 `with` 里的）。能力名、编号写法不对时返回 `bad_params`；安装的那一份没有声明的能力、不是这一次安装的编号，核心都忽略；不写 `approve` 时安装以后不批准，打开时仍然用 `package.enable`、`extension.enable` 的 `approve` 批准。批准过的能力记进 `extensions.json` 的 `approved`，计算方法是「(源和来源仓库都没有变化时以前批准过的 ∪ 这一次批准的) ∩ 这一份声明的」，所以新版不再需要的能力会被去掉，以后再加回来时需要重新批准。手动开启（`start = "manual"`）的扩展安装好以后打开时不需要再批准，开启着就运行的扩展安装以后当场运行。

真正安装时，核心向这个连接推送 `package.progress`：`{"package", "stage", "index", "count", "done"?, "total"?}`。`stage` 是 `download`（带 `done`、`total` 字节数）、`verify`（核对之前）、`install`（安装之前）；`index`、`count` 表示第几个（从 1 开始数）、一共几个。从软件包文件夹、包文件安装时只推送 `verify`、`install` 各一次（`1`、`1`）。老版本的核心只推送 `{"package", "done", "total"}`，没有 `stage` 时当作 `download`。

安装、卸载当场生效：带进程的软件包按开关启动或停下，它的工具进入或离开工具目录，开着的会话在下一轮换上新的工具。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 两个参数都写了；路径不是绝对路径 |
| `path_unreadable` | 无法读取那个文件夹 |
| `package_invalid` | 清单写错了，详情见 `data.problem`、`data.line`；软件包需要更新的 Miyu 才能使用时，`data.problem` 是 `needs_newer_miyu` |
| `package_conflict` | 软件包带的名字和已安装的其他软件包冲突：子命令、功能、工具、技能、MCP 服务、知识库、斜杠命令、人格、预设、前缀、总线名字（`bus`：一个名字只能由一家提供，一个名字不能一家提供、另一家接入） |
| `not_permitted` | 会话里的 `package` 工具：会话不属于主人（在场所里替外人工作的会话）时，安装、升级、卸载、装回软件包，以及启用扩展都会被拒绝；列出、查看、搜索仍然可以使用 |
| `package_exists` | 和出厂的软件包编号相同 |
| `package_file_bad` | 包文件无法解开；从软件源安装时，包里的编号和索引里那一条对不上 |
| `download_failed` | 从软件源安装时，包文件从哪一处都没有下载成功（大小、哈希对不上也算），`data.why` 是每一处的原因 |
| `wrong_target` | 不支持当前系统：清单写了 `targets`，但其中没有本机这一种；或者写了 `systems`，但其中没有本机的系统。从软件源安装时，核心根据索引就会拒绝，不会下载 |
| `depends_missing` | 依赖的软件包没有安装，或者已安装的版本比清单写的下限低；`data.missing` 是缺少的那几个，按清单写的原样（带下限的写成 `"onebot>=1.2.0"`） |
| `package_depended` | 其他已安装的软件包依赖它，见 `data.by` |
| `unknown_package` | 软件包没有安装，或者没有卸载过 |
| `unknown_source` | `from` 写的软件源不存在（没有添加，或者写错了） |

## `package.sync`

这个方法从软件源获取索引。参数 `force`（可以不写）：为 `true` 时无论本机那份是否够新都重新获取；不写时，如果本机那份不到 24 小时就直接使用。地址按系统配置的 `packages.mirrors`、`packages.source` 的先后尝试，索引用编译进来的官方公钥验证签名，比本机那份旧的索引不接受。

回应 `{"generated", "fetched", "packages", "stale"?}`：分别是索引生成的时刻、获取到的时刻（都是 RFC 3339）、有几个软件包；这一次一处都没有获取成功、使用的是本机那份时，带 `stale`（最后一处的原因，英文）。

其他软件源：系统配置里写了 `url`、`key`（Ed25519 公钥的 base64）的 `packages.sources.<名字>`。核心获取完官方的索引以后再获取它们，各自用各自的公钥验证；回应多一个 `sources: [{"source", "generated", "fetched", "packages", "stale"?}]`，获取失败的那一项是 `{"source", "error"}`。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `source_unreachable` | 一处都没有获取成功，本机也没有索引；`data.why` 是最后一处的原因 |

## `package.search`

这个方法在软件源里搜索。参数 `query`（可以不写）：几个词，用空白分开，每个词都必须出现在编号、名字、说明（哪种语言都算）里，不区分大小写；不写或者为空时列出全部。搜索之前先按 `package.sync` 获取索引（本机那份不到 24 小时时直接使用）。

回应 `{"packages": [...], "generated", "stale"?}`，每一项是 `{"package", "version", "name", "summary", "official", "installed", "blocked", "contents", "provides", "depends", "kind"?, "icon"?, "logo"?, "license"?, "published"?, "downloads"?, "downloads_week"?, "unsupported"?}`：名字、说明按连接的语言挑选（没有这种语言时依次使用英文、中文、日文），都没有时为 `null`；`installed` 是已安装的版本（已安装但没有写版本时是空字符串），没有安装时为 `null`；`official` 表示是不是项目主人自己的仓库；`blocked` 是被拉黑的理由，没有被拉黑时为 `null`；`contents` 是给哪几个软件包带了内容（和 `package.list` 相同），没有带时是空数组；`provides` 是带了哪几样内容：从种类映射到名字，种类有 `command`（子命令）、`features`（功能）、`mcp`（MCP 服务）、`slash`（斜杠命令）、`content`（带了内容的软件包）、`skills`、`scripts`、`personas`、`presets`、`knowledge`（知识库），没有带的种类不写，什么都没有带时是 `{}`；`depends` 是依赖的软件包，按索引的原样：编号，或者带版本下限的 `<编号>>=<版本>`，没有依赖时是空数组；`kind` 是软件包的种类：清单里不写种类，生成索引时根据软件包带了哪几张表识别出来，是 `ui`、`process`、`builtin`、`worker`、`mascot`、`theme`、`bundle` 之一（以后增加了新的种类，也按原样返回），以前生成的索引没有这一格；`source` 是来自哪个软件源（`official` 或者配置里起的名字），其他源里的软件包 `official` 一律是 `false`；`icon` 是 Lucide 图标名，`logo` 是软件包自带图标的版本（和 `package.list` 相同，图片用带 `from` 的 `package.logo` 读取），索引里有时才有；`license` 是清单写的许可证（建议写成 SPDX 表达式，核心不检查），索引里有时才有；`published` 是这一版第一次进入这个软件源的时刻（RFC 3339），`downloads` 是各个版本、各个系统一共被下载了几次，`downloads_week` 是最近 7 天被下载了几次，索引里有、并且写法正确时才有（头根据它们按最新、下载量、趋势排列，没有这几格的排在最后）。不支持本机系统的软件包（清单写了 `systems`、`targets`，但其中没有本机的）默认不列出；配置 `packages.other_systems` 开启时仍然列出，那一行带 `unsupported: true`，安装时仍然会被拒绝，返回 `wrong_target`。出错和 `package.sync` 相同。

## `package.clean`

这个方法清除下载过的、不是已安装那一版的包文件。没有参数，回应 `{"removed", "freed"}`：删除了几个文件、释放了多少字节。

## `package.upgrades`、`package.upgrade`

这两个方法从软件源升级软件包。它们只处理从软件源安装到数据根里的软件包；从文件夹、包文件安装的软件包不处理，出厂的和系统的包管理器安装的（`distro`）软件包也不处理，因为它们由 Miyu 自己的安装方式和系统的包管理器升级。核心不比较版本的大小，只要索引里的版本和已安装的不一样就升级。如果索引里的新版要求比正在运行的更新的 Miyu（清单的 `[package] miyu`），这个软件包不算可以升级，放进 `needs_newer_miyu`。

| 方法 | 参数 | 回应 |
|---|---|---|
| `package.upgrades` | 无 | `{"upgrades": [{"package", "from", "to"}], "moved": [{"package", "was", "now"}], "gone", "blocked": [{"package", "reason"}], "held", "needs_newer_miyu": [{"package", "to", "miyu"}], "generated", "stale"?}`：依次是可以升级的；来源仓库变了、因此不升级的；软件源里已经没有的；被拉黑的；被系统配置 `packages.hold` 跳过的；新版要求更新的 Miyu、因此不升级的（`to` 是索引里的版本，`miyu` 是它要求的 Miyu 版本） |
| `package.upgrade` | `packages`（可以不写，写了时只升级这几个）、`preview`、`approve`（和 `package.install` 相同） | 预览时返回 `{"upgrades", "download", "with", "notice"?}`：`upgrades` 的每一项多 `source`、`feed`、`capabilities`（新版声明的能力，带 `held`），`with` 是一起安装的新依赖（和 `package.install` 预览的那一格相同）；真正升级时返回 `{"upgraded": [{"package", "from", "to"}]}`，下载时和 `package.install {"source"}` 一样推送进度 |

## `package.enable`、`package.disable`、`package.restart`

**参数**：`package`；`enable` 另外可以带 `approve`：批准它声明的能力（见 [扩展](extensions.md) 的「能力」）。**回应**：和 `package.list` 的一项相同。

- 扩展：开启是启动进程，关闭是请它退出，它登记的工具离开工具目录。
- `bundle`：开启是启动它带的 MCP 服务，关闭是停下这些服务。
- 出厂的内置包：关闭是卸载，开启是装回来。
- `restart`：重启扩展的进程和它带的 MCP 服务，连续失败的次数从零开始重新计算。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `unknown_package` | 没有这个软件包 |
| `program_missing` | 找不到软件包的程序（软件包文件夹的 `bin/` 里、`miyu` 旁边都没有） |
| `needs_approval` | 需要的能力还有没批准的，见 `data.capabilities` |
| `package_depended` | 要关闭的软件包是其他软件包的依赖 |
| `package_off`、`not_restartable` | 对一个关闭着的软件包调用 `restart`；没有可以重启的进程或服务 |

## `package.file`、`package.call`、`package.page`

**`package.file`**：参数是 `package`、`path`、`offset`。回应 `{"data", "size", "eof"}`，一次最多返回 512 KiB。网页界面的后台页、吉祥物的模型文件、外观包的主题都用它读取。对后台页，`path` 是后台页目录里的相对路径；对吉祥物包，只能是 `model` 那一份；对外观包，相对于软件包目录来写，只能是终端主题的 `file` 那一份，或者网页主题目录下的任何一份普通文件（例如 `web/index.js`）。核心按真实路径查找：链接指向软件包目录以外、网页目录以外的，目录本身，以及其他路径，都返回 `not_found`。

**`package.call`**：参数是 `package`、`method`、`params`（可以不写）。核心把它转给那个软件包的程序（`method.call`），程序返回什么，核心就返回什么。这个方法在后台回应。界面块上的按钮、整页表单的保存都通过它进行。

**`package.page`**：参数是 `package`。回应 `{"url"}`：这是核心按需启动的本机后台页服务上这个软件包的宿主页（`http://127.0.0.1:<端口>/h/<票>/`），交给浏览器打开即可，不需要安装网页界面（`miyu pkg page` 就是用它打开的）。只适用于后台页放在软件包目录里的软件包（清单写 `[page] dir`）；服务在第一次需要时启动，闲置十分钟（没有请求，也没有开着的推送流）以后关闭，关闭以后地址作废，再请求一次即可。宿主页按网页界面的做法把页面放进隔离的框里，替它调用 `settings.get`、`settings.set`、`call`，深色、浅色跟随系统。

`package.call` 是给人的连接使用的（各个头、命令行、人在终端里启动的程序）：哪个软件包登记的方法都能调用到，无论有没有写进 `[export]`，转过去的调用都不带 `from`。扩展（核心启动的软件包的程序）按软件包编号只能调用到自己：`package` 写了其他软件包的编号时返回 `own_package_only`，无论那个软件包是否存在、方法是否导出，都是这个回应；调用自己需要声明能力 `packages.call`，并且只能调用自己清单 `[export]` 里列出的方法，转过去的调用带 `from`（自己的编号）。扩展要调用其他软件包，需要按名字使用 [`bus.call`](bus.md)。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `no_page`、`not_found` | `package.file`：没有后台页；没有这份文件、路径超出了目录、不是普通文件。`package.page`：没有放在软件包目录里的后台页（`no_page`） |
| `program_not_running` | 程序没有连接着 |
| `unregistered` | 程序没有登记这个方法，见 `data.method` |
| `method_failed` | 程序返回了错误，见 `data.message`、`data.code`（扩展返回的带文字的错误码放在 `error.data.code`，核心按它返回；没有时使用 `error.code`）。程序返回的那一行超过 1 MiB 时核心不读取，此时 `data.code` 是 `line_too_long` |
| `method_timeout` | 到了时限还没有回应 |
| `own_package_only` | 由扩展调用时：`package` 不是它自己，`data.package` 是它写的那个编号。跨软件包要使用 `bus.call` |
| `needs_approval`、`not_exported` | 扩展调用自己时：没有声明 `packages.call`；清单 `[export]` 里没有这个方法（见 `data.method`） |

## `extension.status`、`extension.enable`、`extension.disable`、`extension.restart`

**参数**：`status` 没有参数；其他方法的参数是 `package`，`enable` 另外可以带 `approve`。**回应**：`status` 返回 `{"extensions": [...]}`，其他方法返回那一个扩展。每一个扩展的字段如下：

| 名字 | 说明 |
|---|---|
| `package`、`name` | 编号、名字 |
| `start` | 清单写的启动时机 |
| `on` | 开关 |
| `state` | `off`、`starting`、`running`、`waiting`（失败了，正在等待重新启动）、`stopped` |
| `failures` | 连续失败了几次 |
| `pid` | 正在运行的进程号 |
| `retry_in` | 正在等待时才有：多少毫秒以后重新启动 |
| `reason`、`stderr` | 停下时才有：停下的原因（`failed_repeatedly`、`config_error`、`needs_approval`、网址形式的服务需要浏览器登录时的 `needs_login`……）、标准错误输出的最后几行 |
| `capabilities`、`unapproved` | 声明的能力 `[{id, name, summary}]`、还没有批准的能力 |

要一直获得最新的状态，就订阅 `extensions` 流。

**出错**：`unknown_package`；`not_an_extension`（这是界面包）；`extension_off`（重启了一个关闭着的扩展）；`needs_approval`。

## `driver.check`

这个方法检查一个软件包提供的模型驱动编码是否稳定（作者用 `miyu dev driver-check <包编号>` 调用它）。只给人调用。**它会重启那个扩展**。

核心用内置的一小套样本请求（一共五份：纯文字、工具面、带私有数据的一份（同一条消息里的思考、供应商侧工具和工具调用，各带私有数据）、看不了图时的转述、续写；供应商侧工具是这个驱动自己家的 `hosted_tool`，带结果、带私有数据，和真正发送时一样原样交过去）调用扩展的 `driver.encode`：每一份编码两次；然后重启扩展、等它重新握手，再编码一次；三次的结果必须逐字节相同。另外还有一对只在末尾多加一条消息的样本：最后一条稳定区消息里放着记号字 `miyustableend7f3a`，两份编码出来的字节里都必须找得到它，并且到它为止逐字节相同；找不到记号字时，按「无法检查稳定区」算作没有通过。

**参数**：`package`。**回应** `{"package", "drivers": [...]}`，每个驱动一项：

| 名字 | 说明 |
|---|---|
| `driver` | `<包编号>/<驱动名>` |
| `cache` | 清单写的缓存类型 |
| `result` | `passed`、`failed`、`skipped` |
| `samples` | `[{"sample", "ok", "why"?}]`：`plain`、`tools`、`private`、`image`、`continuation`、`pair`；没有通过的样本带 `why`，是一句英文，写明哪里不一样（第几个字节） |
| `why` | 没有实现 `driver.encode` 时才有：`driver.encode is not implemented`。此时 `cache = "contract"` 的驱动算 `failed`，其他驱动算 `skipped` |

**出错**：`unknown_package`；`no_drivers`（清单里没有 `[[driver]]`）；`program_not_running`（没有连接上，或者重启以后等不到握手）；`driver_busy`（此刻有会话正在通过它的驱动调用模型，`data.running` 是还没有回应完的调用次数：需要等它们完成以后再检查，此时不重启）；`extension_off`、`needs_approval`（和 `extension.restart` 相同）。

## `mcp.list`、`mcp.restart`、`mcp.set`、`mcp.remove`

这几个方法管理系统配置（`config.toml` 的 `[mcp.<名字>]`）里的 MCP 服务。无论是手动修改系统配置，还是用 `mcp.set`、`mcp.remove` 修改，都当场生效：新写的服务启动，修改了的服务停下再启动，删除的服务停下。

**`mcp.list`**：没有参数。回应 `{"servers": [...]}`，每一项有 `name`、`command`（网址形式的服务没有这一格，而是 `url`）、`venues`、`state`、`failures`；正在运行的有 `pid`（网址形式的服务没有），正在等待的有 `retry_in`，停下的有 `reason`、`stderr`；覆盖了哪些软件包里的同名服务写在 `covers` 里。知道有哪些工具时另外带 `tools`：这个服务此刻在工具目录里的工具名（`mcp__<服务名>__<工具名>`），启动后列出的、根据目录缓存先登记的都算；声明了资源的服务另外有 `mcp__<服务名>__list_resources`、`mcp__<服务名>__read_resource` 两件工具；还不知道时（从来没有启动过、也没有缓存）没有这一格。这一次列出来、但是没有登记上的工具带在 `skipped: [{"tool", "why"}]` 里（提示词是 `{"prompt", "why"}`），按原名排列：`tool` 是服务那边的原名（没有写名字时为空），`why` 是 `name`（没有写名字）、`parameters`（参数格式不是对象）、`name_taken`（截到 64 个字以后和其他名字冲突，或者其他工具已经叫这个名字）之一；全部都登记上了、或者根据目录缓存先登记还没有列出过时，没有这一格，`miyu check` 根据它报告 `mcp_tool_skipped`。`on` 是这个服务的全局开关（`features.mcp-<服务名>`，见 `feature.set`），没有写时为 `true`。通过浏览器登录过的服务（存着 `mcp.login` 换来的令牌）带 `logged_in: true`，没有登录过的、`mcp.logout` 以后的服务没有这一格：头根据它把「登录」换成「登出」；这一格只表示是否存着令牌，令牌本身不会通过协议交出去。为了编辑时能先填上原来写的内容，回应还带有：`env`（值是字符串，或者 `{"secret": …}`、`{"env": …}`；没有写时没有这一格）、`timeout_ms`（不是默认值时才带），网址形式的服务另外有 `bearer`、`headers`（写法和 `env` 相同）。这几格的内容按原样给出（它们本来就以明文写在系统配置里），头在显示时遮住它们，在编辑框里再给出完整的值；需要隐藏的值应该写成密钥的引用；`command` 是程序加上参数，提交回 `mcp.set` 时要拆成 `command`、`args`。

**`mcp.restart`**：参数是 `name`。回应是重启的那一项。**出错**：`unknown_server`。

**`mcp.set`**：在系统配置里添加一个服务，或者把同名的服务整条替换掉。

| 名字 | 类型 | 说明 |
|---|---|---|
| `name` | 字符串，必写 | 服务名：以小写字母开头，只能包含小写字母、数字和 `-`，最多 32 个 |
| `command` | 字符串 | 程序；和 `url` 二者只写一个 |
| `args` | 字符串数组 | 程序后面的参数；需要同时有 `command` |
| `env` | 对象 | 环境变量：值是字符串，或者 `{"secret": …}`、`{"env": …}`；和 `command` 一起使用 |
| `url` | 字符串 | 网址形式的服务：以 `https://` 开头，本机的服务可以用 `http://`；和 `command` 二者只写一个 |
| `bearer` | 字或对象 | 令牌，以 `Authorization: Bearer <值>` 的形式发送；写法和 `env` 的值相同；和 `url` 一起使用 |
| `headers` | 对象 | 其他请求头，值的写法和 `env` 的值相同；`Authorization`、`Mcp-Session-Id`、`MCP-Protocol-Version`、`Mcp-Method`、`Mcp-Name` 留给核心使用；和 `url` 一起使用 |
| `venues` | 字符串数组 | 给哪种会话使用：`local`、`private`、`group`；不写时只给本机的会话 |
| `timeout_ms` | 整数 | 一次调用最多等待多久，取值 1000 到 3600000；不写时是 600000 |
| `clicall` | 布尔 | AI 在命令里用 `miyu call` 能不能调用这个服务的工具；不写时保留原来写的值，原来也没有写时可以调用 |

没有提交的格子不写入：整条替换时，原来写着、这一次没有提交的内容也会消失。写入之前按读取服务表的规矩检查，写入方式和 `config.set` 相同（带版本，不覆盖同时进行的手动修改）。回应 `{"name"}`；服务随后按新写的内容更换，状态变化时推送 `mcp.changed`。**出错**：`mcp_invalid`（写错了，`data.problem` 说明哪里不对：`command`、`url` 都没有写，或者两样混着写，也返回这个错误）；`bad_params`（提交了 `args` 但没有提交 `command`）；`server_from_package`（系统配置里没有写这个名字，但是有软件包带着同名的服务：随软件包来的服务不能通过它修改）；`config_file_broken`（系统配置无法读取，或者 `mcp` 写成了放不下服务的样子）；`config_conflict`。

**`mcp.remove`**：参数是 `name`。从系统配置里删除这个服务（连同它的子表），回应 `{}`。软件包里被它覆盖的同名服务会按软件包的开关重新启动。**出错**：`unknown_server`；`config_file_broken`；`config_conflict`。

## `mcp.login`

网址形式的 MCP 服务返回 `401`、需要浏览器登录时（`reason` 是 `needs_login`，这张表自己没有写令牌），用这个方法获取授权页：人在浏览器里点击同意以后，核心换取令牌、存进密钥文件（`mcp-oauth-<服务名>`，值不会通过协议交出去），服务自己连接上（`mcp.changed`、软件包列表的推送照常进行）。只给人调用，远程连接也可以调用：核心只返回网址，头在它自己那一边打开；回调监听的是核心所在机器的 `127.0.0.1`，浏览器不在那台机器上时，用 `mcp.login_finish` 把地址贴回来。

**参数**：`service`（服务名：系统配置里的，或者软件包里带的）。

**回应**：`{"url"}`：授权页的地址，由头打开它。核心在本机 `127.0.0.1` 上等待回调，最多 10 分钟；同一个服务再次登录时，上一次正在等待的登录会被停掉。

**出错**：`unknown_server`：没有这个服务；`bad_params`：不是网址形式的服务，或者这张表自己写了令牌（`bearer`、`headers` 里的 `Authorization`）；`oauth_unregistered`：没有客户端，授权服务器又不接受动态注册，需要人去服务那边注册、把 `client_id` 写进这张表；`oauth_failed`：找不到或者无法使用授权服务器（一路上的地址只接受 `https`，`127.0.0.1` 这类本机地址只在服务本身就在本机时才接受），或者授权服务器拒绝了，`data.why` 说明原因（是授权服务器的原话，经过截短）。

登录以后：令牌快过期、或者服务返回 `401` 时，核心自己用 `refresh_token` 续期，把续期后的令牌写回密钥时不重新启动服务；续期失败、或者服务说范围不够（`403 insufficient_scope`）时，服务停在 `needs_login`，需要再调用一次 `mcp.login`（这一次请求的范围是以前的范围加上这一次需要的）。

## `mcp.login_finish`、`mcp.logout`

**`mcp.login_finish`**：浏览器不在核心所在的机器上时（网页界面是从其他机器连进来的），授权完成后浏览器会跳转到一个打不开的 `http://127.0.0.1:<端口>/callback?…`，把地址栏里的那一整段交回来即可。参数是 `service`、`url`。核心在和本机收到回调时相同的地方检查 `state`、`iss`。只给人调用，远程连接也可以调用。**回应**：`{}`：令牌已经换到、存好，服务自己连接上。**出错**：`not_waiting`：这个服务没有正在等待的登录，需要先调用 `mcp.login`；`oauth_failed`：地址对不上（那一次登录仍然在等待，可以再贴一次），或者换取令牌被拒绝。

**`mcp.logout`**：删除存着的那一份内容（令牌、客户端、回调的端口），正在等待的登录也一起取消；服务重新启动，停在 `needs_login`。核心不会吊销授权服务器那边的令牌。参数是 `service`。只给人调用，远程连接也可以调用。**回应**：`{}`。**出错**：`not_logged_in`：这个服务没有登录过。

## `skill.list`

**参数**：没有。**回应**：`{"skills": [...], "dir"}`（`dir` 是你自己放技能的文件夹，还没有创建时也会写出来，家目录下的路径写成 `~/…`），列出核心此刻能使用的技能，按 `name` 排列；同名的技能在几处都有时，只列出核心实际使用的那一份（你自己放的优先；软件包之间，出厂的优先于你安装的，同一层按编号排）。每一项的字段如下：

| 名字 | 说明 |
|---|---|
| `name` | 技能名 |
| `display_name`、`summary` | 给人看的名字、一句说明，按连接的语言挑选（这种语言没有写时依次使用英文、中文、日文）；都没有写时没有这一格 |
| `from` | 从哪里来：`home`（你自己放的，`skills/`）、`shipped`（出厂的软件包带的）、`package`（你安装的软件包带的）、`agents`（配置 `skills.external.enabled` 开启时，来自 `skills.external.dirs` 里列出的其他目录，默认是 `~/.agents/skills`，Codex 这些其他客户端也识别这个目录；配置关闭时不列出） |
| `dir` | `from` 是 `agents` 时才有：它在哪个目录，按配置里写的原样（`~` 不展开） |
| `package` | 随软件包来的技能才有：软件包的编号 |
| `path` | `SKILL.md` 在哪里：家目录下的路径写成 `~/…` |
| `on` | 全局开关（`features.skill-<名字>`，见 `feature.set`）：没有写时为 `true` |
| `warnings` | 技能已经被接受、但是不符合格式的地方：`[{"code", "message"}]`，`code` 是以下之一：`skill_name_mismatch`（名字和文件夹不同名，使用写的名字）、`skill_name_from_folder`（名字没有写或者写错了，使用文件夹的名字）、`skill_yaml_lenient`（开头不符合 YAML，按宽松的读法读取）、`skill_description_cut`（说明超过 1024 个字，被截断了）、`skill_compatibility`、`skill_metadata`（这两格写得不符合格式）；`message` 使用连接的语言；`miyu check` 报告同样的码。没有这类问题时不带这一格 |

## `skill.remove`

**参数**：`names`，技能名的数组，至少一个，不能重复。**回应**：`{"trash": [...]}`，是每个技能的文件夹在系统回收站里的位置，按 `names` 的先后排列。这个方法只删除你自己放的技能（`from` 是 `home`，整个 `skills/<文件夹>/` 移进回收站）；如果它覆盖了软件包里的同名技能，删除以后软件包里的那一份会重新列出来。只给人调用，扩展进程调用时返回 `local_only`。

**出错**：没有写名字、名字重复时返回 `bad_params`。没有这个技能时返回 `unknown_skill`；随软件包来的技能返回 `skill_from_package`；其他目录里的技能（`from` 是 `agents`）返回 `skill_shared`（这三种都带 `data.name`；几个名字里只要有一个不行，就一个都不删除）。无法移进回收站时返回 `trash_failed`。

## `script.list`

**参数**：没有。**回应**：`{"scripts": [...], "dir"}`（`dir` 是你自己放脚本的文件夹，写法和 `skill.list` 相同），列出此刻登记成工具的脚本，按 `tool` 排列。每一项的字段如下：

| 名字 | 说明 |
|---|---|
| `tool` | 工具名 |
| `display_name`、`description` | 给人看的名字、一句说明（来自脚本开头的 `Display name`、`显示名称`、`描述`），按连接的语言挑选，挑选方法和 `skill.list` 相同；都没有写时没有这一格 |
| `from`、`package` | 和 `skill.list` 相同：`home` 是你自己放的（`scripts/`） |
| `path` | 脚本文件在哪里，写法和 `skill.list` 相同 |
| `timeout` | 一次调用最多等待多少秒 |
| `on` | 全局开关（`features.script-<工具名>`）：没有写时为 `true` |

## `feature.set`

技能、脚本、MCP 服务那几页修改开关时使用这个方法。回应 `{}`。

**`feature.set`**：修改一个技能、脚本或者 MCP 服务的全局开关（`features.<功能编号>`），写入系统配置，写入方式和 `config.set` 修改一项相同（带版本，不覆盖同时进行的手动修改，写入成功后推送 `config.changed`），从下一个回合开始生效。`unlisted` 为 `on` 的预设跟随这个开关；预设里单独写了开启或关闭的功能，按预设执行（见 `preset.get` 每个功能的 `by`）。`unlisted` 为 `off` 的预设不跟随这个开关：预设里没有写的功能仍然关闭。

| 名字 | 类型 | 说明 |
|---|---|---|
| `feature` | 字符串，必写 | 功能编号：`skill-<名字>`、`script-<工具名>`、`mcp-<服务名>` |
| `on` | 布尔或 `null`，必写 | 为 `null` 时删除这一项，回到跟随预设 |

**出错**：`bad_params`（名字无法写进键，不是这三类功能，或者缺少 `on`）；`config_file_broken`；`config_conflict`。

## `command.run`

这个方法执行一条斜杠命令：头把人输入的原文交过来，核心识别命令、判断谁能使用，然后执行。

**参数**

| 名字 | 类型 | 说明 |
|---|---|---|
| `session` | 字符串，必写 | 哪个会话 |
| `text` | 字符串，必写 | 人输入的原文，例如 `/stop` |
| `cwd` | 字符串 | 头所在的目录，只用来解析 `/workspace` 后面的相对路径 |
| `as` | 对象 | 场所会话使用：代表平台上的人，和 `session.send` 相同 |
| `prefix` | `"keep"` 或 `"compact"` | 开始新一轮的命令（模板、MCP 提示词、`/goal` 的设定和继续）在前缀要改变时人做的选择，和 `session.send` 的「前缀要变」相同 |

**回应**：`{"command", "events", "said"}`：分别是正式名称（别名会换成正式名称）、这一次追加的事件、回执的那一句话（使用连接的语言，原样给人看）。打开整页的命令返回 `{"command", "opens": {"package", "widget"}, "said": ""}`，头打开那一页。

**核心识别的命令**：

| 命令 | 做什么 |
|---|---|
| `/clear`（`/reset`） | 清空上下文和待办，和 `session.clear` 相同 |
| `/stop` | 删除这个会话的循环，打断这一轮，停掉这个会话派出去的后台命令和子代理，以及正在运行的整理 |
| `/workspace <路径>` | 更换工作目录；不带路径时告诉你现在在哪里 |
| `/context <数值>`、`/context unset`（`off` 也行）、`/context` | 设置这个会话的上下文上限（`128000`、`128k`、`1m` 都可以），和 `session.configure` 的 `context` 相同；去掉上限；告诉你此刻的上限和压缩线 |
| `/todo`、`/todo clear` | 查看待办；清空待办 |
| `/remember <话>` | 记下一条记忆 |
| `/dream`、`/dream stop` | 立即整理记忆（在后台回应）；停下正在运行的整理，已经合并的部分保留 |
| `/clear-memory`（`/reset-memory`） | 清除这个会话记下的记忆 |
| `/goal` 系列 | 查看、设定、暂停、继续、修改、清除长期目标 |
| `/loop <间隔> <话>`、`/loop <话>`、`/loop`、`/loop off` | 每隔一段时间说一次；由模型自己决定下一次什么时候运行（如果它那一轮里没有决定，循环就结束，这一行被删除）；由模型自己决定做什么、下一次什么时候（自主循环）；停掉这个会话的循环（`/loop clear`、`/loop stop` 效果相同）。一个会话只有一个循环，再输入一次就替换原来的循环；会话正在运行一轮时输入的命令（到点触发的也一样）会等这一轮结束再运行，不插进这一轮；会话的预设没有开启循环时，没有这条命令 |

不是这些命令时，核心依次在这几处查找：你自己放的命令模板（`commands/<名字>.md`）、扩展声明的命令（核心转给扩展，`command.call`）、软件包带的命令模板。模板会展开成一句话，当作这个人说的话发出去：回应的 `events` 是那一句话，`said` 是空的。

名字是 `<服务名>:<提示词名>`（恰好一个 `:`）的命令是 MCP 服务的提示词：核心用 `prompts/get` 去获取，人在后面跟的文字按空白切开（成对的英文双引号中间算一个，引号会去掉），按提示词声明的参数的先后对应，最后一个参数拿到剩下的整段；获取回来的文字、图片合并成这个人说的一句话发出去，和模板相同。这类命令只在本机的会话里识别，跟随服务的功能 `mcp-<服务名>` 的开关。

**谁能使用**：本机的会话里都能使用；场所会话里是主人和管理的人能使用。系统配置 `commands.<名字>.access` 可以改成 `anyone`（谁都可以使用）或者 `off`（停用）。`/workspace` 只给主人本人使用。

同一个 `id` 再次发送只算一次：参数相同时，拿到的是第一次的回应（`events`、`said` 都是当时的），命令不再执行；参数改变时返回 `id_reused`，命令不执行。核心记住最近的 1024 条，重启以后不再记得，此时命令会再执行一遍。被拒绝的请求不记录，再次发送时重新判断。

**出错**

| `data.reason` | 什么时候 |
|---|---|
| `bad_params`（`-32602`） | 不以 `/` 开头；`cwd` 不是绝对路径；命令后面跟的文字不对（`/loop` 的 `data.why` 是用连接的语言写的用法，一行一种写法）；`/context` 设得太小（`data.field` 是 `context`，`data.min` 是最少需要多少） |
| `unknown_command` | 没有这个命令 |
| `command_not_allowed` | 这个人不能使用这个命令，或者命令已经停用、预设没有开启，`data.why` 是给人看的一句话 |
| `owner_only` | 只有主人本人能使用 |
| `command_unavailable`、`command_failed` | 扩展的命令：扩展没有连接上、一分钟内没有回应；扩展回应了错误（`data.why` 是原话）、回应里没有 `said`（`data.why` 使用连接的语言）。MCP 的提示词：服务没有在运行、没有列出这一个提示词、不是本机的会话、到时间没有回应（按服务的 `timeout_ms`，最多一分钟）；缺少必填的参数（`data.why` 用连接的语言说明缺哪几个）、服务返回了错误（`data.why` 是原话） |
| `memory_unavailable`、`goal_unavailable` | 这个会话没有记忆，或者没有长期目标，见 `data.why` |
| 命令自己的 | `/clear` 的 `turn_running`、`nothing_to_clear`，`/workspace` 的 `path_unreadable`……，和对应的方法相同 |

## `command.catalog`

**参数**：`session`（可以不写）：写了时只列出这个连接在这个会话里输入后不会被拒绝的命令；不写时列出全部，随软件包来的命令（`/loop`、`/goal`、`/todo`）按你的默认预设筛选，默认预设没有开启那个软件包时不列出。

**回应**：`{"commands": [{"name", "aliases", "summary", "argument"?, "usage"?, "opens"?}…], "skills": [{"name", "summary"}…]}`，按名字排列。技能另外列在 `skills` 里：在输入框里用 `$技能名` 点名，发送 `session.send` 时写进 `skills`（见 [会话](sessions.md)），一句话里可以点名几个技能；`user-invocable: false` 的技能不列出，写了会话时只列出预设开启着的技能。正在运行的 MCP 服务的提示词也在里面：名字是 `<服务名>:<提示词名>`，`summary` 是服务写的 `title`（没有时使用 `description`，不区分语言），`argument` 按声明的参数拼成（必填的写成 `<名字>`，可选的写成 `[名字]`）。`argument` 是名字后面跟什么的提示，`usage` 是有几种写法的命令，一行一种（`{"form", "summary"}`），`opens` 表示打开整页的命令。`summary`、`argument` 使用连接的语言。

头自己的命令（例如 `/help`）由头自己加进菜单。

## `check`

**参数**：`cwd`（从这里查找项目配置）、`file`（只检查这一份）。

**回应**：`{"problems": [...]}`，每处问题一项：`kind`（`config`、`secrets`、`persona`、`preset`、`package`、`script`、`skill`、`slash`、`mcp`、`schedule`（定时表里无法解析的行，`schedule_unreadable`）；软件包自己的检查按它报告的原样返回，例如 `venue`）、`file`、`code`、`level`、`message`，有行列信息时带 `line`、`column`，配置的问题另外带 `key`、`got`、`suggest`、`using`，软件包自己的检查可以另外带 `key`、`rule`、`source`。

不写 `file` 时检查全部：系统配置、`cwd` 的项目配置、密钥文件、每一层的人格和预设、软件包清单、软件包自己的检查；无法读取的、被覆盖的、链接指向数据根里无法识别的位置的脚本、技能、命令模板，因为名字冲突没有登记上的脚本，和自带命令同名的命令模板，MCP 服务列出来却没有登记上的工具、提示词；安装好以后被修改、缺少的文件。回应另外带 `embedding`：这是语义模型的状态，不是问题（`model`，无法使用时 `model` 为 `null` 并带 `why`；后台正在补算时带 `filling`；`memory`、`turns` 每个人格一项 `{persona, have, total}`；`kb` 每个知识库一项 `{id, name, have, total}`；`no_recall` 是这个模型不自动联想的几样，取值是 `memory`、`kb`：门槛表里没有这个模型时不自动联想，搜索仍然照常工作）。

**出错**：`unknown_file`：`file` 不是 Miyu 读取的那几种文件。
