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

命令行版的压缩包由 `cargo xtask dist` 打（施工 RL-1，设计 `59-发版.md` 第三节第 1 条）：本机打本机那一种
`miyu-<版本>-<系统>.tar.gz`（Windows 是 `.zip`），照 `heads.toml` 取两个头一起编，里面是五个程序、`resources/`（带两个头的包文件夹）、
`LICENSE`、合了核心和两个头的 `THIRD-PARTY-LICENSES.txt`、`README.txt`。放在 cargo 的 target 目录下的 `dist/`，`--out` 换地方。`cargo xtask dist-check <压缩包>` 发布前查包（施工 RL-2）；推 `v<版本>`
标签时 `.github/workflows/release.yml` 在四种机器上打包、查包，写 `SHA256SUMS`、用发版钥匙签、加构建出处证明，建草稿 Release。

Linux 两种和 macOS 上 `cargo xtask dist` 另打三份给系统源（AUR、Homebrew）分包用（施工 SD-2，设计 `60-系统源分包.md`），Windows 不打：

- `miyu-core-<版本>-<系统>.tar.gz`：一层同名目录，`miyu`、`miyu-sandbox`、`miyu-extract`、只有核心的 `resources/`（不带两个头）、`LICENSE`、
  核心的 `THIRD-PARTY-LICENSES.txt`。整层放进 `/usr/lib/miyu/`（Homebrew 是 `libexec/`），`miyu` 链进 `bin/`。
- `miyu-tui-<终端版本>-<系统>.tar.gz`、`miyu-webui-<网页版本>-<系统>.tar.gz`：里面直接是包文件夹，程序在它的 `bin/`。原样拷进
  `share/miyu/packages/`（`/usr/share/miyu/packages/miyu-tui/`），核心照系统加装那一层认它。版本照头自己的清单。

`dist-check` 也认这三种：头那一份要同一目录里有核心那一份，照系统加装层放好问核心认不认得出。

安装脚本（施工 RL-3）：`install.sh`（Linux、macOS）、`install.ps1`（Windows）从 GitHub Releases 下载这一种的压缩包，核签名（openssl 认
Ed25519 的才核得了，不认的说一句）、核 SHA-256、跑一次新的 `miyu --version`，装进 `~/.local/lib/miyu/`（链到 `~/.local/bin/miyu`）或者
`%LOCALAPPDATA%\Programs\Miyu\`（加进用户的 PATH），旁边写 `install.toml`（`channel = "script"`，`miyu upgrade` 照它自己升级）。参数：
`--prerelease`、`--version <版本>`、`--uninstall`（Windows 是 `-Prerelease`、`-Version`、`-Uninstall`）。`test-install.sh`、`test-install.ps1`
在干净的机器上装、核、卸一遍（`.github/workflows/install-test.yml`），别在自己的家目录跑。

系统源（施工 RL-5，设计 `60-系统源分包.md`）：`aur/` 下 `miyu-core`、`miyu-tui`、`miyu-webui`、`miyu-agent`（合集）的 `PKGBUILD.in` 和
`miyu-agent-git` 的 `PKGBUILD`，`homebrew/` 下四个 formula 模板。发版那天照这一次发布的 `SHA256SUMS` 填好再推：
`cargo xtask packaging v<版本> --sums <SHA256SUMS> --out <目录>`；托盘 `miyu-applet` 的 PKGBUILD、cask 照托盘仓库自己的发布填，加
`--applet v<托盘版本> --applet-sums <托盘的 SHA256SUMS>`（施工 RL-5补）。核心进 `/usr/lib/miyu/`（Homebrew 的 `libexec/`），终端、网页进
`share/miyu/packages/`。`test-aur.sh`、`test-brew.sh` 是实测（`.github/workflows/packaging-test.yml`）。步骤见施工单 RL-5「发版那天」。

每个安装包的根目录都放仓库根上的 `LICENSE`（AGPL-3.0 全文）和 `THIRD-PARTY-LICENSES.txt`（编进程序的第三方包的许可证原文，`cargo xtask notices` 生成，蓝图 `licenses.md`「第三方许可证清单」）。带了头的，头的那一份也放进去（施工 AU-2）。

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
- **沙盒**：命令的沙盒在 Linux 上用 Landlock，不要用户命名空间。systemd 的加固项（`PrivateUsers=`、`ProtectHome=` 这些）没在这里写，要加的先在自己的机器上试过沙盒还起得来。
- **本机的命令行**：在第二个本机入口（`run/public.sock`，施工 Y-1 下）做好以前，核心只认 `miyu` 这个用户自己连，本机要用命令行的 `sudo -u miyu env MIYU_HOME=/var/lib/miyu miyu …`。
- **网页软件**现在只听 `127.0.0.1`：从别的机器进来等「开网络」那一段（局域网、TLS），之前用 ssh 的端口转发。
- 日志：`journalctl -u miyu`，核心自己的运行日志在 `/var/lib/miyu/state/logs/`。备份、搬家照数据根 `/var/lib/miyu`。
