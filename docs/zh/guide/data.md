# 数据和备份

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

这一页说明 Miyu 把你的会话、设置、key 和记忆放在哪里，哪些需要备份、哪些可以删除，以及怎么迁移到另一台电脑或者换一个存放位置。

## 数据根的位置

Miyu 的数据几乎都放在一个目录里，这个目录叫“数据根”（默认是 `~/.miyu`）：

| 系统 | 默认位置 |
|---|---|
| Linux、macOS | `~/.miyu` |
| Windows | `%USERPROFILE%\.miyu` |

安装、升级和卸载 Miyu 都不会改动数据根。

### 更换数据根的位置

如果想把数据根放到其他地方，设置环境变量 `MIYU_HOME`，指向另一个目录即可。例如：

```bash
export MIYU_HOME=/data/miyu
```

```powershell
$env:MIYU_HOME = 'D:\miyu'
```

这样设置只对当前这一个 shell 窗口有效。想让以后都使用这个位置，可以看下面的表格和表格后面的说明。

- `MIYU_HOME` 要写绝对路径，开头的 `~` 会被替换成家目录。如果写了相对路径，会报错 `MIYU_HOME 要写绝对路径，写的是 …`。
- 设置成空值等于没有设置。
- 如果目录不存在，第一次使用时会自动创建。在 Linux 和 macOS 上，新建的目录只有你能访问（`0700`）；在 Windows 上，新建的目录继承上一级目录的访问控制。

`MIYU_HOME` 是在程序启动的那一刻从环境里读取的，所以一个程序从哪里启动，就看哪里的环境：

