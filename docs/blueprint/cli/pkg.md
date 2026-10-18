## `miyu pkg`

### 是什么

在终端里看装了哪些软件包，从一个包目录装、把卸掉的出厂的装回来、卸掉；看一个包的信息、装了哪些文件、一个文件归哪个包、装好的文件改没改（施工 F-8 下，照 pacman 的 `-Qi`、`-Ql`、`-Qo`、`-Qk`）；装、卸之前照 pacman 印一份要做什么、问一句（施工 F-8 下补）（施工 T-3，设计 `30-插件框架.md` 第九节、第十二节；名字照 `22-命令行.md` 第五节）。连上核心（没在跑就拉起来），走 `package.list`、`package.install`、`package.remove`（`packages.md`「装卸」）、`package.info`、`package.files`、`package.owns`、`package.check`（`packages.md`「本地库」），不另起一套：装卸当场生效，和头上装卸的是同一条路。给人看的只放人要的：编号、名字，卸掉了的、写错了的标一句；种类、哪一层、版本、功能不印（2026-10-08 项目主人定的设计原则）。软件源（设计 `37-软件源.md`）：`update`、`search`、从软件源装、`clean`（施工 G-3、G-4）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `pkg`；换上帮助页 |
| `crates/miyu-cli/src/pkg.rs` | 参数、找数据根、连核心、握手、照子命令发请求、退出码 |
| `crates/miyu-cli/src/pkg/shown.rs` | 一个软件包印成一行（纯的，测试里照它算） |
| `crates/miyu-cli/src/pkg/query.rs` | `info`、`files`、`owns`、`check` 印成几行，字节数写成给人看的（纯的，施工 F-8 下） |
| `crates/miyu-cli/src/pkg/confirm.rs` | 装、卸之前印的那一份、问的那一句、读答（施工 F-8 下补） |
| `crates/miyu-cli/src/language/confirm.rs` | 印的那一份、问的那一句的字（施工 F-8 下补） |
| `crates/miyu-cli/src/language/pkg.rs` | 给人看的几句，中英文 |
| `crates/miyu-cli/src/help/{zh,en}/pkg.txt` | 帮助页 |

### 对外的样子

