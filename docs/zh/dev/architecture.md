# 架构

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

这一页写给要接入 Miyu 或者给 Miyu 改代码的开发者，讲 Miyu 由哪些进程和仓库组成，一句话从输入到模型回答经过了哪些部件，以及你要修改、要接入的那一块在哪里。协议的每个方法可以查 [协议参考](../../reference/README.md)，写软件包可以看 [作者教程](../../authors/README.md)。

## 进程

核心是一个没有界面的进程，所有的状态都由它掌握。头（连接核心的界面程序，例如终端界面、网页界面）只负责显示和输入，通过协议和核心通信。各个进程之间的关系如下：

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

| 进程 | 程序 | 由谁启动 | 怎么和核心通信 |
|---|---|---|---|
| 核心 | `miyu core`（主程序的隐藏子命令） | 第一个连不上核心的头；或者由 `miyu service enable` 安装的系统服务启动 | 在本机套接字上等待连接 |
| 终端 | `miyu-tui` | 人运行 `miyu`（会按照配置 `ui.head` 换成对应界面包的程序）或者 `miyu tui` 时启动 | 本机套接字 |
| 命令行 | `miyu ask`、`miyu config` 这些子命令 | 人 | 本机套接字 |
| 网页服务 | `miyu-web serve` | `miyu web` 把命令转交给 `miyu-web open`，由它启动 | 本机套接字；浏览器连接它的 `/ws`，它把消息原样转给核心 |
| 托盘 | `miyu-applet` | 人手动启动，或者登录时自动启动 | 通过本机套接字查看状态、接收通知；打开网页界面时调用 `miyu web`，停止时先调用 `miyu web stop`，再调用 `miyu core stop` |
| 扩展 | 软件包的 `bin/<程序>` | 核心按照开关启动；核心退出时会请它们一起退出 | 标准输入输出，使用同一套协议 |
| MCP 服务 | 清单或配置里写的命令 | 核心 | 通过标准输入输出使用 MCP 通信；写成网址的服务不启动进程 |
| 沙盒助手 | `miyu-sandbox` | 核心，每条需要关起来的命令启动一个 | 不使用协议：它先按照核心写好的规格收紧自己的权限，再替换成那条命令 |
| 抽字 | `miyu-extract` | 核心，知识库抽取 PDF、网页的文字时，每一份文件启动一次 | 在标准输出交出一个 JSON 字符串数组，然后退出 |
| worker | 软件包 `[worker]` 里写的程序，例如 `miyu-embed` | 核心在需要时启动，空闲一段时间后自己退出 | 标准输入输出，一问一答 |
| 包加的命令 | 软件包 `[command]` 里写的程序 | 主程序：`miyu <名字>` 不是内置子命令时，原样转交给它 | 由软件包自己决定 |

进程之间有几条规则：

- 一个数据根只运行一个核心，由 `run/core.lock` 文件锁保证。头连不上核心时，先拿到 `run/spawn.lock`，再连接一次，还连不上才启动 `miyu core`，等核心在标准输出写出 `ready` 那一行之后再连接。
- 头启动核心时，使用它拿到的主程序路径：终端认 `MIYU_CORE_BIN`、`MIYU_PROGRAM`，网页认 `MIYU_PROGRAM`。主程序转交命令、核心启动扩展时，都会把 `MIYU_PROGRAM` 设成主程序的真实位置。
- 套接字的实际位置写在数据根的 `run/socket` 里，本机令牌在 `run/token` 里。头握手时要出示令牌；扩展通过标准输入输出握手，不需要出示凭据。
- 核心没有连接、没有正在进行的一轮对话，并且空闲时间达到 `core.idle_exit` 分钟时就会退出；默认值是 `0`，表示不退出。worker 在运行不算核心繁忙。
- 命令里的脚本通过 `miyu call` 调用会话的工具：运行命令的工具每次启动命令之前，会开一个只给这一次使用的端点，把地址和令牌放在 `MIYU_CALL_SOCK`、`MIYU_CALL_TOKEN` 里。

## 一轮对话的流程

一句话从发出到回答、再显示到界面上，依次经过下面这些步骤：