| 程序 | 读取方式 |
|---|---|
| 命令行、终端界面、网页界面、shell 集成 | 读取你运行命令的那个 shell 的环境。如果把它写在 shell 的配置里（bash、zsh 的 `~/.bashrc`、`~/.zshrc` 这类文件，PowerShell 的 `$PROFILE`），之后新开的 shell 都会读到；在 Windows 上也可以把它设置成用户环境变量，见表格后面的说明 |
| 软件包、脚本、MCP 服务、AI 运行的命令 | 由核心启动它们时传给它们，所以和核心使用的是同一个 |
| `miyu service enable` 安装的服务 | 如果安装时设置了，就会写进服务文件。更换数据根之后，需要再运行一次 `miyu service enable`。在 Windows 上，设置了 `MIYU_HOME` 时无法安装服务，详细的见 [常驻和服务器](service.md#开机或登录时启动核心) |
| 系统托盘 | 使用命令行参数 `--home <数据根>`；没有写这个参数时使用 `MIYU_HOME`。`开机时启动` 会把它写进启动项，在 Windows 上也能带上，详细的见 [系统托盘](heads/applet.md#开机时启动) |
| herdr 恢复的窗格 | 不保存写在命令前面的临时变量，详细的见 [终端界面](heads/tui.md#和-herdr-一起用) |

从应用菜单打开的系统托盘读不到 shell 配置里的变量；在 macOS 上，从访达或程序坞打开的 `Miyu.app` 也一样。解决方法是在终端里带上 `--home` 参数打开系统托盘，再勾选 `开机时启动`，这样启动项里会记下这个参数，以后登录时就会使用这个数据根启动：

| 系统 | 在终端里运行 |
|---|---|
| Linux | `miyu-applet --home /data/miyu` |
| macOS | `/Applications/Miyu.app/Contents/MacOS/miyu-applet --home /data/miyu`（如果 `Miyu.app` 不在 `应用程序` 里，换成它实际的位置） |

在 Windows 上，可以用 `setx MIYU_HOME D:\miyu` 设置用户环境变量，之后新打开的程序都会读到它。开机启动请使用系统托盘的 `开机时启动`，不要用 `miyu service`，因为设置了 `MIYU_HOME` 时无法安装服务。

### 标记文件

数据根的最上层有一个 `.miyu-layout` 文件，里面只有一行 `This directory is a Miyu data root (layout 3).`。Miyu 只把带有这个标记的目录当作数据根：

- 如果目录不存在或者是空的：Miyu 会写入标记，把它当作新的数据根。
- 如果目录里有其他东西、但没有标记：Miyu 不会改动其中的任何一个字节，并报错 `… 里有别的东西，认不出是 Miyu 的数据根，没有动它。是 0.7 版的数据目录的话，先 mv … …-0.7，再 miyu pkg install migrate、miyu migrate …-0.7；不是的话把 MIYU_HOME 设成一个空目录`。0.7 旧版的数据目录就属于这种情况，详细的见 [从 0.7 旧版迁过来](migrate.md)。
- 如果只有旧的标记 `.miyu-root`：说明这是正式发布之前的开发版留下的数据根，Miyu 会报错 `… 是旧版 Miyu 的数据根，新版不认，没有动它。把 MIYU_HOME 设成一个空目录`。
- 如果标记里的布局号不是这一版的：Miyu 会报错 `… 的布局不是这一版的，没有动它`。

不要删除或修改这个文件。

## 数据根里的内容

数据根里的目录结构如下：

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

下表的「手动修改」一列中，`可以` 表示你可以直接编辑这些文件；如果写的是一条命令，表示需要用那条命令修改。「删除后」一列说明在核心停止时删除它会发生什么。

| 位置 | 内容 | 手动修改 | 删除后 | 需要备份 |
|---|---|---|---|---|
| `etc/config.toml` | 系统配置，见 [配置](config.md) | 可以 | 设置回到出厂状态 | 需要 |
| `etc/private/secrets.toml` | 密钥，由 `miyu provider login` 写入 | 可以 | 保存的 key 全部丢失 | 需要 |
| `etc/private/access.json` | 网页界面访问密码的哈希 | 不要 | 访问密码丢失，需要重新设置 | 需要 |
| `etc/private/logins.json` | 各个浏览器登录用的令牌的哈希 | 不要 | 其他设备都需要重新登录 | 需要 |
| `etc/trust.toml` | 信任过哪些项目配置 | 用 `miyu config trust` | 项目配置需要重新信任 | 需要 |
| `etc/extensions.json` | 扩展是否打开、批准过哪些能力 | 不要 | 扩展需要重新打开、重新批准 | 需要 |
| `etc/kb.toml` | 软件包自带的知识库的自动想起开关 | 不要 | 回到出厂状态 | 需要 |
| `etc/removed/<编号>` | 你卸载的出厂软件包、系统包管理器安装的软件包的记号 | 不要 | 卸载掉的软件包会重新加载 | 需要 |
| `etc/leftover/<编号>.toml` | 卸载软件包时留在配置里的设置项属于哪个软件包 | 不要 | 不要删除 | 需要 |
| `etc/<编号>/` | 软件包自己的配置，卸载软件包时默认保留 | 按照各软件包的说明 | 那个软件包回到出厂状态 | 需要 |
| `opt/<编号>/` | 你安装的软件包，见 [软件包](packages.md) | 不要，改动后 `miyu pkg check` 会报告出来 | 软件包丢失，需要重新安装 | 需要 |
| `root/sessions/<会话编号>/` | 会话的日志，由核心写入 | 不要 | 会话丢失 | 需要 |
| `root/personas/`、`root/presets/` | 你的人格和预设，见 [人格和预设](personas-presets.md) | 可以 | 人格和预设丢失 | 需要 |
| `root/prompts/`、`root/tools/` | 对提示词和工具说明的覆盖 | 可以 | 回到出厂的说法 | 需要 |
| `root/skills/`、`root/scripts/`、`root/commands/` | 你的技能、脚本和斜杠命令模板，见 [扩展 Miyu](extending.md) | 可以 | 全部丢失 | 需要 |
| `root/kb/<编号>/` | 知识库，见 [知识库](knowledge.md) | 可以 | 知识库丢失 | 需要 |
| `root/memory/<人格编号>/` | 记忆日志，见 [记忆](memory.md) | 不要，用 `miyu memory` | 记忆丢失 | 需要 |
| `root/blobs/` | 会话里的图片、附件和太长的工具输出，按内容哈希存放 | 不要 | 会话里的图片和附件打不开 | 需要 |
| `root/workspace/<会话编号>/` | 会话的默认工作区，AI 在这里存放文件 | 可以 | 里面的文件丢失 | 需要 |
| `root/pictures/`、`videos/`、`documents/`、`downloads/` | AI 替你做出来、下载下来的文件，用到时才创建 | 可以 | 文件丢失 | 需要 |
| `root/trash/` | 删除的会话和人格，保留 7 天 | 不要 | 那些会话和人格彻底丢失 | 不需要 |
| `root/schedules.db` | 定时任务，见 [让 AI 自己接着干](automation.md#定时任务) | 不要，用 `miyu schedule` | 定时任务丢失 | 需要 |
| `root/projects.json` | 网页界面里的项目 | 不要 | 项目丢失 | 需要 |
| `root/state/` | 各个终端窗口进入哪个会话、脚本的状态 | 不要 | 终端窗口不再接着原来的会话，脚本的状态丢失 | 需要 |
| `root/avatar.<png\|jpg\|webp>` | 你的头像 | 不要 | 头像丢失 | 需要 |
| `var/lib/journal.jsonl` | 配置、密钥、信任的改动记录，以及已清除会话的用量 | 不要 | 已清除会话的用量查不到了 | 需要 |
| `var/lib/models/` | 使用中积累下来的信息：学到的上下文窗口、模型池轮到哪一个、供应商改过的名字 | 不要 | 不要删除 | 需要 |
| `var/lib/packages/` | 安装了哪些软件包、什么版本、从哪里来、每个文件的哈希 | 不要 | 不要删除 | 需要 |
| `var/lib/<编号>/` | 软件包的状态，例如通讯平台的聊天记录 `var/lib/im/` | 不要 | 那个软件包的状态丢失；卸载软件包时会一起删除 | 需要 |
| `var/lib/mcp/<服务名>/` | 本机程序形式的 MCP 服务的工作目录 | 按照那个服务的说明 | 那个服务的状态丢失 | 需要 |
| `var/cache/` | 会话列表、记忆、知识库的索引，用量汇总，模型列表，软件源索引和下载的软件包，脚本的缓存，各软件包的缓存 | 不要 | 需要时重新生成、重新下载 | 不需要 |
| `var/log/` | 核心的 `core.log`、内存占用记录 `memory.jsonl`，各软件包的日志 | 不要 | 日志丢失 | 不需要 |
| `var/tmp/` | 跨重启保留的暂存 | 不要 | 没有影响 | 不需要 |
| `tmp/` | 短期的暂存：安装软件包时的解包目录，以及各软件包每次启动前清空的目录 | 不要 | 没有影响 | 不需要 |
| `run/` | 锁、套接字位置、本机令牌（套接字本身在哪里见 [出了问题怎么查](troubleshooting.md#核心无法启动或无法连接)） | 不要 | 如果在核心运行时删除，界面会连不上核心，见 [出了问题怎么查](troubleshooting.md#旧版核心仍在运行) | 不需要 |

在 Linux 和 macOS 上，`etc/private/` 下的文件只有你能读写（`0600`）。在 Windows 上，Miyu 不另外设置权限，这些文件继承数据根的访问控制。

## 数据根之外的文件

除了数据根，Miyu 还会在这些位置放文件：

| 内容 | Linux | macOS | Windows |
|---|---|---|---|
| 缓存目录 | `~/.cache/miyu`（如果设置了 `XDG_CACHE_HOME`，就在它下面） | `~/Library/Caches/Miyu` | `%LOCALAPPDATA%\Miyu\cache` |
| 程序 | 见 [安装、升级、卸载](install.md#安装的内容) | 同左 | 同左 |
| 开机启动核心的服务文件 | `~/.config/systemd/user/miyu.service` 或 `~/.config/autostart/miyu.desktop`（如果设置了 `XDG_CONFIG_HOME`，就在它下面） | `~/Library/LaunchAgents/io.github.miyu-agent.core.plist` | 任务计划 `Miyu`，`%USERPROFILE%\AppData\Local\Miyu\miyu-task.xml` |
| 系统托盘的开机启动 | `~/.config/autostart/miyu-applet.desktop` | `~/Library/LaunchAgents/io.github.miyu-agent.miyu-applet.plist` | 注册表 `Run` 项里的 `Miyu` |
| 系统托盘自己的目录 | `~/.local/state/miyu-applet` | `~/Library/Application Support/miyu-applet` | `%LOCALAPPDATA%\miyu-applet` |
| shell 集成 | `~/.bashrc`、`~/.zshrc` 里带记号的一段，或者 `~/.config/fish/conf.d/miyu-shellhook.fish` | 同左 | PowerShell 的 `$PROFILE` 里带记号的一段 |

缓存目录由整台电脑共用，不会跟着 `MIYU_HOME` 改变。在 Linux 上，`XDG_CACHE_HOME` 必须是绝对路径才有效；在 Windows 上，缓存目录根据环境变量 `LOCALAPPDATA` 查找。缓存目录里存放的是模型目录和各家供应商的图标（`models/`）、终端界面的崩溃记录和图片缓存（`tui/`），以及 shell 集成的暂存（`shellhook/`）。这些都可以删除，需要时会重新生成；如果终端界面崩溃了、需要报告问题，先把 `tui/crash.log` 保留下来。

这几个位置的详细说明可以看 [常驻和服务器](service.md)、[系统托盘](heads/applet.md)、[终端界面](heads/tui.md#shell-集成)。应急用的 LiteMiyu 有自己单独的目录，详细的见 [应急修复](rescue.md)。

## 备份

需要备份的是 `etc/`、`opt/`、`root/`、`var/lib/` 这四个目录，加上最上层的 `.miyu-layout`。`var/cache/`、`var/log/`、`var/tmp/`、`tmp/`、`run/` 不需要备份：因为索引和用量汇总可以根据会话的日志重新计算，模型列表和软件源索引可以重新下载。

备份之前先停止网页界面和核心，避免备份到写了一半的文件。Linux、macOS 上运行：

```bash
miyu web stop
miyu core stop
tar -czf ~/miyu-backup-$(date +%F).tar.gz -C ~ \
  --exclude=.miyu/var/cache --exclude=.miyu/var/tmp \
  --exclude=.miyu/tmp --exclude=.miyu/run \
  .miyu
```

如果设置了 `MIYU_HOME`，把 `-C ~` 换成它的上一级目录，把 `.miyu`（包括 `--exclude` 里的）换成它的目录名。在 Windows 的 PowerShell 里，使用系统自带的 `tar`：

```powershell
miyu web stop
miyu core stop
tar -czf "$HOME\miyu-backup.tar.gz" -C $HOME --exclude=.miyu/var/cache --exclude=.miyu/var/tmp --exclude=.miyu/tmp --exclude=.miyu/run .miyu
```

`miyu core stop` 停止核心时，通讯平台这类扩展也会跟着停止。备份完成后，打开 `miyu` 或网页界面，核心就会重新启动；如果安装了开机启动服务，并且希望服务继续管理核心，再运行 `miyu service enable`，详细的见 [常驻和服务器](service.md#让服务立即接管)。

> 备份里有明文的 key（`etc/private/secrets.toml`），以及全部的对话和记忆。把备份放到别处之前，先自己加密，例如 `gpg -c miyu-backup-2026-11-14.tar.gz`。

如果只想迁移一两个会话，可以使用会话导出，详细的见 [会话](sessions.md#导出导入)。

## 恢复和迁移到另一台电脑

按照下面的步骤，把备份恢复到新电脑上：

1. 在新电脑上安装 Miyu，见 [安装、升级、卸载](install.md)。安装之后先不要打开。
2. 停止核心：`miyu core stop`。
3. 如果新电脑上已经有数据根，先把它移到别处：

   ```bash
   mv ~/.miyu ~/.miyu.old
   ```

   ```powershell
   Rename-Item $HOME\.miyu .miyu.old
   ```

4. 解压备份：

   ```bash
   tar -xzf miyu-backup-2026-11-14.tar.gz -C ~
   ```

   ```powershell
   tar -xzf $HOME\miyu-backup.tar.gz -C $HOME
   ```

5. 运行 `miyu check`，按照它的提示修改。
6. 打开 `miyu`。

数据根之外的东西不在备份里，需要的话在新电脑上重新设置一遍：`miyu service enable`、系统托盘的 `开机时启动`、`miyu shellhook install`。

### 版本要求

恢复时要使用同一版或者更新版本的 Miyu。

- 如果某个版本改变了数据根的布局，标记里的布局号也会跟着改变。对于布局号对不上的数据根，Miyu 会报错 `… 的布局不是这一版的，没有动它`，不做任何修改。
- 如果用旧版打开新版写过的数据根：由于定时任务表是新版创建的，旧版这一次不会使用它，也不会改动它；其他内容能读取的照常读取。不要这样使用。

### 迁移到其他操作系统

从 Linux 迁移到 macOS 或 Windows，或者反过来迁移时，有几样东西无法直接带过去：

- 带程序的软件包（扩展、MCP 服务的程序，例如 `im`、`embed`）是针对每个系统单独打包的，在新系统上无法运行。需要逐个重新安装：用 `miyu pkg list` 查看装了哪些，再用 `miyu pkg install <编号>` 安装适合当前系统的那一份。
- 写成绝对路径的地方：会话的工作目录、网页界面里项目的目录、挂载进来的知识库目录（`kb.toml` 的 `mount`）、配置里的路径（`skills.external.dirs`、MCP 服务的命令）。如果这些路径在新电脑上不存在，需要改成新的位置。
- 脚本的 `#!` 解释器，以及 `.ps1` 这类只能在一种系统上运行的脚本。

## 清理空间

数据根占用的空间变大时，可以先查看哪里占得多。Linux、macOS 上运行：

```bash
du -h -d 2 ~/.miyu | sort -h | tail -20
```

在 PowerShell 里，下面的命令会列出数据根下面两层的每个目录各占多少 MB：

```powershell
Get-ChildItem $HOME\.miyu\*\* -Directory | ForEach-Object { [pscustomobject]@{ MB = [math]::Round(((Get-ChildItem $_.FullName -Recurse -File -Force -ErrorAction SilentlyContinue | Measure-Object Length -Sum).Sum) / 1MB, 1); Path = $_.FullName } } | Sort-Object MB | Select-Object -Last 20
```

下面这些可以删除，删除之前先运行 `miyu core stop`：

| 删除的内容 | 删除后的影响 | 是否会自动清理 |
|---|---|---|
| 整个 `~/.miyu/var/cache/` | 索引和用量汇总会在下次用到时重建，所以第一次打开会话列表、搜索记忆时会慢一点；模型列表和软件源索引会重新下载 | 30 天没有用过的脚本缓存，会在核心启动时清理 |
| 软件包的下载缓存 | 没有影响 | 不会自动清理，需要运行 `miyu pkg clean`：它会删除不是当前安装版本的软件包文件 |
| `~/.miyu/var/log/` 下的旧日志 | 没有影响 | `core.log` 满 10 MB 后换一个新文件，最多保留 5 份旧的；扩展的 `stderr` 太大时会被移成 `stderr.old` |
| `~/.miyu/var/tmp/` | 没有影响 | 7 天没有用过的文件，会在核心启动时清理 |
| `~/.miyu/tmp/` | 没有影响 | 安装软件包时的解包暂存，会在核心启动时清空；各软件包自己的目录，会在每次启动那个软件包之前清空 |
| `~/.miyu/root/trash/` | 删除的会话和人格会彻底丢失 | 保留 7 天，核心启动时清理满 7 天的内容 |
| `~/.miyu/root/workspace/<会话编号>/` | AI 在那个会话的默认工作区里做的文件会丢失 | 不会自动清理：删除会话时不会删除它的工作区，需要你自己删除 |
| 缓存目录（`~/.cache/miyu` 等） | 模型目录和图标会重新下载 | 不会自动清理 |
| 通讯平台的聊天记录 | 见 [接入通讯平台](heads/im.md#聊天记录) | 不会自动清理，可以在 im 的后台页里清理 |

不要在核心运行时删除 `run/`。`root/blobs/` 不会因为删除会话而变小，也没有清理它的命令；由于里面的内容可能被几个会话共用，所以不要手动删除。

## 在同一台电脑上使用多个数据根

如果想把工作和私人用途分开，可以各用一个数据根，它们互不影响：会话、设置、key、记忆和安装的软件包都是各自独立的，各自运行自己的核心。在 bash 里可以这样设置：

```bash
alias miyu-work='MIYU_HOME=$HOME/miyu-work miyu'
miyu-work            # 用工作的那一份
miyu                 # 用默认的 ~/.miyu
```

在 PowerShell 里写成函数，运行结束后会把这个窗口的 `MIYU_HOME` 改回原来的值：

```powershell
function miyu-work { $old = $env:MIYU_HOME; $env:MIYU_HOME = Join-Path $HOME 'miyu-work'; try { miyu @args } finally { $env:MIYU_HOME = $old } }
miyu-work            # 用工作的那一份
miyu                 # 用默认的 ~/.miyu
```

把 `alias` 或 `function` 那一行放进 shell 的配置文件（`~/.bashrc`、`~/.zshrc`、`$PROFILE`），以后新开的窗口里都可以使用。

- 两个核心可以同时运行，命令行、终端界面、网页界面会根据各自的 `MIYU_HOME` 连接到对应的那一个。
- 网页界面：如果两份都没有设置过端口，后打开的那一个会自动换到从 8301 开始的空闲端口。如果希望每次的网址都一样，可以给第二份设置一个端口：`miyu-work config set web.port 8310`。
- 通讯平台：如果两份都安装了 `im`，后台页和 QQ 连接的端口要错开，详细的见 [接入通讯平台](heads/im.md)。
- 一个数据根对应一个系统托盘，用 `--home <数据根>` 打开第二个系统托盘。系统托盘的 `开机时启动` 每个用户也只有一份，使用的是最后勾选它的那个系统托盘的数据根。
- 开机启动核心的服务每个用户只能安装一份，使用的是最后一次运行 `miyu service enable` 时的数据根。在 Windows 上，由于服务带不了 `MIYU_HOME`，只有默认的数据根能安装服务，第二个数据根无法安装。
- 缓存目录由两份共用，不会有影响。

## 常见问题

| 看到的现象 | 解决方法 |
|---|---|
| `MIYU_HOME 要写绝对路径，写的是 …` | 写成绝对路径，或者以 `~` 开头的路径 |
| `… 里有别的东西，认不出是 Miyu 的数据根，没有动它。…` | `MIYU_HOME` 指向了错误的目录，或者 `.miyu-layout` 被删除了。把它指向一个空目录；如果确实是自己的数据根、只是标记丢了，从备份里找回 `.miyu-layout` |
| `… 的布局不是这一版的，没有动它` | 恢复的备份是更新版本的 Miyu 写的，升级 Miyu 即可 |
| 恢复以后网页界面需要重新登录、扩展需要重新打开 | `etc/private/` 和 `etc/extensions.json` 没有备份上，按照提示重新设置 |
| 更换数据根以后，开机启动的仍然是旧的数据根 | 再运行一次 `miyu service enable`；系统托盘的 `开机时启动` 先关掉再打开 |