| 子命令 | 做什么 | 发什么 |
|---|---|---|
| `list`（不写子命令也是它） | 列出来，照编号 | `package.list` |
| `install <包目录\|包文件\|编号>` | 带 `/`、`\`，是 `.`、`..`，或者以 `.toml`、`.miyu`（包文件，施工 G-1）结尾的是一个包目录（或它里面的 `package.toml`）、包文件的路径，相对的照现在的目录换成绝对的，`.`、`..` 照字面去掉（施工 F-8 上）；别的是编号：出厂包的把卸掉的装回来，别的从软件源装（施工 G-4 下，核心判断）。从软件源装的印的那一份另有「一起装：」（依赖和版本）、「下载：」（多大）、「来源：」（来源仓库，官方的接「（官方）」，社区的接「（社区，没人审过）」）三行；问过以后印「正在下载 <多大>」，装好了一起装的依赖、要的那个各印一行「装好了：<编号>」 | `package.install`，带 `path` 或者 `package` |
| `remove <编号>` | 卸掉：家目录里装的删掉，出厂的记一笔；它的设置和状态一并删掉（`packages.md`「装卸」第 7 条） | `package.remove` |
| `info <编号>` | 一个包的信息：名称、版本、大小、安装时间 | `package.info`，名称照 `package.list` |
| `files <编号>` | 一个包装了哪些文件，一个一行、绝对路径 | `package.files` |
| `owns <路径>` | 一个文件归哪个包；相对的照现在的目录换成绝对的 | `package.owns` |
| `check [<编号>]` | 装好的文件改了、少了、多出来的；不写是家目录里记了的全部 | `package.check` |
| `update` | 从软件源拿新的索引（施工 G-3 上）：不管本机那份新不新都去拿；印「软件源已更新：N 个包（<索引的时刻>）」；一处都没拿成、用的是本机那份的在标准错误上说「软件源连不上，用的是本机的索引（<时刻>）：<原因>」，退出码 1；本机也没有的照核心的原话，退出码 1。`--format json` 原样印回应 | `package.sync`，带 `force: true` |
| `clean` | 清掉缓存里不是装着那一版的包文件（施工 G-4 下）：印「清掉了 N 个包文件，腾出 X」，一个都没有的印「缓存里没有要清的包文件」。`--format json` 原样印回应 | `package.clean` |
| `search [<词>…]` | 在软件源里搜（施工 G-3 下）：一行一个，`<编号>  <名字>  <说明>`，编号照最长的对齐；装着的接「已装」，不是官方的接「社区」，被拉黑的接「被拉黑：理由」；版本不印。一个都没搜到的什么都不印、退出码 1（照 pacman）；用的是本机旧索引的在标准错误上说一句。`--format json` 原样印回应 | `package.search` |
| `pack <包目录>` | 把包目录打成包文件（施工 G-1，设计 `37-软件源.md` 第二节）：照装的那一套读清单，要写了版本；清单写了 `targets` 的用 `--target` 挑一种、没写的不许写；写到 `-o` 给的目录（没有的建上，不写的是现在的目录），文件名 `<编号>-<版本>[-<系统>].miyu`；印三行：打好了的路径、`SHA-256: <哈希>`、大小（`--format json` 印 `{"package", "version", "target", "path", "sha256", "size"}`）。不连核心 | 不发 |

pacman 的写法当别名（设计 `31-软件包.md` 第一节，定了的 A）：`-U` 是 `install`，`-R` 是 `remove`，`-Q` 是 `list`，`-Qi`、`-Ql`、`-Qo`、`-Qk` 是 `info`、`files`、`owns`、`check`，`-Sy` 是 `update`（施工 G-3 上），`-Ss` 是 `search`，`-Sys` 先拿再搜（施工 G-3 下）；`-S <编号>…` 从软件源装这几个、`-Sy <编号>…` 先拿索引再装、`-Sc` 是 `clean`（施工 G-4 下）；`-S` 什么都不带的照 pacman 在标准错误上说「错误：没有指定目标（使用 -h 获取帮助）」（英文 `error: no targets specified (use -h for help)`），退出码 2（项目主人 2026-10-19 定）。升级的字母随 G-5 加。`-Q` 后面的字母最多一个；`-Qi`、`-Ql`、`-Qo` 要接编号或路径，`-Q` 不带字母的不接（参数不对，退出码 2）。

| 选项 | 做什么 |
|---|---|
| `--format text\|json` | `list`、`info`、`files`、`owns`、`check`：`json` 原样印核心回的（`list` 印那一串，别的印整个回应），给脚本 |
| `--yes`、`--noconfirm` | `install`、`remove`：不问，直接做（施工 F-8 下补，`--noconfirm` 是 pacman 的写法），给脚本 |
| `--target <系统>`、`-o, --out <目录>` | `pack`：打给哪一种系统、写到哪个目录（施工 G-1） |

界面语言照 `cli/main.md`。用到的环境变量：`MIYU_HOME`。

### 怎么走

1. **找数据根、连核心、握手**：照 `miyu memory`（`cli/memory.md` 第 1 步）。请求的编号是 `pkg-<16 位十六进制>-<序号>`。
2. **发**：照上面的表。`install` 一律先发同一条请求带 `preview: true`（要的能力打开时一并批，下面第 2½ 步），`remove` 没写 `--yes` 的也先发；没写 `--yes` 的（施工 F-8 下补，`protocol.md`），照回应印一份要做什么（第 3 步），空一行问「继续安装？[Y/n]」「继续卸载？[Y/n]」，读标准输入的一行：空的、`y`、`yes`（不分大小写）接着发真的那一条；别的在标准错误上印「已取消」，退出码 1；标准输入关着的（脚本里、管道读完了）不做，标准输出补一个换行，标准错误印「已取消：没有确认（不问用 --yes）」，退出码 1。看一眼被拒的（写错的、必需的……）照第 4 步，不问。
2½. **装好以后问一句「打开？」**（作者说明第九节第 1 条，2026-10-11 项目主人照推荐定，施工 F-8 四补）：装好的那一项（`package.install` 的回应）有开关、程序在，关着的；或者开着却没在跑（不是 `running`、`starting`）、看一眼时又列了要的能力的（清单写了一起来就拉起，能力还没批）：空一行问「现在打开？[Y/n]」，读一行，空的、`y`、`yes` 发 `package.enable {package, approve}`，`approve` 是看一眼时列出的那几个能力的编号，印「打开了：<编号>」；别的、标准输入关着的不打开，印「没有打开：以后在「软件包」页里打开」，照样算装好了，退出码 0。`--yes` 当是，不问。没有开关的（界面、小程序、只带技能脚本的）、程序不在的、已经在跑的不问。打开被拒的照第 4 步。
3. **印**（标准输出）：
   - `list`：一个一行，`<编号>  <名字>`，照编号排（核心把卸掉了的出厂的排在最后，这里照编号排进去），编号照最长的那个对齐、后面空两格；卸掉了的出厂的后面接「（已卸载）」；写错的没有名字，接「（写错了：<哪里不对>）」。
   - 装、卸之前的那一份（施工 F-8 下补）：第一行「将安装 <编号> <版本>」，升级的接「（替换 <原来的版本>）」（原来没写版本的「（替换已装的）」），装回出厂的「将装回」，卸的「将卸载」；「包含：」程序（界面程序、扩展程序、小程序、内置功能）、命令 miyu <名字>、后台页、吉祥物、接入 <平台>、系统账号、每个 MCP 服务一项「MCP 服务 <名字>」（施工 X-2）、技能、脚本（施工 X-13 上）、每个斜杠命令模板一项「斜杠命令 /<名字>」（施工 X-14 上补）、几项设置；「需要：」声明的能力，照核心给的名字；卸的「一并删除：」写了的设置的键、状态目录；「大小：」照 `info` 的写法，出厂的卸掉不删文件、没有这一行。几样之间用「、」隔开，没有的那一行不印。
   - `install`：「装好了：<编号>」，接着照第 2½ 步问、打开的印「打开了：<编号>」；`remove`：「卸掉了：<编号>」。
   - `info`（施工 F-8 下）：一格一行，前面的词照最宽的那个对齐、后面空两格：名称（写错的包没有）、版本（写了的）、大小（字节数照 pacman 写成 B、KiB、MiB、GiB，一位小数，接几个文件）、安装时间（照这台机器此刻的时区到分钟；出厂的写「出厂自带」，本地库没记的没有这一行）。从哪装的、哪一层不印。
   - `files`：一个文件一行，`<编号> <绝对路径>`（照 pacman 的 `-Ql`），照路径排。
   - `owns`：「<路径> 属于 <编号>」；在家目录里装的包的目录里、本地库没记的（包自己后来写的）「<路径> 在 <编号> 的目录里，安装时没有」；哪个包都没有的「没有软件包包含 <路径>」印在标准错误上，退出码 1（照 pacman）。路径照换成的绝对路径印。
   - `check`：每个包改了的「<编号>：已修改 <路径>」、少了的「缺失」、多出来的「多出」，一个一行；都没问题的一行「<编号>：正常」；家目录里一个记了的都没有的「没有可检查的软件包」。有改了、少了的退出码 1（照 pacman 的 `-Qk`），多出来的只是提示。
4. **被拒绝**：核心的原话照原样印在标准错误上，退出码 1（`protocol.md` 的 `package.*`）。清单装不上的（`package_invalid`）例外：核心的原话是给程序看的（说的是去 `data` 里看），这里照 `data.problem`、`data.line` 说「装不上：<哪里不对>（第 <几> 行）」。和装着的包撞了名的（`package_conflict`，施工 F-8 六补）也例外，照 pacman 列冲突：第一行「装不上：名字已经有了」（施工 P-11 下改，原来是「和已装的软件包冲突」：人格、预设还会和家目录、系统区、出厂的撞），接着照 `data.conflicts` 一处一行「<哪一样> <名字> 已在 <包> 中」（子命令、功能、工具、技能、MCP 服务、知识库、斜杠命令、人格、预设；撞了一层的 `<包>` 写「出厂的」「系统区」「你的家目录」；英文 `Cannot install: names already in use`、`<kind> <name> exists in <package>`，那一层写 `the shipped ones`、`the system area`、`your home`）。看一眼印的那一份多「人格 <编号>」「预设 <编号>」（施工 P-11 下）。核心断开、请求写不出去照 `miyu rename`。

### 样子

```text
basesystem  基础系统
memory      人格记忆
mermaid     画 mermaid 图（已卸载）
net         联网
```

`miyu pkg info pudding`、`miyu pkg -Ql pudding`、`miyu pkg -Qk`（施工 F-8 下）：

```text
名称      布丁
版本      1.2.0
大小      1.5 KiB，3 个文件
安装时间  2026-10-11 14:03

