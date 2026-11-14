# 常驻和服务器

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

让核心开机就起来、放在没有桌面的服务器上跑、从别的机器和手机打开网页、看日志、停掉，都在这一页。怎么装见 [安装、升级、卸载](install.md)。

## 核心平时怎么跑

核心是 `miyu` 在后台跑的样子，会话、模型、定时任务都在它里面。终端、网页、托盘这些头只是连上它。

- 打开头时连不上核心，头就拉起一个，等它说好了再连。不装服务也能用。
- 一个数据根（默认 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`；设了 `MIYU_HOME` 的是它）同时只跑一个核心。
- 核心默认一直开着，头都关了也不退。

要它空闲了自己退出，在设置页（终端、网页里敲 `/settings`）「高级 → 核心 → 核心空闲多久退出」里填分钟数，或者：

```sh
miyu config set core.idle_exit 30
```

没有连接、也没有在跑的一轮，连续这么多分钟以后退出。`0` 是一直开着（默认），能写 `0` 到 `525600`。改了当场生效，只能写在 `~/.miyu/etc/config.toml`。

手动停：

```sh
miyu core stop
```

它叫在跑的核心停下，等它退了再回来；没在跑的什么都不做。正在跑的这一轮先停下来，下次载入这个会话时接着干。

只想关网页、核心照常跑：`miyu web --stop`。

## 开机或登录就起来

装了服务，开机或登录时核心就起来，通讯平台、定时任务不用等你打开头。

```sh
miyu service enable
```

| 命令 | 做什么 |
|---|---|
| `miyu service enable` | 写好服务文件、交给系统的服务管理器 |
| `miyu service disable` | 停掉服务、从服务管理器里拿掉、删掉服务文件 |
| `miyu service status` | 看装没装、开没开着 |

三个命令都能加 `--format json`，印一行 `{"installed", "on", "file", "manager"}`，出错的是 `{"error", …}`。

服务跑的是你现在敲的这个 `miyu`（链接解开后的真实位置）。设了 `MIYU_HOME` 的，一并写进服务。换了 `miyu` 的位置、换了数据根的，再敲一次 `miyu service enable` 重写。

各系统用的办法：

| 系统 | 用什么 | 写哪个文件 | 什么时候起 |
|---|---|---|---|
| Linux，有 systemd 用户会话 | systemd 用户服务 | `~/.config/systemd/user/miyu.service` | 马上起，以后登录时起 |
| Linux，没有 systemd 用户会话 | 桌面自启动 | `~/.config/autostart/miyu.desktop` | 下次登录桌面时 |
| macOS | launchd 用户代理 | `~/Library/LaunchAgents/io.github.miyu-agent.core.plist` | 马上起，以后登录时起 |
| Windows | 任务计划，名字 `Miyu` | `%USERPROFILE%\AppData\Local\Miyu\miyu-task.xml` | 下次登录时 |

Linux 上设了 `XDG_CONFIG_HOME` 的，前两个文件写在它下面。

`miyu service enable` 成了说「已启用并启动 …」（systemd、launchd）、「已启用 …，下次登录桌面时启动」（桌面自启动）或「已启用 …，下次登录时启动」（任务计划）。`status` 说「已安装，已启用：…」「已安装，未启用：…」「已安装，登录桌面时启动：…」或者「未安装」。它问服务管理器用的命令：

| 系统 | 命令 |
|---|---|
| systemd | `systemctl --user is-active miyu.service`（看的是现在跑没跑着） |
| launchd | `launchctl print gui/<你的 uid>/io.github.miyu-agent.core`（看的是登记了没有） |
| 任务计划 | `schtasks /Query /TN Miyu`（看的是登记了没有） |
| 桌面自启动 | 问不了，只看文件在不在 |

### 几个系统各自要知道的

| | systemd 用户服务 | 桌面自启动 | launchd | 任务计划 |
|---|---|---|---|---|
| 核心退了 | 异常退出的 5 秒后重起，正常退出的不重起 | 不重起，下次打开头时头把它拉起来 | 退出码不是 `0` 的重起，是 `0` 的不重起 | 失败时每隔 1 分钟重试，最多 3 次 |
| 没人登录时 | 登录时起、注销后停；敲 `sudo loginctl enable-linger $USER` 以后没人登录也开着 | 不跑 | 不跑 | 不跑 |
| 设了 `MIYU_HOME` | 写进服务文件 | 写进服务文件 | 写进服务文件 | 装不了 |

- macOS、Windows 上没有让核心在没人登录时也开着的做法。Linux 上另可以照 [放到服务器上](#放到服务器上) 装成系统服务。
- 任务计划不要管理员，只在你登录时起。
- Windows 上设了 `MIYU_HOME` 的，`miyu service enable` 说「无法安装服务：Windows 任务计划无法携带 MIYU_HOME」，下一行「提示：取消设置 MIYU_HOME 后重试」。要用别的数据根又要登录时起的，用托盘的「开机时启动」，见 [数据和备份](data.md#换一个位置)。

### 让服务马上接手

已经有一个头拉起的核心在跑时，服务起的那个看到有核心在跑就正常退出（退出码 `0`），不会顶掉它。要服务马上接手：

1. 停掉在跑的核心：`miyu core stop`。
2. 再装一次服务：`miyu service enable`。它照现在的 `miyu` 位置和 `MIYU_HOME` 重写服务文件，再交给服务管理器。

| 系统 | 第 2 步以后 |
|---|---|
| Linux（systemd 用户服务）、macOS | 服务当场把核心起来 |
| Linux（桌面自启动）、Windows | 只重新登记，下次登录才由服务起；在那之前打开头时，头照常拉起一个核心 |

升级以后让核心换成新版，也是这两步。

### 服务和环境变量

服务起的核心看不到你在 shell 里设的变量。`miyu service enable` 写的服务文件里只带 `MIYU_HOME`（设了的话），别的变量一个都不带；Windows 的任务计划连 `MIYU_HOME` 也不带。

配置里 `{ env = "变量" }` 这样写的 key，核心照它自己起来时的环境取，不认你 shell 里的。要服务起的核心用得上，改用密钥文件：`miyu provider login` 把 key 存进去，配置里写 `{ secret = "名字" }`，核心从密钥文件取，不靠环境变量。

一定要用环境变量的：

- systemd 用户服务：`systemctl --user edit miyu` 加 `Environment=` 那一行（写法见 [环境变量里的 key](#环境变量里的-key)）。直接改 `miyu.service` 的，下次 `miyu service enable` 会盖掉。
- launchd 的 plist、桌面自启动的 `miyu.desktop`：手改的，下次 `miyu service enable` 同样会盖掉。

装了托盘的，也可以用托盘菜单里的「开机时启动」让托盘登录时起，见 [托盘](heads/applet.md)。

## 放到服务器上

这一节只适用 Linux。macOS、Windows 上没有服务器的样例，要核心常驻用上一节的 `miyu service enable`。

服务器上用一个专门的系统用户跑核心，数据根放在 `/var/lib/miyu`，不挂在某个人的家目录下。`miyu service` 只管你自己这个用户，系统级的服务要照下面手动装。

Miyu 仓库的 `packaging/` 里有现成的样例，都以 `miyu` 用户跑 `/usr/bin/miyu core`，数据根 `/var/lib/miyu`，缓存 `/var/cache/miyu`：

| 初始化系统 | 样例 | 装到哪 |
|---|---|---|
| systemd | `packaging/systemd/system/miyu.service` | `/etc/systemd/system/` |
| systemd，网页 | `packaging/systemd/system/miyu-web.service` | `/etc/systemd/system/` |
| OpenRC | `packaging/init/openrc/miyu` | `/etc/init.d/miyu`，加执行权限 |
| runit | `packaging/init/runit/miyu/run` | 整个目录放进 `/etc/sv/miyu`，`run` 加执行权限，再链进服务目录 |
| dinit | `packaging/init/dinit/miyu` | `/etc/dinit.d/miyu`，环境变量写进 `/etc/dinit.d/miyu.env` |

`miyu` 不在 `/usr/bin` 的（安装脚本装的），把样例里的路径换成它的位置。

### 用 systemd 起核心

1. 建系统用户：

   ```sh
   sudo useradd --system --home-dir /var/lib/miyu --shell /usr/sbin/nologin miyu
   ```

2. 取样例、装上：

   ```sh
   curl -fsSLO https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/systemd/system/miyu.service
   sudo cp miyu.service /etc/systemd/system/
   sudo systemctl enable --now miyu
   ```

   systemd 照样例里的 `StateDirectory`、`CacheDirectory` 建好 `/var/lib/miyu`、`/var/cache/miyu`，属于 `miyu`，数据根权限 `0700`。OpenRC 的样例每次启动前自己建这两个目录；runit、dinit 的要你先建好、`chown miyu:miyu`、数据根 `chmod 0700`。

3. 看它起来没有：`sudo systemctl status miyu`。

### 在服务器上敲命令

核心只认和它同一个系统用户、同一个数据根的命令行。在服务器上敲 `miyu` 的命令都要这样套一层：

```sh
sudo -u miyu env MIYU_HOME=/var/lib/miyu XDG_CACHE_HOME=/var/cache miyu config get models.chat
```

接模型（见 [模型](models.md)）也这样，key 从管道读：

```sh
echo "$DEEPSEEK_API_KEY" | sudo -u miyu env MIYU_HOME=/var/lib/miyu XDG_CACHE_HOME=/var/cache miyu provider login deepseek
```

### 环境变量里的 key

配置里写 `{ env = "变量" }` 的 key，核心照它自己起来时的环境取，不认你登录 shell 里的。服务起的核心要把变量写进服务：

- systemd：`sudo systemctl edit miyu`，加

  ```ini
  [Service]
  Environment=DEEPSEEK_API_KEY=sk-example
  ```

  或者 `EnvironmentFile=` 指一个只有 root 能读的文件。改完 `sudo systemctl restart miyu`。
- dinit：写进 `/etc/dinit.d/miyu.env`。
- OpenRC、runit：在样例里 `export` 那一行旁边加。

不想把 key 放进服务文件的，用 `miyu provider login` 存进密钥文件，配置里写 `{ secret = "名字" }`。

自己这台电脑上用 `miyu service enable` 装的服务见 [服务和环境变量](#服务和环境变量)。

### 用 systemd 跑网页

网页软件 `miyu-web` 默认没有浏览器连着、也没有在给媒体文件，连续 600 秒就退出，退出码是 0，服务管理器不会再起它；打开了局域网访问的不退。当服务跑先让它不退：

1. 关掉空闲退出：

   ```sh
   sudo -u miyu env MIYU_HOME=/var/lib/miyu XDG_CACHE_HOME=/var/cache miyu config set web.idle_seconds 0
   ```

2. 取样例：

   ```sh
   curl -fsSLO https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/systemd/system/miyu-web.service
   ```

3. 把 `ExecStart=` 换成 `miyu-web` 的真实位置，后面照旧带 `serve`。AUR 装的是 `/usr/share/miyu/packages/webui/bin/miyu-web`，安装脚本装的在 `miyu` 旁边：

   ```ini
   ExecStart=/usr/share/miyu/packages/webui/bin/miyu-web serve
   ```

4. 装上：

   ```sh
   sudo cp miyu-web.service /etc/systemd/system/
   sudo systemctl enable --now miyu-web
   ```

起网页服务时连带起核心服务，核心服务停了它跟着停。网页默认只听服务器本机的 `127.0.0.1`，从别的机器怎么打开见下一节。

## 从别的机器、手机打开网页

网页的几项配置都只能写在 `~/.miyu/etc/config.toml`：

| 键 | 默认 | 说明 |
|---|---|---|
| `web.listen` | `127.0.0.1` | 听哪个地址。`127.0.0.1` 只有这台电脑能开；`0.0.0.0` 是这台电脑的所有地址；也可以写其中一个 |
| `web.port` | `8300` | 端口，本机和别的设备都用它，`1` 到 `65535` |
| `web.idle_seconds` | `600` | 没有浏览器连着、也没有在给媒体文件，连续多少秒后退出，`0` 到 `86400`，`0` 是不退。打开了局域网访问的不退。设置页上不显示 |

从别的设备进来都要访问密码。在这台电脑上用 `miyu web` 打开的不用。

### 局域网

1. 在这台电脑上敲 `miyu web --settings access`，打开「设置 → 通用 → 访问」。
2. 打开「允许局域网访问」。还没设访问密码的，先弹「设置访问密码」，设好就打开。
3. 设置页上列出别的设备用的网址，还有一个二维码，手机扫码、敲访问密码就能进。

地址、端口在设置页里改、用 `miyu config set` 改、手改配置文件，网页都当场换过去。`miyu config set` 这时印的「重启后生效」不准。

局域网访问是明文 HTTP，同一个网络里的人看得到访问密码、登录令牌和对话内容。只在自己家这样信得过的网络里开。在外面用，先用 Tailscale、WireGuard 这类加密的隧道连回这个网络，再开局域网的网址。macOS 上别的设备打不开的，到「系统设置 → 网络 → 防火墙 → 选项」里允许 `miyu-web` 的传入连接。

忘了访问密码：在这台电脑上敲 `miyu web --reset`，打开的页面里重设。要所有浏览器都重新登录：`miyu web --logout`。

### 没有桌面的服务器

服务器上没有浏览器，先从你自己的电脑经 SSH 端口转发进去设访问密码：

1. 在你的电脑上敲（两边端口要一样，都写服务器的 `web.port`）：

   ```sh
   ssh -L 8300:127.0.0.1:8300 you@server.example
   ```

2. 在你的电脑上用浏览器打开 `http://127.0.0.1:8300`。网页把这条连接当成服务器自己，不要密码就进得去。
3. 到「设置 → 通用 → 访问」设好访问密码，要局域网的再打开「允许局域网访问」。

