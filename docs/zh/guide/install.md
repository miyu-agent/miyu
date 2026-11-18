# 安装、升级、卸载

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

这一页是 Miyu 的安装、升级和卸载方法，也说明了安装之后电脑上多了哪些东西。如果想让核心一直运行，或者部署到服务器上，可以看 [常驻和服务器](service.md)。

## 选择安装方式

不同的系统可以用的安装方式不同：

| 系统 | 架构 | 安装脚本 | AUR | Homebrew | 托盘安装包 |
|---|---|---|---|---|---|
| Linux | x86_64 | `install.sh` | 有 | 没有 | deb、rpm |
| Linux | aarch64 | `install.sh` | 有 | 没有 | 没有 |
| macOS | Apple 芯片 | `install.sh` | 没有 | 有 | dmg |
| Windows | x86_64 | `install.ps1` | 没有 | 没有 | msi |

Intel 芯片的 Mac，以及 32 位和 ARM 的 Windows 目前没有发行包。

不同的安装方式装上的内容也不同：

| 安装方式 | 核心和命令行 | 终端界面 | 网页界面 | 系统托盘 |
|---|---|---|---|---|
| 安装脚本 | 有 | 有 | 有 | 没有 |
| AUR 的 `miyu-agent`、Homebrew 的 `miyu-agent` 加 cask | 有 | 有 | 有 | 有 |
| 托盘安装包 | 有 | 没有 | 有 | 有 |

同一台电脑只使用一种安装方式，混用会互相冲突。

## 使用安装脚本

安装脚本会从 GitHub 的发行版页面下载对应你系统的压缩包，校验通过之后再安装。不需要管理员权限，也不需要 `sudo`。

### Linux、macOS

运行：

```sh
curl -fsSL https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/install.sh | sh
```

> 需要系统里有 `curl` 和 `tar`。

可以在 `sh -s --` 后面加参数：

| 参数 | 作用 |
|---|---|
| `--version <版本>` | 安装指定的版本，例如 `--version 2026.11.0` |
| `--prerelease` | 把预发布版也算进来，安装其中最新的一版 |
| `--uninstall` | 卸载 |

例如安装 2026.11.0 这一版：

```sh
curl -fsSL https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/install.sh | sh -s -- --version 2026.11.0
```

安装完成后：

- 压缩包里的整个目录放在 `~/.local/lib/miyu/`，旁边的 `install.toml` 记录了安装方式，内容是 `channel = "script"`。
- `~/.local/bin/miyu` 是指向 `~/.local/lib/miyu/miyu` 的链接。
- 在 Linux 上，脚本还会写入 `~/.local/share/mime/packages/miyu.xml`（如果设置了 `XDG_DATA_HOME`，就写在它下面），让系统能识别 `.miyu` 软件包文件。
- 脚本不会修改 shell 的配置。如果 `~/.local/bin` 不在 `PATH` 里，脚本最后会提示 `提示：… 不在 PATH 里`，把 `export PATH="$HOME/.local/bin:$PATH"` 加进 shell 的配置文件即可。如果 `PATH` 里先找到的是别处的 `miyu`，脚本也会提示。

如果已经安装过，脚本会提示 `已经装了 …，升级用 miyu upgrade`，不做任何修改。

### Windows

在 PowerShell 里运行（Windows PowerShell 5.1 和 PowerShell 7 都可以）：

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/install.ps1)))
```

参数加在命令的最后：`-Version <版本>`、`-Prerelease`、`-Uninstall`，作用和上面的表格相同。例如：

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/install.ps1))) -Version 2026.11.0
```

安装完成后：

