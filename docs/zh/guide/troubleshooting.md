# 排查问题

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

如果 Miyu 无法启动、连不上、没有反应，或者报了看不懂的错，就从这一页开始查。每个功能自己特有的报错写在那个功能的页面的「出了问题」一节里，这一页收集的是通用的问题。

## 先运行 miyu check

`miyu check` 会检查你手写的、Miyu 会读取的文件，是排查问题的第一步。运行：

```sh
miyu check
```

它检查的是磁盘上文件现在的内容，所以刚改完、核心还没有重新读取的文件也能查到。不写文件名时，它会检查下面的全部内容：

| 查什么 | 在哪 |
|---|---|
| 系统配置 | `~/.miyu/etc/config.toml` |
| 当前目录的项目配置 | 从当前目录往上找到的 `.miyu/config.toml` |
| 密钥文件，以及配置里引用了却取不到的密钥和环境变量 | `~/.miyu/etc/private/secrets.toml` |
| 配置里写的人格、预设、知识库、供应商、池是否存在 | 系统配置 |
| 已经卸载的软件包留在配置里的设置 | 系统配置 |
| 每个人格的 `persona.toml`、`prompts/examples.md` | 人格目录 |
| 每一份预设 | 预设目录 |
| 每个软件包的清单，包括出厂的和你安装的 | 软件包目录 |
| 软件包自己带的检查 | 软件包声明的程序 |
| 没有加载上的脚本、技能、斜杠命令模板，以及被同名的盖住的 | 软件包、`~/.miyu/root/` 下 |
| 写错的 MCP 服务，以及列出来却没有登记上的工具 | 系统配置、软件包清单 |
| 定时任务表里读不懂的行 | 定时任务表 |
| 写坏了的工具说明、提示词覆盖 | 覆盖文件 |
| 扩展提供的模型驱动：软件包没有批准，或者该配置的密钥没有配置 | 系统配置 |
| 安装好的软件包文件被修改过或者缺失 | 软件包目录 |

上面的 `~/.miyu` 指的是数据根（默认是 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`）；如果设置了环境变量 `MIYU_HOME`，就是它指向的目录。如果只想检查一份文件，就把它写在命令后面：

```sh
miyu check ~/.miyu/root/personas/miyu/prompts/examples.md
```

### 读懂检查结果

每处问题占一行，依次是文件、行、列，`错误` 或 `警告`，以及问题的说明。问题之后的一行是合计，再往下是语义模型的状态。例如：

```text
~/.miyu/etc/config.toml:3:9 错误：log.level 只能是 error、warn、info、debug、trace 或 off，写的是 verbose
~/.miyu/etc/config.toml:12:1 警告：没有 models.chatt 这一项。是不是想写 models.chat？
~/.miyu/etc/config.toml:20:7 警告：providers.acme.key 引用的密钥 acme 还没有设置
1 处错误，2 处警告
语义模型：local:granite-embedding-97m-multilingual-r2，正在建立索引
  记忆：engineer 120/120
  知识库：handbook 30/42