pudding /home/me/.miyu/home/me/packages/pudding/package.toml
pudding /home/me/.miyu/home/me/packages/pudding/skills/draw/SKILL.md

pudding：已修改 skills/draw/SKILL.md
pudding：多出 cache.db
```

`miyu pkg install ./pudding`、`miyu pkg remove pudding`（施工 F-8 下补）：

```text
将安装 pudding 1.3.0（替换 1.2.0）
包含：扩展程序、命令 miyu pudding、后台页、2 项设置
需要：联网
大小：1.5 KiB，3 个文件

继续安装？[Y/n]

将卸载 pudding 1.3.0
一并删除：设置 pudding.port、状态目录
大小：1.5 KiB，3 个文件

继续卸载？[Y/n]
```

### 退出码

| 码 | 什么时候 |
|---|---|
| 0 | 做成了 |
| 1 | 找不到数据根、连不上核心；被拒绝（没有这个包、必需的、清单装不上、和出厂的撞了……）；核心断开；`owns` 哪个包都没有；`check` 查出改了、少了的；装、卸之前问了答不、没得答 |
| 2 | 参数不对：子命令写错、少了清单或编号（`cli/main.md`） |

### 给人看的字

| 键 | 中文 | 英文 |
|---|---|---|
| 装好了 | 装好了：{id} | Installed {id} |
| 卸掉了 | 卸掉了：{id} | Removed {id} |
| 装不上 | 装不上：{problem}（第 {line} 行）、装不上：{problem} | Cannot install: {problem} (line {line}), Cannot install: {problem} |
| 已卸载 | （已卸载） | (removed) |
| 写错了 | （写错了：{problem}） | (broken: {problem}) |
| `info` 的四个词 | 名称、版本、大小、安装时间 | Name、Version、Size、Installed |
| 大小 | {size}，{n} 个文件 | {size}, {n} files（一个的写 1 file） |
| 出厂的 | 出厂自带 | shipped with Miyu |
| 属于 | {path} 属于 {id} | {path} is owned by {id} |
| 在目录里 | {path} 在 {id} 的目录里，安装时没有 | {path} is in the directory of {id} but was not installed with it |
| 没有包 | 没有软件包包含 {path} | No package owns {path} |
| 查文件 | {id}：已修改 {path}、{id}：缺失 {path}、{id}：多出 {path}、{id}：正常 | {id}: modified {path}、missing、extra、{id}: OK |
| 没得查 | 没有可检查的软件包 | No packages to check |
| 将做什么 | 将安装 {id}、将装回 {id}、将卸载 {id} | Install {id}、Restore {id}、Remove {id} |
| 替换 | （替换 {version}）、（替换已装的） | (replaces {version}), (replaces the installed one) |
| 程序 | 界面程序、扩展程序、小程序、内置功能 | interface、extension、worker program、built-in feature |
| 带的 | 命令 miyu {name}、后台页、吉祥物、接入 {platform}、系统账号、{n} 项设置 | command miyu {name}、admin page、mascot、{platform} connection、system account、{n} settings |
| 行首 | 包含：、需要：、一并删除：、大小： | Includes: 、Needs: 、Also deletes: 、Size:  |
| 删的 | 设置 {keys}、状态目录 | settings {keys}、state directory |
| 问 | 继续安装？[Y/n] 、继续卸载？[Y/n]  | Proceed with installation? [Y/n] 、Proceed with removal? [Y/n]  |
| 不做 | 已取消 | Cancelled |
| 没得答 | 已取消：没有确认（不问用 --yes） | Cancelled: not confirmed (use --yes to skip the question) |

核心拒绝时说的话照核心写的原样印。

**帮助页**：`-h`、`--help`、`miyu help pkg`、各个子命令的 `-h` 印的都是这一页，规矩见 `cli/main.md`「帮助页」（`crates/miyu-cli/src/help/zh/pkg.txt`、`en/pkg.txt`）。

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-cli/src/pkg/tests.rs` | 一行的样子：照编号排、对齐，卸掉了的、写错了的各接一截，别的不印；带 `/`、以 `.toml` 结尾的是清单（照现在的目录换成绝对的），别的是编号；pacman 的写法换成正式的子命令，字母写多了、少了接的都不认（施工 F-8 下） |
| `crates/miyu-cli/src/pkg/confirm/tests.rs`（施工 F-8 下补） | 升级、装回、卸的那一份：每一样怎么说、没有的那一行不印；回车、`y`、`yes` 接着做，别的和标准输入关着的不做、各说一句 |
| `crates/miyu-cli/src/pkg/query.rs` 的测试（施工 F-8 下） | `info` 对齐、从哪装的不印、没写版本的不印那一行、出厂的写「出厂自带」；`files` 是编号加绝对路径；`owns` 分装的时候带的和后来写的；`check` 一个一行、只有改了少了的算出错；字节数的写法 |
| `crates/miyu/tests/pkg.rs` | 真跑：列出、照相对路径装、卸；卸掉出厂的标「已卸载」、照编号装回来；必需的卸不掉、写错的清单装不上，退出码 1；`--format json`；`-h`、子命令的 `-h`、`help pkg` 印的都是那一页，跟着界面语言；`-U` 装、`info`、`-Ql` 的绝对路径、`-Qo` 归哪个、没有的退出码 1、`-Qk` 正常和改了的退出码 1（施工 F-8 下）；装之前印的那一份、答不就没装、回车装上、标准输入关着的不卸、`--noconfirm` 不问（施工 F-8 下补） |

### 还没有的

- 搜索、软件源、停用（`22-命令行.md` 第五节的 `miyu pkg …`）：随软件源那一段。
