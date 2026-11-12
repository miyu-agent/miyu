# 写一个 Miyu 软件包

给第三方作者。英文版：[README.en.md](README.en.md)。这个目录里的说明、模板和例子用 Apache-2.0（仓库根目录 `LICENSES/Apache-2.0.txt`），照着写的东西用什么许可证都行；只经协议和 Miyu 说话的软件包、扩展是独立的作品，核心的 AGPL 管不到它们。协议的每个方法查 [协议参考](../reference/README.md)。

## 包能带什么

**一个文件夹就是一个包**，文件夹名就是编号，清单是文件夹里的 `package.toml`。一个包能带下面这些，几样混着带也行：

| 想做的 | 带什么 | 看哪一篇 |
|---|---|---|
| 接一个现成的 MCP 服务 | `[[mcp]]` | [接 MCP 服务](mcp.md) |
| 给 AI 一件小工具，几十行脚本就够 | `scripts/` 下一个脚本 | [放文件就能用的](scripts-skills.md) |
| 教 AI 一套做事的办法 | `skills/` 下一个技能 | 同上 |
| 一条斜杠命令，展开成一句话发给 AI | `commands/` 下一个模板 | 同上 |
| 一套给她查的资料（设定集、手册、笔记） | `kb/` 下一个知识库 | 同上 |
| 一个人格、一个预设 | `personas/`、`presets/` 下 | 同上 |
| 常驻的程序：一组工具、接一个聊天平台、往界面上放东西、自己的斜杠命令 | 扩展（`[process]`） | [写扩展](extensions.md) |
| 网页里的一个设置页 | 后台页（`[page]`） | [写扩展](extensions.md) |

