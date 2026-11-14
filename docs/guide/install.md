# 安装、升级、卸载

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

在自己的机器上装 Miyu、看装了什么、升级、卸干净，都在这一页。装好以后怎么让核心常驻、放到服务器上，见 [常驻和服务器](service.md)。

## 哪些系统能装

| 系统 | 架构 | 安装脚本 | AUR | Homebrew | 托盘安装包 |
|---|---|---|---|---|---|
| Linux | x86_64 | `install.sh` | 有 | 没有 | deb、rpm |
| Linux | aarch64 | `install.sh` | 有 | 没有 | 没有 |
| macOS | Apple 芯片 | `install.sh` | 没有 | 有 | dmg |
| Windows | x86_64 | `install.ps1` | 没有 | 没有 | msi |

Intel 芯片的 Mac、32 位和 ARM 的 Windows 没有发行包。

几种装法装的东西不一样：

| 装法 | 核心和命令行 | 终端 | 网页 | 托盘 |
|---|---|---|---|---|
| 安装脚本 | 有 | 有 | 有 | 没有 |
| AUR 的 `miyu-agent`、Homebrew 的 `miyu-agent` 加 cask | 有 | 有 | 有 | 有 |
| 托盘安装包 | 有 | 没有 | 有 | 有 |

同一台机器选一种装法。

## 用安装脚本装

安装脚本从 GitHub 上 Miyu 的发行版下载你这种系统的压缩包，核对过才装。不要管理员，不用 `sudo`。

### Linux、macOS

```sh
curl -fsSL https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/install.sh | sh
```

要 `curl` 和 `tar`。参数写在 `sh -s --` 后面：

| 参数 | 做什么 |
|---|---|
| `--version <版本>` | 装这一版，例如 `--version 2026.11.0` |
| `--prerelease` | 预发布的也算，装最新的那一个 |
| `--uninstall` | 卸载 |

```sh
curl -fsSL https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/install.sh | sh -s -- --version 2026.11.0
```

装完是这样：

- 压缩包里的整个目录放进 `~/.local/lib/miyu/`，旁边写一个 `install.toml`，里面是 `channel = "script"`。
- `~/.local/bin/miyu` 链到 `~/.local/lib/miyu/miyu`。
- Linux 上另写 `~/.local/share/mime/packages/miyu.xml`（设了 `XDG_DATA_HOME` 的写在它下面），让系统认得 `.miyu` 软件包文件。
- 不改 shell 的配置。`~/.local/bin` 不在 `PATH` 里的，脚本最后印一行「提示：… 不在 PATH 里」，照它把 `export PATH="$HOME/.local/bin:$PATH"` 加进 shell 的配置。`PATH` 里先找到的是别处的 `miyu` 时也提示一句。

已经装过的，脚本说「已经装了 …，升级用 miyu upgrade」，什么都不动。

### Windows

在 PowerShell 里敲（Windows PowerShell 5.1、PowerShell 7 都行）：

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/install.ps1)))
```

参数跟在最后：`-Version <版本>`、`-Prerelease`、`-Uninstall`，意思同上表。

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/install.ps1))) -Version 2026.11.0
```

装完是这样：

