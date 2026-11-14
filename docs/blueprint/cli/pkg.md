## `miyu pkg`

### 是什么

在终端里看装了哪些软件包，从一个包目录装、把卸掉的出厂的装回来、卸掉；看一个包的信息、装了哪些文件、一个文件归哪个包、装好的文件改没改（施工 F-8 下，照 pacman 的 `-Qi`、`-Ql`、`-Qo`、`-Qk`）；装、卸之前照 pacman 印一份要做什么、问一句（施工 F-8 下补）（施工 T-3，设计 `30-插件框架.md` 第九节、第十二节；名字照 `22-命令行.md` 第五节）。连上核心（没在跑就拉起来），走 `package.list`、`package.install`、`package.remove`（`packages.md`「装卸」）、`package.info`、`package.files`、`package.owns`、`package.check`（`packages.md`「本地库」），不另起一套：装卸当场生效，和头上装卸的是同一条路。给人看的只放人要的：编号、名字，卸掉了的、写错了的标一句；种类、哪一层、版本、功能不印（2026-10-08 项目主人定的设计原则）。软件源（设计 `37-软件源.md`）：`update`、`search`、从软件源装、`clean`（施工 G-3、G-4）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `pkg`；换上帮助页 |
| `crates/miyu-cli/src/pkg.rs` | 参数、找数据根、连核心、握手、照子命令发请求、退出码 |
| `crates/miyu-cli/src/pkg/session.rs` | 会话里的 `miyu pkg`（施工 BS-1 中下）：经 `miyu call` 的通道，子命令换成调 `packages`、`package` |
| `crates/miyu-cli/src/pkg/shown.rs` | 一个软件包印成一行（纯的，测试里照它算） |
| `crates/miyu-cli/src/pkg/query.rs` | `files`、`owns`、`check` 印成几行，字节数写成给人看的（纯的，施工 F-8 下） |
| `crates/miyu-cli/src/pkg/info.rs` | `info` 问核心、印成「键 : 值」（印的那几行是纯的，施工 IN-2）；写的是路径的看那个包目录、包文件（施工 IN-4） |
| `crates/miyu-cli/src/pkg/path.rs` | 写的是路径还是编号、相对的照现在的目录换成绝对的（`install`、`info`、`owns` 共用，施工 IN-4 从 `pkg.rs` 挪出来） |
| `crates/miyu-cli/src/pkg/confirm.rs` | 装、卸之前印的那一份、问的那一句、读答（施工 F-8 下补） |
| `crates/miyu-cli/src/pkg/install.rs` | `install`、`-S <编号>`：看一眼、印、问、装、问启用；从软件源装的分阶段走、推荐的答「是」的接着装（施工 IN-3） |
| `crates/miyu-cli/src/pkg/summary.rs` | 从软件源装之前印的「软件包 (N)」「推荐 (N)」和几行「键 : 值」、问的那一句、推荐的那一段（纯的，施工 IN-3、PK-3 下） |
| `crates/miyu-cli/src/pkg/approve.rs` | 装、升级的时候就批（施工 PK-3 下）：核心认不认 `approve`、照看一眼拼表、能力一项一行、「注意」那一行、批了哪些（纯的） |
| `crates/miyu-cli/src/pkg/upgrade.rs` | `upgrade`、`-Su`：盘点、看一眼、每个包下面列新多要的能力、问、升（施工 G-5、PK-3 下） |
| `crates/miyu-cli/src/pkg/progress.rs` | 照 `package.progress` 印下载、核对、安装：终端里画进度条、刷新同一行，管道里只印行；一行进度条怎么排是纯的（施工 IN-3） |
| `crates/miyu-cli/src/language/confirm.rs` | 印的那一份、问的那一句的字（施工 F-8 下补） |
| `crates/miyu-cli/src/language/pkg.rs` | 给人看的几句，中英文；`info` 的键、来源的说法在 `language/pkg/info.rs`（施工 IN-2）；从软件源装分阶段的字在 `language/pkg/install.rs`（施工 IN-3）；来源、能力、「注意：」、带批准问的那几句、批了哪些在 `language/pkg/approve.rs`（施工 PK-3 下） |
| `crates/miyu-cli/src/help/{zh,en}/pkg.txt` | 帮助页 |

### 对外的样子