```

- `错误`：写法不对，这一处的设置不生效；如果 TOML 本身就读不懂，整份文件都不生效。有错误时，退出码是 `1`。
- `警告`：不认识的键，或者引用的密钥、环境变量、人格、预设、知识库还不存在。Miyu 照常运行，只是这几处设置没有用上。只有警告时，退出码是 `0`。
- 没有任何问题时，会显示 `没有问题`。
- 语义模型那几行不算问题，它们说明正在使用哪个模型，以及记忆、以前的对话、知识库各有多少条建好了索引（建好的/总共）。没有安装语义模型时显示 `语义模型：未安装，仅按关键词搜索`，并提示运行 `miyu pkg install embed`；关闭了语义模型时显示 `语义模型：已关闭（models.embedding = "off"），仅按关键词搜索`。如果换成了远程模型，末尾会多一行 `不自动联想：记忆、知识库（这个模型没有实测的阈值，搜索不受影响）`，因为只有内置的模型会自动想起记忆和知识库，其他模型只在搜索时用到。

加上 `--format json` 时会输出一行 `{"problems": […]}`，每一处问题带 `kind`、`file`、`code`、`level`、`message`，有行列的还带 `line`、`column`；检查全部内容时另外带 `embedding`。

`miyu check` 需要连接核心，如果核心没有在运行，它会先启动核心。因此核心无法启动时，`miyu check` 也无法运行：这种情况先看下面的「核心无法启动或无法连接」，还解决不了就使用 [应急修复](rescue.md)。

## 日志

出了问题时，日志里通常记着原因。各个程序的日志位置如下：

| 谁的 | 在哪 |
|---|---|
| 核心 | `~/.miyu/var/log/core.log`，写满 10 MB 换一份，保留 `core.log.1` 到 `core.log.5` |
| 软件包、扩展 | `~/.miyu/var/log/<编号>/`；扩展程序往标准错误写的内容，核心记在这个目录的 `stderr` 里 |
| 网页界面 | `~/.miyu/var/log/webui/web.log` |
| 系统托盘 | `~/.miyu/var/log/miyu-applet.log` |
| 终端界面崩溃 | 缓存目录下的 `tui/crash.log`：Linux 上是 `~/.cache/miyu/tui/crash.log`（设置了 `XDG_CACHE_HOME` 时在它下面），macOS 上是 `~/Library/Caches/Miyu/tui/crash.log`，Windows 上是 `%LOCALAPPDATA%\Miyu\cache\tui\crash.log` |

查看最近的几十行，运行：

```sh
tail -n 50 ~/.miyu/var/log/core.log
```

在 Windows PowerShell 里运行：`Get-Content -Tail 50 $HOME\.miyu\var\log\core.log`。

核心的日志一行一条，用英文写，不记录对话内容和密钥。核心每次启动时会记一行 `starting`，带着版本和进程号。如果要记录得更详细，就调整 `log.level`：

```sh
miyu config set log.level debug
```

`log.level` 可以写 `error`、`warn`、`info`（出厂默认）、`debug`、`trace`、`off`，修改后立即生效。如果只想让这一次记录得详细一些，先运行 `miyu core stop`，再用下面的方式打开 Miyu：这样启动的核心这一次按 `debug` 记录，优先于 `log.level`。

```bash
MIYU_LOG=debug miyu
```

```powershell
$env:MIYU_LOG = 'debug'
miyu
```

更多内容见 [常驻和服务器](service.md#日志)。

## 核心无法启动或无法连接

界面和命令行打开时会先连接核心，连不上就启动一个核心，并等待它启动完成。如果 3 秒后还没有连上，命令行会先输出 `miyu: 核心还在启动…`，然后继续等待，最多等待 10 分钟；如果核心在启动过程中退出了，或者给出了错误，就会立即报错，不会等满 10 分钟。等待时按 `Ctrl+C` 可以立即退出。一个数据根同一时间只运行一个核心，这个核心占着 `~/.miyu/run/core.lock`。

| 看到的 | 原因 | 怎么办 |
|---|---|---|
| `核心无法启动：…` | 核心启动时出了错，冒号后面是核心给出的原因。 | 按照原因修改。如果看不懂原因，就看 `core.log` 的最后几行。 |
| `核心没有启动，也没有给出原因，可以查看数据根中的 …` | 核心没有给出原因就退出了。 | 查看 `~/.miyu/var/log/core.log` 的最后几行。 |
| `miyu: 核心还在启动…` | 这不是错误：核心启动得比平时慢，例如磁盘繁忙，或者几个核心同时启动。 | 等待即可。 |
| `等待核心启动超时` | 核心的进程还在，但是 10 分钟后仍然没有启动完成，多半是卡住了。 | 查看 `core.log` 的最后几行，然后按照下面「旧版核心仍在运行」一节的办法，用 `core.log` 里最后一行 `starting` 记下的进程号结束它，再重新打开 Miyu。 |
| `其他程序正在启动核心，等待超时` | 另一个界面正在启动核心，10 分钟后还没有完成。 | 和上一行一样。 |
| `miyu: 核心还没有回应，继续等待…`，最后是 `核心一直没有回应，已停止等待` | 已经连上了核心，但是核心一直没有回应，最多等待 10 分钟，多半是核心卡住了。 | 和 `等待核心启动超时` 一样。 |
| `无法启动核心：…` | 找不到核心程序，或者没有运行它的权限。 | 重新安装 Miyu，见 [安装、升级、卸载](install.md)。 |
| `… 无法确认它是 Miyu 的数据根 …` | 数据根的位置上已经有其他东西。0.7 旧版的数据目录也会报这一句。 | 如果是 0.7 旧版的数据目录，按照这句话后半部分说的把它挪开、安装迁移软件包、进行迁移，见 [从 0.7 旧版迁过来](migrate.md)；如果不是，就把 `MIYU_HOME` 设成一个空目录。 |
| `… 是旧版 Miyu 的数据根，当前版本无法使用 …` | 这是正式发布之前的开发版留下的数据根。 | 把 `MIYU_HOME` 设成一个空目录。 |
| `… 的目录布局与当前版本不符 …` | 更新版本的 Miyu 写过这个数据根。 | 使用写过它的那个版本，或者换一个数据根。 |
| `… 不属于当前用户，或者其他用户也能访问，因此没有连接`（Linux、macOS）；`命名管道 … 的另一端不是当前用户的进程，因此没有连接`（Windows） | Linux、macOS：存放套接字的目录不属于你，或者其他用户也能进入，所以 Miyu 不连接这个目录里的套接字。Windows：命名管道另一头的程序不是你这个用户运行的。 | Linux、macOS：把那个目录改成只有你能进入，例如运行 `chmod 700 <目录>`。Windows：关掉占着这个管道的程序，再重新打开 Miyu。 |
| `核心无法启动：套接字路径 … 过长，改放到临时目录下也超出长度限制`（Linux、macOS） | 数据根的路径太长。 | 换一个短一点的 `MIYU_HOME`，或者把 `TMPDIR` 设成路径短的目录。 |
| `核心无法启动：… 已被占用，可能是一个不是套接字的文件，或者其他程序正在监听。没有修改它`（Linux、macOS）；`核心无法启动：命名管道 … 已经被其他程序占用。没有修改它`（Windows） | Linux、macOS：套接字的位置被其他文件或程序占用了。Windows：命名管道的名字被其他程序占用了。 | 如果占用它的不是 Miyu，就把那个文件挪走，或者关掉那个程序。 |
| `与核心的连接已断开` | 核心中途退出或者崩溃了。 | 查看 `core.log`，然后重新打开 Miyu。 |

套接字在各个系统上的位置：

| 系统 | 在哪 |
|---|---|
| Linux | 设置了 `XDG_RUNTIME_DIR` 时是 `$XDG_RUNTIME_DIR/miyu-<指纹>/core.sock`，否则是 `~/.miyu/run/core.sock`；路径太长时退到 `$TMPDIR/miyu-<uid>/` |
| macOS | `~/.miyu/run/core.sock`；路径太长时退到 `$TMPDIR/miyu-<uid>/` |
| Windows | 命名管道 `\\.\pipe\miyu-<指纹>`，只对你这个用户开放 |

### 旧版核心仍在运行

升级以后，已经在运行的核心仍然是旧版，新版的界面连接上去时可能报 `协议版本不兼容，请升级到同一版本。`。此时先停掉旧核心，下次打开 Miyu 时启动的就是新版核心。运行：

```sh
miyu core stop
```

如果你安装了开机启动服务，并且希望继续由服务管理核心，再运行一次 `miyu service enable`，见 [常驻和服务器](service.md#让服务立即接管)。

如果 `miyu core stop` 提示 `核心还在运行，但无法连接：数据根 run/ 里的令牌或套接字位置文件已经不存在`，说明 `~/.miyu/run/` 被删除了，而核心还在运行。按照提示到 `core.log` 里找最后一行 `starting` 记下的进程号，然后结束这个进程：

```bash
kill <进程号>
```

```powershell
Stop-Process -Id <进程号>
```

### 核心停止得很慢

核心收到停止信号（例如停止开机启动服务，或者运行 `kill <进程号>`）以后，会先停下正在运行的会话和扩展，再退出。这一步最多用 30 秒，超过 30 秒核心会直接退出。如果不想等，再发一次停止信号即可，核心收到第二次会立即退出，并在 `core.log` 里记下 `second stop signal, exiting now`。

### 所有界面都打不开

如果核心坏了、所有界面都打不开，就用 `miyu rescue` 进入 LiteMiyu 的应急模式来修复，应急模式不经过核心。LiteMiyu 只有 Linux x86_64 的版本，macOS、Windows 上没有。详细的见 [应急修复](rescue.md)。

## 没有配置模型或密钥不对

Miyu 需要配置好模型和密钥才能对话。常见的提示和处理方法如下：

| 看到的 | 怎么办 |
|---|---|
| `未配置模型`，提示运行 `miyu setup`；`miyu ask` 的退出码是 `5` | 运行 `miyu setup`，见 [模型](models.md)。 |
| `miyu check` 警告 `… 引用的密钥 … 未设置` | 运行 `miyu provider login <名字>`，把密钥存进去。 |
| `miyu check` 警告 `… 引用的环境变量 … 在核心启动时未设置` | 核心是在设置这个变量之前启动的。运行 `miyu core stop`，再在设置了这个变量的 shell 里重新打开 Miyu。开机启动服务启动的核心看不到 shell 里的变量，见 [常驻和服务器](service.md#服务和环境变量)。 |
| `模型请求失败。`、认证失败 | 密钥填错了或者已经过期，重新运行 `miyu provider login <名字>`。 |

## 模型持续报错

`模型冷却中，请稍后重试。` 和 `没有可用的模型。` 说明供应商限流了、额度用完了或者连不上，于是 Miyu 把这个模型暂时搁置一段时间。重试多少次、冷却多久、怎么用池换用其他模型，见 [模型](models.md#限流额度用完时)。

## 命令被拒绝或一直等待确认

AI 运行的命令可能被权限设置拦下，或者停在等待确认的状态。常见的情况如下：

| 看到的 | 原因 |
|---|---|
| `只读模式，未执行` | 会话是只读的，不能写入。 |
| 确认框一直在等待 | 它在等你点击允许或不允许；到时间没有人点击就会自动拒绝，并显示 `无人确认，已自动拒绝`。 |
| `需要确认，无人确认`；`miyu ask` 的退出码是 `4` | 在 `miyu ask` 和通讯平台上没有人能当场确认，所以需要确认的那几步不会执行。 |

怎么切换权限级别、哪些命令需要确认、怎么关闭确认，见 [权限和沙盒](permissions.md)。

## 软件包无法安装或扩展无法启动

安装软件包或者启动扩展失败时，常见的提示和处理方法如下：

| 看到的 | 怎么办 |
|---|---|
| `清单有误，无法安装。` | 运行 `miyu pkg info <包目录>`，它只查看、不安装，会说明清单哪里不对。 |
| `缺少依赖的软件包。` | 先安装它依赖的软件包，可以用 `miyu pkg info <编号>` 查看依赖。 |
| `不支持当前系统。` | 这个软件包没有适合你这种系统的版本。 |
| `无法连接软件源。` `软件包下载失败。` | 检查网络，过一会儿运行 `miyu pkg update` 再试。 |
| `没有找到 <程序>：清单 … 写明 miyu <名字> 由它运行。重新安装这个软件包后再试。` | 运行 `miyu pkg install <编号>` 重新安装。 |
| `界面 'tui' 没有安装` | 运行 `miyu pkg install tui` 重新装回来。 |
| 扩展停了 | 查看 `~/.miyu/var/log/<编号>/stderr`，那里是扩展自己报的错。 |

`miyu pkg check` 可以检查安装好的文件有没有被修改过。详细的见 [软件包](packages.md)。

## 终端界面显示异常

终端界面显示不正常时，常见的情况和处理方法如下：

| 看到的 | 怎么办 |
|---|---|
| 图标显示成方块、问号 | 字体里没有 Nerd Font 的图标。安装一个 Nerd Font，或者运行 `miyu config set tui.icons plain` 改用普通字符。 |
| 颜色不对、太刺眼 | 在设置页 `外观 → 配色方案` 里调整 `配色`、`明暗`、`背景填充`，对应的配置键是 `tui.theme`、`tui.theme_mode`、`tui.theme_background`。 |
| 命令行的输出里有像乱码一样的颜色符号 | 设置环境变量 `NO_COLOR=1`。 |
| 终端界面崩溃了 | 崩溃记录在 `tui/crash.log` 里，位置见上面的「日志」。 |

如果安装了 shell 集成，可以运行 `miyu shellhook doctor`，它会先列出装进了哪几个 shell，再逐条检查核心、终端、剪贴板。输出类似这样：

```text
✓ 连得上核心
✗ 终端没说自己是真彩色（COLORTERM 不是 truecolor）：配色最多 256 色
✗ 问不到终端的颜色：配色跟随终端时只用终端的 16 色（tmux 这类要开透传）
✓ Ctrl+V 读剪贴板用 wl-paste
```

这是 Linux 上 Wayland 桌面里的输出。在其他环境里，最后一行是：

| 系统 | 最后一行 |
|---|---|
| Linux，没有 Wayland | `Ctrl+V 读剪贴板用 xclip` 或 `… 用 xsel` |
| macOS | `Ctrl+V 读剪贴板用 pbpaste` |
| Windows | `Ctrl+V 读剪贴板用 powershell` |

如果一个读剪贴板的命令都找不到，会显示 `✗ 找不到读剪贴板的命令（…）：Ctrl+V 只能用终端自己的粘贴`，括号里是在这台机器上试过的那几个命令。

终端界面的设置见 [终端](heads/tui.md)。

## 网页界面无法打开

网页界面打不开或者登录不了时，常见的提示和处理方法如下：

| 看到的 | 怎么办 |
|---|---|
| `端口 8300 已被占用`，提示用 `miyu web --port <端口>` 换一个 | 换一个端口。如果从来没有指定过端口，8300 被占用时 Miyu 会依次尝试 8301 到 8309，全都被占用才报这个错。 |
| `无法打开浏览器，请手动打开上面的地址` | 按照输出的地址自己在浏览器里打开。没有桌面的机器使用 `miyu web --print`。 |
| 登录页显示 `访问密码错误` | 如果忘了密码，在这台电脑上运行 `miyu web --reset`，在打开的页面里重新设置。 |
| 整个页面显示 `还没设访问密码` | 在运行 Miyu 的电脑上运行 `miyu web --settings access` 设置密码。 |
| `尝试次数过多，N 秒后再试` | 等提示里的秒数过去之后再试。 |

网页界面自己的日志在 `~/.miyu/var/log/webui/web.log`。如果要从其他设备或者通过反向代理打开网页界面，见 [常驻和服务器](service.md#从其他电脑和手机打开网页界面)。

## 配置文件写错

配置文件写错时，Miyu 的处理方式如下：

- `miyu ask` 开头提示 `配置中有 N 处错误`，并提示运行 `miyu check` 查看。按照 `miyu check` 输出的行号修改即可。
- 如果核心启动时某一份配置整份读不进来，这一份按空的处理，核心照常启动。如果核心运行时你手动把配置改坏了，核心会继续使用上一次成功读取的那一份。
- 文件读不进来时，`miyu config set` 不会修改它，而是报 `配置文件有误，请先修复。`。`miyu config edit` 在保存之前会先检查。
- 如果某一项设置不生效，运行 `miyu config explain <键>`，它会列出每一层写了什么、哪一层生效，以及写了却不生效的原因。
- 如果提示 `项目配置 … 未信任，本次没有使用`，就在那个目录里运行 `miyu config trust`，看过内容之后再决定是否信任。

详细的见 [配置](config.md)。

## 特定系统上的问题

有些问题只在某个系统上出现，下表按提示列出：

| 看到的 | 系统 | 原因 | 怎么办 |
|---|---|---|---|
| `miyu ask` 开头显示 `沙盒不可用：内核没有可用的 Landlock`，下一行是 `提示：需要 Linux 5.13 或更高版本，并在启动参数 lsm= 里启用 Landlock` | Linux | 内核版本低于 5.13，或者启动参数 `lsm=` 里没有 `landlock`。 | 按照提示更换内核、修改启动参数。在那之前，只读模式下的命令改为询问你。 |
| `miyu ask` 开头显示 `沙盒不可用：无法应用 Seatbelt 配置，Miyu 可能运行在另一个沙盒里` | macOS | Seatbelt 配置无法应用。 | 不要在其他沙盒里运行 Miyu。在那之前，只读模式下的命令改为询问你。 |
| `miyu ask` 开头显示 `沙盒不可用：这台 Windows 无法启动 MXC 的基础容器` | Windows | 沙盒需要的基础容器在这台 Windows 上无法启动。 | 只读模式下的命令改为询问你。 |
| 只读模式下命令的输出是 `miyu-sandbox: cannot confine: no desktop session, programs in the sandbox cannot start here; use Miyu from the desktop, or turn the sandbox off`，退出码 `125` | Windows | 核心不在桌面会话里，例如核心是通过 SSH 连进来以后启动的。 | 从已登录的桌面使用 Miyu，或者切换到开放权限，见 [常驻和服务器](service.md#windows-上的只读会话)。 |
| 只读模式下命令的输出是 `miyu-sandbox: cannot confine: this Windows has no base container (MXC tier …)` | Windows | 这台 Windows 无法启动沙盒需要的基础容器。 | 切换到开放权限再运行。 |
| 只读模式下命令的输出后面跟着 `The sandbox blocks WMI. Commands that need it work outside the sandbox.` | Windows | 这条命令需要用到 WMI，而沙盒里连不上 WMI。 | 切换到开放权限，再让 AI 运行一次，见 [权限和沙盒](permissions.md#三个平台)。 |
| `无法安装服务：Windows 任务计划无法携带 MIYU_HOME`，下一行是 `提示：取消设置 MIYU_HOME 后重试`；终端界面设置页的 `开机启动核心` 一项显示 `设置了 MIYU_HOME 时，Windows 上不能设为开机启动` | Windows | 任务计划无法携带环境变量。 | 用默认的数据根安装服务；如果要使用其他数据根，就使用系统托盘的 `开机时启动`，见 [数据和备份](data.md#更换数据根的位置)。 |
| `core.log` 里有 `catalog cache unavailable`，原因写的是 `no LOCALAPPDATA` | Windows | 环境变量 `LOCALAPPDATA` 不存在，或者不是绝对路径，因此算不出缓存目录。 | 把 `LOCALAPPDATA` 设成绝对路径，然后运行 `miyu core stop`，再重新打开 Miyu。 |
| `未找到 litemiyu`，下一行是 `提示：目前只支持 Linux` | macOS、Windows | LiteMiyu 只有 Linux x86_64 的版本，所以 `miyu rescue` 无法使用。 | 按照这一页上面的「核心无法启动或无法连接」排查，见 [应急修复](rescue.md#其他系统)。 |
| `无法升级：没有这种系统的发行包`，下一行是 `提示：发行包只有 Linux x86_64、aarch64，macOS Apple 芯片，Windows x86_64；其他系统到 … 下载源码自行编译` | Intel 芯片的 Mac、ARM 的 Windows 这类系统 | Miyu 没有为这种系统发布发行包。 | 按照提示到发行页下载源码自己编译，见 [安装、升级、卸载](install.md#选择安装方式)。 |
| `不支持当前系统。` | 取决于软件包，例如 `im` 没有 Linux aarch64 的版本 | 这个软件包没有为你这种系统打包。 | 换到这个软件包支持的系统，或者等它发布这种系统的版本。 |
| `miyu call … --json '…'` 报 `--json takes one JSON object.`，后面一句是 `Its double quotes were lost on the way, as Windows PowerShell 5.1 does. Pipe the JSON instead: '<json>' \| miyu call <tool> --json -` | Windows | Windows PowerShell 5.1 把参数交给程序时吃掉了参数里的双引号。 | 按照提示改用管道传入：`'<json>' \| miyu call <工具> --json -`，见 [命令行](cli.md)。 |
| `miyu shellhook install` 提示 `PowerShell 的执行策略是 …，不加载启动文件，装了也不生效：在 PowerShell 里执行 Set-ExecutionPolicy -Scope CurrentUser RemoteSigned 再开新窗口` | Windows | 在这个执行策略下，PowerShell 不加载 `$PROFILE`，所以 shell 集成即使写进去也不生效。 | 按照提示执行一次那条命令，然后打开新窗口，见 [终端](heads/tui.md#shell-集成)。 |
| 终端界面里显示 `Linux 控制台显示不了中文：可在设置里把界面语言换成英文` | Linux | 终端界面是在 Linux 的文字控制台（`TERM=linux`）里打开的。 | 在设置页里把界面语言换成英文，或者在图形桌面的终端程序里使用。 |
| 卸载脚本报 `删不掉 …（…）：还有 Miyu 的程序开着的，关掉再卸` | Windows | 还有 Miyu 的程序开着，所以安装目录删不掉。 | 关掉终端界面、网页界面这些 Miyu 的程序之后再卸载，见 [安装、升级、卸载](install.md#卸载)。 |
| `旧版 Miyu 还在运行，请先停止它` | Windows | 迁移时旧版 Miyu 还开着。 | 关掉旧版 Miyu 之后再迁移，见 [从 0.7 旧版迁过来](migrate.md)。 |

## 原因码

核心拒绝执行一件事时，会给出一个原因码和一句说明。界面和命令行显示的是那句说明；直接通过协议连接核心的程序读取 `data.reason`。常见的原因码如下：

| 原因码 | 显示的话 | 怎么办 |
|---|---|---|
| `no_model` | 没有可用的模型。 | 运行 `miyu setup`，或者在 `models.chat` 里写一个能用的模型。 |
| `cooling` | 模型冷却中，请稍后重试。 | 等待一段时间，或者配置一个池，见 [模型](models.md)。 |
| `model_failed` | 模型请求失败。 | 查看界面上的详情；如果是认证失败，就重新保存密钥。 |
| `unknown_model` | 模型或模型池不存在。 | 写成 `供应商/模型` 或 `@池`，并且供应商要配置好。 |
| `unknown_provider` | 供应商不存在。 | 先配置这一家供应商，见 [模型](models.md)。 |
| `unknown_secret` | 密钥不存在。 | 运行 `miyu provider login <名字>`。 |
| `session_not_found` | 会话不存在。 | 把会话编号写完整；已经删除的会话无法找回。 |
| `turn_running` | 回合进行中。 | 等这一轮结束，或者先打断它，再撤回、压缩或删除。 |
| `archived` | 这是存档，不能继续对话。 | 不带 `--archive` 重新导出一次。 |
| `session_read_only` | 会话日志坏了，只能看到坏的地方以前的内容，不能接着聊。要继续请新开一个会话。 | 新开一个会话。 |
| `session_unavailable` | 会话暂时无法读取，请稍后重试。 | 过一会儿再试；这种情况常见于磁盘出错、打开的文件太多。 |
| `dir_too_wide` | 目录范围过大。 | 换一个范围小一点的目录。 |
| `attachment_too_big` | 附件过大（最大 20 MiB，图片 5 MiB、8000 像素）。 | 缩小附件，或者只提供路径。 |
| `attachment_in_data_root` | 不能附加数据目录里的文件。 | 把文件复制到数据根外面，再作为附件添加。 |
| `export_exists` | 文件已存在。 | 换一个文件名，因为导出不会覆盖已有的文件。 |
| `unknown_config_key` | 配置项不存在。 | 运行 `miyu config get` 查看有哪些键。 |
| `config_invalid` | 配置有误，未保存。 | 按照报错修改值，可以用 `miyu config explain <键>` 查看能写什么。 |
| `config_file_broken` | 配置文件有误，请先修复。 | 运行 `miyu check`。 |
| `unknown_package` | 软件包不存在。 | 运行 `miyu pkg search` 查找软件包的编号。 |
| `package_invalid` | 清单有误，无法安装。 | 运行 `miyu pkg info <包目录>` 查看哪里不对。 |
| `depends_missing` | 缺少依赖的软件包。 | 先安装依赖的软件包。 |
| `extension_off` | 扩展已停用。 | 在软件包页面里打开它。 |
| `bad_password` | 访问密码错误。 | 运行 `miyu web --reset`。 |
| `protocol_mismatch` | 协议版本不兼容，请升级到同一版本。 | 把界面和核心升级到同一个版本，然后运行 `miyu core stop`。 |
| `internal_error` | 内部错误，详见运行日志。 | 查看 `core.log`，再按照下面的方法报告问题。 |

全部原因码见 [协议参考：出错](../reference/errors.md)。

## 报告问题

如果按照上面的方法解决不了，就到核心仓库提交问题：<https://github.com/miyu-agent/miyu/issues>。提交时附上：

1. `miyu --version` 输出的版本。
2. 系统和系统版本，例如 Arch Linux、macOS 15、Windows 11；如果是终端界面的问题，再加上终端程序的名字。
3. `miyu check` 的输出。
4. 出问题那一刻前后 `core.log` 里的几行；如果是软件包的问题，附上 `~/.miyu/var/log/<编号>/` 里的日志；如果终端界面崩溃了，附上 `crash.log` 里的那一段。
5. 怎样操作能让问题再出现一次。

贴出来之前，先自己看一遍。日志和 `miyu check` 的输出里有你的路径、用户名、会话编号、软件包名；导出的会话会原样带着对话、命令输出和文件内容，不会打码。不要贴出 `~/.miyu/etc/private/secrets.toml`。
