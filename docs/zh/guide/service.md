# 常驻和服务器

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

这一页说明怎么让核心开机自动启动、怎么在没有桌面的服务器上运行、怎么从其他电脑和手机打开网页界面，以及怎么查看日志、停止核心。安装方法可以看 [安装、升级、卸载](install.md)。

## 核心的运行方式

核心就是 `miyu` 在后台运行时的状态，会话、模型、定时任务都在核心里。终端界面、网页界面、系统托盘这些界面只是连接到核心。

- 打开界面时如果连接不上核心，界面会自己启动一个核心，等核心准备好以后再连接。所以不安装服务也可以正常使用。
- 一个数据根（默认是 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`；如果设置了 `MIYU_HOME`，就是它指向的目录）同一时间只运行一个核心。
- 核心默认一直运行，所有界面都关掉了也不会退出。

如果想让核心空闲时自己退出，可以在设置页（在终端界面或网页界面里输入 `/settings`）的 `高级 → 核心 → 空闲退出时间` 里填写分钟数，或者运行：

```sh
miyu config set core.idle_exit 30
```

设置之后，如果连续这么多分钟没有任何连接，也没有正在进行的一轮对话，核心就会退出。`0` 表示一直运行（默认值），可以填写的范围是 `0` 到 `525600`。修改后立即生效，这一项只能写在 `~/.miyu/etc/config.toml` 里。

手动停止核心，运行：

```sh
miyu core stop
```

这条命令会通知正在运行的核心停下，等核心退出以后再返回；如果核心没有在运行，就什么都不做。正在进行的那一轮对话会先停下来，下次载入这个会话时再继续。

手动启动核心、查看核心是否在运行，可以使用：

```sh
miyu core start
miyu core status
```

`start` 只启动核心，不打开界面，也不打开浏览器；如果核心已经在运行，就什么都不做。`status` 会显示核心是否在运行、版本和数据根，详细的见 [`miyu core`](cli.md#miyu-core)。

如果只想关掉网页界面、让核心继续运行，可以运行 `miyu web stop`；查看网页界面是否在运行，可以运行 `miyu web status`。

## 开机或登录时启动核心

安装服务之后，核心会在开机或登录时自动启动，这样通讯平台和定时任务就不需要等你打开界面才开始工作。运行：

```sh
miyu service enable
```

| 命令 | 作用 |
|---|---|
| `miyu service enable` | 写好服务文件，交给系统的服务管理器 |
| `miyu service disable` | 停止服务，从服务管理器里移除，并删除服务文件 |
| `miyu service status` | 查看服务是否已安装、是否已启用 |

三个命令都可以加 `--format json`，此时会输出一行 `{"installed", "on", "file", "manager"}`；出错时输出 `{"error", …}`。

服务运行的是你现在运行的这个 `miyu`（解析链接之后的真实位置）。如果设置了 `MIYU_HOME`，它也会一起写进服务。如果之后移动了 `miyu` 的位置，或者换了数据根，需要再运行一次 `miyu service enable` 重写服务文件。

各个系统使用的方式如下：

| 系统 | 使用的方式 | 写入的文件 | 启动的时机 |
|---|---|---|---|
| Linux，有 systemd 用户会话 | systemd 用户服务 | `~/.config/systemd/user/miyu.service` | 立即启动，以后每次登录时启动 |
| Linux，没有 systemd 用户会话 | 桌面自启动 | `~/.config/autostart/miyu.desktop` | 下次登录桌面时 |
| macOS | launchd 用户代理 | `~/Library/LaunchAgents/io.github.miyu-agent.core.plist` | 立即启动，以后每次登录时启动 |
| Windows | 任务计划，名字是 `Miyu` | `%USERPROFILE%\AppData\Local\Miyu\miyu-task.xml` | 下次登录时 |

在 Linux 上，如果设置了 `XDG_CONFIG_HOME`，前两个文件会写在它下面。

`miyu service enable` 成功时会提示 `已启用并启动 …`（systemd、launchd）、`已启用 …，下次登录桌面时启动`（桌面自启动）或 `已启用 …，下次登录时启动`（任务计划）。`status` 会显示 `已安装，已启用：…`、`已安装，未启用：…`、`已安装，登录桌面时启动：…` 或者 `未安装`。`status` 向服务管理器查询时使用的命令是：

| 系统 | 命令 |
|---|---|
| systemd | `systemctl --user is-active miyu.service`（查的是核心现在是否在运行） |
| launchd | `launchctl print gui/<你的 uid>/io.github.miyu-agent.core`（查的是是否已登记） |
| 任务计划 | `schtasks /Query /TN Miyu`（查的是是否已登记） |
| 桌面自启动 | 无法查询，只检查文件是否存在 |

### 各系统的差异

不同系统的服务管理器在核心退出、没有人登录等情况下的行为不一样：

| | systemd 用户服务 | 桌面自启动 | launchd | 任务计划 |
|---|---|---|---|---|
| 核心退出后 | 异常退出 5 秒后重新启动，正常退出不重新启动 | 不重新启动，下次打开界面时由界面启动核心 | 退出码不是 `0` 时重新启动，是 `0` 时不重新启动 | 失败时每隔 1 分钟重试一次，最多 3 次 |
| 没有人登录时 | 登录时启动、注销后停止；运行 `sudo loginctl enable-linger $USER` 之后，没有人登录也会一直运行 | 不运行 | 不运行 | 不运行 |
| 设置了 `MIYU_HOME` | 写进服务文件 | 写进服务文件 | 写进服务文件 | 无法安装 |

- 在 macOS 和 Windows 上，没有办法让核心在没有人登录时也保持运行。在 Linux 上，还可以按照 [部署到服务器](#部署到服务器) 安装成系统服务。
- 任务计划不需要管理员权限，只在你登录时启动核心。
- 在 Windows 上，如果设置了 `MIYU_HOME`，`miyu service enable` 会提示 `无法安装服务：Windows 任务计划无法携带 MIYU_HOME`，下一行是 `提示：取消设置 MIYU_HOME 后重试`。如果既想使用其他数据根，又想在登录时启动，可以使用系统托盘的 `开机时启动`，详细的见 [数据和备份](data.md#更换数据根的位置)。

### 让服务立即接管

如果已经有一个由界面启动的核心在运行，服务启动的核心发现已经有核心在运行，就会正常退出（退出码 `0`），不会替换掉正在运行的那个。想让服务立即接管核心：

1. 停止正在运行的核心：`miyu core stop`。
2. 再安装一次服务：`miyu service enable`。它会按照现在的 `miyu` 位置和 `MIYU_HOME` 重写服务文件，再交给服务管理器。

| 系统 | 第 2 步之后 |
|---|---|
| Linux（systemd 用户服务）、macOS | 服务立即启动核心 |
| Linux（桌面自启动）、Windows | 只是重新登记，下次登录时才由服务启动核心；在那之前打开界面时，界面照常会自己启动一个核心 |

升级之后想让核心换成新版，也是这两步。

### 服务和环境变量

服务启动的核心看不到你在 shell 里设置的变量。`miyu service enable` 写的服务文件里只带 `MIYU_HOME`（如果设置了的话），不带其他任何变量；Windows 的任务计划连 `MIYU_HOME` 也不带。

配置里写成 `{ env = "变量" }` 的 key，核心会从它自己启动时的环境里读取，读不到你 shell 里的变量。如果想让服务启动的核心用上这个 key，需要改用密钥文件：用 `miyu provider login` 把 key 存进密钥文件，配置里写 `{ secret = "名字" }`，这样核心会从密钥文件读取，不依赖环境变量。

如果一定要使用环境变量：

- systemd 用户服务：运行 `systemctl --user edit miyu`，加上 `Environment=` 那一行（写法见 [环境变量里的 key](#环境变量里的-key)）。如果直接修改 `miyu.service`，下次运行 `miyu service enable` 时修改会被覆盖。
- launchd 的 plist 和桌面自启动的 `miyu.desktop`：手动修改的内容，同样会在下次运行 `miyu service enable` 时被覆盖。

如果安装了系统托盘，也可以使用托盘菜单里的 `开机时启动`，让系统托盘在登录时启动，详细的见 [系统托盘](heads/applet.md)。

## 部署到服务器

这一节只适用于 Linux。macOS 和 Windows 上没有服务器的样例，想让核心常驻，可以使用上一节的 `miyu service enable`。

在服务器上，用一个专门的系统用户运行核心，数据根放在 `/var/lib/miyu`，而不是放在某个人的家目录下。由于 `miyu service` 只管理你自己这个用户，系统级的服务需要按照下面的步骤手动安装。

Miyu 仓库的 `packaging/` 目录里有现成的样例，它们都以 `miyu` 用户运行 `/usr/bin/miyu core`，数据根是 `/var/lib/miyu`，缓存是 `/var/cache/miyu`：

| 初始化系统 | 样例 | 安装位置 |
|---|---|---|
| systemd | `packaging/systemd/system/miyu.service` | `/etc/systemd/system/` |
| systemd，网页界面 | `packaging/systemd/system/miyu-web.service` | `/etc/systemd/system/` |
| OpenRC | `packaging/init/openrc/miyu` | `/etc/init.d/miyu`，加上执行权限 |
| runit | `packaging/init/runit/miyu/run` | 把整个目录放进 `/etc/sv/miyu`，给 `run` 加上执行权限，再链接到服务目录 |
| dinit | `packaging/init/dinit/miyu` | `/etc/dinit.d/miyu`，环境变量写进 `/etc/dinit.d/miyu.env` |

如果 `miyu` 不在 `/usr/bin`（例如用安装脚本安装的），把样例里的路径换成它实际的位置。

### 用 systemd 启动核心

1. 创建系统用户：

   ```sh
   sudo useradd --system --home-dir /var/lib/miyu --shell /usr/sbin/nologin miyu
   ```

2. 下载样例并安装：

   ```sh
   curl -fsSLO https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/systemd/system/miyu.service
   sudo cp miyu.service /etc/systemd/system/
   sudo systemctl enable --now miyu
   ```

   systemd 会按照样例里的 `StateDirectory`、`CacheDirectory` 创建 `/var/lib/miyu` 和 `/var/cache/miyu`，属主是 `miyu`，数据根的权限是 `0700`。OpenRC 的样例每次启动前会自己创建这两个目录；runit 和 dinit 则需要你先创建好这两个目录，运行 `chown miyu:miyu`，并对数据根运行 `chmod 0700`。

3. 确认核心已经启动：运行 `sudo systemctl status miyu`。

### 在服务器上运行命令

核心只接受和它是同一个系统用户、使用同一个数据根的命令行。因此在服务器上运行的每一条 `miyu` 命令都要像这样套一层：

```sh
sudo -u miyu env MIYU_HOME=/var/lib/miyu XDG_CACHE_HOME=/var/cache miyu config get models.chat
```

接入模型（见 [模型](models.md)）也是同样的写法，key 从管道读取：

```sh
echo "$DEEPSEEK_API_KEY" | sudo -u miyu env MIYU_HOME=/var/lib/miyu XDG_CACHE_HOME=/var/cache miyu provider login deepseek
```

### 环境变量里的 key

配置里写成 `{ env = "变量" }` 的 key，核心会从它自己启动时的环境里读取，读不到你登录 shell 里的变量。所以如果核心是由服务启动的，要把变量写进服务里：

- systemd：运行 `sudo systemctl edit miyu`，加上：

  ```ini
  [Service]
  Environment=DEEPSEEK_API_KEY=sk-example
  ```

  或者用 `EnvironmentFile=` 指向一个只有 root 能读取的文件。修改完成后运行 `sudo systemctl restart miyu`。
- dinit：写进 `/etc/dinit.d/miyu.env`。
- OpenRC、runit：加在样例里 `export` 那一行旁边。

如果不想把 key 放进服务文件，可以用 `miyu provider login` 把 key 存进密钥文件，配置里写 `{ secret = "名字" }`。

如果是在自己的电脑上用 `miyu service enable` 安装的服务，可以看 [服务和环境变量](#服务和环境变量)。

### 用 systemd 运行网页界面

网页界面的程序 `miyu-web` 出厂设置是一直运行、不会自己退出（`web.idle_seconds` 是 `0`），所以可以直接作为服务运行。作为服务运行时，不要把 `web.idle_seconds` 改成非 0 的值：因为空闲退出的退出码是 0，服务管理器不会再重新启动它。

1. 下载样例：

   ```sh
   curl -fsSLO https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/systemd/system/miyu-web.service
   ```

2. 把 `ExecStart=` 里的路径换成 `miyu-web` 的真实位置，后面仍然带上 `serve`。从 AUR 安装时路径是 `/usr/share/miyu/packages/webui/bin/miyu-web`，用安装脚本安装时在 `miyu` 旁边：

   ```ini
   ExecStart=/usr/share/miyu/packages/webui/bin/miyu-web serve
   ```

3. 安装服务：

   ```sh
   sudo cp miyu-web.service /etc/systemd/system/
   sudo systemctl enable --now miyu-web
   ```

启动网页界面的服务时会连带启动核心的服务；核心的服务停止时，网页界面的服务也会跟着停止。网页界面默认只监听服务器本机的 `127.0.0.1`，从其他电脑打开的方法见下一节。

## 从其他电脑和手机打开网页界面

想从其他设备打开网页界面，需要修改网页界面的几项配置，这几项都只能写在 `~/.miyu/etc/config.toml` 里：

| 键 | 默认值 | 说明 |
|---|---|---|
| `web.listen` | `127.0.0.1` | 监听的地址。`127.0.0.1` 表示只有这台电脑能打开；`0.0.0.0` 表示这台电脑的所有地址；也可以写其中某一个地址 |
| `web.port` | `8300` | 端口，这台电脑和其他设备都使用这个端口，范围是 `1` 到 `65535` |
| `web.idle_seconds` | `0` | 没有浏览器连接、也没有在提供媒体文件的状态持续多少秒后退出，范围是 `0` 到 `86400`，`0` 表示不退出。打开了局域网访问时不会退出。设置页上不显示这一项 |

从其他设备访问时都需要输入访问密码；在这台电脑上用 `miyu web` 打开时不需要。

### 局域网访问

1. 在这台电脑上运行 `miyu web --settings access`，打开 `设置 → 通用 → 访问`。
2. 打开 `允许局域网访问`。如果还没有设置访问密码，会先弹出 `设置访问密码`，设置好之后开关就会打开。
3. 设置页上会列出其他设备使用的网址，还有一个二维码。用手机扫码，输入访问密码就可以进入。

无论是在设置页里修改地址和端口、用 `miyu config set` 修改，还是手动修改配置文件，网页界面都会立即切换过去。此时 `miyu config set` 输出的 `重启后生效` 并不准确。

> 局域网访问使用的是明文 HTTP，同一个网络里的人可以看到访问密码、登录令牌和对话内容。所以只在自己家这样信得过的网络里打开。在外面使用时，先用 Tailscale、WireGuard 这类加密隧道连回这个网络，再打开局域网的网址。在 macOS 上，如果其他设备打不开，到 `系统设置 → 网络 → 防火墙 → 选项` 里允许 `miyu-web` 的传入连接。

如果忘了访问密码，在这台电脑上运行 `miyu web --reset`，在打开的页面里重新设置即可。如果想让所有浏览器都重新登录，运行 `miyu web --logout`。

### 没有桌面的服务器

服务器上没有浏览器，所以要先从你自己的电脑通过 SSH 端口转发连进去，设置访问密码：

1. 在你的电脑上运行（两边的端口要一样，都写服务器的 `web.port`）：

   ```sh
   ssh -L 8300:127.0.0.1:8300 you@server.example
   ```

2. 在你的电脑上用浏览器打开 `http://127.0.0.1:8300`。网页界面会把这条连接当成服务器本机，不需要密码就能进入。
3. 到 `设置 → 通用 → 访问` 设置访问密码；如果需要局域网访问，再打开 `允许局域网访问`。

