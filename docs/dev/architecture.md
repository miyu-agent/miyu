# 架构

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

给要接入 Miyu 或给它改代码的开发者：Miyu 由哪些进程和仓库组成，一句话从敲下去到模型回答走过哪些部件，你要改、要接的那一块在哪。协议的每个方法查 [协议参考](../reference/README.md)，写软件包查 [作者教程](../authors/README.md)。

## 进程

核心是一个没有界面的进程，所有状态都在它手里。头只管显示和输入，经协议和核心说话。

```text
     人敲 miyu、miyu tui        miyu ask …        浏览器          托盘
            │                      │               │ HTTP、WebSocket /ws
         miyu-tui                  │         miyu-web serve      miyu-applet
            │                      │               │                 │
            └──────────────────────┴───────┬───────┴─────────────────┘
                              本机套接字（Windows：命名管道）
                                一行一条 JSON-RPC 2.0
                                           │
                                   miyu core（核心）
      ┌──────────────┬───────────────┬─────┴─────────┬────────────────┐
   扩展进程       MCP 服务       miyu-sandbox     miyu-extract      miyu-embed
  标准输入输出   标准输入输出    包住一条命令     抽一份文件的字    worker，标准输入输出
```

| 进程 | 程序 | 谁拉起 | 怎么和核心说话 |
|---|---|---|---|
| 核心 | `miyu core`（主程序的隐藏子命令） | 第一个连不上核心的头；或 `miyu service enable` 装的系统服务 | 在本机套接字上等连接 |
| 终端 | `miyu-tui` | 人敲 `miyu`（照配置 `ui.head` 换成那个界面包的程序）或 `miyu tui` | 本机套接字 |
| 命令行 | `miyu ask`、`miyu config` 这些子命令 | 人 | 本机套接字 |
| 网页服务 | `miyu-web serve` | `miyu web` 转交给 `miyu-web open`，由它拉起 | 本机套接字；浏览器连它的 `/ws`，它原样转给核心 |
| 托盘 | `miyu-applet` | 人、登录时启动 | 本机套接字看状态、收通知；打开网页、停核心调 `miyu web`、`miyu core stop` |
| 扩展 | 软件包的 `bin/<程序>` | 核心，照开关；核心退出时一起请它们退出 | 标准输入输出，同一套协议 |
| MCP 服务 | 清单或配置写的命令 | 核心 | 标准输入输出说 MCP；写网址的服务不起进程 |
| 沙盒助手 | `miyu-sandbox` | 核心，每条要关起来的命令一个 | 不说协议：照核心写的规格先收紧自己，再换成那条命令 |
| 抽字 | `miyu-extract` | 核心，知识库抽 PDF、网页的字时一份一次 | 标准输出交一个 JSON 字符串数组就退 |
| worker | 软件包 `[worker]` 写的程序，例如 `miyu-embed` | 核心，要用时拉起，空闲一阵退出 | 标准输入输出，一条一条问 |
| 包加的命令 | 软件包 `[command]` 写的程序 | 主程序：`miyu <名字>` 不是内置子命令时原样转交 | 由包自己定 |

几条规矩：

- 一个数据根只跑一个核心，靠 `run/core.lock` 文件锁。头连不上时拿 `run/spawn.lock`，再连一次，还不行才拉起 `miyu core`，等它在标准输出写来 `ready` 那一行再连。
- 头拉起核心照它拿到的主程序路径：终端认 `MIYU_CORE_BIN`、`MIYU_PROGRAM`，网页认 `MIYU_PROGRAM`。主程序转交命令、核心拉起扩展时都把 `MIYU_PROGRAM` 设成主程序的真实位置。
- 套接字的实际位置写在数据根的 `run/socket`，本机令牌在 `run/token`。头握手时出示令牌；扩展经标准输入输出握手，不用出示凭据。
- 核心没有连接、没有在跑的一轮、空闲够 `core.idle_exit` 分钟就退出；默认 `0`，不退。worker 不算核心忙。
- 命令里的脚本经 `miyu call` 调会话的工具：跑命令的工具每次起命令前开一个只给这一次的端点，地址和令牌放在 `MIYU_CALL_SOCK`、`MIYU_CALL_TOKEN` 里。

