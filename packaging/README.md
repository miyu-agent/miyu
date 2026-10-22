# packaging

安装包、系统要用的文件。个人机器上那四份（systemd 用户服务、XDG 自启动、launchd、Windows 任务计划）也是 `miyu service enable` 写的样子：它照这里的编进程序，换上 `miyu` 的真实位置（施工 Z-1 下），改了这里的跟着变。写打包脚本（PKGBUILD、deb、rpm、Homebrew、安装脚本）的那一步照这里装。设计见 `docs/designs/12-进程形态与分发.md` 第二、四节。

| 目录 | 是什么 |
|---|---|
| `mime/miyu.xml` | `.miyu` 包文件的类型（施工 G-14）：装进 `share/mime/packages/` |
| `systemd/user/miyu.service` | 个人机器上让核心一直开着：systemd 用户服务（施工 Z-1 上） |
| `xdg/miyu.desktop` | 同上，给没有 systemd 用户会话的桌面：XDG 自启动 |
| `macos/miyu.plist` | 同上，macOS：launchd 的用户代理 |
| `windows/miyu.xml` | 同上，Windows：登录时启动的任务计划，不要管理员 |
| `systemd/system/miyu.service`、`miyu-web.service` | 服务器的样例：systemd 系统服务，核心和网页软件 |
| `init/{openrc,runit,dinit}/` | 服务器的样例：别的初始化系统 |

## 个人机器（第一版的重点）

核心默认一直开着，不再空闲 10 分钟就退（施工 Z-1 上）；要省的在系统配置里写 `core.idle_exit = <分钟>`（`0` 是不退）。头连不上核心时照旧自己拉起，所以不装服务也能用；装了的开机、登录就起，通讯平台的桥、定时任务不等人打开头。

- **Linux（systemd）**：`systemd/user/miyu.service` 装进 `~/.config/systemd/user/`（安装包装进 `/usr/lib/systemd/user/`），`systemctl --user enable --now miyu`。没人登录也要开着的再 `sudo loginctl enable-linger <账号>`。日志：`journalctl --user -u miyu`，核心自己的运行日志照旧在 `~/.miyu/state/logs/`。
- **Linux（没有 systemd 用户会话的桌面）**：`xdg/miyu.desktop` 放进 `~/.config/autostart/`，登录桌面时起。
- **macOS**：`macos/miyu.plist` 改名成 `io.github.miyu-agent.core.plist` 放进 `~/Library/LaunchAgents/`，`launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/io.github.miyu-agent.core.plist`。
- **Windows**：`schtasks /Create /TN Miyu /XML windows\miyu.xml`。

`miyu` 不在文件里写的那个位置的（安装脚本装在 `~/.local/lib/miyu/miyu`、Windows 的 `%LOCALAPPDATA%\Programs\Miyu\miyu.exe`），把路径换成它。或者直接 `miyu service enable`：替你写好、交给服务管理器，路径照这一个 `miyu`，设了 `MIYU_HOME` 的一并写上；`miyu service status` 看、`disable` 卸（施工 Z-1 下，`docs/blueprint/cli/service.md`）。

已经有一个核心在跑（头先拉起了）时，服务管理器起的这一个说一句 `running`、退出码 0 就走，不会重起它；要服务马上接手的，先让头拉起的那一个退干净（`miyu` 里退出，或者等它停），再重启服务。

## 服务器（样例，给自己有服务器的人用）

用一个专门的系统用户跑，数据根放在 `/var/lib/miyu`，不挂在某个人的家目录下面（那个人的账号停了、搬了，核心跟着没了）。

```sh
sudo useradd --system --home-dir /var/lib/miyu --shell /usr/sbin/nologin miyu
sudo cp systemd/system/miyu.service systemd/system/miyu-web.service /etc/systemd/system/
sudo systemctl enable --now miyu
sudo -u miyu env MIYU_HOME=/var/lib/miyu miyu config set --system web.idle_seconds 0
sudo systemctl enable --now miyu-web
```

网页软件默认空闲 600 秒就退、退出码 0（`miyu web` 用的时候才起它，退了下次再起），当服务跑时服务管理器不会再起它。所以要先在系统配置里写 `idle_seconds = 0`（`[web]` 那一节，不退，网页软件 W-349 起认）再起 `miyu-web`。`miyu-web.service` 跑的是网页软件自己的 `miyu-web serve`，不在 `/usr/bin` 的把 `ExecStart` 换成它的位置。

- systemd 照 `StateDirectory=miyu`、`CacheDirectory=miyu` 建好 `/var/lib/miyu`、`/var/cache/miyu`，都属于 `miyu`。别的初始化系统（`init/` 下）先自己建好这两个目录、`chown miyu:miyu`、数据根 `chmod 0700`。
- **沙盒**：命令的沙盒要用户命名空间，systemd 的加固项不要写 `RestrictNamespaces=`，会拦了 bubblewrap，沙盒起不来。别的加固项（`PrivateUsers=`、`ProtectHome=` 这些）没在这里写，要加的先在自己的机器上试过沙盒还起得来。
- **本机的命令行**：在第二个本机入口（`run/public.sock`，施工 Y-1 下）做好以前，核心只认 `miyu` 这个用户自己连，本机要用命令行的 `sudo -u miyu env MIYU_HOME=/var/lib/miyu miyu …`。
- **网页软件**现在只听 `127.0.0.1`：从别的机器进来等「开网络」那一段（局域网、TLS），之前用 ssh 的端口转发。
- 日志：`journalctl -u miyu`，核心自己的运行日志在 `/var/lib/miyu/state/logs/`。备份、搬家照数据根 `/var/lib/miyu`。