- 压缩包里的整个目录放在 `%LOCALAPPDATA%\Programs\Miyu\`，旁边的 `install.toml` 内容是 `channel = "script"`。
- 脚本会把这个目录加进当前用户的 `PATH`，只修改你自己的环境变量。之后新开的终端可以直接运行 `miyu`，运行安装脚本的那个窗口也可以立即使用。

### 脚本的校验步骤

安装脚本在安装之前会按顺序做三项检查：

1. 下载这一版的 `SHA256SUMS` 和它的签名 `SHA256SUMS.sig`，用写在脚本里的发版公钥校验签名。
2. 下载压缩包，校验 SHA-256。
3. 解压，运行一次新的 `miyu --version`，确认版本正确。

任何一步不通过，脚本都会停下，不安装任何东西。校验签名需要支持 Ed25519 的 `openssl`（OpenSSL 3 及以上；Windows 上用 Git for Windows 自带的即可）。macOS 自带的是 LibreSSL，无法校验；Windows 自带的工具也不支持 Ed25519。在无法校验签名的情况下，脚本只校验 SHA-256，然后继续安装，并给出提示：

| 情形 | 脚本的提示 |
|---|---|
| `openssl` 不支持 Ed25519（macOS 上使用自带的就是这种情况） | `这台的 openssl 不认 Ed25519，签名没核（不是签名坏了）；只核 SHA-256。…` |
| Linux、macOS 上没有 `openssl` | `这台没有 openssl，没法核签名；只核 SHA-256。…` |
| Windows 上没有 `openssl` | `这台没有 openssl（Windows 自带的工具不认 Ed25519），签名没核（不是签名坏了）；只核 SHA-256。…` |

如果想自己校验签名，可以使用提示里给出的 `gh attestation verify <文件> -R miyu-agent/miyu`。

如果设置了 `GITHUB_TOKEN`，脚本访问 GitHub 接口时会带上它（下载文件时不带）。由于匿名访问每小时只有 60 次，几个人共用一个出口的网络容易触发限流，此时脚本会提示 `GitHub 限流了`。

## 从 AUR 安装（Arch Linux）

Arch Linux 上可以从 AUR 安装 Miyu。使用你习惯的 AUR 助手（例如 `paru`）运行：

```sh
paru -S miyu-agent
```

AUR 上有这几个包：

| 包 | 内容 | 安装位置 |
|---|---|---|
| `miyu-agent` | 合集，本身不带文件，依赖下面四个包 | |
| `miyu-core` | 核心和命令行 | `/usr/lib/miyu/`，`/usr/bin/miyu` 是指向它的链接，另外安装 `/usr/share/mime/packages/miyu.xml` |
| `miyu-tui` | 终端界面 | `/usr/share/miyu/packages/tui/` |
| `miyu-webui` | 网页界面 | `/usr/share/miyu/packages/webui/` |
| `miyu-applet` | 系统托盘，需要 `gtk3` 等系统库 | `/usr/bin/miyu-applet`，以及应用菜单项和图标 |
| `miyu-agent-git` | 用最新源码编译的核心、终端界面和网页界面，合在一个包里，不含系统托盘 | 和 `miyu-core`、`miyu-tui`、`miyu-webui` 相同 |
| `miyu` | 给 0.7 旧版用户的过渡包，本身不带文件，依赖 `miyu-agent` | |

- 如果不需要系统托盘，可以安装 `miyu-core miyu-tui miyu-webui` 这三个包，或者只安装需要的几个。终端界面和网页界面都依赖 `miyu-core`。
- `miyu-agent-git` 和 `miyu-core`、`miyu-tui`、`miyu-webui`、`miyu-agent` 互斥。编译它需要 `git`、`rustup`、`python`，编译时会联网获取依赖。如果需要系统托盘，要另外安装 `miyu-applet`。
- `miyu-core` 和 0.7 那一代的旧包 `miyu` 互斥，安装时 pacman 会让你确认替换旧包。
- `install.toml` 写在 `/usr/lib/miyu/`，内容是 `channel = "aur"`。

## 使用 Homebrew 安装（macOS）

macOS 上可以用 Homebrew 安装 Miyu。Miyu 的 tap（Homebrew 的第三方软件源）是 `miyu-agent/miyu`，依次运行：

```sh
brew tap miyu-agent/miyu
brew install miyu-agent
brew install --cask miyu-applet
```

这几条命令装上的内容：

| 名字 | 种类 | 内容 |
|---|---|---|
| `miyu-agent` | formula | 合集，依赖下面三个 formula；系统托盘是 cask，要另外安装 |
| `miyu-core` | formula | 核心和命令行，放在 formula 的 `libexec/` 里，`miyu` 链接到 Homebrew 的 `bin/`；`install.toml` 的内容是 `channel = "brew"` |
| `miyu-tui` | formula | 终端界面，安装到 Homebrew 前缀下的 `share/miyu/packages/tui/` |
| `miyu-webui` | formula | 网页界面，安装到 `share/miyu/packages/webui/` |
| `miyu-applet` | cask | 系统托盘 `Miyu.app`，依赖 `miyu-core`、`miyu-webui` |

这些包都只能在 Apple 芯片的 Mac 上安装。

## 系统托盘安装包

系统托盘的安装包把核心、网页界面和系统托盘打包在一起。安装后从应用菜单打开 `Miyu` 就可以使用，不需要打开终端；终端界面不包含在里面。到系统托盘的发行版页面 <https://github.com/miyu-agent/miyu-applet/releases> 下载：

| 系统 | 文件 |
|---|---|
| Linux x86_64 | `.deb`、`.rpm`，需要 GTK 3 |
| macOS Apple 芯片 | `.dmg`，把 `Miyu.app` 拖进 `应用程序` |
| Windows x86_64 | `.msi`，附带一份 PowerShell 7 |

> 这些安装包没有签名，发行版都标记为预发布。

系统软件源里的 `miyu-core` 和这些安装包装上的核心会冲突，所以不要和 AUR、Homebrew 的安装方式混用。

系统托盘的用法可以看 [系统托盘](heads/applet.md)。

## 确认安装成功

运行：

```sh
miyu --version
```

如果输出 `miyu 2026.11.0` 这样的一行，说明安装成功。版本号的格式是「年.月.这个月第几版」。

在终端里运行 `miyu` 就会打开界面。第一次打开时会进入新手引导，带你配置模型供应商、选择主题和人格，详细的见 [快速上手](quickstart.md)。

## 安装的内容

安装脚本装的是一体包，一个目录里包含以下内容：

- 五个程序：`miyu`（核心和命令行；核心就是 `miyu` 在后台运行时的状态）、`miyu-sandbox`、`miyu-extract`、`miyu-tui`（终端界面）、`miyu-web`（网页界面）。在 Windows 上它们都带 `.exe` 后缀，例如 `miyu.exe`、`miyu-sandbox.exe`。
- `resources/`：核心的出厂资源，以及终端界面、网页界面两个软件包（`resources/packages/tui/`、`resources/packages/webui/`）。
- `LICENSE`、`THIRD-PARTY-LICENSES.txt`、`README.txt`。`README.txt` 说明了自己解压压缩包之后怎么使用：Linux、macOS 上是 `把这个目录加进 PATH，或者在 ~/.local/bin 里放一个指向 miyu 的链接`，Windows 上是 `把这个目录加进 PATH（或者直接写 miyu.exe 的完整路径）`。
- 安装脚本写入的 `install.toml`。

终端界面和网页界面是两个软件包，各有自己的版本号，每一版 Miyu 都带着和它配套的那一份。在 `miyu pkg list` 里可以看到它们，编号是 `tui`、`webui`。AUR 和 Homebrew 把它们拆成单独的包，安装到系统目录下的 `share/miyu/packages/`，核心同样能找到它们。核心会到这些位置查找：

| 系统 | 系统包管理器安装的软件包的查找位置 |
|---|---|
| Linux | `XDG_DATA_DIRS` 里每一个目录下的 `miyu/packages/`；没有设置时使用 `/usr/local/share:/usr/share`。Linuxbrew 安装的软件包不会被自动找到，需要你自己把 Linuxbrew 的 `share` 目录加进 `XDG_DATA_DIRS` |
| macOS | Homebrew 前缀下的 `share/miyu/packages/`，前缀是 `miyu` 所在目录的上一级 |
| Windows | 没有这个位置，只查找安装目录和数据根里的软件包 |

因为每次升级都会整个替换 `resources/`，所以不要在安装目录里放自己的东西。

你的会话、设置和 key 都不在安装目录里，而是在数据根（Miyu 存放你的数据的目录）里：默认是 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`；如果设置了环境变量 `MIYU_HOME`，就是它指向的目录。安装、升级和卸载都不会改动数据根。