## 一轮对话怎么走

1. 头发 `session.send`。协议端点（`miyu-endpoint`）在 `methods.rs` 分发，`connection/rights.rs` 查这个连接能不能调。
2. 会话 actor（`miyu-session`）收下：一个会话一个异步任务，人的命令、执行器的回报都进它的收件箱，一条一条送进内核。
3. 内核（`miyu-kernel`）是纯逻辑的状态机：送进一条输入，交出一串动作（请求模型、调工具、追加事件），自己不做 I/O。
4. 拼请求：
   - 策略快照（`miyu-policy`）：造会话时照人格、预设拼好的 system、工具面和几样开关，按内容哈希存成 blob。每一轮开始时，人格、预设的文件或包的工具变了，actor 换一份新快照（`miyu-session` 的 `actor/persona.rs`）。
   - 事实：环境（时间、时区、工作目录）、权限级别、会话编号，由内核在变了的时候注入（`miyu-kernel` 的 `facts`）。
   - 一轮开始时的注入：记忆、知识库，和扩展经挂接点交来的字（`miyu-session` 的 `actor/injects.rs`；问扩展的那一路在 `miyu-endpoint` 的 `hooks.rs`）。
   - 组装（`miyu-assemble`）：稳定区（工具面、system、示范对话）在前，接着最近一次压缩的摘要，再是有效历史，排成一份统一的请求。
5. 发请求：路由（`miyu-session` 的 `route.rs`）照配置挑供应商、key、模型，出错换候选；驱动（`miyu-drivers`）把统一的请求编码成各家接口的字节；HTTP 执行器（`miyu-http`）发出去、流式读回来，边读边交给驱动解码；内核把增量拼成完整的内容块。扩展提供的驱动由核心发 `driver.call` 给扩展。
6. 工具调用：执行前先过权限策略（`miyu-session` 的 `guard.rs`；文件路径经 `miyu-fs` 换成真实位置、查边界）和扩展的守卫，要问人的等头答复。执行：
   - 内置工具登记在工具目录（`miyu-tool`），读写文件、跑命令那几件在 `miyu-basesystem`，记忆、知识库、长期目标、定时任务各在自己的 crate；
   - 只读下的命令由 `miyu-session` 的 `sandbox.rs` 写规格，经 `miyu-sandbox` 包起来跑；
   - 扩展的工具由核心发 `tool.call` 给扩展；MCP 服务的工具经 `miyu-mcp` 调。
7. 落盘：事件追加进会话日志（`miyu-store` 的 `log.rs`），一批一次写入、一次同步。落了盘才回应命令、才推给头。
8. 推给头：`miyu-view` 把事件算成头直接画的条目，端点推给订了 `view` 的连接（`view.add`、`view.update`、`view.append` 这些）；订了 `events` 的收原样的事件。

## crate 地图

核心仓库的 workspace 成员是 `crates/*` 和 `xtask`。crate 分六层，上层能依赖下层，下层不许依赖上层；1、2 两层是纯逻辑，不碰文件、网络、子进程、环境变量、线程、时钟。