- 压缩包里的整个目录放进 `%LOCALAPPDATA%\Programs\Miyu\`，旁边写 `install.toml`（`channel = "script"`）。
- 这个目录加进当前用户的 `PATH`，只改你自己的环境变量。新开的终端里认得 `miyu`；装的这个窗口里当场也认得。

### 脚本核对什么

1. 下载这一版的 `SHA256SUMS` 和它的签名 `SHA256SUMS.sig`，用写在脚本里的发版公钥核签名。
2. 下载压缩包，核 SHA-256。
3. 解开，跑一次新的 `miyu --version`，版本要对。

哪一步不对都停下，什么都不装。签名要用认 Ed25519 的 `openssl` 才核得了（OpenSSL 3 起；Windows 上 Git for Windows 带的就行）。macOS 自带的是 LibreSSL，核不了；Windows 自带的工具也不认 Ed25519。核不了签名的，脚本只核 SHA-256 接着装，说一句：

| 情形 | 脚本说的 |
|---|---|
| `openssl` 不认 Ed25519（macOS 上用自带的就是这样） | 「这台的 openssl 不认 Ed25519，签名没核（不是签名坏了）；只核 SHA-256。…」 |
| Linux、macOS 上没有 `openssl` | 「这台没有 openssl，没法核签名；只核 SHA-256。…」 |
| Windows 上没有 `openssl` | 「这台没有 openssl（Windows 自带的工具不认 Ed25519），签名没核（不是签名坏了）；只核 SHA-256。…」 |

要自己核签名的，用它提示的 `gh attestation verify <文件> -R miyu-agent/miyu`。

设了 `GITHUB_TOKEN` 的，脚本问 GitHub 接口时带上它（下载不带）。几个人共用一个出口的网络容易撞上匿名每小时 60 次的限流，撞上了脚本说「GitHub 限流了」。

## 用 AUR 装（Arch Linux）

用你习惯的 AUR 助手，例如 `paru`：

```sh
paru -S miyu-agent
```

AUR 上的几个包：

| 包 | 装什么 | 装到哪 |
|---|---|---|
| `miyu-agent` | 合集，自己不带文件，依赖下面四个 | |
| `miyu-core` | 核心和命令行 | `/usr/lib/miyu/`，`/usr/bin/miyu` 链过去，另装 `/usr/share/mime/packages/miyu.xml` |
| `miyu-tui` | 终端 | `/usr/share/miyu/packages/tui/` |
| `miyu-webui` | 网页 | `/usr/share/miyu/packages/webui/` |
| `miyu-applet` | 托盘，要 `gtk3` 这些系统库 | `/usr/bin/miyu-applet` 和应用菜单、图标 |
| `miyu-agent-git` | 从最新源码编的核心、终端、网页，一个包，不带托盘 | 同 `miyu-core`、`miyu-tui`、`miyu-webui` |
| `miyu` | 给 0.7 旧版用户的过渡包，自己不带文件，依赖 `miyu-agent` | |

- 不要托盘的，装 `miyu-core miyu-tui miyu-webui` 三个，或者只装要的那几个。终端、网页都依赖 `miyu-core`。
- `miyu-agent-git` 和 `miyu-core`、`miyu-tui`、`miyu-webui`、`miyu-agent` 互斥。编它要 `git`、`rustup`、`python`，编的时候联网取依赖。要托盘的另装 `miyu-applet`。
- `miyu-core` 和 0.7 那一代的旧包 `miyu` 互斥，装的时候 pacman 会让你确认换掉它。
- `install.toml` 写在 `/usr/lib/miyu/`，是 `channel = "aur"`。

## 用 Homebrew 装（macOS）

Miyu 的 tap 是 `miyu-agent/miyu`：

```sh
brew tap miyu-agent/miyu
brew install miyu-agent
brew install --cask miyu-applet
```

| 名字 | 种类 | 装什么 |
|---|---|---|
| `miyu-agent` | formula | 合集，依赖下面三个 formula；托盘要另装 cask |
| `miyu-core` | formula | 核心和命令行，放进 formula 的 `libexec/`，`miyu` 链进 Homebrew 的 `bin/`；`install.toml` 是 `channel = "brew"` |
| `miyu-tui` | formula | 终端，装进 Homebrew 前缀下的 `share/miyu/packages/tui/` |
| `miyu-webui` | formula | 网页，装进 `share/miyu/packages/webui/` |
| `miyu-applet` | cask | 托盘 `Miyu.app`，依赖 `miyu-core`、`miyu-webui` |

都只在 Apple 芯片的 Mac 上装得了。

## 托盘安装包

托盘的安装包把核心、网页、托盘打在一起，装上从应用菜单打开「Miyu」就能用，不用开终端。终端不在里面。到托盘的发行版页面 <https://github.com/miyu-agent/miyu-applet/releases> 下载：

| 系统 | 文件 |
|---|---|
| Linux x86_64 | `.deb`、`.rpm`，要 GTK 3 |
| macOS Apple 芯片 | `.dmg`，把 `Miyu.app` 拖进「应用程序」 |
| Windows x86_64 | `.msi`，带一份 PowerShell 7 |

这些安装包没有签名，发行版都标着预发布。系统源里的 `miyu-core` 和它们装的核心会撞，别和 AUR、Homebrew 的装法混用。

托盘怎么用见 [托盘](heads/applet.md)。

## 装好了没有

```sh
miyu --version
```

印出 `miyu 2026.11.0` 这样一行就装好了。版本号是「年.月.这个月第几版」。

在终端里敲 `miyu` 打开界面。第一次打开会带你配模型供应商、选主题和人格，见 [快速上手](quickstart.md)。

## 装了些什么

安装脚本装的是一体包，一个目录里放着：

- 五个程序：`miyu`（核心和命令行，核心就是 `miyu` 在后台跑的样子）、`miyu-sandbox`、`miyu-extract`、`miyu-tui`（终端）、`miyu-web`（网页）。Windows 上都带 `.exe`：`miyu.exe`、`miyu-sandbox.exe` 这样。
- `resources/`：核心的出厂资源，加上终端、网页两个软件包（`resources/packages/tui/`、`resources/packages/webui/`）。
- `LICENSE`、`THIRD-PARTY-LICENSES.txt`、`README.txt`。`README.txt` 说自己解开压缩包怎么用：Linux、macOS 上「把这个目录加进 PATH，或者在 ~/.local/bin 里放一个指向 miyu 的链接」，Windows 上「把这个目录加进 PATH（或者直接写 miyu.exe 的完整路径）」。
- 安装脚本写的 `install.toml`。

终端、网页是两个软件包，各有自己的版本号，每一版 Miyu 带着和它配好的那一份。`miyu pkg list` 里看得到它们，编号是 `tui`、`webui`。AUR、Homebrew 把它们分成单独的包，装进系统目录下的 `share/miyu/packages/`，核心照样认得出。核心到这些地方找：

| 系统 | 系统包管理器装的软件包在哪找 |
|---|---|
| Linux | `XDG_DATA_DIRS` 里每一处的 `miyu/packages/`，没设的照 `/usr/local/share:/usr/share`。Linuxbrew 装的不会自己认到，要自己把 Linuxbrew 的 `share` 目录写进 `XDG_DATA_DIRS` |
| macOS | Homebrew 前缀下的 `share/miyu/packages/`，前缀是 `miyu` 所在目录的上一级 |
| Windows | 没有这一处，只认安装目录和数据根里的 |

`resources/` 每次升级整个换掉，安装目录里别放自己的东西。

你的会话、设置、key 都不在安装目录里，在数据根（默认 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`；设了环境变量 `MIYU_HOME` 的是它）。装、升级、卸载都不碰数据根。