| 子命令 | 做什么 | 发什么 |
|---|---|---|
| `list`（不写子命令也是它） | 列出来，照编号 | `package.list` |
| `install <包目录\|包文件\|编号>` | 带 `/`、`\`，是 `.`、`..`，或者以 `.toml`、`.miyu`（包文件，施工 G-1）结尾的是一个包目录（或它里面的 `package.toml`）、包文件的路径，相对的照现在的目录换成绝对的，`.`、`..` 照字面去掉（施工 F-8 上）；别的是编号：出厂包的把卸掉的装回来，别的从软件源装（施工 G-4 下，核心判断）。从软件源装的照 pacman 分阶段印（施工 IN-3，设计 `49-装包照pacman.md` 第二节，见「怎么走」第 2、3 步）；装好了一起装的依赖各印一行「已安装 <编号>」，要的那个印「已安装 <编号> <版本>」（没写版本的不接） | `package.install`，带 `path` 或者 `package` |
| `remove <编号>` | 写组名的（`basesystem`，施工 BS-1 下中）照组卸：印「将卸载 <组>」「包含：<编号>、…」、问「卸载 '<组>'？[Y/n]」，一个一行「已卸载 <编号>」；`install <组名>` 同样装回组里卸掉的；组里没有要动的印「<组> 中没有需要处理的软件包」、退出码 0。卸掉：数据根里装的删掉，出厂的记一笔；它的状态一并删掉，配置目录 `etc/<编号>/`、配置里它的设置项照 pacman 留下，写了 `--purge` 的一起删（施工 PK-5、PK-5补，`packages.md`「装卸」第 7 条；写组名的照样传给组里每个包）。已经卸掉、还留着这几样的编号写 `--purge` 的只删留下的，印法同卸包 | `package.remove`，写了 `--purge` 的看一眼、真卸都带 `purge: true`（不写的不带：旧的核心不认这一格） |
| `info <编号>` | 一个包的信息（施工 IN-2，设计 `49-装包照pacman.md` 第一节）：装着的照本地，没装的照软件源，写成「键 : 值」；哪儿都没有的报「未找到软件包 '<编号>'」，退出码 1 | `package.info`，不写 `source`；`-Qi` 写 `false`、`-Si` 写 `true` |
| `info <包目录\|包文件>` | 看一个本机的包目录、包文件（施工 IN-4，照 pacman 的 `-Qip`；项目主人 2026-10-31 定）：认路径的写法同 `install`，相对的照现在的目录换成绝对的；不装，照真装之前查的那几样查一遍，装不上的照第 4 步报，退出码 1 | `package.info`，带 `path` |
| `files <编号>` | 一个包装了哪些文件，一个一行、绝对路径 | `package.files` |
| `owns <路径>` | 一个文件归哪个包；相对的照现在的目录换成绝对的 | `package.owns` |
| `check [<编号>]` | 装好的文件改了、少了、多出来的；不写是数据根里记了的全部 | `package.check` |
| `page <编号>` | 用浏览器打开一个包的后台页，不用装网页（施工 BP-2，`package-pages.md`「不靠网页打开软件后台」第 1、2 条，2026-11-09 项目主人定）：`package.list` 里带 `page_server` 的问它的 `status` 拿 `web` 端口、要一张 `login.ticket`，开 `http://127.0.0.1:<端口>/#miyu=<base64url(JSON {v: 1, ticket, lang})>`（不带颜色、深浅）；程序没在跑、没写 `web` 的报「<编号> 没有在运行，启用它以后再打开」，退出码 1。别的发 `package.page`，开它交回的宿主页；没装、没有后台页的照核心的原话，退出码 1。网址总印一行，浏览器没开成的报「无法打开浏览器，请手动打开上面的地址」（同 `miyu mcp login`）照样退出码 0；`--print` 只印不开 | `package.list`、`package.call`、`login.ticket` 或 `package.page` |
| `update` | 从软件源拿新的索引（施工 G-3 上）：不管本机那份新不新都去拿；一个软件源一行（施工 CL-2 补：官方的在前，配置里 `packages.sources` 另加的照名字），「已更新 <源>：N 个包（<索引的时刻>）」；官方的一处都没拿成、用的是本机那份的报「无法更新 official：<原因>」，退出码 1；另加的没拿成的警告「无法更新 <源>：<原因>」（本机也没有的原因说「无法连接」），不算失败；官方的本机也没有的照核心的原话，退出码 1。`--format json` 原样印回应，报错、警告照样印在标准错误上 | `package.sync`，带 `force: true` |
| `upgrade` | 照软件源升级装着的包（施工 G-5）：先盘点，软件源里没了的、被拉黑的（另起一行提示怎么卸）、来源仓库变了的、`packages.hold` 跳过的在标准错误上各警告一句；没有要升的印「没有可升级的软件包」、退出码 0；有的一律先看一眼（`package.upgrade {"preview": true}`，写了 `--yes` 的也看：新多要的能力要照它批），印「将升级 <编号> <旧版本> → <新版本>」一个一行，下面一项一行「  新要：<能力>」（新版多要的，批过的不印，施工 PK-3 下）；一起装的新依赖「将安装 <编号> <版本>」，下面「  需要：<能力>」；「下载：<多大>」；空一行，核心给了 `notice` 的先一行「注意：…」，问「升级 '<编号>'？[Y/n]」（不止一个的「升级 <几> 个软件包？[Y/n]」；有新要的能力的「升级 '<编号>' 并批准新要的能力？[Y/n]」「升级 <几> 个软件包并批准新要的能力？[Y/n]」），升好了一个一行「已升级 <编号> <旧版本> → <新版本>」，再一个包一行「已批准 <编号> 的能力：<名字>、…」 | `package.upgrades`、`package.upgrade`（带 `approve`，见「怎么走」第 2¼ 步） |
| `clean` | 清掉缓存里不是装着那一版的包文件（施工 G-4 下）：印「已清理 N 个包文件，释放 X」，一个都没有的印「没有可清理的包文件」。`--format json` 原样印回应 | `package.clean` |
| `search [<词>…]` | 在软件源里搜（施工 G-3 下）：一行一个，`<编号>  <名字>  <说明>`，编号照最长的对齐；装着的接「已安装」，不是官方的接「社区」，被拉黑的接「已拉黑：理由」；版本不印。一个都没搜到的什么都不印、退出码 1（照 pacman）；用的是本机旧索引的在标准错误上警告一句。`--format json` 原样印回应 | `package.search` |
| `pack <包目录>` | 把包目录打成包文件（施工 G-1，设计 `37-软件源.md` 第二节）：照装的那一套读清单，要写了版本；清单写了 `targets` 的用 `--target` 挑一种、没写的不许写；写到 `-o` 给的目录（没有的建上，不写的是现在的目录），文件名 `<编号>-<版本>[-<系统>].miyu`；印三行：「已打包 <路径>」、`SHA-256: <哈希>`、大小（`--format json` 印 `{"package", "version", "target", "path", "sha256", "size", "logo_shrunk"}`）。清单写了 `[package] logo` 的（施工 F-18 上，`package-pages.md`「清单多的几格」）：超了 256 KiB 或者宽高超了 512 的，打进去的是缩好的那一份（PNG 照 PNG、WebP 编无损 WebP），源文件不动，先印一句「已把图标 <路径> 缩小到 <宽>×<高>，<字节> 字节（源文件未改动）」；图标不在、不是 PNG / WebP、缩不下去的报「无法打包：…」。不连核心，也不碰数据根：作者的 CI 上没有数据根，那个位置放着别的东西也照打（施工 G-7） | 不发 |

pacman 的写法当别名（设计 `31-软件包.md` 第一节，定了的 A）：`-U` 是 `install`，`-R` 是 `remove`，`-Q` 是 `list`，`-Qi`、`-Ql`、`-Qo`、`-Qk` 是 `info`、`files`、`owns`、`check`（`-Qi` 只查装着的，施工 IN-2），`-Si <编号>` 是只查软件源的 `info`（施工 IN-2；只接一个编号，不和 `-y`、`-s`、`-c`、`-u` 一起写，写了的参数不对，退出码 2），`-Sy` 是 `update`（施工 G-3 上），`-Ss` 是 `search`，`-Sys` 先拿再搜（施工 G-3 下）；`-S <编号>…` 从软件源装这几个、`-Sy <编号>…` 先拿索引再装、`-Sc` 是 `clean`（施工 G-4 下）；`-Su` 是 `upgrade`、`-Syu` 先拿索引再升（施工 G-5）；`-S` 什么都不带的照 pacman 报「没有指定目标」，另起一行提示「运行 'miyu pkg --help' 查看用法」（英文 `miyu: no targets specified`、`hint: run 'miyu pkg --help' for usage`；项目主人 2026-10-19 定照 pacman 说，CL-2 补照设计 47 改写法），退出码 2。`-Q` 后面的字母最多一个；`-Qi`、`-Ql`、`-Qo` 要接编号或路径，`-Q` 不带字母的不接（参数不对，退出码 2）。`-Qi`、`-Si` 只接编号：写的是路径的（施工 IN-4）不发请求，报「-Qi 只接软件包编号」，另起一行提示「运行 'miyu pkg info <路径>' 查看软件包目录或文件」（英文 `-Qi takes a package id`、`hint: run 'miyu pkg info <path>' to look at a package folder or file`；`-Si` 同），退出码 2：pacman 看本地包文件另有 `-p`，这里用 `info` 不另加字母。

| 选项 | 做什么 |
|---|---|
| `--format text\|json` | `list`、`info`、`files`、`owns`、`check`：`json` 原样印核心回的（`list` 印那一串，别的印整个回应），给脚本 |
| `--yes`、`--noconfirm` | `install`、`remove`、`upgrade`：不问，直接做（施工 F-8 下补，`--noconfirm` 是 pacman 的写法），给脚本；从软件源装的推荐的当「否」，不顺手多装（施工 IN-3）；装的、升级的能力照答「是」那样批，输出照样列出批了哪些（施工 PK-3 下，设计 61 定了的 6） |
| `--purge`、`-n` | `remove`：连配置目录 `etc/<编号>/`、配置里它的设置项一起删（施工 PK-5、PK-5补，2026-11-05 项目主人定照 pacman：不写的留下）；已经卸掉的编号写了的，删它留下的；`-n` 是 pacman `-Rn`（`--nosave`）的写法，`-Rn <编号>` 也认 |
| `--target <系统>`、`-o, --out <目录>` | `pack`：打给哪一种系统、写到哪个目录（施工 G-1） |