| crate | 管什么 | 层 |
|---|---|---|
| `miyu-kernel` | 事件、有效历史、统一的请求、会话状态机、给执行器实现的端口 | 1 内核 |
| `miyu-assemble` | 默认的请求组装：有效历史排成统一的请求，也组装压缩、回顾、起标题的请求 | 2 模块 |
| `miyu-config` | 配置清单：每项声明一次，分层合出最终值，生成 JSON Schema；读软件包清单 | 2 模块 |
| `miyu-drivers` | 模型驱动：统一的请求和各家接口之间的编码、解码、出错分类 | 2 模块 |
| `miyu-models` | 供应商的配置项、档案、模型的写法、照会话编号挑 key，models.dev 的目录 | 2 模块 |
| `miyu-policy` | 策略快照：一个会话发请求要用的全部字和开关，按内容哈希存档 | 2 模块 |
| `miyu-recall` | 检索的切词：中文、日文两两切，拼 FTS5 的查询 | 2 模块 |
| `miyu-seeds` | 从一张图挑主题色的候选：人格头像、壁纸共用 | 2 模块 |
| `miyu-view` | 视图投影：事件算成头直接画的条目；会话导出成 Markdown | 2 模块 |
| `miyu-store` | 数据根、会话日志、blob、配置文件、密钥、索引和检索库 | 3 执行器 |
| `miyu-http` | HTTP 执行器：照驱动编码好的字节发请求，流式读回 | 3 执行器 |
| `miyu-log` | 运行日志：一行一条的英文，按大小轮换 | 3 执行器 |
| `miyu-ipc` | 本机传输：套接字、命名管道，本机令牌，单实例锁，头连不上时拉起核心 | 3 执行器 |
| `miyu-pipe` | Windows 命名管道只对本人开放、核对另一头的进程 | 3 执行器 |
| `miyu-call` | 脚本调工具的通道：`miyu call` 连上来的那一头 | 3 执行器 |
| `miyu-tool` | 工具的接口和工具目录 | 3 执行器 |
| `miyu-fs` | 文件工具怎么碰文件：边界表、换成真实位置、安全地打开、系统回收站 | 3 执行器 |
| `miyu-basesystem` | 基础系统：`read`、`glob`、`grep`、`write`、`edit`、`trash`、`shell` 这些工具 | 3 执行器 |
| `miyu-sandbox` | 沙盒：规格、助手 `miyu-sandbox`、探测这台机器能收紧到什么程度 | 3 执行器 |
| `miyu-mermaid` | 可选软件包 `mermaid`：mermaid 源码画成 SVG | 3 执行器 |
| `miyu-net` | 可选软件包 `net`：抓网页做链接卡片、工具 `web_fetch` | 3 执行器 |
| `miyu-memory` | 记忆的工具：`remember`、`forget`、`memory_search` | 3 执行器 |
| `miyu-goal` | 长期目标的工具 `goal` | 3 执行器 |
| `miyu-schedule` | 定时任务、循环的工具：`cron_*`、`schedule_wakeup`、`end_life` | 3 执行器 |
| `miyu-client` | 给头和组件用的门面：连核心、找数据根、协议里的类型、给人看的字 | 3 执行器 |
| `miyu-heap` | 进程的堆：glibc 上限制 arena、空闲时还内存；内存记账 | 3 执行器 |
| `miyu-mcp` | MCP 的客户端：经标准输入输出说 MCP，列工具、调工具 | 3 执行器 |
| `miyu-knowledge` | 知识库的工具：`kb_search`、`kb_read`、`kb_save` | 3 执行器 |
| `miyu-extract` | 小程序 `miyu-extract`：抽一份 PDF 或网页的字 | 3 执行器 |
| `miyu-source` | 软件源：包文件的打包解包、索引、签名、收录；程序 `miyu-index` | 3 执行器 |
| `miyu-wallpaper` | 读桌面壁纸，只在头的进程里用 | 3 执行器 |
| `miyu-session` | 会话 actor：把内核接上磁盘、模型、工具；路由、权限、沙盒规格 | 4 核心 |
| `miyu-endpoint` | 协议端点：连接上的 JSON-RPC、会话表、扩展和 MCP 的看管 | 4 核心 |
| `miyu-core` | 核心进程：建骨架、读配置、开套接字、拉起扩展、空闲退出 | 4 核心 |
| `miyu-cli` | 命令行的头：`miyu ask` 和其余子命令 | 5 头 |
| `miyu` | 主程序：按子命令分发 | 6 主程序 |
| `miyu-perf` | 量尺：拉起 release 的 `miyu` 量启动、内存、请求 | 工具，谁都不依赖它 |
| `xtask` | 门禁和开发工具（`cargo xtask …`，见 [贡献代码](contributing.md)） | 工具，谁都不依赖它 |