## 升级

升级 Miyu 运行：

```sh
miyu upgrade
```

可以加的参数：

| 参数 | 作用 |
|---|---|
| `--prerelease` | 把预发布版也算进来 |
| `--yes` | 不询问 `升级 Miyu … → …？[Y/n]`，直接升级 |
| `-h`、`--help` | 显示帮助 |

`miyu upgrade` 会读取 `miyu` 程序旁边的 `install.toml`，判断 Miyu 是用哪种方式安装的：

| `channel` | 安装方式 | `miyu upgrade` 的行为 |
|---|---|---|
| `script` | 安装脚本 | 自己升级，过程见下文 |
| `aur` | AUR | 提示 `这份 Miyu 由 AUR 安装，用包管理器升级`，下一行是 `paru -Syu` |
| `brew` | Homebrew | 提示 `这份 Miyu 由 Homebrew 安装，用包管理器升级`，下一行是 `brew upgrade` |
| 没有 `install.toml` | 托盘安装包、手动解压的压缩包 | 提示 `这份 Miyu 不是用安装脚本或包管理器装的，不自动升级`，并给出下载地址 |

如果 Miyu 是用安装脚本安装的，`miyu upgrade` 会这样升级：

1. 向 GitHub 查询最近的发行版，选出比当前版本新的最新一版。如果没有更新的版本，会提示 `已是最新版本 …`。
2. 询问你是否升级。直接回车或者输入 `y` 表示升级。如果不是在终端里运行、读不到回答，会提示 `已取消：没有收到确认`，此时需要加上 `--yes`。
3. 下载这一版的压缩包、`SHA256SUMS` 和 `SHA256SUMS.sig`，用编译进程序的发版公钥校验签名、校验 SHA-256，解压后运行一次新的 `miyu --version`。
4. 替换安装目录里属于 Miyu 的文件，不修改 `install.toml`。不属于 Miyu 的东西原样保留，并提示 `安装目录里还有 … 样不是 Miyu 的，原样留着`。旧文件会先移进安装目录里的 `.miyu-old-<旧版本>/`，替换完成后再删除；在 Windows 上，正在运行的 `miyu.exe` 无法删除，会留到下一次运行 `miyu upgrade` 的开头再删除。
5. 提示 `已升级 Miyu … → …`。