界面语言照 `cli/main.md`。用到的环境变量：`MIYU_HOME`。


#### 会话里（施工 BS-1 中下，设计 `44-基础系统拆包.md` 第三节第 4 条）

看见 `MIYU_CALL_SOCK`（这条命令跑在 Miyu 的会话里、开了通道）的，一律经 `miyu call` 的通道带着这条命令的令牌发 `op: "pkg"`（`cli/call.md`），不读本机令牌、不连核心的协议；印核心答回来的（就是 `packages`、`package` 两件工具的输出，英文），照它退出。通道连不上的说 `pkg-off.txt`、退出码 2，不退回去走协议（同 MC11）。

| 子命令 | 换成 |
|---|---|
| `list`、`-Q` | `packages {}` |
| `info <编号>`、`-Qi`、`-Si` | `packages {"package"}`（工具只答装着的，施工 IN-2 不改它）；写的是路径（同 `install` 认路径的写法）的 `packages {"path"}`：看那个包文件夹 |
| `search <词…>`、`-Ss` | `packages {"search"}`，几个词照空格连起来 |
| `install <路径\|编号>`、`-U`、`-S <编号…>` | `package {"install"}`，一个一次（施工 BS-1 中下修起：卸掉的出厂包、组名照装回走，不去软件源，问人的卡片是装回的那一份）；路径照现在的目录换成绝对的、`.`、`..` 照字面去掉，写到 `package.toml` 的取它所在的文件夹 |
| `remove <编号>`、`-R` | `package {"remove"}`；写了 `--purge`（`-Rn`）的说 `pkg-unavailable.txt`（`{command}` 是 `remove --purge`），退出码 2：她的工具不收 `purge`，连配置一起删要人在终端里自己敲（施工 PK-5） |
| `upgrade`、`-Su`、`-Syu` | `package {"upgrade": "all"}`（`-y` 在会话里不另拿索引） |
| `pack` | 照旧在本机打 |
| `files`、`owns`、`check`、`page`（开浏览器是人的事，施工 BP-2）、`update`、`clean`、只有 `-y` 或 `-c` 的 `-S` | 说 `pkg-unavailable.txt`（「miyu pkg files does not work inside a Miyu session.」），退出码 2 |

`--yes` 不认：问不问由权限策略判（完全放开下每次问人、只读时拒，施工 LV-1；卡片照看一眼的那一份），她跳不过；`--format` 不管，印的就是工具的输出。几次调用挨着做，有一次不成的照它的退出码退出、后面的不做。`packages` 包关着、通道没开的会话里没有 `MIYU_CALL_SOCK`，`miyu pkg` 照旧走协议、借本人的身份：同设计 39 MC10、MC11 记着的那一条，设计 44 不加新口子。

### 怎么走

1. **找数据根、连核心、握手**：照 `miyu memory`（`cli/memory.md` 第 1 步）。请求的编号是 `pkg-<16 位十六进制>-<序号>`。
2. **发**：照上面的表。`install` 一律先发同一条请求带 `preview: true`（要的能力装的时候就批，下面第 2¼ 步；旧的核心照旧打开时一并批，第 2½ 步），`remove` 没写 `--yes` 的也先发；没写 `--yes` 的（施工 F-8 下补，`protocol.md`），照回应印一份要做什么（第 3 步），空一行问「安装 '<编号>'？[Y/n]」「卸载 '<编号>'？[Y/n]」（装回出厂的「装回 '<编号>'？[Y/n]」），读标准输入的一行：空的、`y`、`yes`（不分大小写）接着发真的那一条；别的在标准错误上印「已取消」，退出码 1；标准输入关着的（脚本里、管道读完了）不做，标准输出补一个换行，标准错误报「已取消：没有收到确认」、另起一行提示「加 '--yes' 跳过确认」，退出码 1。看一眼被拒的（写错的、必需的……）照第 4 步，不问。
   - 从软件源装（施工 IN-3，设计 `49-装包照pacman.md` 第二节）：写的是编号的（不是包目录、包文件），发看一眼之前先在标准错误上印「正在解析依赖…」「正在检查冲突…」（英文 `resolving dependencies...`、`looking for conflicting packages...`）。看一眼的回应带 `download` 的就是从软件源装：没写 `--yes` 的照第 3 步印那一份，空一行问——一个包的「安装 '<编号>'？[Y/n]」，几个的「安装 N 个软件包？[Y/n]」；回应的 `recommends` 不空的接着（不空行）问「同时安装推荐的 N 个软件包？[Y/n]」（英文 `Also install N recommended packages? [Y/n]`，一个的写 `package`），空的、`y`、`yes` 是「是」，别的、标准输入关着的是「否」，照样装主包。`--yes` 两句都不问：主包当「是」、推荐的当「否」。真装照同一条请求，等回应时核心推来的 `package.progress` 照第 3 步印在标准错误上。主包装完（连第 2½ 步）以后，推荐的答了「是」的一个一个发 `package.install {"source": <编号>}`：先照样看一眼（只为拿版本、要的能力，不印、不问，也不再列它们推荐的），再真装（带 `approve`，照第 2¼ 步），进度、结果行、第 2½ 步照主包那一套；前面已经一起装上的跳过。施工 PK-3 下起问的两句、推荐的那一段见第 2¼ 步。
2¼. **装的时候就批**（施工 PK-3 下，设计 `61-装包说清来源和能力.md` 第三节、第四节第 2 条，定了的 5、6）：
   - **问的那一句**：看一眼里有要批的能力（列了、不是 `held` 的）、核心又认 `approve` 的，问的换成「安装 '<编号>' 并批准它要的能力？[Y/n]」、几个的「安装 N 个软件包并批准它们要的能力？[Y/n]」（英文 `Install '<id>' and approve the capabilities it needs? [Y/n]`、`Install N packages and approve the capabilities they need? [Y/n]`）；全批过的、没有能力的、装回出厂的照旧那一句。核心给了 `notice`（社区的、本机文件的）的，空一行以后先印一行「注意：」接核心的句子（中文的句子自带句号、直接接，英文的空一格接，前缀英文 `Note: `），紧接着问。官方的、出厂的没有这一行。
   - **推荐的那一段**：推荐的里有列了能力的、核心给了 `recommends_notice` 的，第二句之前空一行另印「推荐的包要的能力  : <编号>  <能力>」（一项一行，后面几行缩进到值那一列，编号照最长的补齐）、「注意：…」，问的换成「同时安装推荐的 N 个软件包并批准它（们）要的能力？[Y/n]」（英文 `Also install N recommended package(s) and approve the capabilities it needs / they need? [Y/n]`）；都没有的照旧那一句。答「是」的推荐的另装另批：每一个带 `approve`，批的是这一段列出来的它的那几样（真装之前那一次看一眼多出来的不批）。
   - **带什么**：答了「是」、写了 `--yes`（`--noconfirm`）的都算批，真装那一条带 `approve: {"<编号>": ["<能力>", …]}`：看一眼列出的每个包（要装的、`with` 里一起装的；升级的是 `upgrades`、`with`）的能力编号照原样带上（`held` 的也带，核心照并集记，不会多批）；没列能力的包不写，表是空的不带这一格。
   - **旧的核心**：只在看一眼是 PK-3 上以后的写法时才带——从软件源装的写了 `feed`，从包文件夹、包文件装的 `source` 是 `file`（以前的核心 `package.install` 的参数是 `deny_unknown_fields`，带了 `approve` 整条被拒；以前的看一眼也有 `capabilities`，不能照它判断）。升级照看一眼的 `upgrades`、`with` 里列的能力拼，以前的核心那里没有能力，表是空的、不带。旧的核心照旧问原来那一句，能力照旧在第 2½ 步打开时批。
   - **批了哪些**：装好、升好以后（「已安装」「已升级」那几行后面）一个包一行「已批准 <编号> 的能力：<名字>、…」（英文 `approved capabilities for <id>: <names>`），只列这一次新批的（不是 `held` 的）；答「是」的、`--yes` 的都印（定了的 6：输出照样列出批了哪些）。
