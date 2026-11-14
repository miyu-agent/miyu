# 数据和备份

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

Miyu 把你的会话、设置、key、记忆放在哪，哪些要备份、哪些能删，怎么换机器、换位置，都在这一页。

## 数据根在哪

Miyu 的东西几乎都在一个目录里，叫数据根（默认 `~/.miyu`）：

| 系统 | 默认位置 |
|---|---|
| Linux、macOS | `~/.miyu` |
| Windows | `%USERPROFILE%\.miyu` |

装、升级、卸载 Miyu 都不碰数据根。

### 换一个位置

设环境变量 `MIYU_HOME`，指到另一个目录：

```bash
export MIYU_HOME=/data/miyu
```

```powershell
$env:MIYU_HOME = 'D:\miyu'
```

这样设的只管这一个 shell 窗口。要以后都认，见下面的表和表后的说明。

- 要写绝对路径，开头的 `~` 照家目录换开。写了相对路径的，报「MIYU_HOME 要写绝对路径，写的是 …」。
- 设成空的等于没设。
- 目录不在的，第一次用时建好。Linux、macOS 上新建的目录只有你能进（`0700`）；Windows 上照上一级目录继承的访问控制。

`MIYU_HOME` 照启动那一刻的环境读，谁从哪里起来，就看谁的环境：