> 通过 `ssh -L`、`socat` 这类不添加转发头的方式连进来的人，网页界面一律当成服务器本机：不需要密码，可以读取服务器上的文件，还可以不输入旧密码就改掉访问密码。所以这条路只给你自己用。

### 反向代理

如果通过同一台机器上的 nginx、cloudflared 这类反向代理打开网页界面，网页界面会把访问者当成其他设备：需要访问密码，只在本机能用的功能（选择服务器上的目录、读取服务器上的文件）无法使用。

- 代理一定要加上转发头：`Forwarded`、`X-Forwarded-For`、`X-Real-IP` 这类，有其中一个就可以。如果不加，网页界面会把代理转发来的人当成服务器本机。
- 网页界面只接受 `Host` 是 `127.0.0.1:<端口>`、`localhost:<端口>`、`[::1]:<端口>` 的请求；监听局域网时，也接受这台电脑的 IP 加端口。不接受域名。
- 对话通过 `/ws` 的 WebSocket 进行，只接受 `Origin` 是 `http://` 加上面这些地址的连接。如果代理用自己的域名或者 HTTPS 提供页面，这条连接会被拒绝（403）。

网页界面的用法可以看 [网页界面](heads/web.md)。

## 日志

出问题时可以查看日志。各个部分的日志位置如下：

