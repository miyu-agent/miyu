# 权限和沙盒

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

AI 在你的机器上能动什么、什么时候先问你、哪些目录碰不到，都在这一页。英文版：[permissions.en.md](permissions.en.md)。

## 两种权限级别

每个会话在「开放权限」和「只读」里选一种，各记各的。新会话默认是开放权限。

| | 开放权限 | 只读 |
|---|---|---|
| 文件工具读 | 哪里都能读，数据根除外 | 同左 |
| 文件工具写 | 哪里都能写，数据根除外 | 不写，时间线上记「只读模式，未执行」 |
| 命令 | 不进沙盒；受保护的命令先问你，几类危险命令直接拒 | 进沙盒，哪里都写不了（见下） |
| 扩展里联网的工具 | 直接用 | 直接用 |
| MCP 服务里没标只读的工具，扩展里对外发消息的工具 | 直接用 | 每次问你 |
| 装、卸软件包，打开扩展 | 每次问你 | 不做 |

数据根是 Miyu 自己放数据的目录，默认 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`；设了 `MIYU_HOME` 的是它。文件工具碰到数据根的，不执行，时间线上记「这是 Miyu 自己的数据，谁都不能碰」。几处例外：这个会话的默认工作区 `~/.miyu/root/workspace/<会话编号>/` 和 `~/.miyu/root/` 下的 `pictures`、`videos`、`documents`、`downloads` 照常读写；开着的技能文件夹、这个会话的附件只能读。

开放权限下，命令能动你这个系统用户能动的一切。Miyu 只照命令的字面认出几类危险写法（下面「要确认的命令」），防的是 AI 一时糊涂，挡不住刻意绕开的写法。要 AI 什么都改不了，用只读。

只读就是不让写，不替你藏东西：`~/.ssh`、浏览器的数据、别的程序的登录这些，只读下 AI 照样读得到，读了的进上下文、发给模型供应商。不想让它读到的，自己把 Miyu 关进另一层沙盒，或者换一个系统用户跑。Miyu 自己的数据根哪一种权限下都碰不到。

只读下的命令：

- 整盘能读（Windows 上是本机的固定盘），数据根读不到。
- 哪里都写不了，临时目录也不行，写的命令报没有权限。
- 能联网。macOS 和内核够新的 Linux 上，连不上 Docker、ssh-agent 这类系统服务。
- 这台机器的沙盒用不了时，每条命令先问你，你允许了就不进沙盒跑。

## 怎么切

- 终端、网页：在输入框里按 `Tab` 或 `Shift+Tab`。终端里命令列表、`@` 文件列表开着时 `Tab` 归列表，`Shift+Tab` 照样切。网页也可以点输入框下面那一行左边的「⏵⏵ 开放权限」「⏸ 只读」。
- 命令行：新开会话时写 `--permission`，值是 `read-only` 或 `full`。只管新开的会话，不能和 `-c`、`-s` 一起写：

  ```bash
  miyu ask -C ~/src/app --permission read-only "检查这里的代码"
  ```

  ```powershell
  miyu ask -C $HOME\src\app --permission read-only "检查这里的代码"
  ```

- 新会话一开始就只读：设置页「通用 → 新会话 → 新会话开局只读」，或者：

  ```sh
  miyu config set permission.start_read_only true
  ```

AI 正在干活时也能切。切到只读当场生效，这一步里还没跑的写操作直接拒；切回开放权限从 AI 下一次请求模型起生效。

## 要确认的命令

开放权限下，下面五类命令先问你。设置页（终端、网页里敲 `/settings`）「隐私与安全 → 受保护的命令」一类一个开关，出厂都开着。

| 设置页上 | 配置键 | 拦哪些命令 |
|---|---|---|
| 破坏性 git 操作 | `permission.confirm.git` | `git push --force`（`-f`、`+分支` 也算）、`git reset --hard`、`git clean -f`、`git checkout .`、`git restore .`、`git stash drop`、`git stash clear` |
| 递归删除 | `permission.confirm.delete` | 删工作目录或它的上级，例如在 `~/src/app` 里跑 `rm -rf ..`；整个目录往下删、目标要到运行时才知道的，例如 `rm -rf "$OUT"`、`… \| xargs rm -r`。`find` 带 `-delete` 的同样算 |
| 关机和重启 | `permission.confirm.power` | `shutdown`、`reboot`、`halt`、`poweroff`、`init 0`、`init 6`，`systemctl` 的 `poweroff`、`reboot`、`halt`、`suspend`、`hibernate`、`kexec` |
| 下载后直接执行 | `permission.confirm.pipe_to_shell` | `curl` 或 `wget` 下载下来交给 shell、`python`、`python3`、`perl`、`ruby`、`node` 跑：`curl … \| sh`、`bash <(curl …)`、`sh -c "$(curl …)"` |
| 容器使用宿主设备 | `permission.confirm.container` | `docker`、`podman`、`nerdctl` 的 `run`、`create` 带 `--device`、`--device-cgroup-rule`，`--pid`、`--net`、`--network`、`--ipc` 写 `host`，或者挂 `/dev`、`/sys` 下面的某一个，例如 `-v /dev/snd:/dev/snd` |

命令前面套了 `sudo`、`doas`、`env`、`nice`、`nohup`、`timeout`、`time`、`stdbuf`、`command`、`exec`、`xargs`，或者写在 `sh -c '…'`、`bash -lc '…'`、`eval` 里的，照样认得出。命令替换套得太深、看不透的也先问，这一条没有开关。

关掉一类：在设置页把那一项关掉，或者：

```sh
miyu config set permission.confirm.git false
```

从下一轮起生效。

下面几类直接拒，你点了也不跑，也没有开关：

- 删根目录 `/`、家目录或它的上级、根下一级的目录（`/usr`、`/etc` 这样的）、数据根或装着它的目录。
- 格式化磁盘：`mkfs` 开头的、`wipefs`、`mke2fs`、`blkdiscard`、`mkswap`、`sgdisk -Z`（`--zap-all`）。
- 往磁盘设备直接写：`dd of=/dev/…`、`> /dev/…`，`tee`、`shred`、`cp` 写到设备上。设备指 `/dev/` 下 `sd`、`nvme`、`mmcblk`、`disk`、`rdisk`、`vd`、`hd`、`xvd`、`mapper/`、`dm-`、`md`、`loop`、`nbd` 开头的。
- 容器拿走整台机器：`--privileged`，`--cap-add` 加 `ALL` 或 `SYS_ADMIN`，挂宿主的 `/`、`/dev`、`/sys`。

### Windows 上的写法

Windows 上 AI 用 PowerShell 跑命令（见 [工具](tools.md#shell)）。上面这些规则照 PowerShell 和 cmd 的写法认，哪些先问、哪些直接拒同 Linux、macOS，五个开关照样管用：

| 哪一类 | Windows 上认的写法 |
|---|---|
| 破坏性 git 操作、容器使用宿主设备（先问） | 同上面的表；`git.exe`、带路径的 `git.exe` 也认 |
| 递归删除（先问） | `Remove-Item -Recurse`（别名 `rm`、`del`、`erase`、`rd`、`rmdir`、`ri`，cmd 的 `rd /s`、`del /s`）删工作目录，例如 `Remove-Item -Recurse -Force .`；目标要到运行时才知道的，例如 `Remove-Item -Recurse $target`、`… \| Remove-Item -Recurse` |
| 关机和重启（先问） | `Stop-Computer`、`Restart-Computer`；`shutdown` 带开关的都算（`/s`、`/r`、`/h`，注销的 `/l` 也算），只有 `/a`、`/?` 不算 |
| 下载后直接执行（先问） | `irm`、`iwr`、`curl`、`wget` 下载下来交给 `iex`（`Invoke-Expression`）、`powershell`、`pwsh`、`cmd` 跑：`irm … \| iex`、`iex (irm …)` |
| 删根目录、家目录、系统目录、数据根（直接拒） | 盘符本身（`C:\`）算根目录；盘符下一级的目录（`C:\Windows`、`C:\Users`、`C:\Program Files`，自己建的 `C:\dev` 也算）和根下一级的目录一样；`~`、`$HOME`、`$env:USERPROFILE`、`%USERPROFILE%` 是家目录 |
| 格式化磁盘（直接拒） | `Format-Volume`、`Clear-Disk`、`Remove-Partition`、`format D:` |
| 容器拿走整台机器（直接拒） | 同上面的列表 |

- 写在 `cmd /c "…"`、`powershell -Command "…"`、`iex '…'` 里的照样认得出。程序名、参数名、路径不分大小写，参数名只写开头几个字的（`-r`、`-rec`）也认。带 `-WhatIf` 的删除不算。
- Windows 上不认的：`-EncodedCommand` 里编过码的命令，往磁盘设备直接写（`\\.\PhysicalDrive0`），`Set-ExecutionPolicy`，删注册表（`reg delete`、`Remove-Item HKLM:\…`），网络路径（`\\server\share`）上的删除。这些开放权限下不问、不拒。

### 确认框

确认框上有两个选择：

1. 「允许本次」。
2. 「不允许」：可以写一句理由，AI 看得到。

受保护的命令的确认框标着「高危命令」。

等你确认最多等 `permission.confirm.timeout` 秒，默认 `300`，能写 `0` 到 `86400`，`0` 是一直等。确认框上写着「… 后自动拒绝」；到时没人答，时间线上记「无人确认，已自动拒绝」，AI 换别的办法接着干。AI 向你提的问题不受它管。

没有确认框的地方，要确认的事一律不做：`miyu ask` 这一轮结束后退出码是 `4`；通讯平台上的会话也一样。

## 项目配置

仓库里的 `.miyu/config.toml`（项目配置怎么用见 [配置](config.md)）对这一页只能收紧：只认 `permission.start_read_only = true`，写 `false` 不算。`permission.confirm.*` 只认你自己的配置，写进项目配置也不算，`miyu config explain <键>` 看得到它没生效。

## 三个平台

只读时三个平台各用系统自带的办法关住命令：Linux 用 Landlock，macOS 用 Seatbelt，Windows 用微软的 MXC 基础容器。你感觉得到的差别：

| | 你会碰到的 |
|---|---|
| Linux | 内核没有可用的 Landlock 时沙盒用不了。数据根看得到名字，读不到里面。内核够新时，连不上 Docker、ssh-agent 这类系统服务的套接字 |
| macOS | Miyu 自己跑在别的沙盒里时用不了沙盒。连不上 Docker、ssh-agent 这类系统服务的套接字 |
| Windows | 只在登录的桌面会话里能用：核心不在桌面会话里的（例如经 SSH 起的），只读下每条命令都被拒，退出码 `125`。只读得到本机的固定盘，U 盘这类可移动盘、网络盘、光驱读不到。剪贴板、模拟键盘鼠标用不了。用 WMI 的命令（`Get-CimInstance`、`Get-WmiObject`、`wmic`、`systeminfo`、`Get-ComputerInfo` 这类）跑不成，要用就切到开放权限 |

开放权限下的命令三个平台都不进沙盒，上表只管只读。

沙盒用不了时，`miyu ask` 开头印一行「沙盒不可用：<原因>」，能修的另起一行说怎么修。三个平台的原因各是：

| | 原因那一句 | 什么时候 |
|---|---|---|
| Linux | 内核没有可用的 Landlock | 内核旧于 5.13，或者启动参数 `lsm=` 里没有 `landlock` |
| macOS | 无法应用 Seatbelt 配置，Miyu 可能运行在另一个沙盒里 | Seatbelt 配置装不上 |
| Windows | 这台 Windows 起不了 MXC 的基础容器 | 沙盒要的基础容器在这台 Windows 上起不来 |

工作目录太宽时：根目录在三个平台上都算太宽；Windows 上家目录本身、包住数据根的目录也算。太宽的照样当工作目录，命令从那里起，用 `/workspace` 换过去时回执写「工作区已切换到 …（范围较大）。」。

## 通讯平台上的会话

接了通讯平台的，群会话和不是你本人的私聊（你的号写进设置页「高级 → 平台账号」的才算你本人）是关起来的，不看权限级别：

- 文件工具只碰得到这个会话的工作区，开着的技能文件夹只能读。
- 命令在把读也关进工作区的沙盒里跑。Windows 上不能跑命令；Linux 上沙盒管不住 Unix 套接字的也不能。
- 不能装、卸软件包。
- 受保护的命令的开关对它们不起作用。

## 出了问题

| 看到的 | 怎么办 |
|---|---|
| 「这是 Miyu 自己的数据，谁都不能碰」 | 要 AI 处理的文件放到数据根外面，或者 `~/.miyu/root/documents/` 这几处 |
| 「只读模式，未执行」 | 切回开放权限 |
| 「需要确认，无人确认」，`miyu ask` 退出码 `4` | 到终端、网页里做这件事，或者关掉对应的那一类受保护的命令 |
| 「无人确认，已自动拒绝」 | 调大 `permission.confirm.timeout`，`0` 是一直等 |
| 「没有执行：会删掉家目录」这类 | 这几类不能放行，要做的话自己在终端里做 |
| `miyu-sandbox: cannot confine: no desktop session, programs in the sandbox cannot start here; use Miyu from the desktop, or turn the sandbox off`（退出码 `125`） | Windows 上从登录的桌面启动 Miyu，或者切到开放权限 |
| Windows 只读下命令输出后面跟着 `The sandbox blocks WMI. Commands that need it work outside the sandbox.` | 这条命令要用 WMI，沙盒里跑不成。切到开放权限再让 AI 跑 |
| 「沙盒不可用：内核没有可用的 Landlock」 | 换 Linux 5.13 或更新，启动参数 `lsm=` 里加上 `landlock` |