经 `ssh -L`、`socat` 这类不加头的转发进来的人，网页一律当成服务器本机：不要密码，能读服务器上的文件，能不带旧密码改掉访问密码。这条路只给你自己用。

### 反向代理

经同一台机器上的 nginx、cloudflared 这类反向代理打开的，网页当成别的设备：要访问密码，只在本机能用的功能（选服务器上的目录、读它的文件）用不了。

- 代理一定要加上转发的头：`Forwarded`、`X-Forwarded-For`、`X-Real-IP` 这类，有一个就行。不加的，网页把代理转来的人当成服务器本机。
- 网页只认 `Host` 是 `127.0.0.1:<端口>`、`localhost:<端口>`、`[::1]:<端口>` 的请求；听局域网时也认这台电脑的 IP 加端口。不认域名。
- 对话走 `/ws` 的 WebSocket，只接 `Origin` 是 `http://` 加上面这些地址的。代理用自己的域名或 HTTPS 给出页面时，这条连接会被拒（403）。

网页怎么用见 [网页](heads/web.md)。

## 日志

| 谁的 | 在哪 |
|---|---|
| 核心 | `~/.miyu/var/log/core.log`，满 10 MB 换一份，留 `core.log.1` 到 `core.log.5` |
| 网页 | `~/.miyu/var/log/webui/web.log` |
| 托盘 | `~/.miyu/var/log/miyu-applet.log` |
| 软件包 | `~/.miyu/var/log/<编号>/` |

