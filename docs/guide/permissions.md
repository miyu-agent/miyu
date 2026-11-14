# 权限和沙盒

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

AI 在你的机器上能动什么、什么时候先问你、哪些目录碰不到，都在这一页。

## 两种权限级别

每个会话在「开放权限」和「只读」里选一种，各记各的。新会话默认是开放权限。

| | 开放权限 | 只读 |
|---|---|---|
| 文件工具读 | 哪里都能读，数据根除外 | 同左；读私密目录先问你 |
| 文件工具写 | 哪里都能写，数据根除外 | 不写，时间线上记「只读模式，未执行」 |
| 命令 | 不进沙盒；受保护的命令先问你，几类危险命令直接拒（Windows 上这两样都没有，见 [Windows 上不认](#windows-上不认)） | 进沙盒，哪里都写不了（见下） |
| MCP 服务里没标只读的工具，扩展里联网、对外发消息的工具 | 直接用 | 每次问你 |
| 装、卸软件包，打开扩展 | 每次问你 | 不做 |

数据根是 Miyu 自己放数据的目录，默认 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`；设了 `MIYU_HOME` 的是它。文件工具碰到数据根的，不执行，时间线上记「这是 Miyu 自己的数据，谁都不能碰」。几处例外：这个会话的默认工作区 `~/.miyu/root/workspace/<会话编号>/` 和 `~/.miyu/root/` 下的 `pictures`、`videos`、`documents`、`downloads` 照常读写；开着的技能文件夹、这个会话的附件只能读。

开放权限下，命令能动你这个系统用户能动的一切。Miyu 只照命令的字面认出几类危险写法（下面「要确认的命令」），防的是 AI 一时糊涂，挡不住刻意绕开的写法；Windows 上连这一层也没有。要 AI 什么都改不了，用只读。

只读下的命令：

- 整盘能读（Windows 上是本机的固定盘），数据根和私密目录读不到。
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

### Windows 上不认

要确认的、直接拒的这些规则只认 bash、zsh 的写法。Windows 上 AI 用 PowerShell 跑命令（见 [工具](tools.md#shell)），这些规则整套不查：

- 开放权限下，五类受保护的命令不问你，直接拒的那几类也照跑。`git push --force`、`git reset --hard` 这类在 PowerShell 里写法一样的命令同样不问。
- 设置页「受保护的命令」的几个开关在 Windows 上不起作用。
- 只读不受影响：三个平台的只读本来就不看这些规则，命令在沙盒里跑，哪里都写不了。

在 Windows 上让 AI 碰不熟的仓库、做你没把握的事，用只读。

### 确认框

确认框上有三个选择：

1. 「允许本次」。
2. 「此目录不再询问」：只在读写某个目录要确认时有（只读下读私密目录就是这种）。这个会话里，这个目录往下以后不再问。
3. 「不允许」：可以写一句理由，AI 看得到。

受保护的命令、装包这类确认只有「允许本次」「不允许」。受保护的命令的确认框标着「高危命令」。

等你确认最多等 `permission.confirm.timeout` 秒，默认 `300`，能写 `0` 到 `86400`，`0` 是一直等。确认框上写着「… 后自动拒绝」；到时没人答，时间线上记「无人确认，已自动拒绝」，AI 换别的办法接着干。AI 向你提的问题不受它管。

没有确认框的地方，要确认的事一律不做：`miyu ask` 这一轮结束后退出码是 `4`；通讯平台上的会话也一样。

## 私密目录

私密目录放别的程序的钥匙、登录和浏览器数据。只在只读时起作用：命令读不到，文件工具读之前先问你。开放权限下不拦。

出厂的清单照系统不同。三个平台都有：`~/.ssh`、`~/.gnupg`、`~/.claude`、`~/.miyu`、`~/.config/gh`、`~/.codex`、`~/.gemini`、`~/.aws`、`~/.azure`、`~/.config/gcloud`、`~/.kube`、`~/.docker`、`~/.git-credentials`、`~/.netrc`、`~/.pypirc`、`~/.cargo/credentials.toml`、`~/.vault-token`。

Linux 另有：`~/.oci`、`~/.config/git/credentials`、`~/.gem/credentials`、`~/.terraform.d`、`~/.pgpass`、`~/.password-store`、`~/.local/share/keyrings`、`~/.local/share/kwalletd`、`~/.mozilla`、`~/.thunderbird`、`~/.config/google-chrome`、`~/.config/google-chrome-beta`、`~/.config/google-chrome-unstable`、`~/.config/chromium`、`~/.config/BraveSoftware`、`~/.config/microsoft-edge`、`~/.config/vivaldi`、`~/.config/opera`，`~/.var/app/` 下的 `org.mozilla.firefox`、`org.mozilla.Thunderbird`、`com.google.Chrome`、`org.chromium.Chromium`、`com.brave.Browser`、`com.microsoft.Edge`、`com.vivaldi.Vivaldi`、`com.opera.Opera`，`~/snap/firefox`、`~/snap/chromium`。

macOS 另有：`~/Library/Keychains`、`~/Library/Cookies`、`~/Library/Safari`，`~/Library/Application Support/` 下的 `Google/Chrome`、`Chromium`、`BraveSoftware`、`Microsoft Edge`、`Vivaldi`、`Firefox`、`1Password`、`Bitwarden`。

Windows 另有：`~/.terraform.d`、`~/_netrc`、`~/NTUSER.DAT`，`~/AppData/Roaming/` 下的 `gnupg`、`GitHub CLI`、`gcloud`、`Mozilla/Firefox`、`Opera Software`、`1Password`、`Bitwarden`、`KeePass`、`KeePassXC`、`Microsoft/Credentials`、`Microsoft/Protect`、`Microsoft/Vault`、`Microsoft/Crypto`、`Microsoft/SystemCertificates`，`~/AppData/Local/` 下的 `Google/Chrome/User Data`、`Microsoft/Edge/User Data`、`BraveSoftware/Brave-Browser/User Data`、`Chromium/User Data`、`Vivaldi/User Data`、`Microsoft/Credentials`、`Microsoft/Protect`、`Microsoft/Vault`、`Microsoft/Crypto`、`Microsoft/SystemCertificates`、`Microsoft/IdentityCache`、`Microsoft/TokenBroker`、`Microsoft/OneAuth`、`Microsoft/Windows/UsrClass.dat`。

工具照环境变量把凭据放到别处的，新位置也算私密：`GNUPGHOME`、`AWS_SHARED_CREDENTIALS_FILE`、`AWS_CONFIG_FILE`、`KUBECONFIG`、`DOCKER_CONFIG`、`GH_CONFIG_DIR`、`AZURE_CONFIG_DIR`、`NETRC`，`CARGO_HOME` 下的 `credentials.toml`；`~/.config/` 下的几项照 `XDG_CONFIG_HOME` 找。变量照核心启动时的环境读。

在设置页「隐私与安全 → 私密目录」里加减，或者用 `miyu config set`。列表照 TOML 的数组写，写的是整份：

```bash
miyu config set sandbox.private.add '["~/notes/keys", "/srv/secrets"]'   # 加
miyu config set sandbox.private.remove '["~/.docker"]'                   # 去掉出厂的
miyu config set sandbox.private.enabled false                            # 整个关掉
```

PowerShell 里外面用双引号，里面的路径用 TOML 的单引号字符串，Windows PowerShell 5.1 和 PowerShell 7 都能照抄：

```powershell
miyu config set sandbox.private.add "['~/notes/keys', 'D:\secrets']"     # 加
miyu config set sandbox.private.remove "['~/.docker']"                   # 去掉出厂的
miyu config set sandbox.private.enabled false                            # 整个关掉
```

- `sandbox.private.add`：写 `~/…` 或绝对路径。`~` 后面用 `/`，Windows 上也是，`~\…` 不认。家目录本身和它的上级不能加。
- `sandbox.private.remove`：只能写出厂清单里的。去掉的命令读得到、文件工具不再问；以后升级不会自己回来，新版本加的出厂项照样生效。
- `sandbox.private.enabled`：关掉以后只读时也不藏、不问。数据根照旧碰不到。

三项都从下一轮起生效。

## 项目配置

仓库里的 `.miyu/config.toml`（项目配置怎么用见 [配置](config.md)）对这一页只能收紧：只认 `permission.start_read_only = true`，写 `false` 不算。`permission.confirm.*`、`sandbox.private.*` 只认你自己的配置，写进项目配置也不算，`miyu config explain <键>` 看得到它没生效。

## 三个平台

只读时三个平台各用系统自带的办法关住命令：Linux 用 Landlock，macOS 用 Seatbelt，Windows 用微软的 MXC 基础容器。你感觉得到的差别：

| | 你会碰到的 |
|---|---|
| Linux | 内核没有可用的 Landlock 时沙盒用不了。私密目录、数据根看得到名字，读不到里面。内核够新时，连不上 Docker、ssh-agent 这类系统服务的套接字 |
| macOS | Miyu 自己跑在别的沙盒里时用不了沙盒。连不上 Docker、ssh-agent 这类系统服务的套接字 |
| Windows | 只在登录的桌面会话里能用：核心不在桌面会话里的（例如经 SSH 起的），只读下每条命令都被拒，退出码 `125`。只读得到本机的固定盘，U 盘这类可移动盘、网络盘、光驱读不到。剪贴板、模拟键盘鼠标用不了。用 WMI 的命令（`Get-CimInstance`、`Get-WmiObject`、`wmic`、`systeminfo`、`Get-ComputerInfo` 这类）跑不成，要用就切到开放权限 |

开放权限下的命令三个平台都不进沙盒，上表只管只读。

沙盒用不了时，`miyu ask` 开头印一行「沙盒不可用：<原因>」，能修的另起一行说怎么修。三个平台的原因各是：

| | 原因那一句 | 什么时候 |
|---|---|---|
| Linux | 内核没有可用的 Landlock | 内核旧于 5.13，或者启动参数 `lsm=` 里没有 `landlock` |
| macOS | 无法应用 Seatbelt 配置，Miyu 可能运行在另一个沙盒里 | Seatbelt 配置装不上 |
| Windows | 此版本在 Windows 上还不能把命令关进沙盒 | 这台 Windows 起不了沙盒要的基础容器 |

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
| 「… 是家目录或它的上级，不能设为私密目录」 | 写更深的一层目录 |
| 「… 不是出厂的私密目录：只能去掉出厂清单里的」 | 自己加的要从 `sandbox.private.add` 里删 |