`miyu-core` 的 cargo 开关 `mermaid`、`net` 默认开着，关掉就是不带这两个可选软件包的核心。

## 落盘的东西

都在数据根里，默认 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`；设了 `MIYU_HOME` 的是它。目录的完整说明见 [数据和备份](../guide/data.md)。

| 东西 | 在哪 | 格式在哪定 |
|---|---|---|
| 会话 | `~/.miyu/root/sessions/<会话编号>/`：事件日志，只追加，按段存成 JSONL | `miyu-kernel` 的 `event`；参考 [事件](../reference/events.md) |
| 大内容 | `~/.miyu/root/blobs/`，按内容哈希存 | `miyu-store` 的 `blob` |
| 配置 | `~/.miyu/etc/config.toml`；密钥 `~/.miyu/etc/private/secrets.toml` | 各模块用 `miyu-config` 的 `settings!` 声明，核心照它生成 JSON Schema 到 `~/.miyu/var/cache/config/config.schema.json` |
| 软件包 | 装上的在 `~/.miyu/opt/<编号>/`，包的状态在 `~/.miyu/var/lib/<编号>/` | [清单](../authors/manifest.md) |
| 索引 | 会话列表的索引在 `~/.miyu/var/cache/index/`；用量汇总、检索库也是 SQLite | 都是派生的，删了照日志重建 |
| 运行时 | `~/.miyu/run/`：锁、套接字的位置（套接字本身在哪见下面 [三个平台](#三个平台)）、本机令牌 | `miyu-ipc` |
| 运行日志 | `~/.miyu/var/log/` | `miyu-log` |

会话的状态都是事件日志的投影：会话列表、视图、用量都从日志算出来。

## 协议

头和扩展说同一套协议：JSON-RPC 2.0，一行一条，UTF-8，一行最长 1 MiB。连上以后第一条是 `hello`，带协议版本 `[最低, 最高]`。协议版本 1 是冻结的：方法列在 [methods-v1.txt](../reference/methods-v1.txt)，以后只加不改不删，删、改名、改意思的只能升到协议 2。核心主动发的是推送，没有 `id`，只发给订了那条流的连接：`view`（给界面画的条目和会话状态）、`events`（原样的事件）、`config`（配置变了、模型列表变了），另有会话列表、软件包、扩展、定时任务这些流。每个方法、每种推送见 [协议参考](../reference/README.md)。

## 软件包

一个文件夹就是一个软件包，文件夹名是编号，清单是 `package.toml`。核心从三层找包：出厂的在资源目录的 `packages/<编号>/`，系统包管理器装的在系统加装那几处，你装的在数据根的 `opt/<编号>/`；同一个编号几层都有的，装上的盖过系统加装的，系统加装的盖过出厂的。扩展在清单的 `[process] capabilities` 里声明要的能力，人在装、打开的时候批，批过的记在 `~/.miyu/etc/extensions.json`；出厂的包算批过，`login.page`、`provider.key` 两项除外。核心拉起扩展时给它 `MIYU_APP_DIR`、`MIYU_STATE_DIR` 这些目录，扩展第一条发 `hello`，再发 `provide` 登记工具；标准输入读到头就是请它退出，退出码 `1` 是「重启也没用」。怎么写见 [写一个 Miyu 软件包](../authors/README.md)、[写扩展](../authors/extensions.md)、[清单](../authors/manifest.md)。

## 仓库

组织 `miyu-agent` 下的仓库：

| 仓库 | 是什么 | 怎么用核心 |
|---|---|---|
| `miyu` | 核心、主程序 `miyu`、命令行、协议参考和作者教程 | — |
| `miyu-tui` | 终端，程序 `miyu-tui`，包编号 `tui` | Cargo git 依赖 `miyu-client` |
| `miyu-web` | 网页服务 `miyu-web` 和页面，包编号 `webui` | Cargo git 依赖 `miyu-client`、`miyu-ipc` |
| `miyu-desktop` | 托盘 `miyu-applet`，安装包把核心、网页一起打进去 | Cargo git 依赖 `miyu-client`；安装包照 `pins.toml` 取核心、网页的提交 |
| `miyu-im` | 通讯平台的桥，包编号 `im`，是一个扩展 | Cargo git 依赖 `miyu-client` 和几样内核、配置的 crate |
| `miyu-migrate` | 把旧版数据迁进来，包编号 `migrate`，加命令 `miyu migrate` | Cargo git 依赖 `miyu-client`、`miyu-kernel`、`miyu-store`，运行时调协议 |
| `miyu-embed` | 内置语义模型，worker 包 `embed` | Cargo git 依赖 `miyu-recall` |
| `miyu-render` | 把 Markdown 画成图，扩展包 `render` | Cargo git 依赖核心仓库 `third_party/` 里的 `mermaid-rs-renderer` |
| `miyu-ocr` | 本机识别图片里的字，扩展包 `ocr` | 经 `miyu-sdk` |
| `miyu-memes` | 表情包，扩展包 `memes` | 只说协议 |
| `miyu-sdk` | 写扩展的开发包：Python 的 `miyu_ext.py`、Rust 的 `miyu-ext` | 只说协议 |
| `miyu-package-template` | 软件包的模板仓库 | — |
| `packages` | 官方软件源：收录、签名、索引，打包工作流 `pack.yml` | 文件 `miyu-rev` 写一个核心的提交，照它编 `miyu-index` 收录、编 `miyu` 打包 |
| `litemiyu` | 单进程的精简 harness，`miyu rescue` 拉起它的应急模式 | 没有代码依赖 |
| `miyu-scripts-*`、`miyu-skills-*` | 一个仓库一个脚本包或技能包，名单在 `packages` 的 `official.toml` | 只是文件 |

```text
miyu（核心）
 ├─ Cargo git 依赖，钉核心的一个提交：
 │    miyu-tui、miyu-web、miyu-desktop ──────── miyu-client
 │    miyu-im、miyu-migrate ─────────────────── miyu-client 加几样核心的 crate
 │    miyu-embed ────────────────────────────── miyu-recall
 │    miyu-render ───────────────────────────── third_party/mermaid-rs-renderer
 │    packages（miyu-rev）────────────────────── miyu-index、miyu
 └─ heads.toml 钉 miyu-tui、miyu-web 的提交 ──── 发版时编进一体包