| 日志 | 位置 |
|---|---|
| 核心 | `~/.miyu/var/log/core.log`，满 10 MB 后换一个新文件，保留 `core.log.1` 到 `core.log.5` |
| 网页界面 | `~/.miyu/var/log/webui/web.log` |
| 系统托盘 | `~/.miyu/var/log/miyu-applet.log` |
| 软件包 | `~/.miyu/var/log/<编号>/` |

在 Linux 服务器上，把 `~/.miyu` 换成 `/var/lib/miyu`。

`core.log` 是核心自己写的，三个平台都有；无论核心是由服务启动的还是由界面启动的，日志都写在这里。如果核心由 systemd 管理，还可以用 `journalctl --user -u miyu`（服务器上的系统服务用 `journalctl -u miyu`）查看 systemd 为这个服务记录的输出。launchd 的 plist 和任务计划都没有设置输出的位置，所以只有 `core.log`。

核心的日志一行一条，用英文记录，不记录对话内容和 key。日志的详细程度用 `log.level` 调整，可以在设置页的 `高级 → 运行日志 → 运行日志的级别` 里修改，或者运行：

```sh
miyu config set log.level debug
```

| 值 | 记录的内容 |
|---|---|
| `error` | 只记录错误 |
| `warn` | 错误和警告 |
| `info` | 常规信息（默认） |
| `debug` | 详细信息，排查问题时使用 |
| `trace` | 最详细 |
| `off` | 不记录 |

