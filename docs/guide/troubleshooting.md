# 出了问题怎么查

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

Miyu 起不来、连不上、没反应、报了看不懂的错，从这一页查起。各功能自己特有的报错写在各页的「出了问题」里，这里收总的。

## 先跑 miyu check

```sh
miyu check
```

它查你手写的、Miyu 读的文件，照磁盘上现在的字查：刚改完、核心还没重读的也查得到。不写文件的查下面全部：

| 查什么 | 在哪 |
|---|---|
| 系统配置 | `~/.miyu/etc/config.toml` |
| 当前目录的项目配置 | 从当前目录往上找到的 `.miyu/config.toml` |
| 密钥文件，和配置里引用了却取不到的密钥、环境变量 | `~/.miyu/etc/private/secrets.toml` |
| 配置里写的人格、预设、知识库、供应商、池存不存在 | 系统配置 |
| 已经卸掉的软件包留在配置里的设置 | 系统配置 |
| 每个人格的 `persona.toml`、`prompts/examples.md` | 人格目录 |
| 每一份预设 | 预设目录 |
| 每个软件包的清单，出厂的和你装的 | 软件包目录 |
| 软件包自己带的检查 | 软件包声明的程序 |
| 没加载上的脚本、技能、斜杠命令模板，和被同名的盖住的 | 软件包、`~/.miyu/root/` 下的 |
| MCP 服务写错的、列出来却没登记上的工具 | 系统配置、软件包清单 |
| 定时任务表里读不懂的行 | 定时任务表 |
| 写坏了的工具说明、提示词的覆盖 | 覆盖文件 |
| 扩展提供的模型驱动：软件包没批、该配 key 的没配 | 系统配置 |
| 装好的软件包文件被改过、少了 | 软件包目录 |

上面的 `~/.miyu` 是数据根（默认 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`），设了环境变量 `MIYU_HOME` 的是它。只查一份的，把它写在后面：

```sh
miyu check ~/.miyu/root/personas/miyu/prompts/examples.md
```

### 结果怎么读

一处问题一行：文件、行、列，「错误」或「警告」，说的是什么。倒数第一段是合计，再往下是语义模型的状态：

```text
~/.miyu/etc/config.toml:3:9 错误：log.level 只能是 error、warn、info、debug、trace 或 off，写的是 verbose
~/.miyu/etc/config.toml:12:1 警告：没有 models.chatt 这一项。是不是想写 models.chat？
~/.miyu/etc/config.toml:20:7 警告：providers.acme.key 引用的密钥 acme 还没设
1 处错误，2 处警告
语义模型：local:granite-embedding-97m-multilingual-r2，正在建立索引
  记忆：engineer 120/120
  知识库：handbook 30/42