| 谁 | 怎么认 |
|---|---|
| 命令行、终端、网页、shell 集成 | 照你敲命令的那个 shell 的环境。设在 shell 的配置里（bash、zsh 的 `~/.bashrc`、`~/.zshrc` 这类，PowerShell 的 `$PROFILE`），新开的 shell 都认；Windows 上也可以设成用户环境变量，见表后 |
| 软件包、脚本、MCP 服务、AI 跑的命令 | 核心拉起它们时交给它们，和核心是同一个 |
| `miyu service enable` 装的服务 | 装的时候设了的，写进服务文件。换了数据根要再敲一次 `miyu service enable`。Windows 上设了 `MIYU_HOME` 装不了服务，见 [常驻和服务器](service.md#开机或登录就起来) |
| 托盘 | 照命令行的 `--home <数据根>`，没写的照 `MIYU_HOME`。「开机时启动」把它写进启动项，Windows 上也带得上，见 [托盘](heads/applet.md#开机时启动) |
| herdr 恢复窗格 | 不存命令前面的临时变量，见 [终端](heads/tui.md#和-herdr-一起用) |

从应用菜单打开的托盘看不到 shell 配置里的变量；macOS 上从访达、程序坞打开的 `Miyu.app` 也一样。在终端里带上 `--home` 打开托盘，勾上「开机时启动」，启动项里记下这个参数，以后登录就照这个数据根起：

| 系统 | 在终端里敲 |
|---|---|
| Linux | `miyu-applet --home /data/miyu` |
| macOS | `/Applications/Miyu.app/Contents/MacOS/miyu-applet --home /data/miyu`（`Miyu.app` 不在「应用程序」里的，换成它的位置） |

Windows 上设用户环境变量 `setx MIYU_HOME D:\miyu`，之后新开的程序都认；开机启动用托盘的「开机时启动」，不用 `miyu service`：设了 `MIYU_HOME` 的装不了服务。

### 标记文件

数据根顶上有一个 `.miyu-layout`，里面一行 `This directory is a Miyu data root (layout 3).`。Miyu 只认有这个标记的目录：

- 目录不在或者是空的：写下标记，当成新的数据根。
- 有别的东西、没有标记：一个字节都不动，报「… 里有别的东西，认不出是 Miyu 的数据根，没有动它。是 0.7 版的数据目录的话，先 mv … …-0.7，再 miyu pkg install migrate、miyu migrate …-0.7；不是的话把 MIYU_HOME 设成一个空目录」。0.7 旧版的数据目录就是这一种，见 [从 0.7 旧版迁过来](migrate.md)。
- 只有旧标记 `.miyu-root`：是发版前的开发版留下的数据根，报「… 是旧版 Miyu 的数据根，新版不认，没有动它。把 MIYU_HOME 设成一个空目录」。
- 标记里的布局号不是这一版的：报「… 的布局不是这一版的，没有动它」。

别删、别改这个文件。

## 里面每一处

```text
~/.miyu/
├── .miyu-layout          标记
├── etc/                  配置和凭据
│   ├── config.toml
│   ├── trust.toml
│   ├── extensions.json
│   ├── kb.toml
│   ├── private/          secrets.toml、access.json、logins.json
│   ├── removed/
│   ├── leftover/
│   └── <编号>/           各软件包的配置
├── opt/<编号>/           你装的软件包
├── root/                 你的东西
│   ├── sessions/  personas/  presets/  prompts/  tools/
│   ├── skills/  scripts/  commands/  kb/  memory/  blobs/
│   ├── workspace/  pictures/  videos/  documents/  downloads/
│   ├── trash/  state/
│   ├── schedules.db  projects.json  avatar.<png|jpg|webp>
├── var/
│   ├── lib/              Miyu 和各软件包的状态
│   ├── cache/            算得出来、下得回来的缓存
│   ├── log/              日志
│   └── tmp/              跨重启的暂存
├── tmp/                  短命的暂存
└── run/                  锁、套接字位置、本机令牌
```

「手改」一格：能，是你可以直接编辑这些文件；用命令的，是请用那条命令改。「删」一格说核心停着时删了会怎样。

| 位置 | 是什么 | 手改 | 删 | 备份 |
|---|---|---|---|---|
| `etc/config.toml` | 系统配置，见 [配置](config.md) | 能 | 设置回到出厂 | 要 |
| `etc/private/secrets.toml` | 密钥，`miyu provider login` 写的 | 能 | 存的 key 全没了 | 要 |
| `etc/private/access.json` | 网页访问密码的哈希 | 不 | 访问密码没了，要重设 | 要 |
| `etc/private/logins.json` | 各浏览器登录用的令牌的哈希 | 不 | 别的设备都要重新登录 | 要 |
| `etc/trust.toml` | 信任过哪些项目配置 | 用 `miyu config trust` | 项目配置要重新信任 | 要 |
| `etc/extensions.json` | 扩展开没开、批过哪些能力 | 不 | 扩展要重新打开、重新批 | 要 |
| `etc/kb.toml` | 软件包带的知识库的自动想起开关 | 不 | 回到出厂 | 要 |
| `etc/removed/<编号>` | 你卸掉的出厂包、系统包管理器装的包的记号 | 不 | 卸掉的又加载 | 要 |
| `etc/leftover/<编号>.toml` | 卸包时留在配置里的设置项是哪个包的 | 不 | 别删 | 要 |
| `etc/<编号>/` | 软件包自己的配置，卸包默认留着 | 照各包的说明 | 那个包回到出厂 | 要 |
| `opt/<编号>/` | 你装的软件包，见 [软件包](packages.md) | 不，改了 `miyu pkg check` 报出来 | 包没了，重装 | 要 |
| `root/sessions/<会话编号>/` | 会话的日志，核心写 | 不 | 会话没了 | 要 |
| `root/personas/`、`root/presets/` | 你的人格、预设，见 [人格和预设](personas-presets.md) | 能 | 人格、预设没了 | 要 |
| `root/prompts/`、`root/tools/` | 提示词、工具说明的覆盖 | 能 | 回到出厂的说法 | 要 |
| `root/skills/`、`root/scripts/`、`root/commands/` | 你的技能、脚本、斜杠命令模板，见 [扩展 Miyu](extending.md) | 能 | 没了 | 要 |
| `root/kb/<编号>/` | 知识库，见 [知识库](knowledge.md) | 能 | 库没了 | 要 |
| `root/memory/<人格编号>/` | 记忆日志，见 [记忆](memory.md) | 不，用 `miyu memory` | 记忆没了 | 要 |
| `root/blobs/` | 会话里的图片、附件、太长的工具输出，按内容哈希存 | 不 | 会话里的图片、附件打不开 | 要 |
| `root/workspace/<会话编号>/` | 会话的默认工作区，AI 在这里放东西 | 能 | 里面的文件没了 | 要 |
| `root/pictures/`、`videos/`、`documents/`、`downloads/` | AI 替你做出来、下下来的文件，用到才建 | 能 | 文件没了 | 要 |
| `root/trash/` | 删掉的会话、人格，留 7 天 | 不 | 那些会话、人格就真没了 | 不用 |
| `root/schedules.db` | 定时任务，见 [让 AI 自己接着干](automation.md#定时任务) | 不，用 `miyu schedule` | 定时任务没了 | 要 |
| `root/projects.json` | 网页里的项目 | 不 | 项目没了 | 要 |
| `root/state/` | 各终端窗口进哪个会话、脚本的状态 | 不 | 终端窗口不再接着原来的会话，脚本的状态没了 | 要 |
| `root/avatar.<png\|jpg\|webp>` | 你的头像 | 不 | 头像没了 | 要 |
| `var/lib/journal.jsonl` | 配置、密钥、信任的改动记录，清掉的会话的用量 | 不 | 清掉的会话的用量查不到了 | 要 |
| `var/lib/models/` | 用出来的：学到的上下文窗口、池轮到哪一个、供应商改过的名字 | 不 | 别删 | 要 |
| `var/lib/packages/` | 装了哪些包、什么版本、从哪来、每个文件的哈希 | 不 | 别删 | 要 |
| `var/lib/<编号>/` | 软件包的状态，例如通讯平台的聊天记录 `var/lib/im/` | 不 | 那个包的状态没了，卸包时一起删 | 要 |
| `var/lib/mcp/<服务名>/` | 本机程序的 MCP 服务的工作目录 | 照那个服务 | 那个服务的状态没了 | 要 |
| `var/cache/` | 会话列表、记忆、知识库的索引，用量汇总，模型列表，软件源索引和下载的包，脚本的缓存，各包的缓存 | 不 | 照需要重建、重下 | 不用 |
| `var/log/` | 核心的 `core.log`、内存占用 `memory.jsonl`，各包的日志 | 不 | 日志没了 | 不用 |
| `var/tmp/` | 跨重启的暂存 | 不 | 无 | 不用 |
| `tmp/` | 短命的暂存：装包解包、各包每次起来前清空的 | 不 | 无 | 不用 |
| `run/` | 锁、套接字位置、本机令牌（套接字本身在哪见 [出了问题怎么查](troubleshooting.md#核心起不来连不上)） | 不 | 核心跑着时删了，头连不上它，见 [出了问题怎么查](troubleshooting.md#旧核心还在跑) | 不用 |

`etc/private/` 下的文件 Linux、macOS 上只有你能读写（`0600`）。Windows 上 Miyu 不另设权限，照数据根继承的访问控制。

## 数据根外面的

| 什么 | Linux | macOS | Windows |
|---|---|---|---|
| 缓存目录 | `~/.cache/miyu`（设了 `XDG_CACHE_HOME` 的在它下面） | `~/Library/Caches/Miyu` | `%LOCALAPPDATA%\Miyu\cache` |
| 程序 | 见 [安装、升级、卸载](install.md#装了些什么) | 同左 | 同左 |
| 开机启动核心的服务文件 | `~/.config/systemd/user/miyu.service` 或 `~/.config/autostart/miyu.desktop`（设了 `XDG_CONFIG_HOME` 的在它下面） | `~/Library/LaunchAgents/io.github.miyu-agent.core.plist` | 任务计划 `Miyu`，`%USERPROFILE%\AppData\Local\Miyu\miyu-task.xml` |
| 托盘的开机启动 | `~/.config/autostart/miyu-applet.desktop` | `~/Library/LaunchAgents/io.github.miyu-agent.miyu-applet.plist` | 注册表 `Run` 项里的 `Miyu` |
| 托盘自己的目录 | `~/.local/state/miyu-applet` | `~/Library/Application Support/miyu-applet` | `%LOCALAPPDATA%\miyu-applet` |
| shell 集成 | `~/.bashrc`、`~/.zshrc` 里带记号的一段，或 `~/.config/fish/conf.d/miyu-shellhook.fish` | 同左 | PowerShell 的 `$PROFILE` 里带记号的一段 |

缓存目录整台机器共用，不跟着 `MIYU_HOME` 换。Linux 上 `XDG_CACHE_HOME` 要是绝对路径才算；Windows 上照环境变量 `LOCALAPPDATA` 找。里面是模型目录和各家供应商的图标（`models/`）、终端的崩溃记录和图片缓存（`tui/`）、shell 集成的暂存（`shellhook/`）。都能删，用到时再生成；终端崩了要报问题的，先把 `tui/crash.log` 留下。

这几处的细节见 [常驻和服务器](service.md)、[托盘](heads/applet.md)、[终端](heads/tui.md#shell-集成)。应急用的 LiteMiyu 另有自己的目录，见 [应急修复](rescue.md)。

## 备份

要备的是 `etc/`、`opt/`、`root/`、`var/lib/` 四处，加上顶上的 `.miyu-layout`。`var/cache/`、`var/log/`、`var/tmp/`、`tmp/`、`run/` 不用备：索引、用量汇总照会话的日志重算，模型列表、软件源索引重新拉。

备份前先停掉网页和核心，免得备到写了一半的文件：

```bash
miyu web --stop
miyu core stop
tar -czf ~/miyu-backup-$(date +%F).tar.gz -C ~ \
  --exclude=.miyu/var/cache --exclude=.miyu/var/tmp \
  --exclude=.miyu/tmp --exclude=.miyu/run \
  .miyu
```

设了 `MIYU_HOME` 的，把 `-C ~` 换成它的上一级，`.miyu`（`--exclude` 里的也算）换成它的名字。Windows 的 PowerShell 里用系统带的 `tar`：

```powershell
miyu web --stop
miyu core stop
tar -czf "$HOME\miyu-backup.tar.gz" -C $HOME --exclude=.miyu/var/cache --exclude=.miyu/var/tmp --exclude=.miyu/tmp --exclude=.miyu/run .miyu
```

`miyu core stop` 停下核心，通讯平台这类扩展跟着停。备完打开 `miyu` 或网页，核心就起来了；装了开机启动服务、要服务接着管的，敲 `miyu service enable`，见 [常驻和服务器](service.md#让服务马上接手)。

备份里有明文的 key（`etc/private/secrets.toml`）、全部对话和记忆。放到别处之前自己加密，例如 `gpg -c miyu-backup-2026-11-14.tar.gz`。

只搬一两个会话，用会话导出，见 [会话](sessions.md#导出导入)。

## 恢复、搬到另一台机器

1. 在新机器上装 Miyu，见 [安装、升级、卸载](install.md)。装上以后先别打开。
2. 停掉核心：`miyu core stop`。
3. 数据根已经有了的，挪开：

   ```bash
   mv ~/.miyu ~/.miyu.old
   ```

   ```powershell
   Rename-Item $HOME\.miyu .miyu.old
   ```

4. 解开备份：

   ```bash
   tar -xzf miyu-backup-2026-11-14.tar.gz -C ~
   ```

   ```powershell
   tar -xzf $HOME\miyu-backup.tar.gz -C $HOME
   ```

5. 敲 `miyu check`，照它说的改。
6. 打开 `miyu`。

数据根外面的东西不在备份里，要的在新机器上再做一遍：`miyu service enable`、托盘的「开机时启动」、`miyu shellhook install`。

### 版本

用同一版或更新的 Miyu 恢复。

- 数据根的布局变了的版本，标记里的布局号跟着变。布局号对不上的数据根，Miyu 报「… 的布局不是这一版的，没有动它」，什么都不改。
- 用旧版打开新版写过的数据根：定时任务表是新版建的，旧版这一次不用它，不动它；别的照能读的读。不要这样用。

### 换了系统

从 Linux 搬到 macOS、Windows，或者反过来，有几样带不过去：

- 带程序的软件包（扩展、MCP 服务的程序，例如 `im`、`embed`）是照系统打的，在新系统上跑不起来。逐个重装：`miyu pkg list` 看装了哪些，`miyu pkg install <编号>` 装这个系统的那一份。
- 写成绝对路径的地方：会话的工作目录、网页里项目的目录、挂进来的知识库目录（`kb.toml` 的 `mount`）、配置里的路径（`sandbox.private.add`、MCP 服务的命令）。路径在新机器上不在的，照新位置改。
- 脚本的 `#!` 解释器、`.ps1` 这类只在一种系统上跑的脚本。

## 清理

先看哪里占得多：

```bash
du -h -d 2 ~/.miyu | sort -h | tail -20
```

PowerShell 里列数据根下面两层的目录各占多少 MB：

```powershell
Get-ChildItem $HOME\.miyu\*\* -Directory | ForEach-Object { [pscustomobject]@{ MB = [math]::Round(((Get-ChildItem $_.FullName -Recurse -File -Force -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum) / 1MB, 1); Path = $_.FullName } } | Sort-Object MB | Select-Object -Last 20
```

能删的，删之前先 `miyu core stop`：

| 删什么 | 删了会怎样 | 自己会清吗 |
|---|---|---|
| `~/.miyu/var/cache/` 整个 | 索引、用量汇总下次用到时重建，第一次打开会话列表、搜记忆会慢一点；模型列表、软件源索引重新拉 | 脚本的缓存 30 天没碰的，核心起来时清 |
| 软件包的下载缓存 | 无 | 不清，敲 `miyu pkg clean`：删掉不是装着那一版的包文件 |
| `~/.miyu/var/log/` 下的旧日志 | 无 | `core.log` 满 10 MB 换一份，最多留 5 份旧的；扩展的 `stderr` 太大时挪成 `stderr.old` |
| `~/.miyu/var/tmp/` | 无 | 7 天没碰的，核心起来时清 |
| `~/.miyu/tmp/` | 无 | 装包的解包暂存核心起来时清空；各包的那一格每次拉起那个包之前清空 |
| `~/.miyu/root/trash/` | 删掉的会话、人格就真没了 | 留 7 天，核心起来时清掉满 7 天的 |
| `~/.miyu/root/workspace/<会话编号>/` | AI 在那个会话的默认工作区里做的文件没了 | 不清：删会话不删它的工作区，自己删 |
| 缓存目录（`~/.cache/miyu` 这些） | 模型目录、图标重新拉 | 不清 |
| 通讯平台的聊天记录 | 见 [接入通讯平台](heads/im.md#聊天记录) | 不清，在 im 的后台页里清 |

`run/` 在核心跑着时别删。`root/blobs/` 不随删会话变小，也没有清它的命令；里面的东西几个会话可能共用，别手删。

## 同一台机器用几个数据根

工作、私人分开用，各用一个数据根，互不相干：会话、设置、key、记忆、装的软件包都各是各的，各跑各的核心。

```bash
alias miyu-work='MIYU_HOME=$HOME/miyu-work miyu'
miyu-work            # 用工作的那一份
miyu                 # 用默认的 ~/.miyu
```

PowerShell 里写成函数，跑完把这个窗口的 `MIYU_HOME` 改回原样：

```powershell
function miyu-work { $old = $env:MIYU_HOME; $env:MIYU_HOME = Join-Path $HOME 'miyu-work'; try { miyu @args } finally { $env:MIYU_HOME = $old } }
miyu-work            # 用工作的那一份
miyu                 # 用默认的 ~/.miyu
```

`alias`、`function` 那一行放进 shell 的配置（`~/.bashrc`、`~/.zshrc`、`$PROFILE`），以后新开的窗口都有。

- 两个核心可以同时开着，命令行、终端、网页照各自的 `MIYU_HOME` 连自己的那一个。
- 网页：两份都没写过端口的，后打开的那一个自己换到 8301 起的空端口。想每次网址一样，给第二份设一个：`miyu-work config set web.port 8310`。
- 通讯平台：两份都装了 `im` 的，后台页、QQ 连接的端口要错开，见 [接入通讯平台](heads/im.md)。
- 托盘一个数据根一个，用 `--home <数据根>` 打开第二个。托盘的「开机时启动」一个用户也只有一份，跟着最后勾上它的那个托盘的数据根。
- 开机启动核心的服务一个用户只能装一份，跟着最后一次 `miyu service enable` 的数据根。Windows 上服务带不了 `MIYU_HOME`，只有默认的数据根装得了服务，第二个数据根装不了。
- 缓存目录两份共用，不碍事。

## 出了问题

| 看到的 | 怎么办 |
|---|---|
| 「MIYU_HOME 要写绝对路径，写的是 …」 | 写成绝对路径，或者 `~` 开头的 |
| 「… 里有别的东西，认不出是 Miyu 的数据根，没有动它。…」 | `MIYU_HOME` 指错了目录，或者 `.miyu-layout` 被删了。指到一个空目录；是自己的数据根、只是标记没了的，从备份里拿回 `.miyu-layout` |
| 「… 的布局不是这一版的，没有动它」 | 恢复的备份是更新的版本写的，升级 Miyu |
| 恢复以后网页要重新登录、扩展要重新打开 | `etc/private/`、`etc/extensions.json` 没备上，照提示重设 |
| 换了数据根以后开机起来的还是旧的 | 再敲一次 `miyu service enable`；托盘的「开机时启动」关掉再打开 |