Linux 服务器上把 `~/.miyu` 换成 `/var/lib/miyu`。

核心的 `core.log` 是核心自己写的，三个平台都有，服务起的、头拉起的都写在这里。systemd 管着的核心，另外能用 `journalctl --user -u miyu`（服务器上的系统服务是 `journalctl -u miyu`）看 systemd 给这个服务记的输出。launchd 的 plist、任务计划都没有设输出写到哪，只有 `core.log`。

核心的日志一行一条，英文，不记对话内容和 key。记到哪一级用 `log.level` 调，设置页「高级 → 运行日志 → 运行日志的级别」，或者：

```sh
miyu config set log.level debug
```

| 值 | 记什么 |
|---|---|
| `error` | 只记错误 |
| `warn` | 错误和警告 |
| `info` | 常规（默认） |
| `debug` | 详细，排查用 |
| `trace` | 最详细 |
| `off` | 不记 |

改了当场生效，只能写在 `~/.miyu/etc/config.toml`。设了环境变量 `MIYU_LOG` 的，那一次启动照它。先 `miyu core stop`，再在设了它的窗口里打开头：

```bash
MIYU_LOG=debug miyu
```

```powershell
$env:MIYU_LOG = 'debug'
miyu
```

PowerShell 里这样设的，这个窗口以后起的都带着它；不要了敲 `Remove-Item Env:MIYU_LOG`。