修改后立即生效，这一项只能写在 `~/.miyu/etc/config.toml` 里。如果设置了环境变量 `MIYU_LOG`，那一次启动会按照它的值记录。先运行 `miyu core stop`，再在设置了这个变量的窗口里打开界面：

```bash
MIYU_LOG=debug miyu
```

```powershell
$env:MIYU_LOG = 'debug'
miyu
```

在 PowerShell 里这样设置以后，这个窗口之后启动的程序都会带着这个变量；不需要时运行 `Remove-Item Env:MIYU_LOG` 即可。

## 停止核心和服务

根据想停止的对象，使用不同的命令：

| 要停止的对象 | 方法 |
|---|---|
| 这一次运行的核心，包括服务启动的 | `miyu core stop` |
| 这一次运行的网页界面 | `miyu web stop` |
| 自己安装的开机启动 | `miyu service disable` |
| 服务器上的系统服务 | `sudo systemctl disable --now miyu-web miyu`，其他初始化系统按照各自的方法操作 |

用 `miyu core stop` 停下的核心属于正常退出（退出码 `0`）。由于 systemd 用户服务和 launchd 只在异常退出时重新启动核心，所以此时不会再启动它；systemd 的 `miyu service status` 会随之变成 `已安装，未启用`，而 launchd 查询的是是否已登记，所以仍然显示 `已安装，已启用`。下次登录时，服务照常启动核心。

`miyu service disable` 会从服务管理器里移除服务并删除服务文件；对于 systemd 用户服务（`disable --now`）和 launchd（`launchctl bootout`），还会同时停止服务启动的核心。对于任务计划和桌面自启动，Miyu 只移除登记、删除文件，不会另外停止核心，想停止的话再运行 `miyu core stop`。

## Windows 上的只读会话

Windows 的沙盒需要有桌面会话。如果核心不在桌面会话里（例如通过 SSH 连接进来启动的），只读会话里的每一条命令都无法运行，会输出 `miyu-sandbox: cannot confine: no desktop session, programs in the sandbox cannot start here; use Miyu from the desktop, or turn the sandbox off`，退出码是 `125`。解决方法是从已登录的桌面使用 Miyu，或者切换到开放权限。详细的见 [权限和沙盒](permissions.md)。