2½. **装好以后问一句「打开？」**（作者说明第九节第 1 条，2026-10-11 项目主人照推荐定，施工 F-8 四补）：装好的那一项（`package.install` 的回应）有开关、程序在，关着的；或者开着却没在跑（不是 `running`、`starting`）、看一眼时又列了要的能力的（清单写了一起来就拉起，能力还没批）：空一行问「现在启用 '<编号>'？[Y/n]」，读一行，空的、`y`、`yes` 发 `package.enable {package, approve}`，`approve` 是看一眼时列出的那几个能力的编号，印「已启用 <编号>」；别的、标准输入关着的不打开，印「未启用 <编号>」、另起一行「提示：之后可以在界面的软件包页面启用」，照样算装好了，退出码 0。`--yes` 当是，不问。没有开关的（界面、小程序、只带技能脚本的）、程序不在的、已经在跑的不问。打开被拒的照第 4 步。
3. **印**（标准输出）：
   - `list`：一个一行，`<编号>  <名字>`，照编号排（核心把卸掉了的出厂的排在最后，这里照编号排进去），编号照最长的那个对齐、后面空两格；卸掉了的出厂的后面接「（已卸载）」；写错的没有名字，接「（清单有误：<哪里不对>）」。
   - 装、卸之前的那一份（施工 F-8 下补）：第一行「将安装 <编号> <版本>」，升级的接「（替换 <原来的版本>）」（原来没写版本的「（替换已装的）」），装回出厂的「将装回」，卸的「将卸载」；「包含：」程序（界面程序、扩展程序、小程序、内置功能）、命令 miyu <名字>、后台页、吉祥物、接入 <平台>、每个 MCP 服务一项「MCP 服务 <名字>」（施工 X-2）、技能、脚本（施工 X-13 上）、每个斜杠命令模板一项「斜杠命令 /<名字>」（施工 X-14 上补）、人格、预设、每个知识库一项「知识库 <编号>」（施工 PV-3）、几项设置；「需要：」声明的能力，一项一行（施工 PK-3 下）：核心给的名字接「：」和一句说明（英文接 `: `，没给说明的只有名字），批过的接「（已批准）」（英文 ` (approved)`），后面几行缩进到「需要：」后面那一列；「来源：」不经软件源装的从哪来（施工 PK-3 下，照看一眼的 `source`）：本机文件、出厂、系统的包管理器（英文 `local file`、`shipped with Miyu`、`system package manager`），不写路径（是人自己敲的），旧的核心不写 `source` 的不印；卸的「一并删除：」写了的设置的键；核心说了状态目录在哪多大的（施工 PK-4），另起一行「会删掉 <路径>（<大小>）」（英文 `Will delete <path> (<size>)`），大小照 `info` 的写法（照 pacman：不到 1 KiB 的写字节数，往上一位小数的 KiB、MiB、GiB），旧的核心只说有没有的（`state: true`）照旧在「一并删除」里列「状态目录」；核心说了配置目录、设置项的（施工 PK-5、PK-5补，`config: {path?, kept, settings?}`），再起一行：留下的「配置留在 <路径>」（英文 `Config kept in <path>`），另有设置项的接「，另有 N 项设置」（`, plus N setting(s)`，英文一项的说单数），没有配置目录的「N 项设置留在配置里」（`N setting(s) kept in the config`）；写了 `--purge` 的「会删掉 <路径>」（英文 `Will delete <path>`），要删的设置项照旧列在「一并删除」里、不再说几项；没说的（没东西可留、旧的核心）不印；「大小：」照 `info` 的写法，出厂的卸掉不删文件、没有这一行。几样之间用「、」隔开，没有的那一行不印。
   - 从软件源装之前的那一份（施工 IN-3，设计 49 第二节第 2 条）：「软件包 (N)」接要装的，照装的先后（依赖在前，要装的那个最后），`<编号>-<版本>`，空两格隔开；有推荐的另起一行「推荐 (N)」接没装的推荐的；两行的开头照宽的那个按显示宽度补齐、再空两格。空一行，几行「键 : 值」，对齐同 `info`：下载大小（`download`）、安装后大小（`installed`，核心没给的不印）、来源（施工 PK-3 下照每个包的 `source`、`feed` 说：官方的「官方，签名已验证」；官方源里社区收录的「社区，签名已验证」，别人加的源写「<源的名字>，签名已验证」，不是官方的另接「（<来源仓库>）」（核心给了 `origin` 的）；英文 `official, signature verified`、`community, signature verified (<origin>)`、`<feed>, signature verified (<origin>)`。几个包都一样的只写一行，不一样的一个包一行、前面接编号，编号照这一次要装的最长的补齐、再空两格，后面几行缩进到值那一列。旧的核心（看一眼没有 `feed`，`source` 是源的名字）照旧「官方，签名已验证」「<源的名字>，签名已验证，没人审过」，英文 `<source>, signature verified, not reviewed`）、需要的能力（一项一行，写法同上一条「需要：」；几个包的前面接编号，同来源；没有的不印）。从包目录、包文件装的、装回出厂的、卸的照上一条。
   - 装的进度（施工 IN-3，设计 49 第二节第 4 条，标准错误）：照 `package.progress` 的 `stage` 印，没写 `stage` 的（老的核心）当 `download`。下载：第一次推来时印「正在下载…」（英文 `downloading packages...`）；标准错误是终端的，每个包一行进度条 ` <编号>-<版本>  <大小>  [####----] <百分比>`：名字照这一次要装的最宽的补齐，大小右对齐占十格，条照终端宽度（量不到的当 80 列）占满、最后一格留着不写，放不下十格的不画条、再放不下的从后面截掉；照推来的刷新同一行（`\r` 加擦行），下完换行。不是终端的不画，下完一个印一行「已下载 <编号>-<版本> <大小>」（英文 `downloaded …`）。核对：「(i/N) 正在核对完整性」（英文 `(i/N) checking package integrity`），终端里刷新同一行、只留最后那个数，管道里一个一行。安装：「(i/N) 正在安装 <编号>」（英文 `(i/N) installing <id>`），一个一行。从包目录、包文件装的没有下载那一段，核对、安装各一行 `(1/1)`。印到一半的那一行在结果行、报错之前换行。
   - `install`：「已安装 <编号> <版本>」（没写版本的不接），接着照第 2½ 步问、启用的印「已启用 <编号>」；`remove`：「已卸载 <编号>」；卸的是系统的包管理器装的（回应的 `layer` 是 `distro`，施工 SD-1），接一行「提示：<编号> 是系统的包管理器装的，文件还在，Miyu 不再加载它；要删用系统的包管理器」。
   - `info`（施工 F-8 下起，IN-2 照设计 49 第一节改）：一格一行写成「键 : 值」，键照印出来的最宽的那个按显示宽度补齐（中文一个字两格），再空一格、冒号、空一格（同设计 49 的样子）。没有值的行不印，「依赖」「推荐」没有的写「无」。没装的（`from: "source"`）照这个先后：软件源（源的名字）、来源（来源仓库接「（官方）」或「（社区，没有经过官方审核）」；紧跟在软件源下面，施工 CM-1：官方源里社区收录的包「软件源」也写 `official`，下一行就分清官方、社区）、编号、名称、版本、描述、依赖、推荐（几个编号空两格隔开，依赖同）、提供（照装之前那一份「包含」的写法，索引里的子命令取第一个、功能和给别的包带的内容不列）、需要的能力（照核心给的名字）、作者、上游、许可证（施工 PV-3，清单写了的才有，照原样）、下载大小、安装后大小；装着的少软件源、下载大小，「来源」照旧在「需要的能力」后面，「安装后大小」后面多「被依赖」（空两格隔开）、最后一行「安装时间」（照这台机器此刻的时区到分钟；出厂的写「出厂自带」，本地库没记的没有这一行），来源写「出厂」「软件源」「本地」，从软件源装的接来源仓库（本地库只记来源仓库，分不出官方的源还是别人的源，不说官方）。几样之间用「、」连；大小照 pacman 写成 B、KiB、MiB、GiB，一位小数。从哪装的路径、哪一层、几个文件不印。用的是本机旧索引的不另说（`--format json` 里有 `stale`）。看包目录、包文件的（`from: "file"`，施工 IN-4）照装着的那几行，没有来源、被依赖、安装时间；「提供」后面多一行「无法读取」（英文 `Unreadable`），读不成的技能、脚本、斜杠命令模板一样一项，写法同装之前那一份：`<包里的路径>（<为什么>）`。
   - `files`：一个文件一行，`<编号> <绝对路径>`（照 pacman 的 `-Ql`），照路径排。
   - `owns`：「<路径> 属于 <编号>」；在数据根里装的包的目录里、本地库没记的（包自己后来写的）「<路径> 在 <编号> 的目录里，不是安装时带的」；哪个包都没有的报「没有软件包包含 <路径>」，退出码 1（照 pacman）。路径照换成的绝对路径印。
   - `check`：每个包改了的「<编号>：已修改 <路径>」、少了的「缺失」、多出来的「多出」，一个一行；都没问题的一行「<编号>：正常」，系统的包管理器装的（`layer` 是 `distro`，施工 SD-1）是「<编号>：正常（系统的包管理器装的，文件归它管）」、清单读不成的「<编号>：清单读不成（<代码>）」（算坏了）；数据根里一个记了的、系统加装的都没有的「没有可检查的软件包」。有改了、少了的退出码 1（照 pacman 的 `-Qk`），多出来的只是提示。