## 停掉

| 要停的 | 怎么做 |
|---|---|
| 这一次的核心，服务起的也算 | `miyu core stop` |
| 这一次的网页 | `miyu web --stop` |
| 自己装的开机启动 | `miyu service disable` |
| 服务器上的系统服务 | `sudo systemctl disable --now miyu-web miyu`，别的初始化系统照它自己的办法 |

`miyu core stop` 停下的核心是正常退出（退出码 `0`）。systemd 用户服务、launchd 只在异常退出时重起，这时不再起它；systemd 的 `miyu service status` 跟着变成「已安装，未启用」，launchd 问的是登记了没有，照旧是「已安装，已启用」。下次登录时服务照常起。

`miyu service disable` 从服务管理器里拿掉、删掉服务文件，systemd 用户服务（`disable --now`）、launchd（`launchctl bootout`）同时停掉服务起的核心。任务计划、桌面自启动的，Miyu 只拿掉登记、删掉文件，不另去停核心，要停再敲 `miyu core stop`。

## Windows 上的只读会话

Windows 的沙盒要有桌面会话。核心不在桌面会话里时（例如经 SSH 连进来起的），只读的会话里每条命令都跑不了，输出 `miyu-sandbox: cannot confine: no desktop session, programs in the sandbox cannot start here; use Miyu from the desktop, or turn the sandbox off`，退出码 `125`。从登录的桌面用 Miyu，或者切到开放权限。见 [权限和沙盒](permissions.md)。