终端界面和网页界面在同一个压缩包里，会跟着一起升级。任何一步出错，升级都会停下，已经安装的版本保持不变。

升级完成后，已经在运行的核心仍然是旧版。运行：

```sh
miyu core stop
```

这样下次打开终端界面或网页界面时，启动的就是新版。如果安装了开机启动服务，并且希望服务继续管理核心，再运行 `miyu service enable`：Linux 的 systemd 用户服务和 macOS 会立即启动新版；Linux 的桌面自启动和 Windows 会在下次登录时启动新版。详细的见 [常驻和服务器](service.md#让服务立即接管)。

如果是用托盘安装包安装的，下载新的安装包覆盖安装即可。

### 更换安装方式

`channel` 由安装方式决定，不要手动修改 `install.toml`。如果想从一种安装方式换到另一种：

1. 按照下面「卸载」一节卸载现在的 Miyu。
2. 用新的方式安装。

数据根不受影响，更换之后会话和设置都还在。

如果想使用预发布版：用安装脚本安装时，升级时加上 `--prerelease`；从系统软件源安装时，按照包管理器自己的方法操作。

## 卸载

每种安装方式卸载时都只删除程序，保留数据根。

| 安装方式 | 卸载方法 |
|---|---|
| 安装脚本（Linux、macOS） | `curl -fsSL https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/install.sh \| sh -s -- --uninstall` |
| 安装脚本（Windows） | `& ([scriptblock]::Create((irm https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/install.ps1))) -Uninstall` |
| AUR | `sudo pacman -Rns miyu-agent miyu-core miyu-tui miyu-webui miyu-applet`；如果装了过渡包，把 `miyu` 也一起写上 |
| Homebrew | `brew uninstall --cask miyu-applet`，然后运行 `brew uninstall miyu-agent miyu-tui miyu-webui miyu-core` |
| 托盘安装包 | 按照系统卸载软件的方法：`apt remove`、`dnf remove`；Windows 在 `设置 → 应用` 里卸载；macOS 把 `Miyu.app` 拖进废纸篓 |

卸载脚本会先停掉开机启动的服务（`miyu service disable`）和核心（`miyu core stop`），再删除安装目录、`~/.local/bin/miyu` 链接和 `.miyu` 文件类型；在 Windows 上还会从你的 `PATH` 里去掉安装目录。如果目录不是安装脚本安装的，卸载脚本不会改动它。最后脚本会提示 `已卸载 Miyu。数据还在 …，不要了自己删。`

在 Windows 上，如果还有 Miyu 的程序开着，安装目录就删不掉，此时脚本会报错 `删不掉 …（…）：还有 Miyu 的程序开着的，关掉再卸` 并停下，`PATH` 也不会修改。关掉终端界面、网页界面等 Miyu 的程序，再卸载一次即可。

其他安装方式需要你在卸载之前自己先停掉服务和核心：

```sh
miyu service disable
miyu core stop
```

### 删除数据

如果连数据也不要了，卸载程序之后再删除这几个位置：

| 内容 | Linux | macOS | Windows |
|---|---|---|---|
| 数据根 | `~/.miyu` | `~/.miyu` | `%USERPROFILE%\.miyu` |
| 缓存 | `~/.cache/miyu`（如果设置了 `XDG_CACHE_HOME`，就在它下面） | `~/Library/Caches/Miyu` | `%LOCALAPPDATA%\Miyu\cache` |

如果设置了 `MIYU_HOME`，数据根就是它指向的目录。**数据根里是全部的会话、记忆、设置和保存的 key，删除之后找不回来**，需要保留的内容先备份。

## 从 0.7 旧版迁移

旧版的数据不会自动迁移过来。由于旧版的数据目录也是 `~/.miyu`，而这一版的核心识别不了它、会无法启动，所以要先把它移到别处。Linux、macOS 上运行：

```bash
mv ~/.miyu ~/.miyu-0.7
```

Windows 上在 PowerShell 里运行：

```powershell
Rename-Item $HOME\.miyu .miyu-0.7
```

如果是在 AUR 上从旧的 `miyu` 包升级上来的，pacman 会输出 `运行 miyu migrate 把旧版数据搬过来`。`miyu migrate` 是迁移软件包带的命令，需要先安装这个软件包：

```sh
miyu pkg install migrate
```

然后按照 [从 0.7 旧版迁过来](migrate.md) 一步步操作即可。