```

- 「错误」：写法不对，这一处不算数；TOML 本身读不懂的，整份文件都不算。有错误的，退出码 `1`。
- 「警告」：不认识的键，引用的密钥、环境变量、人格、预设、知识库还没有这类。Miyu 照样跑，这几处没用上。只有警告的，退出码 `0`。
- 什么都没有的印「没有问题」。
- 语义模型那几行不算问题：用的哪个模型，记忆、以前的对话、知识库各有几条建好了索引（建好的/总共）。没装的印「语义模型：未安装，仅按关键词搜索」，提示 `miyu pkg install embed`；关了的印「语义模型：已关闭（models.embedding = "off"），仅按关键词搜索」。换成远程模型的，末尾多一行「不自动联想：记忆、知识库（这个模型没有测过门槛，搜索照常）」：只有内置的模型会自动想起记忆和知识库，别的模型只在搜索时用到。

`--format json` 印一行 `{"problems": […]}`，每一处带 `kind`、`file`、`code`、`level`、`message`，有行列的带 `line`、`column`；查全部时另带 `embedding`。

`miyu check` 要连核心，核心没在跑的先拉起来。核心起不来的，它也跑不了：先看下面「核心起不来、连不上」，还不行用 [应急修复](rescue.md)。

## 日志

| 谁的 | 在哪 |
|---|---|
| 核心 | `~/.miyu/var/log/core.log`，满 10 MB 换一份，留 `core.log.1` 到 `core.log.5` |
| 软件包、扩展 | `~/.miyu/var/log/<编号>/`；扩展程序往标准错误写的，核心记在这里的 `stderr` |
| 网页 | `~/.miyu/var/log/webui/web.log` |
| 托盘 | `~/.miyu/var/log/miyu-applet.log` |
| 终端崩溃 | 缓存目录下的 `tui/crash.log`：Linux `~/.cache/miyu/tui/crash.log`（设了 `XDG_CACHE_HOME` 的在它下面），macOS `~/Library/Caches/Miyu/tui/crash.log`，Windows `%LOCALAPPDATA%\Miyu\cache\tui\crash.log` |

看最近的几十行：

```sh
tail -n 50 ~/.miyu/var/log/core.log
```

Windows PowerShell 里：`Get-Content -Tail 50 $HOME\.miyu\var\log\core.log`。

核心的日志一行一条，英文，不记对话内容和 key。核心每次起来记一行 `starting`，带版本和进程号。要记得更细，调 `log.level`：

```sh
miyu config set log.level debug
```

能写 `error`、`warn`、`info`（出厂）、`debug`、`trace`、`off`，改了当场生效。只想这一次记得细的，先 `miyu core stop`，再这样打开：拉起的核心这一次照 `debug` 记，压过 `log.level`。

```bash
MIYU_LOG=debug miyu
```

```powershell
$env:MIYU_LOG = 'debug'
miyu
```

见 [常驻和服务器](service.md#日志)。

## 核心起不来、连不上

头和命令行打开时先连核心，连不上就拉起一个，最多等 60 秒。一个数据根同时只跑一个核心，它占着 `~/.miyu/run/core.lock`。

| 看到的 | 原因 | 怎么办 |
|---|---|---|
| 「核心起不来：…」 | 核心起来时出了错，冒号后面是它说的原因 | 照原因改。原因看不懂的，看 `core.log` 最后几行 |
| 「核心没起来，也没说为什么：…」 | 核心没说就退了 | 看 `~/.miyu/var/log/core.log` 最后几行 |
| 「等核心起来等了太久」 | 60 秒还没起好，多半是磁盘忙 | 再试一次；还不行看 `core.log` |
| 「别的程序正在拉起核心，等了太久」 | 另一个头在拉起核心，60 秒还没好 | 同上 |
| 「拉不起核心：…」 | 找不到程序、没权限跑 | 重装 Miyu，见 [安装、升级、卸载](install.md) |
| 「… 认不出是 Miyu 的数据根 …」 | 数据根的位置上有别的东西。0.7 旧版的数据目录也报这一句 | 是 0.7 旧版的数据目录的，照这句话后半说的挪开、装迁移软件包、迁，见 [从 0.7 旧版迁过来](migrate.md)；不是的把 `MIYU_HOME` 设成一个空目录 |
| 「… 是旧版 Miyu 的数据根，新版不认 …」 | 发版前的开发版留下的数据根 | 把 `MIYU_HOME` 设成一个空目录 |
| 「… 的布局不是这一版的 …」 | 更新版本的 Miyu 写过这个数据根 | 用写它的那一版，或者换一个数据根 |
| 「… 不是自己的，或者别人也能进：不连」 | Linux、macOS：放套接字的目录不归你、或者别的用户也能进，Miyu 不往里连。Windows：命名管道另一头的程序不是你这个用户的 | Linux、macOS：把那个目录改成只有你能进，例如 `chmod 700 <目录>`。Windows：关掉占着这个管道的程序再打开 |
| 「套接字的路径太长，放不下：…；临时目录下也放不下」（Linux、macOS） | 数据根的路径太长 | 换一个短一点的 `MIYU_HOME`，或者把 `TMPDIR` 设成短的目录 |
| 「… 上有别的东西：不是套接字的文件，或者别的程序正在听。不动它」 | Linux、macOS：套接字的位置被别的文件、程序占了。Windows：命名管道的名字被别的程序占了 | 不是 Miyu 的就挪走、关掉它 |
| 「与核心的连接已断开」 | 核心中途退了或者崩了 | 看 `core.log`，再打开一次 |

套接字的位置：

| 系统 | 在哪 |
|---|---|
| Linux | 设了 `XDG_RUNTIME_DIR` 的是 `$XDG_RUNTIME_DIR/miyu-<指纹>/core.sock`，别的是 `~/.miyu/run/core.sock`；路径太长时退到 `$TMPDIR/miyu-<uid>/` |
| macOS | `~/.miyu/run/core.sock`；路径太长时退到 `$TMPDIR/miyu-<uid>/` |
| Windows | 命名管道 `\\.\pipe\miyu-<指纹>`，只对你这个用户开放 |

### 旧核心还在跑

升级以后，已经在跑的核心还是旧版，新的头连上去可能报「协议版本不兼容，请升级到同一版本。」。停掉它，下次打开拉起的就是新版：

```sh
miyu core stop
```

装了开机启动服务、要服务接着管的，再敲 `miyu service enable`，见 [常驻和服务器](service.md#让服务马上接手)。

`miyu core stop` 说「核心还在跑，但连不上它：数据根 run/ 里的令牌或套接字位置不见了」的，是 `~/.miyu/run/` 被删了，核心还开着。照提示到 `core.log` 里找最后一行 `starting` 的进程号，结束这个进程：

```bash
kill <进程号>
```

```powershell
Stop-Process -Id <进程号>
```

### 坏到头都打不开

核心坏了、头打不开时，用 `miyu rescue` 进 LiteMiyu 的应急模式修，它不经过核心。LiteMiyu 只有 Linux x86_64 的版本，macOS、Windows 上没有。见 [应急修复](rescue.md)。

## 没配模型、key 不对

| 看到的 | 怎么办 |
|---|---|
| 「未配置模型」，提示运行 `miyu setup`；`miyu ask` 退出码 `5` | 跑 `miyu setup`，见 [模型](models.md) |
| `miyu check` 警告「… 引用的密钥 … 还没设」 | `miyu provider login <名字>` 把 key 存进去 |
| `miyu check` 警告「… 引用的环境变量 … 核心起来时没有设」 | 核心是在设这个变量以前起来的。`miyu core stop`，在设了变量的 shell 里再打开。开机启动服务起的核心看不到 shell 里的变量，见 [常驻和服务器](service.md#服务和环境变量) |
| 「模型请求失败。」、认证失败 | key 填错了或者过期了，重新 `miyu provider login <名字>` |

## 模型一直报错

「模型冷却中，请稍后重试。」「没有可用的模型。」是供应商限流、额度用完、连不上，Miyu 把这个模型先放一阵。重试多少次、冷却多久、怎么用池换别的模型，见 [模型](models.md#限流额度用完时)。

## 命令被拒、一直等确认

| 看到的 | 原因 |
|---|---|
| 「只读模式，未执行」 | 会话是只读的，写不了 |
| 「这是 Miyu 自己的数据，谁都不能碰」 | 动的是数据根 |
| 确认框一直在等 | 要你点允许或不允许，到时没人点的自动拒绝：「无人确认，已自动拒绝」 |
| 「需要确认，无人确认」；`miyu ask` 退出码 `4` | `miyu ask`、通讯平台上没人能当场确认，要确认的那几步不做 |

切级别、哪些命令要确认、怎么关，见 [权限和沙盒](permissions.md)。

## 软件包装不上、扩展起不来

| 看到的 | 怎么办 |
|---|---|
| 「清单有误，无法安装。」 | `miyu pkg info <包目录>` 只看不装，说清单哪里不对 |
| 「缺少依赖的软件包。」 | 先装它依赖的，`miyu pkg info <编号>` 看依赖 |
| 「不支持本机的系统。」 | 这个包没有给你这种系统的版本 |
| 「软件源连不上。」「软件包下载失败。」 | 查网络，过一会儿 `miyu pkg update` 再试 |
| 「没找到 <程序>：清单 … 说 miyu <名字> 由它跑。重装这个软件包再试。」 | `miyu pkg install <编号>` 重装 |
| 「界面 'tui' 没有安装」 | `miyu pkg install tui` 装回来 |
| 扩展停了 | 看 `~/.miyu/var/log/<编号>/stderr`，那是扩展自己报的错 |

`miyu pkg check` 查装好的文件有没有被改过。详细见 [软件包](packages.md)。

## 终端显示乱

| 看到的 | 怎么办 |
|---|---|
| 图标是方块、问号 | 字体里没有 Nerd Font 的图标。装一个 Nerd Font，或者 `miyu config set tui.icons plain` 改用普通字符 |
| 颜色不对、太刺眼 | 设置页「外观 → 配色方案」里的「配色」「明暗」「铺底色」，键是 `tui.theme`、`tui.theme_mode`、`tui.theme_background` |
| 命令行的输出里有乱码一样的颜色符号 | 设环境变量 `NO_COLOR=1` |
| 终端崩了 | 崩溃记录在 `tui/crash.log`，位置见上面「日志」 |

装了 shell 集成的，`miyu shellhook doctor` 先列出装进了哪几个 shell，再一条条查核心、终端、剪贴板：

```text
✓ 连得上核心
✗ 终端没说自己是真彩色（COLORTERM 不是 truecolor）：配色最多 256 色
✗ 问不到终端的颜色：配色跟随终端时只用终端的 16 色（tmux 这类要开透传）
✓ Ctrl+V 读剪贴板用 wl-paste
```

这是 Linux 上 Wayland 桌面里的样子。最后一行在别处写的是：

| 系统 | 最后一行 |
|---|---|
| Linux，没有 Wayland | 「Ctrl+V 读剪贴板用 xclip」或「… 用 xsel」 |
| macOS | 「Ctrl+V 读剪贴板用 pbpaste」 |
| Windows | 「Ctrl+V 读剪贴板用 powershell」 |

一个都找不到的写「✗ 找不到读剪贴板的命令（…）：Ctrl+V 只能用终端自己的粘贴」，括号里是这台机器上试的那几个命令。

终端的设置见 [终端](heads/tui.md)。

## 网页打不开

| 看到的 | 怎么办 |
|---|---|
| 「端口 8300 已被占用」，提示用 `miyu web --port <端口>` 换一个 | 换一个端口。没写过端口的，8300 被占时会依次试 8301 到 8309，都占了才报 |
| 「无法打开浏览器，请手动打开上面的地址」 | 照印出来的地址自己打开。没有桌面的机器用 `miyu web --print` |
| 登录页写「访问密码错误」 | 忘了的，在这台电脑上敲 `miyu web --reset`，打开的页面里重设 |
| 整屏写「还没设访问密码」 | 在跑 Miyu 的电脑上 `miyu web --settings access` 设 |
| 「尝试次数过多，N 秒后再试」 | 等它写的秒数过了再试 |

网页自己的日志在 `~/.miyu/var/log/webui/web.log`。从别的设备、反向代理打开的，见 [常驻和服务器](service.md#从别的机器手机打开网页)。

## 配置写错了

- `miyu ask` 开头说「配置中有 N 处错误」，提示运行 `miyu check` 查看。照 `miyu check` 印的行号改。
- 核心起来时整份读不进来的，那一份照空的算，核心照样起。核心开着时你手改坏了的，接着用上一次读好的那一份。
- 文件读不进来时 `miyu config set` 不改，报「配置文件有误，请先修复。」。`miyu config edit` 存之前先检查。
- 某一项不生效的，`miyu config explain <键>` 列出每一层写的什么、哪一层算数、写了不算的为什么。
- 「项目配置 … 尚未信任，本次没有使用」：在那个目录里 `miyu config trust` 看过再决定。

见 [配置](config.md)。

## 只在某个系统上出现的

| 看到的 | 系统 | 原因 | 怎么办 |
|---|---|---|---|
| `miyu ask` 开头「沙盒不可用：内核没有可用的 Landlock」，下一行「提示：需要 Linux 5.13 或更新，并在启动参数 lsm= 里启用 Landlock」 | Linux | 内核旧于 5.13，或者启动参数 `lsm=` 里没有 `landlock` | 照提示换内核、改启动参数。在那之前只读下的命令改成问你 |
| `miyu ask` 开头「沙盒不可用：无法应用 Seatbelt 配置，Miyu 可能运行在另一个沙盒里」 | macOS | Seatbelt 配置装不上 | 不在别的沙盒里跑 Miyu。在那之前只读下的命令改成问你 |
| `miyu ask` 开头「沙盒不可用：此版本在 Windows 上还不能把命令关进沙盒」 | Windows | 这台 Windows 起不了沙盒要的基础容器 | 只读下的命令改成问你 |
| 只读下命令的输出是 `miyu-sandbox: cannot confine: no desktop session, programs in the sandbox cannot start here; use Miyu from the desktop, or turn the sandbox off`，退出码 `125` | Windows | 核心不在桌面会话里，例如经 SSH 连进来起的 | 从登录的桌面用 Miyu，或者切到开放权限，见 [常驻和服务器](service.md#windows-上的只读会话) |
| 只读下命令的输出是 `miyu-sandbox: cannot confine: this Windows has no base container (MXC tier …)` | Windows | 这台 Windows 起不了沙盒要的基础容器 | 切到开放权限再跑 |
| 只读下命令输出后面跟着 `The sandbox blocks WMI. Commands that need it work outside the sandbox.` | Windows | 这条命令要用 WMI，沙盒里连不上 | 切到开放权限再让 AI 跑，见 [权限和沙盒](permissions.md#三个平台) |
| 「无法安装服务：Windows 任务计划无法携带 MIYU_HOME」，下一行「提示：取消设置 MIYU_HOME 后重试」；终端设置页「开机启动核心」那一项写「设了 MIYU_HOME 时 Windows 上装不了」 | Windows | 任务计划带不了环境变量 | 用默认的数据根装服务；要用别的数据根的，用托盘的「开机时启动」，见 [数据和备份](data.md#换一个位置) |
| `core.log` 里有 `catalog cache unavailable`，原因写 `no LOCALAPPDATA` | Windows | 环境变量 `LOCALAPPDATA` 没有，或者不是绝对路径，缓存目录算不出来 | 把 `LOCALAPPDATA` 设成绝对路径，再 `miyu core stop`、重新打开 |
| 「未找到 litemiyu」，下一行「提示：暂只支持 Linux」 | macOS、Windows | LiteMiyu 只有 Linux x86_64 的版本，`miyu rescue` 用不了 | 照这一页上面「核心起不来、连不上」查，见 [应急修复](rescue.md#别的系统) |
| 「无法升级：没有这种系统的发行包」 | Intel 芯片的 Mac、ARM 的 Windows 这类 | Miyu 没给这种系统出发行包 | 没有办法升级，见 [安装、升级、卸载](install.md#哪些系统能装) |
| 「不支持本机的系统。」 | 看包，例如 `im` 没有 Linux aarch64 的版本 | 这个软件包没给你这种系统打包 | 换这个包支持的系统，或者等它出这种系统的版本 |
| `miyu call … --json '…'` 报 `--json takes one JSON object.`，后面一句 `Its double quotes were lost on the way, as Windows PowerShell 5.1 does. Pipe the JSON instead: '<json>' \| miyu call <tool> --json -` | Windows | Windows PowerShell 5.1 交给程序时吃掉了参数里的双引号 | 照它说的从管道传：`'<json>' \| miyu call <工具> --json -`，见 [命令行](cli.md) |
| `miyu shellhook install` 说「PowerShell 的执行策略是 …，不加载启动文件，装了也不生效：在 PowerShell 里执行 Set-ExecutionPolicy -Scope CurrentUser RemoteSigned 再开新窗口」 | Windows | 这个执行策略下 PowerShell 不加载 `$PROFILE`，shell 集成写进去也不生效 | 照它说的执行一次，开新窗口，见 [终端](heads/tui.md#shell-集成) |
| 终端里「Linux 控制台显示不了中文：可在设置里把界面语言换成英文」 | Linux | 在 Linux 的文字控制台（`TERM=linux`）里打开终端 | 设置页里把界面语言换成英文，或者在图形桌面的终端程序里用 |
| 卸载脚本报「删不掉 …（…）：还有 Miyu 的程序开着的，关掉再卸」 | Windows | 还有 Miyu 的程序开着，安装目录删不掉 | 关掉终端、网页这些 Miyu 的程序再卸，见 [安装、升级、卸载](install.md#卸载) |
| `old Miyu is still running; stop it first` | Windows | 迁移时旧版 Miyu 还开着 | 关掉旧版 Miyu 再迁，见 [从 0.7 旧版迁过来](migrate.md) |

## 原因码

核心拒绝一件事时给一个原因码和一句话。界面、命令行显示那句话；直接连协议的程序看 `data.reason`。常碰到的：

| 原因码 | 显示的话 | 怎么办 |
|---|---|---|
| `no_model` | 没有可用的模型。 | `miyu setup`，或者 `models.chat` 写一个能用的 |
| `cooling` | 模型冷却中，请稍后重试。 | 等，或者配一个池，见 [模型](models.md) |
| `model_failed` | 模型请求失败。 | 看界面上的详情；认证失败的重存 key |
| `unknown_model` | 模型或模型池不存在。 | 写成 `供应商/模型` 或 `@池`，供应商要配好 |
| `unknown_provider` | 供应商不存在。 | 先配这一家，见 [模型](models.md) |
| `unknown_secret` | 密钥不存在。 | `miyu provider login <名字>` |
| `session_not_found` | 会话不存在。 | 编号写全；删了的会话找不回来 |
| `turn_running` | 回合进行中。 | 等这一轮结束或先打断，再撤销、压缩、删除 |
| `archived` | 这是存档，不能继续对话。 | 不带 `--archive` 再导一次 |
| `session_read_only` | 会话日志坏了，只能看到坏的地方以前的内容，不能接着聊。要继续请新开一个会话。 | 新开一个会话 |
| `session_unavailable` | 会话暂时无法读取，请稍后重试。 | 过一会儿再试；常见于磁盘出错、打开的文件太多 |
| `dir_too_wide` | 目录范围过大。 | 换一个小一点的目录 |
| `attachment_too_big` | 附件过大（最大 20 MiB，图片 5 MiB、8000 像素）。 | 缩小，或者只给路径 |
| `attachment_in_data_root` | 不能附加数据目录里的文件。 | 复制到数据根外面再附 |
| `export_exists` | 文件已存在。 | 换个文件名，导出不覆盖 |
| `unknown_config_key` | 配置项不存在。 | `miyu config get` 看有哪些键 |
| `config_invalid` | 配置有误，未保存。 | 照报错改值，`miyu config explain <键>` 看能写什么 |
| `config_file_broken` | 配置文件有误，请先修复。 | `miyu check` |
| `unknown_package` | 软件包不存在。 | `miyu pkg search` 找编号 |
| `package_invalid` | 清单有误，无法安装。 | `miyu pkg info <包目录>` 看哪里不对 |
| `depends_missing` | 缺少依赖的软件包。 | 先装依赖 |
| `extension_off` | 扩展已停用。 | 在软件包页打开它 |
| `bad_password` | 访问密码错误。 | `miyu web --reset` |
| `protocol_mismatch` | 协议版本不兼容，请升级到同一版本。 | 头和核心升到同一版，`miyu core stop` |
| `internal_error` | 内部错误，详见运行日志。 | 看 `core.log`，照下面报问题 |

全部原因码见 [协议参考：出错](../reference/errors.md)。

## 报问题

到核心仓库提：<https://github.com/miyu-agent/miyu/issues>。附上：

1. `miyu --version` 印的版本。
2. 系统和版本，例如 Arch Linux、macOS 15、Windows 11；终端里的问题加上终端程序的名字。
3. `miyu check` 的输出。
4. 出问题那一刻前后 `core.log` 里的几行；软件包的问题附 `~/.miyu/var/log/<编号>/` 里的；终端崩了附 `crash.log` 里的那一段。
5. 怎么做能再出一次。

贴之前自己看一遍。日志、`miyu check` 的输出里有你的路径、用户名、会话编号、软件包名；导出的会话原样带着对话、命令输出和文件内容，不打码。不要贴 `~/.miyu/etc/private/secrets.toml`。