1. 头发送 `session.send`。协议端点（`miyu-endpoint`）在 `methods.rs` 里分发这个请求，`connection/rights.rs` 检查这个连接有没有权限调用它。
2. 会话 actor（`miyu-session`）接收请求：一个会话对应一个异步任务，人的命令和执行器的回报都进入它的收件箱，再一条一条送进内核。
3. 内核（`miyu-kernel`）是纯逻辑的状态机：送进一条输入，它交出一串动作（请求模型、调用工具、追加事件），自己不做任何 I/O。
4. 拼装请求：
   - 策略快照（`miyu-policy`）：创建会话时，按照人格、预设拼好 system、工具面和几个开关，按内容哈希存成 blob。每一轮开始时，如果人格、预设的文件或者软件包的工具变了，actor 会换上一份新的快照（`miyu-session` 的 `actor/persona.rs`）。
   - 事实：环境（时间、时区、工作目录）、权限级别、会话编号，由内核在它们变化时注入（`miyu-kernel` 的 `facts`）。
   - 一轮开始时的注入：记忆、知识库，以及扩展通过挂接点交来的文字（`miyu-session` 的 `actor/injects.rs`；询问扩展的那一路在 `miyu-endpoint` 的 `hooks.rs`）。
   - 组装（`miyu-assemble`）：稳定区（工具面、system、示范对话）排在最前，接着是最近一次压缩的摘要，然后是有效历史，排成一份统一的请求。
5. 发送请求：路由（`miyu-session` 的 `route.rs`）按照配置挑选供应商、key、模型，出错时换用候选；驱动（`miyu-drivers`）把统一的请求编码成各家接口要的字节；HTTP 执行器（`miyu-http`）把请求发出去并流式读回响应，边读边交给驱动解码；内核再把增量拼成完整的内容块。如果驱动是扩展提供的，核心会向扩展发送 `driver.call`。
6. 工具调用：执行之前，先经过权限策略（`miyu-session` 的 `guard.rs`；文件路径通过 `miyu-fs` 换成真实位置并检查边界）和扩展的守卫，需要询问人的就等待头的答复。执行时：
   - 内置工具登记在工具目录（`miyu-tool`）里，读写文件、运行命令的那几个工具在 `miyu-basesystem`，记忆、知识库、长期目标、定时任务各在自己的 crate 里；
   - 只读模式下的命令由 `miyu-session` 的 `sandbox.rs` 写好规格，再由 `miyu-sandbox` 包起来运行；
   - 扩展的工具由核心向扩展发送 `tool.call` 来调用；MCP 服务的工具通过 `miyu-mcp` 调用。
7. 写入磁盘：事件追加进会话日志（`miyu-store` 的 `log.rs`），一批事件写入一次、同步一次。只有写入磁盘之后，核心才回应命令、才推送给头。
8. 推送给头：`miyu-view` 把事件算成头可以直接绘制的条目，端点再把它们推送给订阅了 `view` 的连接（`view.add`、`view.update`、`view.append` 这些）；订阅了 `events` 的连接收到的是原样的事件。

## crate 地图

核心仓库的 workspace 成员是 `crates/*` 和 `xtask`。crate 分成六层，上层可以依赖下层，下层不许依赖上层；1、2 两层是纯逻辑，不接触文件、网络、子进程、环境变量、线程、时钟。