miyu-sdk ──── miyu-ocr
只说协议：miyu-memes、脚本包、技能包
各包发版 ──── packages/.github/workflows/pack.yml
```

- 头和组件只经门面 `miyu-client` 用核心的东西；门面上有的就是能用的，改了、少了照改协议的规矩办。`miyu-client` 的 `http`、`tool`、`wallpaper`、`view` 几个开关默认关，用到的才开。
- 各仓库钉的核心提交各不相同，升级就是改 `Cargo.toml` 里的 `rev`。
- 核心发版时照 `heads.toml` 取终端、网页两个仓库钉住的那一份，编好程序放在 `miyu` 旁边，包文件夹放进资源目录的 `packages/tui/`、`packages/webui/`。
- 软件包仓库发版都在 `release.yml` 里调 `miyu-agent/packages/.github/workflows/pack.yml@main`：推 `v<版本>` 标签，模板照装包的规矩查清单、打成 `.miyu`、传进 Release。

## 许可证

- 核心仓库自己写的代码和资源用 AGPL-3.0-or-later。终端、网页、托盘、im、migrate、embed、render、ocr、memes、litemiyu 也是 AGPL-3.0-or-later。
- 核心仓库的 `docs/reference/`、`docs/authors/`、`resources/packages/base-authoring/` 用 Apache-2.0（`LICENSES/Apache-2.0.txt`）；`miyu-sdk` 用 Apache-2.0；`miyu-package-template` 用 MIT。
- 只经协议和 Miyu 说话的程序是独立的作品：扩展、软件包、脚本、MCP 服务、另写的头，用什么许可证都行。
- 不是仓库自己写的东西照各自原来的许可证，原文合在各仓库的 `THIRD-PARTY-LICENSES.txt`。

## 三个平台

三个平台做法不同的主要在下面这几处。新加跑命令、碰路径、起子进程的代码，三个平台都要想到；Windows 上的差别最多：没有 Unix 套接字、没有 `SIGTERM`、不设权限位，AI 的命令跑在 PowerShell 里。

| | Linux | macOS（Apple 芯片） | Windows |
|---|---|---|---|
| 本机传输（`miyu-ipc`） | Unix 套接字：设了 `$XDG_RUNTIME_DIR` 的在 `$XDG_RUNTIME_DIR/miyu-<指纹>/core.sock`，没设的在 `~/.miyu/run/core.sock` | Unix 套接字 `~/.miyu/run/core.sock` | 命名管道 `\\.\pipe\miyu-<指纹>`，只对本人开放；头连上以后核对另一头的进程是自己这个用户的（`miyu-pipe`） |
| 只读命令的沙盒（`miyu-sandbox`） | Landlock | Seatbelt | MXC 进程容器；没有桌面会话时每条命令拒绝，退出码 `125` |
| 沙盒把读也关住（`miyu-sandbox` 的 `CONFINES_READS`） | 能；Landlock、seccomp 都管不住 Unix 套接字的机器上，通讯平台上别人的会话不跑命令 | 能 | 不能：通讯平台上别人的会话（群、不是本人的私聊）里命令一律拒（`miyu-session` 的 `guard.rs`） |
| 核心没了，起的命令跟着停 | 每个进程组一个看门的 | 每个进程组一个看门的 | 作业对象 |
| AI 跑命令的 shell（`miyu-basesystem` 的 `shell/program.rs`） | bash：`PATH` 里找，找不到用 `/bin/bash` | zsh：固定 `/bin/zsh` | PowerShell 7：先找资源目录的 `pwsh/pwsh.exe`，再找 `PATH` 里的 `pwsh.exe`；都没有的用 `%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe`（5.1） |
| 危险命令的识别（`shell/hazard.rs`） | 认 bash 的写法 | 认 zsh 的写法 | 不查：只认 bash、zsh 的写法，PowerShell 的命令一条都不认 |
| 停核心的信号（`miyu-core` 的 `serve.rs`） | `Ctrl+C`、`SIGTERM` | `Ctrl+C`、`SIGTERM` | 只有 `Ctrl+C`。`miyu core stop` 走协议，三个平台一样 |
| 常驻（`miyu service enable`） | systemd 用户服务；没有的写 `~/.config/autostart/miyu.desktop` | launchd | 任务计划 |
| 缓存目录（`miyu-store` 的 `root.rs`，不跟 `MIYU_HOME` 变） | `$XDG_CACHE_HOME/miyu`，没设的 `~/.cache/miyu` | `~/Library/Caches/Miyu` | `%LOCALAPPDATA%\Miyu\cache` |
| 系统加装的软件包（`miyu-store` 的 `packages/distro.rs`） | `$XDG_DATA_DIRS` 每一处的 `miyu/packages/`，没设的照 `/usr/local/share:/usr/share` | Homebrew 前缀下的 `share/miyu/packages/`，前缀是 `miyu` 所在目录的上一级 | 没有这一层 |
| 密钥文件、新建的目录（`miyu-store`） | 密钥文件 `0600`，目录 `0700` | 密钥文件 `0600`，目录 `0700` | 不设权限位，照上一级继承的访问控制 |
| 堆（`miyu-heap`） | glibc 上限制 arena、空闲时还内存 | 系统的分配器 | 系统的分配器 |

Unix 上套接字路径太长时退到 `$TMPDIR/miyu-<uid>/<指纹>.sock`。指纹从数据根算，几个数据根的核心互不相撞。