4. **被拒绝**：核心的原话去掉句号接在 `miyu: ` 后面印在标准错误上，退出码 1（`protocol.md` 的 `package.*`）。清单装不上的（`package_invalid`）例外：核心的原话是给程序看的（说的是去 `data` 里看），这里照 `data.problem`、`data.line` 报「无法安装 <写的>：package.toml:<几>：<哪里不对>」（英文 `cannot install <写的>: package.toml:<几>: <哪里不对>`，设计 47 第四节；`<写的>` 是命令里写的包目录、包文件或编号）。和装着的包撞了名的（`package_conflict`，施工 F-8 六补）也例外，照 pacman 列冲突：照 `data.conflicts` 一处一条报错「无法安装 <写的>：<哪一样> <名字> 已在 <包> 中」（子命令、功能、工具、技能、MCP 服务、知识库、斜杠命令、人格、预设；撞了一层的 `<包>` 写「出厂内容」「数据根」；英文 `cannot install <写的>: <kind> <name> exists in <package>`，那一层写 `the shipped content`、`the data root`），核心没列是哪几处的报「无法安装 <写的>：名称已被占用」。看一眼印的那一份多「人格 <编号>」「预设 <编号>」（施工 P-11 下）。`info` 两处都没有的（`unknown_package`，施工 IN-2）照设计 47 报「未找到软件包 '<编号>'」（英文 `package '<id>' not found`）。`info <路径>` 被拒的（施工 IN-4）照 `install` 那一套说：写错的、撞了名的也报「无法安装 <写的>：…」，看的人知道真装会卡在哪。核心断开、请求写不出去照 `miyu rename`。

### 样子

```text
base-memory   人格记忆
base-mermaid  画 mermaid 图（已卸载）
base-net      联网搜索
```

`miyu pkg info im`（没装的，施工 IN-2，照设计 49 第一节；「提供」照索引里有的说）：

```text
软件源      : official
来源        : github.com/miyu-agent/miyu-im（官方）
编号        : im
名称        : 接入 QQ
版本        : 0.2.1
描述        : 让她在 QQ 群和私聊里说话
依赖        : 无
推荐        : im-sponsor  memes
提供        : 命令 miyu im、技能 sticker
需要的能力  : 联网、替主人说话
作者        : miyu-agent
上游        : https://github.com/miyu-agent/miyu-im
下载大小    : 2.2 MiB
安装后大小  : 6.8 MiB
```

`miyu pkg info pudding`（装着的）、`miyu pkg -Ql pudding`、`miyu pkg -Qk`（施工 F-8 下）：

```text
编号        : pudding
名称        : 布丁
版本        : 1.2.0
依赖        : 无
推荐        : 无
提供        : 扩展程序、命令 miyu pudding、后台页、2 项设置
来源        : 本地
安装后大小  : 1.5 KiB
被依赖      : pudding-extra
安装时间    : 2026-10-11 14:03

pudding /home/me/.miyu/home/me/packages/pudding/package.toml
pudding /home/me/.miyu/home/me/packages/pudding/skills/draw/SKILL.md

pudding：已修改 skills/draw/SKILL.md
pudding：多出 cache.db
```

`miyu pkg info ./my-skill`（看本机的包目录，施工 IN-4；包文件 `./my-skill-1.2.miyu` 一样）：

```text
编号        : my-skill
名称        : 我的技能
版本        : 1.2
描述        : 做点事
依赖        : 无
推荐        : 无
提供        : 技能 notes、脚本 hello
无法读取    : skills/bad/SKILL.md（no front matter between --- lines）
作者        : Ann
安装后大小  : 305 B
```

`miyu pkg install im`（从软件源装，施工 IN-3，照设计 49 第二节；答了两个「是」，标准错误不是终端，下载那一段只印行）：