| crate | 管什么 | 层 |
|---|---|---|
| `miyu-kernel` | 事件、有效历史、统一的请求、会话状态机，以及给执行器实现的端口 | 1 内核 |
| `miyu-assemble` | 默认的请求组装：把有效历史排成统一的请求，也组装压缩、回顾、起标题用的请求 | 2 模块 |
| `miyu-config` | 配置清单：每一项只声明一次，分层合并出最终值，生成 JSON Schema；读取软件包清单 | 2 模块 |
| `miyu-drivers` | 模型驱动：在统一的请求和各家接口之间编码、解码，以及给错误分类 | 2 模块 |
| `miyu-models` | 供应商的配置项、档案、模型的写法、按照会话编号挑选 key，以及 models.dev 的目录 | 2 模块 |
| `miyu-policy` | 策略快照：一个会话发请求要用的全部文字和开关，按内容哈希存档 | 2 模块 |
| `miyu-recall` | 检索用的切词：中文、日文按相邻的两个字切分（bigram），拼出 FTS5 的查询 | 2 模块 |
| `miyu-seeds` | 从一张图里挑出主题色的候选，人格头像和壁纸共用 | 2 模块 |
| `miyu-view` | 视图投影：把事件算成头可以直接绘制的条目；把会话导出成 Markdown | 2 模块 |
| `miyu-store` | 数据根、会话日志、blob、配置文件、密钥、索引和检索库 | 3 执行器 |
| `miyu-http` | HTTP 执行器：用驱动编码好的字节发送请求，流式读回响应 | 3 执行器 |
| `miyu-log` | 运行日志：一行一条英文，按大小轮换 | 3 执行器 |
| `miyu-ipc` | 本机传输：套接字、命名管道、本机令牌、单实例锁，以及头连不上时启动核心 | 3 执行器 |
| `miyu-pipe` | 让 Windows 命名管道只对本人开放，并核对另一头的进程 | 3 执行器 |
| `miyu-call` | 脚本调用工具的通道：`miyu call` 连接上来的那一端 | 3 执行器 |
| `miyu-tool` | 工具的接口和工具目录 | 3 执行器 |
| `miyu-fs` | 文件工具怎样接触文件：边界表、换成真实位置、安全地打开、系统回收站 | 3 执行器 |
| `miyu-basesystem` | 基础系统：`read`、`glob`、`grep`、`write`、`edit`、`trash`、`shell` 这些工具 | 3 执行器 |
| `miyu-sandbox` | 沙盒：规格、助手 `miyu-sandbox`，以及探测这台机器能收紧到什么程度 | 3 执行器 |
| `miyu-mermaid` | 可选软件包 `mermaid`：把 mermaid 源码画成 SVG | 3 执行器 |
| `miyu-net` | 可选软件包 `net`：抓取网页生成链接卡片，以及工具 `web_fetch` | 3 执行器 |
| `miyu-memory` | 记忆的工具：`remember`、`forget`、`memory_search` | 3 执行器 |
| `miyu-goal` | 长期目标的工具 `goal` | 3 执行器 |
| `miyu-schedule` | 定时任务和循环的工具：`cron_*`、`schedule_next` | 3 执行器 |
| `miyu-client` | 给头和组件用的门面：连接核心、找到数据根、协议里的类型、给人看的文字 | 3 执行器 |
| `miyu-heap` | 进程的堆：在 glibc 上限制 arena、空闲时归还内存；内存记账 | 3 执行器 |
| `miyu-mcp` | MCP 的客户端：通过标准输入输出使用 MCP，列出工具、调用工具 | 3 执行器 |
| `miyu-knowledge` | 知识库的工具：`kb_search`、`kb_read`、`kb_save` | 3 执行器 |
| `miyu-extract` | 小程序 `miyu-extract`：抽取一份 PDF 或网页的文字 | 3 执行器 |
| `miyu-source` | 软件源：包文件的打包和解包、索引、签名、收录；程序 `miyu-index` | 3 执行器 |
| `miyu-wallpaper` | 读取桌面壁纸，只在头的进程里使用 | 3 执行器 |
| `miyu-session` | 会话 actor：把内核接到磁盘、模型、工具上；路由、权限、沙盒规格 | 4 核心 |
| `miyu-endpoint` | 协议端点：连接上的 JSON-RPC、会话表，以及对扩展和 MCP 的看管 | 4 核心 |
| `miyu-core` | 核心进程：建立骨架、读取配置、打开套接字、启动扩展、空闲退出 | 4 核心 |
| `miyu-cli` | 命令行的头：`miyu ask` 和其余的子命令 | 5 头 |
| `miyu` | 主程序：按照子命令分发 | 6 主程序 |
| `miyu-perf` | 量尺：启动 release 版的 `miyu`，测量启动、内存、请求 | 工具，谁都不依赖它 |
| `xtask` | 门禁和开发工具（`cargo xtask …`，见 [贡献代码](contributing.md)） | 工具，谁都不依赖它 |

