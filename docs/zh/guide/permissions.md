# 权限和沙盒

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

这一页讲 AI 在你的电脑上能做什么、什么时候会先问你、哪些目录碰不到。

## 两种权限级别

每个会话可以在 `开放权限` 和 `只读` 两种权限级别里选择一种，每个会话单独记录自己的选择。新建的会话默认是开放权限。

| | 开放权限 | 只读 |
|---|---|---|
| 文件工具读取 | 可以读取任何位置，包括 Miyu 自己的数据根。 | 和开放权限相同。 |
| 文件工具写入 | 可以写入任何位置，包括 Miyu 自己的数据根。 | 不写入，时间线上会记录 `只读模式，未执行`。 |
| 命令 | 不在沙盒里运行；受保护的命令会先问你，几类危险命令会被直接拒绝。 | 在沙盒里运行，任何位置都写不进（见下文）。 |
| 扩展里联网的工具 | 直接使用。 | 直接使用。 |
| MCP 服务里没有标成只读的工具，以及扩展里对外发消息的工具 | 直接使用。 | 每次使用前都会问你。 |
| 安装、卸载软件包，打开扩展 | 每次都会先问你。 | 不执行。 |

数据根是 Miyu 存放自己数据的目录，默认是 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`；如果设置了 `MIYU_HOME`，数据根就是它指向的目录。Miyu 不会对数据根做额外的保护：开放权限下，文件工具和命令读写数据根和读写其他目录一样，存放密钥的 `~/.miyu/etc/private/secrets.toml` 也读得到；只读下，数据根和其他位置一样读得到、写不进。只有删除整个数据根、把它移走或者扔进回收站时才会先问你，见下面的「要确认的命令」；数据根里面的文件和目录和其他位置一样，不会问你。`glob`、`grep` 从上级目录往下搜索时会跳过数据根，道理和跳过 `.git` 一样；如果搜索的起点就在数据根里，会照常搜索。

开放权限下，命令能做你这个系统用户能做的一切事情。Miyu 只根据命令的字面写法识别几类危险的写法（见下面的「要确认的命令」），目的是防止 AI 一时糊涂，挡不住故意绕开的写法。如果要让 AI 什么都改不了，就使用只读。

只读只是不允许写入，不会替你隐藏任何东西：`~/.ssh`、浏览器的数据、其他程序的登录信息这些，在只读下 AI 照样读得到，读到的内容会进入上下文，并且发送给模型供应商。如果不想让 AI 读到这些内容，需要你自己把 Miyu 关进另一层沙盒，或者换一个系统用户运行 Miyu。Miyu 自己的数据根也是这样：两种权限级别下都读得到，包括存放密钥的密钥文件；开放权限下还写得进。

只读下的命令有以下特点：

- 可以读取整个磁盘（Windows 上是本机的固定磁盘），包括数据根。只读的沙盒不隐藏任何目录，只阻止写入。
- 任何位置都写不进，临时目录也不行，写入的命令会报没有权限。
- 可以联网。在 macOS 和内核足够新的 Linux 上，命令连接不上 Docker、ssh-agent 这类系统服务。
- 如果这台电脑上的沙盒用不了，每条命令都会先问你，你允许之后，命令会在沙盒外运行。

## 切换权限级别

可以用下面几种方式切换权限级别：

- 终端界面、网页界面：在输入框里按 `Tab` 或 `Shift+Tab`。在终端界面里，命令列表或 `@` 文件列表打开时，`Tab` 由列表使用，`Shift+Tab` 仍然可以切换。在网页界面里，也可以点击输入框下方那一行左边的 `▣ 开放权限` 或 `⏸ 只读`。
- 命令行：新建会话时加上 `--permission`，值是 `read-only` 或 `full`。这个参数只对新建的会话有效，不能和 `-c`、`-s` 一起使用。例如：

  ```bash
  miyu ask -C ~/src/app --permission read-only "检查这里的代码"
  ```

  ```powershell
  miyu ask -C $HOME\src\app --permission read-only "检查这里的代码"
  ```

- 让新会话一开始就是只读：在设置页打开 `通用 → 新会话 → 新会话默认只读`，或者运行：

  ```sh
  miyu config set permission.start_read_only true
  ```

AI 正在工作时也可以切换。切换到只读会立即生效，这一步里还没有执行的写操作会被直接拒绝；切换回开放权限，要从 AI 下一次请求模型时开始生效。

## 要确认的命令

开放权限下，下面五类命令会先问你。设置页（在终端界面或网页界面里输入 `/settings` 打开）的 `隐私与安全 → 受保护的命令` 里每一类有一个开关，默认全部打开。

| 设置页上的名字 | 配置键 | 拦截哪些命令 |
|---|---|---|
| `破坏性 git 操作` | `permission.confirm.git` | 包括 `git push --force`（`-f`、`+分支` 也算）、`git reset --hard`、`git clean -f`、`git checkout .`、`git restore .`、`git stash drop`、`git stash clear`。 |
| `递归删除` | `permission.confirm.delete` | 删除工作目录或它的上级目录，例如在 `~/src/app` 里运行 `rm -rf ..`；删除整个 Miyu 数据根或者包含它的目录、把它们移走或扔进回收站，无论用的是哪个命令，例如 `rm -rf ~/.miyu`、`mv ~/.miyu ~/x`、`gio trash ~/.miyu`、`trash-put ~/.miyu`、带 `--remove-source-files` 的 `rsync`，AI 用 `trash` 工具删除也算在内，但数据根里面的文件、目录不会问你；递归删除整个目录、并且目标要到运行时才能确定的命令，例如 `rm -rf "$OUT"`、`… \| xargs rm -r`。带 `-delete` 的 `find` 同样算在内。 |
| `关机和重启` | `permission.confirm.power` | 包括 `shutdown`、`reboot`、`halt`、`poweroff`、`init 0`、`init 6`，以及 `systemctl` 的 `poweroff`、`reboot`、`halt`、`suspend`、`hibernate`、`kexec`。 |
| `下载后直接执行` | `permission.confirm.pipe_to_shell` | 用 `curl` 或 `wget` 下载内容，再交给 shell、`python`、`python3`、`perl`、`ruby`、`node` 运行的命令，例如 `curl … \| sh`、`bash <(curl …)`、`sh -c "$(curl …)"`。 |
| `容器使用宿主设备` | `permission.confirm.container` | `docker`、`podman`、`nerdctl` 的 `run`、`create` 命令带了 `--device`、`--device-cgroup-rule`，或者 `--pid`、`--net`、`--network`、`--ipc` 的值写成 `host`，或者挂载了 `/dev`、`/sys` 下面的某一个路径，例如 `-v /dev/snd:/dev/snd`。 |

命令前面套了 `sudo`、`doas`、`env`、`nice`、`nohup`、`timeout`、`time`、`stdbuf`、`command`、`exec`、`xargs`、`setsid`、`pkexec`、`run0`、`busybox`、`ionice`，或者写在 `sh -c '…'`、`bash -lc '…'`、`eval`、`env -S '…'`、`su -c '…'` 里面，Miyu 照样能认出来。交给 shell 从标准输入读的脚本也一样，例如 `bash <<'EOF'` 的正文、`sh <<< '…'`、`echo '…' | sh`。命令替换嵌套得太深，或者变量展开以后长得离谱（比如一串 `x=$x$x` 每一句都把它翻倍），Miyu 看不透这样的命令，也会先问你，这一条没有开关。

如果要关掉其中一类，在设置页把对应的开关关掉，或者运行：

```sh
miyu config set permission.confirm.git false
```

修改从下一轮开始生效。

下面几类命令会被直接拒绝，即使你同意了也不会执行，也没有开关：

- 删除根目录 `/`、家目录或它的上级目录、根目录下一级的目录（例如 `/usr`、`/etc`）。
- 格式化磁盘：以 `mkfs` 开头的命令，以及 `wipefs`、`mke2fs`、`blkdiscard`、`mkswap`、`sgdisk -Z`（`--zap-all`）。
- 直接写入磁盘设备：`dd of=/dev/…`、`> /dev/…`，以及用 `tee`、`shred`、`cp` 写入设备。这里的设备指 `/dev/` 下以 `sd`、`nvme`、`mmcblk`、`disk`、`rdisk`、`vd`、`hd`、`xvd`、`mapper/`、`dm-`、`md`、`loop`、`nbd` 开头的设备。
- 让容器接管整台电脑：`--privileged`，`--cap-add` 加上 `ALL` 或 `SYS_ADMIN`，挂载宿主的 `/`、`/dev`、`/sys`。

### Windows 上的写法

Windows 上 AI 用 PowerShell 运行命令（见 [工具](tools.md#shell)）。上面这些规则在 Windows 上按 PowerShell 和 cmd 的写法识别，哪些先问、哪些直接拒绝和 Linux、macOS 相同，五个开关同样有效：

| 类别 | Windows 上识别的写法 |
|---|---|
| 破坏性 git 操作、容器使用宿主设备（先问） | 和上面的表相同；`git.exe` 和带路径的 `git.exe` 也能识别。 |
| 递归删除（先问） | 用 `Remove-Item -Recurse`（别名 `rm`、`del`、`erase`、`rd`、`rmdir`、`ri`，以及 cmd 的 `rd /s`、`del /s`）删除工作目录，例如 `Remove-Item -Recurse -Force .`；删除或移走整个数据根或者包含它的目录，移走的命令识别 `Move-Item`（别名 `mi`、`mv`、`move`，以及 cmd 的 `move`）和 `Rename-Item`（`rni`、`ren`）；目标要到运行时才能确定的删除，例如 `Remove-Item -Recurse $target`、`… \| Remove-Item -Recurse`。 |
| 关机和重启（先问） | 包括 `Stop-Computer`、`Restart-Computer`；`shutdown` 带任何开关都算（`/s`、`/r`、`/h`，注销用的 `/l` 也算），只有 `/a`、`/?` 不算。 |
| 下载后直接执行（先问） | 用 `irm`、`iwr`、`curl`、`wget` 下载内容，再交给 `iex`（`Invoke-Expression`）、`powershell`、`pwsh`、`cmd` 运行，例如 `irm … \| iex`、`iex (irm …)`。 |
| 删除根目录、家目录、系统目录（直接拒绝） | 盘符本身（`C:\`）算作根目录；盘符下一级的目录（`C:\Windows`、`C:\Users`、`C:\Program Files`，你自己建的 `C:\dev` 也算）和根目录下一级的目录同样处理；`~`、`$HOME`、`$env:USERPROFILE`、`%USERPROFILE%` 都算作家目录。 |
| 格式化磁盘（直接拒绝） | 包括 `Format-Volume`、`Clear-Disk`、`Remove-Partition`、`format D:`。 |
| 容器接管整台电脑（直接拒绝） | 和上面的列表相同。 |

- 写在 `cmd /c "…"`、`powershell -Command "…"`、`iex '…'` 里面的命令同样能识别出来。程序名、参数名、路径不区分大小写，参数名只写了开头几个字母的（`-r`、`-rec`）也能识别。带 `-WhatIf` 的删除不算。
- Windows 上不识别的写法有：`-EncodedCommand` 里经过编码的命令，直接写入磁盘设备（`\\.\PhysicalDrive0`），`Set-ExecutionPolicy`，删除注册表（`reg delete`、`Remove-Item HKLM:\…`），以及网络路径（`\\server\share`）上的删除。开放权限下，这些命令既不会先问你，也不会被拒绝。

### 确认框

确认框上有两个选项：

1. `允许本次`。
2. `不允许`：可以写一句理由，AI 能看到这句理由。

受保护的命令弹出的确认框会标上 `高危命令`。

确认框最多等待 `permission.confirm.timeout` 秒，默认是 `300`，可以设置为 `0` 到 `86400`，设置为 `0` 时会一直等待。确认框上会显示 `… 后自动拒绝`；如果到时间没有人回答，时间线上会记录 `无人确认，已自动拒绝`，AI 会换其他办法继续工作。AI 向你提出的问题不受这个时限限制。

在没有确认框的地方，需要确认的事情一律不执行：`miyu ask` 在这一轮结束后的退出码是 `4`；通讯平台上的会话也是这样。

## 项目配置

仓库里的 `.miyu/config.toml`（项目配置的用法见 [配置](config.md)）对这一页的设置只能收紧：项目配置里只认 `permission.start_read_only = true`，写 `false` 不生效。`permission.confirm.*` 只认你自己的配置，写进项目配置不生效，用 `miyu config explain <键>` 可以看到它没有生效。

## 三个平台

只读下，三个平台各自用系统自带的机制限制命令：Linux 用 Landlock，macOS 用 Seatbelt，Windows 用微软的 MXC 基础容器。你能感觉到的差别如下：

| | 你会遇到的情况 |
|---|---|
| Linux | 内核没有可用的 Landlock 时，沙盒用不了。内核足够新时，命令连接不上 Docker、ssh-agent 这类系统服务的套接字。 |
| macOS | Miyu 自己运行在其他沙盒里时，沙盒用不了。命令连接不上 Docker、ssh-agent 这类系统服务的套接字。 |
| Windows | 沙盒只在已登录的桌面会话里能用：如果核心不在桌面会话里运行（例如通过 SSH 启动的核心），只读下每条命令都会被拒绝，退出码是 `125`。只读下只能读取本机的固定磁盘，读不到 U 盘这类可移动磁盘、网络磁盘和光驱。剪贴板、模拟键盘鼠标都用不了。使用 WMI 的命令（`Get-CimInstance`、`Get-WmiObject`、`wmic`、`systeminfo`、`Get-ComputerInfo` 这类）无法运行，需要使用时切换到开放权限。 |

开放权限下，三个平台的命令都不在沙盒里运行，上面的表只适用于只读。

沙盒用不了时，`miyu ask` 在开头会输出一行 `沙盒不可用：<原因>`，如果能修复，会另起一行说明修复的方法。三个平台的原因分别是：

| | 输出的原因 | 出现的情况 |
|---|---|---|
| Linux | `内核没有可用的 Landlock` | 内核版本低于 5.13，或者启动参数 `lsm=` 里没有 `landlock`。 |
| macOS | `无法应用 Seatbelt 配置，Miyu 可能运行在另一个沙盒里` | Seatbelt 配置装不上。 |
| Windows | `这台 Windows 无法启动 MXC 的基础容器` | 沙盒需要的基础容器在这台 Windows 上无法启动。 |

工作目录的范围太大时：根目录在三个平台上都算范围太大；在 Windows 上，家目录本身和包含数据根的目录也算。范围太大的目录照样可以作为工作目录，命令从那里开始运行；用 `/workspace` 切换到这样的目录时，回执会显示 `工作区已切换到 …（范围较大）。`。

## 通讯平台上的会话

如果接入了通讯平台，群会话和不是你本人的私聊会被单独限制起来，和权限级别无关（只有写进设置页 `高级 → 平台账号` 的账号才算你本人）：

- 文件工具只能访问这个会话的工作区，开启的技能文件夹只能读取。
- 命令在一个连读取也限制在工作区里的沙盒中运行。Windows 上不能运行命令；在沙盒管不住 Unix 套接字的 Linux 上也不能。
- 不能安装、卸载软件包。
- 受保护的命令的开关对这些会话不起作用。

## 常见问题

| 看到的提示 | 解决方法 |
|---|---|
| `只读模式，未执行` | 切换回开放权限即可。 |
| `需要确认，无人确认`，`miyu ask` 的退出码是 `4` | 到终端界面或网页界面里做这件事，或者关掉对应的那一类受保护的命令。 |
| `无人确认，已自动拒绝` | 调大 `permission.confirm.timeout`，设置为 `0` 时会一直等待。 |
| `未执行：会删除家目录` 这类提示 | 这几类命令无法放行，如果确实要做，需要你自己在终端里操作。 |
| `miyu-sandbox: cannot confine: no desktop session, programs in the sandbox cannot start here; use Miyu from the desktop, or turn the sandbox off`（退出码 `125`） | 在 Windows 上从已登录的桌面启动 Miyu，或者切换到开放权限。 |
| Windows 只读下，命令输出后面跟着 `The sandbox blocks WMI. Commands that need it work outside the sandbox.` | 这条命令要用 WMI，在沙盒里无法运行。切换到开放权限，再让 AI 运行这条命令。 |
| `沙盒不可用：内核没有可用的 Landlock` | 换用 Linux 5.13 或更新的内核，并在启动参数 `lsm=` 里加上 `landlock`。 |
