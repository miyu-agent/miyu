## `miyu service`

### 是什么

让核心开机、登录就起（施工 Z-1 下，`docs/designs/12-进程形态与分发.md` 第二节「交给服务管理器」）：替本人装、卸、看本人这一层的服务。核心本来就一直开着（Z-1 上），这一条管的是没人打开头的时候它也起来，通讯平台的桥、定时任务不等人。系统级的（服务器上专门的用户 `miyu`、`/var/lib/miyu`）要 root、要建用户，不经它，照仓库 `packaging/README.md` 手装。不连核心。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `service`；换上帮助页 |
| `crates/miyu-cli/src/service.rs` | 参数、入口、要说的那一句和退出码 |
| `crates/miyu-cli/src/service/plan.rs` | 只算不做：照服务管理器、`miyu` 的位置、数据根算出写哪个文件、写什么、跑哪几条命令。文件的样子是仓库 `packaging/` 里那几份（`include_str!`，同一份），换上 `miyu` 的真实位置 |
| `crates/miyu-cli/src/service/flow.rs` | 装、卸、看怎么走 |
| `crates/miyu-cli/src/service/shape.rs` | `--format json` 的形状（施工 Z-1 下补） |
| `crates/miyu-cli/src/service/machine.rs` | 碰这台机器：认服务管理器、家目录、`miyu` 的位置、`MIYU_HOME`，写文件、跑命令；测试里换替身 |
| `crates/miyu-cli/src/language/service.rs`、`help/{zh,en}/service.txt` | 给人看的字、帮助页 |

### 对外的样子

| 命令 | 做什么 |
|---|---|
| `enable` | 写好服务、交给服务管理器。已经装过的照现在的 `miyu` 位置、数据根重写一遍 |
| `disable` | 停掉、从服务管理器里拿掉、删掉写的文件；没装的说没装，退出码 0 |
| `status` | 装没装、服务管理器那边开没开着 |

| 平台 | 写在哪 | 跑什么 |
|---|---|---|
| Linux，有 systemd 用户会话（`systemctl --user show-environment` 答得上） | `$XDG_CONFIG_HOME/systemd/user/miyu.service`（没设的 `~/.config`） | `systemctl --user daemon-reload`、`enable --now miyu.service`；卸 `disable --now`；看 `is-active` |
| Linux，没有的桌面 | `$XDG_CONFIG_HOME/autostart/miyu.desktop` | 不跑：下次登录桌面时起；看只看文件在不在 |
| macOS | `~/Library/LaunchAgents/io.github.miyu-agent.core.plist` | 先 `launchctl bootout` 旧的（没有的不算错），再 `bootstrap gui/<uid>`；卸 `bootout`；看 `launchctl print` |
| Windows | `%USERPROFILE%\AppData\Local\Miyu\miyu-task.xml` | `schtasks /Create /TN Miyu /XML … /F`（登录触发带 `UserId`，只在这个人登录时起，不要管理员）；卸 `/Delete`；看 `/Query` |

- 跑的是这一个 `miyu` 的真实位置（`current_exe` 解开链接），参数 `core`。路径照各自的写法转义：systemd 单元里有空白、引号的加双引号，`%` 写两个；`.desktop` 的 `Exec` 照 XDG 桌面条目规范；plist、任务的 XML 照 XML。
- 设了 `MIYU_HOME` 的一并写上（写成绝对路径）：systemd 是 `Environment=`，自启动是 `env MIYU_HOME=…`，launchd 是 `EnvironmentVariables`。Windows 的任务计划带不了环境变量：设了的装不了，说一句，退出码 1。
- 没人登录也要开着的（Linux）：说一句 `sudo loginctl enable-linger`，不替人跑（要 root）。
- 头先拉起了一个核心的，服务管理器起的那一个照握手说 `running`、退出码 0 就走（Z-1 上）。

### 机读格式

`--format json`（施工 Z-1 下补，终端要照它画「开机启动」开关）：三个命令都有，一行 JSON，成没成都印在标准输出上，退出码照旧。字段名稳定、不随界面语言变，给人看的那句不进 JSON。

| 结果 | 形状 |
|---|---|
| 成了（`enable`、`disable`、没装时的 `disable`、`status`） | `{"installed": bool, "on": bool \| null, "file": "<写的那个文件>", "manager": "systemd" \| "autostart" \| "launchd" \| "task"}` |
| 装不了 | `{"error": "data_root_on_windows"}`、`{"error": "no_home"}` |
| 某一步没成 | `{"error": "failed", "step": "<哪一步，命令或文件>", "said": "<系统的原话>"}` |

- `on` 是服务管理器那边开没开着：systemd 是在跑，launchd、任务计划是登记着；自启动问不了，是 `null`。没装的是 `false`（自启动的照旧 `null`）。
- `enable` 成了的 `installed` 是 `true`、`on` 是 `true`（自启动 `null`）；`disable` 成了的、本来没装的 `installed`、`on` 都是 `false`。

### 退出码、样子

成了（连同没装时卸、看）印一句在标准输出上，退出码 0；装不了、没成的印在标准错误上，退出码 1，没成的带是哪一步、系统的原话。例子：

```text
$ miyu service enable
装好了：/home/alice/.config/systemd/user/miyu.service，现在起了，以后登录就起（没人登录也要开着的：sudo loginctl enable-linger）。
$ miyu service status
装了，开着：/home/alice/.config/systemd/user/miyu.service
$ miyu service disable
卸掉了：停了核心的服务，删了 /home/alice/.config/systemd/user/miyu.service。
```

### 测试

| 测试 | 守什么 |
|---|---|
| `service/plan/tests.rs` | 四种管理器写在哪、写什么（位置换上、转义、数据根）、跑哪几条命令；Windows 设了数据根的装不了 |
| `service/shape/tests.rs` | 机读格式：成了的都是那四格、自启动的 `on` 是 `null`、没装的是没开；没成的带代码、哪一步、原话 |
| `service/tests.rs` | 经替身机器：装是先写再跑、跑错了说哪一步；卸是先停再删、没装的不跑命令；launchd 先拿掉旧的；自启动不跑命令；不知道家目录的不装 |