`miyu-core` 的 cargo 开关 `mermaid`、`net` 默认是打开的，关掉它们就得到不带这两个可选软件包的核心。

## 写入磁盘的数据

所有数据都在数据根里，数据根默认是 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`；如果设置了 `MIYU_HOME`，就是它指向的目录。目录的完整说明见 [数据和备份](../guide/data.md)。

| 东西 | 在哪 | 格式在哪定 |
|---|---|---|
| 会话 | `~/.miyu/root/sessions/<会话编号>/`：事件日志，只追加，分段存成 JSONL | `miyu-kernel` 的 `event`；参考 [事件](../../reference/events.md) |
| 大内容 | `~/.miyu/root/blobs/`，按内容哈希存放 | `miyu-store` 的 `blob` |
| 配置 | `~/.miyu/etc/config.toml`；密钥在 `~/.miyu/etc/private/secrets.toml` | 各模块用 `miyu-config` 的 `settings!` 声明配置项，核心据此生成 JSON Schema，写到 `~/.miyu/var/cache/config/config.schema.json` |
| 软件包 | 安装的软件包在 `~/.miyu/opt/<编号>/`，软件包的状态在 `~/.miyu/var/lib/<编号>/` | [清单](../../authors/manifest.md) |
| 索引 | 会话列表的索引在 `~/.miyu/var/cache/index/`；用量汇总、检索库也是 SQLite | 都是派生出来的，删掉以后会根据日志重建 |
| 运行时 | `~/.miyu/run/`：锁、套接字的位置（套接字本身在哪里见下面的 [三个平台](#三个平台)）、本机令牌 | `miyu-ipc` |
| 运行日志 | `~/.miyu/var/log/` | `miyu-log` |

会话的状态都是事件日志的投影：会话列表、视图、用量都是从日志计算出来的。

## 协议

头和扩展使用同一套协议：JSON-RPC 2.0，一行一条消息，UTF-8 编码，一行最长 1 MiB。连接以后的第一条消息是 `hello`，带着协议版本 `[最低, 最高]`。协议版本 1 是冻结的：它的方法列在 [methods-v1.txt](../../reference/methods-v1.txt) 里，以后只增加，不修改也不删除；要删除方法、改名或者改变含义，只能升级到协议 2。核心主动发出的消息叫推送，推送没有 `id`，只发给订阅了那条流的连接：`view`（给界面绘制的条目和会话状态）、`events`（原样的事件）、`config`（配置变了、模型列表变了），另外还有会话列表、软件包、扩展、定时任务这些流。每个方法、每种推送的说明见 [协议参考](../../reference/README.md)。

## 软件包

一个文件夹就是一个软件包，文件夹名是软件包的编号，清单是 `package.toml`。核心从三层位置查找软件包：出厂的软件包在资源目录的 `packages/<编号>/`，系统包管理器安装的在系统加装的那几个位置，你自己安装的在数据根的 `opt/<编号>/`；如果同一个编号在几层里都有，你安装的优先于系统加装的，系统加装的优先于出厂的。扩展在清单的 `[process] capabilities` 里声明它需要的能力，人在安装、打开扩展的时候批准，批准过的记录在 `~/.miyu/etc/extensions.json` 里；出厂的软件包算作已经批准，但 `login.page`、`provider.key` 两项除外。核心启动扩展时，会给它 `MIYU_APP_DIR`、`MIYU_STATE_DIR` 这些目录；扩展发出的第一条消息是 `hello`，接着发送 `provide` 登记工具；标准输入读到结尾就表示核心请它退出，退出码 `1` 表示「重启也没用」。怎么写软件包见 [写一个 Miyu 软件包](../../authors/README.md)、[写扩展](../../authors/extensions.md)、[清单](../../authors/manifest.md)。

## 仓库

Miyu 的代码分在组织 `miyu-agent` 下的多个仓库里：

| 仓库 | 是什么 | 怎么使用核心 |
|---|---|---|
| `miyu` | 核心、主程序 `miyu`、命令行、协议参考和作者教程 | — |
| `miyu-tui` | 终端，程序是 `miyu-tui`，包编号是 `tui` | Cargo git 依赖 `miyu-client` |
| `miyu-web` | 网页服务 `miyu-web` 和页面，包编号是 `webui` | Cargo git 依赖 `miyu-client`、`miyu-ipc` |
| `miyu-desktop` | 托盘 `miyu-applet`，它的安装包把核心、网页一起打包进去 | Cargo git 依赖 `miyu-client`；安装包按照 `pins.toml` 取核心、网页的提交 |
| `miyu-im` | 通讯平台的桥，包编号是 `im`，它是一个扩展 | Cargo git 依赖 `miyu-client` 和几个内核、配置的 crate |
| `miyu-migrate` | 把旧版的数据迁移进来，包编号是 `migrate`，它增加了命令 `miyu migrate` | Cargo git 依赖 `miyu-client`、`miyu-kernel`、`miyu-store`，运行时调用协议 |
| `miyu-embed` | 内置语义模型，worker 包 `embed` | Cargo git 依赖 `miyu-recall` |
| `miyu-render` | 把 Markdown 画成图片，扩展包 `render` | Cargo git 依赖核心仓库 `third_party/` 里的 `mermaid-rs-renderer` |
| `miyu-ocr` | 在本机识别图片里的文字，扩展包 `ocr` | 通过 `miyu-sdk` |
| `miyu-memes` | 表情包，扩展包 `memes` | 只使用协议 |
| `miyu-sdk` | 写扩展用的开发包：Python 的 `miyu_ext.py`、Rust 的 `miyu-ext` | 只使用协议 |
| `miyu-package-template` | 软件包的模板仓库 | — |
| `packages` | 官方软件源：收录、签名、索引，以及打包工作流 `pack.yml` | 文件 `miyu-rev` 里写着一个核心的提交，软件源按照它编译 `miyu-index` 来收录、编译 `miyu` 来打包 |
| `litemiyu` | 单进程的精简 harness，`miyu rescue` 会启动它的应急模式 | 没有代码依赖 |
| `miyu-scripts-*`、`miyu-skills-*` | 一个仓库是一个脚本包或技能包，名单在 `packages` 的 `official.toml` 里 | 只是文件 |

这些仓库之间的依赖关系如下：

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

- 头和组件只通过门面 `miyu-client` 使用核心的东西；门面上有的就是可以用的，如果要修改或删除，就按照修改协议的规矩来办。`miyu-client` 的 `http`、`tool`、`wallpaper`、`view` 几个开关默认是关闭的，用到哪个才打开哪个。
- 各个仓库钉住的核心提交各不相同，升级的方法就是修改 `Cargo.toml` 里的 `rev`。
- 核心发版时，会按照 `heads.toml` 取终端、网页两个仓库钉住的那个提交，把编译好的程序放在 `miyu` 旁边，把包文件夹放进资源目录的 `packages/tui/`、`packages/webui/`。
- 软件包仓库发版时，都在 `release.yml` 里调用 `miyu-agent/packages/.github/workflows/pack.yml@main`：推送 `v<版本>` 标签以后，模板会按照安装软件包的规矩检查清单，打包成 `.miyu`，再上传到 Release。

## 许可证

各个仓库和文档使用的许可证如下：

- 核心仓库自己写的代码和资源使用 AGPL-3.0-or-later。终端、网页、托盘、im、migrate、embed、render、ocr、memes、litemiyu 也使用 AGPL-3.0-or-later。
- 核心仓库的 `docs/reference/`、`docs/authors/`、`resources/packages/base-authoring/` 使用 Apache-2.0（`LICENSES/Apache-2.0.txt`）；`miyu-sdk` 使用 Apache-2.0；`miyu-package-template` 使用 MIT。
- 只通过协议和 Miyu 通信的程序是独立的作品：扩展、软件包、脚本、MCP 服务、另外写的头，可以使用任何许可证。
- 不是仓库自己写的东西沿用各自原来的许可证，原文合并在各个仓库的 `THIRD-PARTY-LICENSES.txt` 里。

## 三个平台

三个平台做法不同的地方主要是下面这几处。新加的代码如果要运行命令、接触路径、启动子进程，三个平台都要考虑到；Windows 上的差别最多：没有 Unix 套接字，没有 `SIGTERM`，不设置权限位，AI 的命令在 PowerShell 里运行。

| | Linux | macOS（Apple 芯片） | Windows |
|---|---|---|---|
| 本机传输（`miyu-ipc`） | Unix 套接字：设置了 `$XDG_RUNTIME_DIR` 时在 `$XDG_RUNTIME_DIR/miyu-<指纹>/core.sock`，没有设置时在 `~/.miyu/run/core.sock` | Unix 套接字 `~/.miyu/run/core.sock` | 命名管道 `\\.\pipe\miyu-<指纹>`，只对本人开放；头连接以后会核对另一头的进程是不是同一个用户的（`miyu-pipe`） |
| 只读命令的沙盒（`miyu-sandbox`） | Landlock | Seatbelt | MXC 进程容器；没有桌面会话时拒绝每一条命令，退出码 `125` |
| 沙盒是否把读取也限制住（`miyu-sandbox` 的 `CONFINES_READS`） | 能；在 Landlock、seccomp 都管不住 Unix 套接字的机器上，通讯平台上别人的会话不运行命令 | 能 | 不能：在通讯平台上别人的会话（群聊、不是本人的私聊）里，命令一律拒绝（`miyu-session` 的 `guard.rs`） |
| 核心退出后，它启动的命令跟着停止 | 每个进程组一个看门狗（watchdog） | 每个进程组一个看门狗（watchdog） | 作业对象 |
| AI 运行命令用的 shell（`miyu-basesystem` 的 `shell/program.rs`） | bash：在 `PATH` 里查找，找不到时用 `/bin/bash` | zsh：固定是 `/bin/zsh` | PowerShell 7：先找资源目录的 `pwsh/pwsh.exe`，再找 `PATH` 里的 `pwsh.exe`；都没有时用 `%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe`（5.1） |
| 危险命令的识别（`shell/hazard.rs`，规则在 `miyu-policy` 的 `hazard`） | 识别 bash 的写法 | 识别 zsh 的写法 | 识别 PowerShell 和 cmd 的写法（`hazard/windows*`）；直接写入磁盘设备的那一类不识别 |
| 停止核心的信号（`miyu-core` 的 `serve.rs`） | `Ctrl+C`、`SIGTERM` | `Ctrl+C`、`SIGTERM` | 只有 `Ctrl+C`。`miyu core stop` 通过协议停止核心，三个平台一样 |
| 常驻（`miyu service enable`） | systemd 用户服务；没有 systemd 时写入 `~/.config/autostart/miyu.desktop` | launchd | 任务计划 |
| 缓存目录（`miyu-store` 的 `root.rs`，不随 `MIYU_HOME` 变化） | `$XDG_CACHE_HOME/miyu`，没有设置时是 `~/.cache/miyu` | `~/Library/Caches/Miyu` | `%LOCALAPPDATA%\Miyu\cache` |
| 系统加装的软件包（`miyu-store` 的 `packages/distro.rs`） | `$XDG_DATA_DIRS` 每一项下的 `miyu/packages/`，没有设置时按 `/usr/local/share:/usr/share` 查找 | Homebrew 前缀下的 `share/miyu/packages/`，前缀是 `miyu` 所在目录的上一级 | 没有这一层 |
| 密钥文件、新建的目录（`miyu-store`） | 密钥文件 `0600`，目录 `0700` | 密钥文件 `0600`，目录 `0700` | 不设置权限位，继承上一级的访问控制 |
| 堆（`miyu-heap`） | 在 glibc 上限制 arena、空闲时归还内存 | 系统的分配器 | 系统的分配器 |

在 Unix 上，套接字路径太长时会退到 `$TMPDIR/miyu-<uid>/<指纹>.sock`。指纹由数据根计算得出，所以不同数据根的核心不会互相冲突。