```text
正在解析依赖…
正在检查冲突…
软件包 (2)  embed-0.3.0  im-0.2.1
推荐 (1)    im-sponsor-0.1.0

下载大小    : 12.3 MiB
安装后大小  : 30.1 MiB
来源        : 官方，签名已验证
需要的能力  : 联网、替主人说话

安装 2 个软件包？[Y/n]
同时安装推荐的 1 个软件包？[Y/n]
正在下载…
已下载 embed-0.3.0 10.1 MiB
已下载 im-0.2.1 2.2 MiB
(1/2) 正在核对完整性
(2/2) 正在核对完整性
(1/2) 正在安装 embed
(2/2) 正在安装 im
已安装 embed
已安装 im 0.2.1
正在下载…
已下载 im-sponsor-0.1.0 0.1 MiB
(1/1) 正在核对完整性
(1/1) 正在安装 im-sponsor
已安装 im-sponsor 0.1.0
```

终端里下载那一段是进度条（80 列）：

```text
正在下载…
 embed-0.3.0        10.1 MiB  [##########################################] 100%
 im-0.2.1            2.2 MiB  [##########################################] 100%
(2/2) 正在核对完整性
```

`miyu pkg install weather`（从软件源装社区的包，施工 PK-3 下，设计 61 第三节；「注意：」后面是核心的字）：

```text
软件包 (1)  weather-0.3.0

下载大小    : 1.2 MiB
安装后大小  : 3.4 MiB
来源        : 社区，签名已验证（github.com/ann/weather）
需要的能力  : 联网：访问网络、开端口
              提供工具：给 AI 加能调用的工具

注意：这是个社区包，没有经过官方审核，可能存在风险。
安装 'weather' 并批准它要的能力？[Y/n]
```

几个包、来源不一样的：

```text
软件包 (2)  onebot-1.2.0  weather-0.3.0

下载大小    : 1.9 MiB
安装后大小  : 5.0 MiB
来源        : onebot   官方，签名已验证
              weather  社区，签名已验证（github.com/ann/weather）
需要的能力  : onebot   联网：访问网络、开端口
              weather  联网：访问网络、开端口
              weather  提供工具：给 AI 加能调用的工具

注意：weather 是社区包，没有经过官方审核，可能存在风险。
安装 2 个软件包并批准它们要的能力？[Y/n] y

推荐的包要的能力  : rec  联网：访问网络、开端口
注意：rec 是社区包，没有经过官方审核，可能存在风险。
同时安装推荐的 1 个软件包并批准它要的能力？[Y/n]
```

`miyu pkg install ./weather`（包文件夹，施工 PK-3 下）、`miyu pkg upgrade`：

```text
将安装 weather 0.3.0
包含：扩展程序、命令 miyu weather、2 项设置
需要：联网：访问网络、开端口
      提供工具：给 AI 加能调用的工具
来源：本机文件
大小：1.5 KiB，12 个文件

注意：这个包来自本机文件，没有经过官方审核，可能存在风险。
安装 'weather' 并批准它要的能力？[Y/n]
已安装 weather 0.3.0
已批准 weather 的能力：联网、提供工具

将升级 app 1.0 → 2.0
  新要：联网：访问网络、开端口
将安装 lib 1.0
  需要：当搜索服务供应商：在联网服务里列出来，选了它的，搜索词会交给它
下载：2.0 KiB

注意：lib 是社区包，没有经过官方审核，可能存在风险。
升级 'app' 并批准新要的能力？[Y/n]
```

`miyu pkg install ./pudding`、`miyu pkg remove pudding`（施工 F-8 下补）：

```text
将安装 pudding 1.3.0（替换 1.2.0）
包含：扩展程序、命令 miyu pudding、后台页、2 项设置
需要：联网
大小：1.5 KiB，3 个文件

安装 'pudding'？[Y/n]

将卸载 pudding 1.3.0
会删掉 /home/a/.miyu/var/lib/pudding（3.5 MiB）
配置留在 /home/a/.miyu/etc/pudding，另有 1 项设置
大小：1.5 KiB，3 个文件

卸载 'pudding'？[Y/n]
```

### 退出码

| 码 | 什么时候 |
|---|---|
| 0 | 做成了 |
| 1 | 找不到数据根、连不上核心；被拒绝（没有这个包、必需的、清单装不上、和出厂的撞了……）；核心断开；`owns` 哪个包都没有；`check` 查出改了、少了的；装、卸之前问了答不、没得答 |
| 2 | 参数不对：子命令写错、少了清单或编号（`cli/main.md`）；`-Qi`、`-Si` 接的是路径（施工 IN-4） |

### 给人看的字