`scripts/`、`skills/`、`commands/`、`kb/`、`personas/`、`presets/` 照约定放，清单里不用写；清单里每一格怎么写见 [清单](manifest.md)。用 Python、Rust 写扩展的，有现成的开发包（[用开发包写](extensions.md#用开发包写)）。

**不想做成包的**：技能、脚本、命令模板直接放进 `~/.miyu/root/skills/`、`scripts/`、`commands/`，MCP 服务写进配置，一样能用，只是不归包管理。

## 一个包长什么样

```text
hello/
├── package.toml          # 清单
├── bin/
│   └── miyu-hello        # 扩展的程序：编出来的程序（Windows 上是 miyu-hello.exe），或者带 #! 的脚本
├── scripts/
│   └── dice.py           # 脚本
├── skills/
│   └── take-notes/
│       └── SKILL.md      # 技能
├── kb/
│   └── hello-docs/       # 知识库：kb.toml 和文档
└── page/
    └── index.html        # 后台页
```

1. **编号**（文件夹名）：小写字母开头，只用小写字母、数字、`-`、`_`，最多 64 个。起个短的、说得出它是什么的名字就行，不用带 `miyu-`，也不用带包的种类（`scripts-`、`skills-` 这些）。不能和随 Miyu 出厂的包撞（出厂的都叫 `base-<名字>`，例如 `base-read`、`base-memory`，所以别用 `base-` 打头），也别和官方的包（`tui`、`webui`、`embed`、`im` 这些）、别人的扩展撞（同一个编号再装是升级，会换掉人家装着的那一个）。要发到软件源的，起名前先 `miyu pkg search <词>` 看占没占：软件源的编号先到先得（下面「发到软件源」）；这几篇里的 `hello`、`dice` 只是例子，`dice` 官方源里已经有了。
2. **带的名字也别撞**：工具名、技能、MCP 服务、斜杠命令和装着的别的包撞了的，装不上。名字建议带上包的编号做前缀。
3. **包文件夹装好以后别往里写**：装的时候每个文件都记了哈希，后来写进去的会被 `miyu pkg check` 报出来。运行时的状态写进 `MIYU_STATE_DIR` 指的状态目录，给人改的配置放 `MIYU_CONFIG_DIR` 指的配置目录（其余几格见扩展那一页）。
4. **人的数据别放状态目录**：卸包时状态目录整个删掉，卸之前那一屏会写「会删掉 <路径>（<大小>）」。人自己的东西（导出的记录、迁移过来的资料、生成给人看的文件）放 `MIYU_DOCUMENTS_DIR`（`root/documents/`）、`MIYU_PICTURES_DIR` 这类人看得见的地方，卸包不动它们；状态目录只放你这个包自己用的（数据库、登录态），卸了就跟着包一起走。
5. **给人改的东西放配置目录**：`MIYU_CONFIG_DIR`（`etc/<编号>/`）照 pacman 卸包默认留下，卸之前那一屏写「配置留在 <路径>」，重装同编号照旧读得到；人用 `miyu pkg remove --purge` 才连它一起删。清单里声明的设置项（`[settings.<名字>]`，写在 `etc/config.toml` 里的）也一样：卸包默认留着、不生效，`miyu check` 只提一句是你的包留下的，重装照原样生效，`--purge` 才删。所以：人手改的设置文件放配置目录，卸包会留；你自己的状态放状态目录，卸包会删。别把要跟着包一起走的东西写进配置目录。

## 装、试、查

1. **先看一眼**：`miyu pkg info ./hello` 照装的那一套查清单，列出带了什么（脚本、技能、人格、命令、设置项、要的能力），什么都不装；清单写错的在这里就报，说第几行。
2. **装**：`miyu pkg install ./hello`。先印一份要做什么（带了什么程序、加什么命令、要什么能力和每一样的说明、从哪来、多大），问一句 `[Y/n]`：答是就装上、同时批了它要的能力；`--yes` 不问，同样算批。装好当场生效，不用重启核心；开着就跑的（`start = "always"`）装上就跑。
3. **打开**：手动开的装好以后问一句「现在打开？」，答是就打开，能力装的时候已经批过，不再问；答不的，以后在终端或网页的「软件包」页里打开。只带技能、脚本、知识库、人格的包没有开关，每个技能、脚本在预设里各自开关。
4. **改了再装**：开发时改完再 `miyu pkg install ./hello` 一次就换上了（就是升级）。想在一份单独的数据里试，不碰自己平常用的：命令前面都加上 `MIYU_HOME=/tmp/miyu-try`，试完 `MIYU_HOME=/tmp/miyu-try miyu core stop` 停掉那一份的核心再删目录。
5. **看**：`miyu pkg list`（装了哪些）、`miyu pkg info hello`（名称、版本、带了什么、大小、装的时间）、`miyu pkg files hello`（装了哪些文件）、`miyu pkg check hello`（装好的文件改没改）、`miyu check`（清单、脚本、技能写没写错）、`miyu kb list`（知识库，包带的写「来自 <包名>」）。「软件包」页上看得到扩展在跑、在等着重启、还是停下了（停下的写原因、标准错误的最后几行）。
6. **日志**：扩展的标准错误在 `~/.miyu/var/log/<编号>/stderr`；核心的运行日志在 `~/.miyu/var/log/core.log`。
   - **让核心一直开着**：核心起来了就不因为空闲退出，扩展跟着它一直在，通讯平台的桥、定时的活不用等人打开 `miyu`。要开机、登录就起的，跑 `miyu service enable`（Linux 的 systemd 用户服务、macOS 的 launchd、Windows 的任务计划）。
7. **卸**：`miyu pkg remove hello`。包文件夹、它放状态的目录一起删；配置目录和设置里它的项默认留着（上面第 5 条），`miyu pkg remove --purge hello` 才一起删；密钥文件里的条目不删（别处可能也在用）。

## 常见报错

| 看到的 | 怎么办 |
|---|---|
| 无法打包：清单没有写版本 | `[package]` 里写 `version = "0.1.0"` |
| 需要指定目标系统 | 清单写了 `targets`：`miyu pkg pack --target <系统>`，一种一个 |
| `[[slash]] needs the commands capability`（`bad_slash`）这类「要某项能力」 | 带了 `[[slash]]`、`[[search]]`、`[guard]`、`[after]`、`[[driver]]` 的，`[process] capabilities` 要写对应的能力（见 [清单](manifest.md) 各节） |
| `package_conflict` | 带的名字（命令、工具、技能、知识库、MCP 服务、前缀……）和装着的别的包撞了，一处一行写着是谁；改名再装 |
| `depends_missing` | `[depends]` 里的包没装、版本不够，先装它 |
| `needs_newer_miyu` | 清单的 `[package] miyu` 比正在跑的 Miyu 新，更新 Miyu |
| 调方法回 `needs_approval` | 清单没声明那项能力，或者人还没批：加进 `capabilities` 再装一次 |
| 扩展停下了 | 「软件包」页看原因，再看 `var/log/<编号>/stderr`；退出码 1 是「重启也没用」，核心不再拉起 |
| 脚本、技能没出现 | `miyu check`：开头写错的、重名的都在这里说为什么跳过 |

## 发到软件源

1. **写版本**：清单的 `[package] version` 照 semver 写（`0.1.0`）。不写的装得上，但打不了包、进不了软件源。每发一次加一次版本：软件源只认版本，同一个版本重发不算更新。
2. **打包**：`miyu pkg pack ./hello` 照装的那一套查清单，打成 `hello-<版本>.miyu`，印 SHA-256 和大小；`-o <目录>` 换输出的地方。清单写了 `targets` 的，一种系统打一个：`--target linux-x86_64`，文件名多一段系统。版本库和缓存不打进去：`.git`、`.github`、`.hg`、`.svn`、`__pycache__`、`.DS_Store`、`*.pyc`，在哪一层都一样；别的都进，要留在包外面的打包前挪走。打出来的文件可以先在一份单独的数据里试装：`miyu pkg install ./hello-0.1.0.miyu`。
3. **发**：包放在自己的 GitHub 仓库里，加一份 `.github/workflows/release.yml`，用官方的打包模板：

   ```yaml
   name: release
   on:
     push:
       tags: ["v*"]
   jobs:
     pack:
       uses: miyu-agent/packages/.github/workflows/pack.yml@main
       permissions:
         contents: write
       with:
         package: hello
   ```

   打 tag（`v` 加上清单的 `version`，`v0.1.0`）推上去，模板照 Miyu 装包的那一套查清单、打成 `.miyu`、传进这个 tag 的 Release；tag 和 `version` 对不上的当场报错。包文件夹不在仓库根目录的写 `folder`，打包前要先编程序的写 `build`（一段 bash），写了 `targets` 的用 `strategy.matrix` 一种系统跑一趟（`runs-on`、`target`）。细节见 [miyu-agent/packages](https://github.com/miyu-agent/packages)。也可以不用模板，自己 `miyu pkg pack`、把 `.miyu` 传进 Release：文件名别改，收录照文件名核对编号、版本、系统。
4. **进社区收录**：给仓库打上话题（topic）`miyu-package`。软件源每天收一次（UTC 03:17 前后），每个仓库只看最新的那个正式 Release（草稿、预发布的不算），收里面的 `.miyu`，签名后发出去，标「社区」：所以发了以后最多等一天。一个 Release 里能放几个包，同一个包的几份文件版本要一样。分叉出来的仓库、存档了的仓库不收；Release 里有一个 `.miyu` 读不成的，这个仓库这一趟整个不收。
5. **编号先到先得**：第一个收录了这个编号的仓库一直占着它（记在软件源的 `names.toml`），别的仓库再用这个编号的不收；同一趟两个仓库头一回抢同一个编号的，建得早的那个得。改名就是换一个新编号。
6. **查收没收进来**：`miyu pkg update` 再 `miyu pkg search <编号>`。没有的，看软件源仓库 Actions 里最近一趟 collect 的摘要，每个没收的仓库一行原因（`skipped, a fork`、`no package file in the latest release`、`hello-0.1.0.miyu should be …` 这类）。作恶的包和仓库会被拉黑（`blocked.toml`）；进官方列表要项目主人点头。

## 经系统源发

也能经系统的包管理器（AUR、Homebrew 这些）发，Miyu 自己的终端、网页就是这样发的（`miyu-tui`、`miyu-webui` 两个包）。Miyu 不另设
审核：系统源由各家自己审，核心只管装上以后照规矩读、查。

1. **装到哪**：把包文件夹原样装进 Miyu 认的「系统加装」那一处，成为 `<那一处>/<编号>/package.toml`：
   - Linux：`$XDG_DATA_DIRS` 里每一处的 `miyu/packages/`（没设时 `/usr/local/share`、`/usr/share`，先列的优先），系统包装进
     `/usr/share/miyu/packages/<编号>/`；Linuxbrew 装在它自己的 `share` 下，用户要把它写进 `XDG_DATA_DIRS`；
   - macOS：Homebrew 前缀的 `share/miyu/packages/`（前缀照 `miyu` 所在的目录往上一级算）；
   - Windows：没有这一处，只经软件源发。
2. **包文件夹的样子**：同 `miyu pkg pack` 打进 `.miyu` 的那一份，清单照原样，程序放在包文件夹的 `bin/`、带可执行权限。别往核心自己的
   目录（`/usr/lib/miyu/`、Homebrew 的 `libexec/`）里放：那是核心的出厂层，归 `miyu-core` 管。
3. **包名和依赖**：包名建议 `miyu-<编号>`。依赖写 `miyu-core`（核心的包；`miyu-agent` 是合集，别依赖它），版本下限写第一个说得了你
   清单 `protocol` 的核心版本（现在协议只有 `1`，发了的核心都说得了，不写下限也行）。依赖别的 Miyu 包的，清单的 `depends` 和系统包的依赖都写上。
4. **装好以后**：核心起来时读一次（已经开着的重启核心才认）。`miyu pkg list` 里这个包的层是 `distro`。清单的协议、适用系统、依赖
   对不上的不加载，`miyu check` 报原因；写错了，坏的只是这一个包。
5. **和软件源一起发**：同一个包两边都可以有。用户用 `miyu pkg install` 装了的进数据根，盖过系统装的；系统装的那份更新时，`miyu check`
   报一条 `shadowed` 警告，说怎么换回去。
6. **卸**：`miyu pkg remove <编号>` 碰到系统装的，只记一笔不再加载，文件不动，删文件要用系统的包管理器；记了的照编号
   `miyu pkg install <编号>` 装回来。
7. **自己先试**：环境变量 `MIYU_DISTRO_PACKAGES` 指一处，核心就只把那一处当系统加装那一层；配一个临时数据根 `MIYU_HOME`，跑
   `miyu pkg list` 看层是不是 `distro`、`miyu pkg check <编号>` 看程序找不找得到。

AUR 的 `PKGBUILD` 大致这样（照自己的包改）：

```sh
pkgname=miyu-hello
depends=(miyu-core)
package() {
  install -d "$pkgdir/usr/share/miyu/packages"
  cp -a hello "$pkgdir/usr/share/miyu/packages/hello"
}
```

Homebrew 的 formula：

```ruby
depends_on "miyu-core"

def install
  (share/"miyu/packages/hello").install Dir["hello/*"]
end
```

## 三个平台

1. 扩展的程序名照清单写，`bin/<名字>` 可以是编出来的程序，也可以是第一行写了 `#!` 的脚本（`#!/usr/bin/env python3` 这样）。Windows 上核心先找 `<名字>.exe`，没有的照 `bin/<名字>` 的 `#!` 找解释器（`python3` 找的是 `python`，写了这台机器上在的绝对路径的照它），三个平台一份文件、一份清单。Miyu 不带解释器：要哪个写进包的说明，请人自己装。脚本工具在 Windows 上没有 `#!` 的照扩展名找（`.py` 用 `python`，`.ps1` 用 PowerShell，`.js` 用 `node`）。
2. Linux、macOS 上程序、脚本要有可执行权限和 `#!` 那一行；装的时候照原样拷，权限跟着走。
3. macOS（只考虑 Apple 芯片）：从网上下载的程序带隔离标记，没签名、没公证的会被 Gatekeeper 拦。发出去的程序请签名并公证。
4. 「请退出」三个平台都是标准输入读到头，别靠信号。

## 给模型看的字

工具和技能的说明、参数说明、你往她上下文里放的字，都是给模型看的，每一轮跟着请求发，越短越省钱：

- 写英文短句。一件工具最多三句：是什么、什么时候用、最容易踩的那一个坑。
- 调用以后才用得上的知识（字段什么意思、怎么翻页）写进工具的**输出**，不写进说明。
- 给人看的名字、说明另有地方写（`display_name`、`summary`，照语言），不混进给模型的字里。