## 升级

```sh
miyu upgrade
```

| 参数 | 做什么 |
|---|---|
| `--prerelease` | 预发布的也算 |
| `--yes` | 不问「升级 Miyu … → …？[Y/n]」，直接升 |
| `-h`、`--help` | 显示帮助 |

`miyu upgrade` 看 `miyu` 程序旁边的 `install.toml` 认是怎么装的：

| `channel` | 怎么装的 | `miyu upgrade` 做什么 |
|---|---|---|
| `script` | 安装脚本 | 自己升，见下 |
| `aur` | AUR | 说「这份 Miyu 由 AUR 安装，用包管理器升级」，下一行是 `paru -Syu` |
| `brew` | Homebrew | 说「这份 Miyu 由 Homebrew 安装，用包管理器升级」，下一行是 `brew upgrade` |
| 没有 `install.toml` | 托盘安装包、手工解开的压缩包 | 说「这份 Miyu 不是用安装脚本或包管理器装的，不自动升级」和下载地址 |

安装脚本装的，`miyu upgrade` 这样升：

1. 问 GitHub 最近的发行版，挑出比现在新的最新一版。没有新的，说「已是最新版本 …」。
2. 问你升不升。直接回车或者敲 `y` 是升。不在终端里跑、读不到回答的，说「已取消：没有收到确认」，要加 `--yes`。
3. 下载这一版的压缩包、`SHA256SUMS`、`SHA256SUMS.sig`，用编进程序的发版公钥核签名、核 SHA-256，解开后跑一次新的 `miyu --version`。
4. 替换安装目录里属于 Miyu 的文件，`install.toml` 不动。不是 Miyu 的东西原样留着，说一句「安装目录里还有 … 样不是 Miyu 的，原样留着」。旧的文件先挪进安装目录里的 `.miyu-old-<旧版本>/`，换好再删；Windows 上正在跑的 `miyu.exe` 删不掉，留到下一次 `miyu upgrade` 开头再删。
5. 说「已升级 Miyu … → …」。