| 键 | 中文 | 英文 |
|---|---|---|
| 装好了 | 已安装 {id}、已安装 {id} {version} | installed {id}, installed {id} {version} |
| 卸掉了 | 已卸载 {id} | removed {id} |
| 装不上（报错） | 无法安装 {what}：package.toml:{line}：{problem}、无法安装 {what}：{problem} | cannot install {what}: package.toml:{line}: {problem}, cannot install {what}: {problem} |
| 撞了名（报错，一处一条） | 无法安装 {what}：{kind} {name} 已在 {holder} 中、无法安装 {what}：{kind} {name} 是核心自带的、无法安装 {what}：名称已被占用 | cannot install {what}: {kind} {name} exists in {holder}, … is built into the core, cannot install {what}: names already in use |
| 已卸载 | （已卸载） | (removed) |
| 写错了 | （清单有误：{problem}） | (invalid: {problem}) |
| `info` 的键（施工 IN-2） | 软件源、编号、名称、版本、描述、依赖、推荐、提供、需要的能力、来源、作者、上游、许可证（施工 PV-3）、下载大小、安装后大小、被依赖、安装时间 | Repository、Name、Title、Version、Description、Depends On、Recommends、Provides、Capabilities、Origin、Author、Upstream、License、Download Size、Installed Size、Required By、Install Date |
| `info` 没有依赖、推荐 | 无 | None |
| `info` 看包目录、包文件时读不成的（施工 IN-4） | 无法读取 | Unreadable |
| `-Qi`、`-Si` 接了路径（报错，施工 IN-4） | {flag} 只接软件包编号；提示：运行 'miyu pkg info <路径>' 查看软件包目录或文件 | {flag} takes a package id; hint: run 'miyu pkg info <path>' to look at a package folder or file |
| `info` 的来源 | 出厂、官方、社区，没有经过官方审核、软件源、本地；{origin}（官方）、{origin}（社区，没有经过官方审核）、{origin}（软件源） | shipped、official、community, not officially reviewed、repository、local；{origin} (official)、{origin} (community, not officially reviewed)、{origin} (repository) |
| `info` 哪儿都没有（报错） | 未找到软件包 '{id}' | package '{id}' not found |
| 大小 | {size}，{n} 个文件 | {size}, {n} files（一个的写 1 file） |
| 出厂的 | 出厂自带 | shipped with Miyu |
| 属于 | {path} 属于 {id} | {path} is owned by {id} |
| 在目录里 | {path} 在 {id} 的目录里，不是安装时带的 | {path} is in the directory of {id} but was not installed with it |
| 没有包（报错） | 没有软件包包含 {path} | no package owns {path} |
| 查文件 | {id}：已修改 {path}、{id}：缺失 {path}、{id}：多出 {path}、{id}：正常 | {id}: modified {path}、missing、extra、{id}: OK |
| 没得查 | 没有可检查的软件包 | no packages to check |
| 将做什么 | 将安装 {id}、将装回 {id}、将卸载 {id}、将升级 {id} | Will install {id}、Will restore {id}、Will remove {id}、Will upgrade {id} |
| 替换 | （替换 {version}）、（替换已装的） | (replaces {version}), (replaces the installed one) |
| 程序 | 界面程序、扩展程序、小程序、内置功能 | interface、extension、worker program、built-in feature |
| 带的 | 命令 miyu {name}、后台页、吉祥物、接入 {platform}、知识库 {name}（施工 PV-3）、{n} 项设置 | command miyu {name}、admin page、mascot、{platform} connection、knowledge base {name}、{n} settings |
| 行首 | 包含：、无法读取：、需要：、一并删除：、大小：、一起装：、下载：、来源： | Includes: 、Unreadable: 、Needs: 、Also deletes: 、Size: 、Also installs: 、Download: 、From:  |
| 来源 | （官方）、（社区，未经审核） | (official), (community, not reviewed) |
| 删的 | 设置 {keys}、状态目录（旧的核心） | settings {keys}、state directory (older cores) |
| 会删掉的状态目录（施工 PK-4） | 会删掉 {path}（{size}） | Will delete {path} ({size}) |
| 配置目录（施工 PK-5） | 配置留在 {path}、会删掉 {path}（`--purge`） | Config kept in {path}、Will delete {path} (`--purge`) |
| 留下的设置项（施工 PK-5补） | 配置留在 {path}，另有 {n} 项设置、{n} 项设置留在配置里 | Config kept in {path}, plus {n} setting(s)、{n} setting(s) kept in the config |
| 问 | 安装 '{id}'？[Y/n] 、装回 '{id}'？[Y/n] 、卸载 '{id}'？[Y/n] 、升级 '{id}'？[Y/n] 、升级 {n} 个软件包？[Y/n]  | Install '{id}'? [Y/n] 、Restore …、Remove …、Upgrade …、Upgrade {n} packages? [Y/n]  |
| 不做 | 已取消 | cancelled |
| 没得答（报错） | 已取消：没有收到确认；提示：加 '--yes' 跳过确认 | cancelled: no confirmation; hint: pass '--yes' to skip the question |
| 问启用 | 现在启用 '{id}'？[Y/n]  | Enable '{id}' now? [Y/n]  |
| 启用了、没启用 | 已启用 {id}；未启用 {id}，提示：之后可以在界面的软件包页面启用 | enabled {id}; left {id} disabled, hint: enable it later on the Packages page |
| 看一眼之前（施工 IN-3） | 正在解析依赖…、正在检查冲突… | resolving dependencies...、looking for conflicting packages... |
| 要装的、推荐的（施工 IN-3） | 软件包 ({n})、推荐 ({n}) | Packages ({n})、Recommended ({n}) |
| 从软件源装的来源（施工 IN-3） | 官方，签名已验证；{source}，签名已验证，没人审过 | official, signature verified; {source}, signature verified, not reviewed |
| 问几个、问推荐的（施工 IN-3） | 安装 {n} 个软件包？[Y/n] 、同时安装推荐的 {n} 个软件包？[Y/n]  | Install {n} packages? [Y/n] 、Also install {n} recommended packages? [Y/n] （一个的写 1 recommended package） |
| 从软件源装的来源（施工 PK-3 下） | 官方，签名已验证；社区，签名已验证（{origin}）；{feed}，签名已验证（{origin}） | official, signature verified; community, signature verified ({origin}); {feed}, signature verified ({origin}) |
| 不经软件源装的来源（施工 PK-3 下） | 本机文件、出厂、系统的包管理器 | local file、shipped with Miyu、system package manager |
| 能力一项（施工 PK-3 下） | {name}：{summary}、批过的接（已批准） | {name}: {summary}、 (approved) |
| 注意（施工 PK-3 下，后面接核心的句子） | 注意： | Note:  |
| 升级时每个包下面（施工 PK-3 下） | `  新要：`、`  需要：`（前面两个空格） | `  New: `、`  Needs: ` |
| 推荐的那一段的键（施工 PK-3 下） | 推荐的包要的能力 | Recommended Capabilities |
| 带批准问（施工 PK-3 下） | 安装 '{id}' 并批准它要的能力？[Y/n] 、安装 {n} 个软件包并批准它们要的能力？[Y/n] 、同时安装推荐的 {n} 个软件包并批准它（们）要的能力？[Y/n] 、升级 '{id}' 并批准新要的能力？[Y/n] 、升级 {n} 个软件包并批准新要的能力？[Y/n]  | Install '{id}' and approve the capabilities it needs? [Y/n] 、Install {n} packages and approve the capabilities they need? [Y/n] 、Also install {n} recommended package(s) and approve the capabilities it needs / they need? [Y/n] 、Upgrade '{id}' and approve the new capabilities? [Y/n] 、Upgrade {n} packages and approve the new capabilities? [Y/n]  |
| 批了哪些（施工 PK-3 下） | 已批准 {id} 的能力：{names} | approved capabilities for {id}: {names} |
| 下载、核对、安装（施工 IN-3） | 正在下载…、已下载 {name} {size}、({i}/{n}) 正在核对完整性、({i}/{n}) 正在安装 {id} | downloading packages...、downloaded {name} {size}、({i}/{n}) checking package integrity、({i}/{n}) installing {id} |
| 拿到了索引（`update`，一个源一行） | 已更新 {source}：{n} 个包（{generated}） | updated {source}: {n} packages ({generated})（一个的写 1 package） |
| 没拿到（`update`：官方的报错，另加的警告） | 无法更新 {source}：{why}；本机也没有的原因：无法连接 | cannot update {source}: {why}; unreachable |
| 搜的时候用的旧索引（警告） | 无法连接软件源，使用本机索引（{generated}）：{why} | cannot reach the package source, using the local index ({generated}): {why} |
| `-S` 什么都没带（报错） | 没有指定目标；提示：运行 'miyu pkg --help' 查看用法 | no targets specified; hint: run 'miyu pkg --help' for usage |
| 清了 | 已清理 {n} 个包文件，释放 {size}、没有可清理的包文件 | removed {n} package files, freed {size}、no package files to clean |
| 搜到的标记 | （已安装）、（社区）、（已拉黑：{reason}） | (installed), (community), (blocked: {reason}) |
| 升级 | 没有可升级的软件包；已升级 {id} {from} → {to} | nothing to upgrade; upgraded {id} {from} → {to} |
| 升级时的警告 | {id} 已不在软件源中，保留已安装的版本；{id} 已被拉黑：{reason}（提示：用 'miyu pkg remove {id}' 卸载）；{id} 改由 {now} 提供，未升级；跳过设为不升级的 {id} | {id} is no longer in the package source, keeping the installed version; {id} is blocked: {reason} (hint: remove it with 'miyu pkg remove {id}'); {id} now comes from {now}, not upgraded; skipping held package {id} |
| 打包 | 已打包 {path}、大小：{n} 字节、已把图标 {logo} 缩小到 {w}×{h}，{n} 字节（源文件未改动） | packed {path}、Size: {n} bytes、shrank logo {logo} to {w}×{h}, {n} bytes (source file unchanged) |
| 打包失败（报错） | 无法打包：{why}（找不到图标 {logo}、图标 {logo} 不是 PNG 或 WebP 图片、清单没有写版本……，能照着改的另起一行提示） | cannot pack: {why}（logo {logo} not found, the manifest has no version, …; hints on their own lines） |