终端、网页在同一个压缩包里，跟着一起升。哪一步不对都停下，装着的那一版不动。

升好以后，已经在跑的核心还是旧版。敲：

```sh
miyu core stop
```

下次打开终端或网页时拉起的就是新版。装了开机启动服务、要服务接着管的，再敲 `miyu service enable`：Linux 的 systemd 用户服务、macOS 当场起新版；Linux 的桌面自启动、Windows 下次登录时起。见 [常驻和服务器](service.md#让服务马上接手)。

托盘安装包装的，下载新的安装包装上去。

### 换装法

`channel` 由装法决定，不要手改 `install.toml`。要从一种装法换到另一种：

1. 照下面「卸载」卸掉现在的。
2. 用新的装法装。

数据根不受影响，换完会话、设置都在。

要用预发布版的，脚本装的升级时加 `--prerelease`；系统源装的照包管理器的办法。

## 卸载

各种装法都只删程序，数据根留着。

| 装法 | 怎么卸 |
|---|---|
| 安装脚本（Linux、macOS） | `curl -fsSL https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/install.sh \| sh -s -- --uninstall` |
| 安装脚本（Windows） | `& ([scriptblock]::Create((irm https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/install.ps1))) -Uninstall` |
| AUR | `sudo pacman -Rns miyu-agent miyu-core miyu-tui miyu-webui miyu-applet`，装了过渡包的连 `miyu` 一起写上 |
| Homebrew | `brew uninstall --cask miyu-applet`，再 `brew uninstall miyu-agent miyu-tui miyu-webui miyu-core` |
| 托盘安装包 | 照系统卸软件的办法：`apt remove`、`dnf remove`，Windows 的「设置 → 应用」，macOS 把 `Miyu.app` 拖进废纸篓 |

卸载脚本先停掉开机启动的服务（`miyu service disable`）、停掉核心（`miyu core stop`），再删安装目录、`~/.local/bin/miyu` 链接和 `.miyu` 文件类型；Windows 上从你的 `PATH` 里去掉安装目录。不是安装脚本装的目录，它不动。最后一句「已卸载 Miyu。数据还在 …，不要了自己删。」

Windows 上还有 Miyu 的程序开着时，安装目录删不掉，脚本报「删不掉 …（…）：还有 Miyu 的程序开着的，关掉再卸」并停下，`PATH` 也不改。关掉终端、网页这些 Miyu 的程序，再卸一次。

别的装法先自己停：

```sh
miyu service disable
miyu core stop
```

### 连数据一起删

卸完程序再删这几处：

| 什么 | Linux | macOS | Windows |
|---|---|---|---|
| 数据根 | `~/.miyu` | `~/.miyu` | `%USERPROFILE%\.miyu` |
| 缓存 | `~/.cache/miyu`（设了 `XDG_CACHE_HOME` 的在它下面） | `~/Library/Caches/Miyu` | `%LOCALAPPDATA%\Miyu\cache` |

设了 `MIYU_HOME` 的，数据根是它指的目录。数据根里是全部会话、记忆、设置和存下的 key，删了找不回来，要留的先备份。

## 从 0.7 旧版来的

旧版的数据不会自己搬过来。旧版的数据目录也在 `~/.miyu`，这一版的核心认不出它、起不来，要先把它挪开：

```bash
mv ~/.miyu ~/.miyu-0.7
```

```powershell
Rename-Item $HOME\.miyu .miyu-0.7
```

AUR 上从旧的 `miyu` 包升级上来的，pacman 会印「运行 miyu migrate 把旧版数据搬过来」：`miyu migrate` 是迁移软件包带的命令，要先装上：

```sh
miyu pkg install migrate
```

然后照 [从 0.7 旧版迁过来](migrate.md) 一步步做。