报错、警告照设计 47 印在标准错误上：`miyu: <事>`（警告是 `miyu: 警告：`，英文 `miyu: warning: `），建议另起一行 `提示：`（英文 `hint: `）；下表报错那几行写的是 `miyu: ` 后面那一截。

核心拒绝时说的话去掉句号照原样印。

**帮助页**：`-h`、`--help`、`miyu help pkg`、各个子命令的 `-h` 印的都是这一页，规矩见 `cli/main.md`「帮助页」（`crates/miyu-cli/src/help/zh/pkg.txt`、`en/pkg.txt`）。

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-cli/src/pkg/tests.rs` | 一行的样子：照编号排、对齐，卸掉了的、写错了的各接一截，别的不印；带 `/`、以 `.toml` 结尾的是清单（照现在的目录换成绝对的），别的是编号；pacman 的写法换成正式的子命令，字母写多了、少了接的都不认（施工 F-8 下）；`-Si`、`-Qi` 换成带 `source` 的 `info`，`-Si` 不接、接两个的不认（施工 IN-2）；`remove --purge`、`-Rn` 认成带 `purge` 的 `remove`（施工 PK-5） |
| `crates/miyu-cli/src/pkg/confirm/tests.rs`（施工 F-8 下补） | 升级、装回、卸的那一份：每一样怎么说、没有的那一行不印（卸的「配置留在」「会删掉」配置目录中英两种，施工 PK-5；留下几项设置、没有配置目录的、`--purge` 的不说几项，中英，施工 PK-5补）；回车、`y`、`yes` 接着做，别的和标准输入关着的不做、各说一句 |
| `crates/miyu-cli/src/pkg/info/tests.rs`（施工 IN-2） | `info` 的「键 : 值」：中文、英文照显示宽度对齐（中文两格），没装的、装着的各几行的先后，没有值的行不印，依赖、推荐空的写「无」，出厂的写「出厂自带」，从哪装的路径不印；`-Si`、`-Qi` 各带的 `source`；看包目录、包文件的那几行、「无法读取」接在「提供」后面（施工 IN-4）；官方源里社区收录的包「来源」紧跟「软件源」、写「（社区，没有经过官方审核）」，官方的写「（官方）」（施工 CM-1） |
| `crates/miyu-cli/src/pkg/summary/tests.rs`（施工 IN-3） | 从软件源装之前那一份的中英两份样子、核心没给的行不印、别的源的说法；一个、几个包问的那一句；有推荐的才问推荐的 |
| `crates/miyu-cli/src/pkg/approve/tests.rs`、`summary/tests.rs`、`confirm/tests.rs`、`upgrade/tests.rs` 里施工 PK-3 下那几条 | 认不认 `approve` 的核心（有 `feed` 或 `source: "file"` 才认）；照看一眼拼表（批过的也带、没列能力的和推荐的不写、空的不带）；批了哪些只列没批过的；能力一项一行带说明、批过的标出来；「注意」照核心的句子原样接。设计 61 第三节三个确认屏中英逐字对；官方的没有「注意」、来源都一样的只写一行；问的那一句有、没有要批的两种（全批过的、没有能力的、旧的核心照旧）；推荐的另一段；升级只列新要的、一起装的新依赖列它要的 |
| `crates/miyu-cli/tests/pkg_approve.rs`（施工 PK-3 下） | 在进程里起的核心：从包文件夹装要能力的扩展，确认屏照核心的字中英逐字对，答「是」的带上 `approve`、开关文件记下、输出列出批了哪些，答「否」的不装不批；`--noconfirm` 不问照样批；升级 2.0 新要联网的列在「将升级」下面、问「并批准」，答「否」不批，`-Su --yes` 批了、列出来；推荐的要能力的另印一段，答「是」另装另批 |
| `crates/miyu-cli/src/pkg/progress/tests.rs`（施工 IN-3） | 一行进度条照终端宽度排（最后一格留着、越宽条越长）、百分比、窄终端不画条再截掉；终端里刷新同一行、管道里一个一行、没写阶段的当下载 |
| `crates/miyu-cli/tests/pkg_page.rs`、`crates/miyu-cli/src/pkg/page/tests.rs`（施工 BP-2） | 在进程里起的核心：目录里的后台页印宿主页的地址；程序自己起服务、程序没在跑的说一句、退出码 1；没装的照核心的原话、退出码 1（都带 `--print`）；片段是 JSON 的 base64url、不带 `=` |
| `crates/miyu-cli/tests/pkg_info_path.rs`（施工 IN-4） | 在进程里起的核心：`info ./包目录`、`info 包目录/` 照现在的目录换成绝对的，中英文的「键 : 值」逐字对、什么都不装，`--format json` 原样印、带绝对路径；写错的清单照真装那样报「无法安装 ./broken：package.toml:5: …」，退出码 1；`-Qi`、`-Si` 接路径的报用法错，退出码 2 |
| `crates/miyu-cli/tests/pkg_stages.rs`（施工 IN-3） | 在进程里起的核心、假的软件源：管道里一整次安装逐字对（中文，推荐的答「是」主包装完再装）；`--yes` 不问、不装推荐的（英文逐字对）；推荐的答「否」照样装主包、不下推荐的 |
| `crates/miyu/tests/pkg_progress.rs`（施工 IN-3，只在 Unix 上） | 真的核心、真的 `miyu pkg`：标准错误是伪终端的画进度条、照终端宽度、刷到 100%，`--yes` 不问推荐的、不装；接管道的只印行、没有 `\r` 和控制序列 |
| `crates/miyu-cli/src/pkg/query.rs` 的测试（施工 F-8 下） | `files` 是编号加绝对路径；`owns` 分装的时候带的和后来写的；`check` 一个一行、只有改了少了的算出错；字节数的写法 |
| `crates/miyu/tests/pkg.rs` | 真跑：列出、照相对路径装、卸；卸掉出厂的标「已卸载」、照编号装回来；必需的卸不掉、写错的清单装不上，退出码 1；`--format json`；`-h`、子命令的 `-h`、`help pkg` 印的都是那一页，跟着界面语言；`-U` 装、`info`（「键 : 值」、`-Qi` 没装的报「未找到软件包」，施工 IN-2）、`-Ql` 的绝对路径、`-Qo` 归哪个、没有的退出码 1、`-Qk` 正常和改了的退出码 1（施工 F-8 下）；装之前印的那一份、答不就没装、回车装上、标准输入关着的不卸、`--noconfirm` 不问（施工 F-8 下补） |

### 还没有的

- 搜索、软件源、停用（`22-命令行.md` 第五节的 `miyu pkg …`）：随软件源那一段。
