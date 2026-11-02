# FH 盘点：核心仓库以外对数据根路径的依赖（2026-11-02）

设计 `55-数据根仿FHS.md`、施工单 `FH-数据根仿FHS.md` 的底稿。

查的时间：2026-11-02。版本：各仓库 `origin/main`（miyu-tui 63cd61f0、miyu-web 99d22287、miyu-desktop b94ffd8、miyu-im bda568a、miyu-memes 1c18283、miyu-memes-shorin-collection 4571e1c、miyu-embed 8d091b3、miyu-render b1d33da、miyu-sdk 74a7b1c、packages 1c717e7）。
miyu-scripts 下只有 `miyu-scripts-ai-draw`（82c7115）、`miyu-skills-office`（0562efd）是 git 仓库，看的是它们的 origin/main；其余 23 个子目录不是 git 仓库，看的是工作目录里的文件。

参照的布局：核心 `docs/blueprint/store.md`「数据根里有什么」（origin/main）。

## 先说清楚三种「依赖」

| 记号 | 意思 | 改布局时 |
|---|---|---|
| **硬** | 自己拼数据根里的子路径。包括经核心门面 `DataRoot::state()` / `run()` / `path()` 拿到一层以后，**下面的子路径自己拼**（例如 `root.state().join("logs")`、`root.run().join("web.lock")`） | 要改代码。就算核心只改 `state()`、`run()` 的返回值，子路径的语义也可能不对：`state()` 改成 `var/lib` 后 `state().join("logs")` 会落到 `var/lib/logs`，而 FHS 里日志该在 `var/log` |
| **钉** | 全经核心门面 `miyu_client`（`DataRoot::locate`、`prepare`、`connect::*`、`miyu_ipc` 的套接字/令牌位置），自己不拼路径 | 不改代码，**要改钉的核心提交、重编、发版**。`run/` 下几样的位置设计 55 里不变，已装的旧头照样连得上新核心，所以要靠新的标记文件名让旧程序认不出新数据根、不往里写（设计 55 第六节，FH 审查第 2 条） |
| **软** | 经环境变量（`MIYU_PACKAGE_STATE`、`MIYU_PACKAGE_DIR`、`MIYU_SCRIPT_*_DIR`、`MIYU_PICTURES_DIR`、`MIYU_PROGRAM`…）、核心给的工作目录、协议里的字段（`package.list` 的 `state`、`tool.call` 的 `cwd`/`pictures`、`skill.list` 的 `dir`、握手的 `host.*`）拿路径 | 不用改（只要核心继续给这些变量、字段、工作目录） |

各头连核心用的核心提交（`Cargo.toml` 里的 `rev`）：miyu-tui `e79835a4`、miyu-web `64bc6ecd2`、miyu-im `f69013f2`，三个各不相同。

---

## miyu-tui

### 产品代码

| 文件:行 | 拼的路径 | 方式 | 用途 |
|---|---|---|---|
| `src/core/connect.rs:20-41` | 数据根、套接字、令牌 | 钉：`DataRoot::locate` + `prepare`，`miyu_client::connect::connect_or_start` / `connect`，令牌由门面读出来以后放进 `hello` 的 `token`（:51） | 连核心。**不自己拼 `run/`** |
| `src/app/build.rs:36-42` | `<state()>/tui/history.jsonl` | **硬**（`root.state().join("tui").join("history.jsonl")`） | 输入历史的**老位置**（9-1 以前）：连上以后搬进包状态目录；拿不到状态目录时照旧用它 |
| `src/core/packages.rs:43` | 包状态目录 `state` | 软：协议（`package.list` 每一项的 `state`） | 输入历史的新位置，`<state>/history.jsonl`（`src/input/saved.rs` 的 `place`） |
| `src/app/look.rs:66-83` | `<cache_root>/tui/look/<fnv(数据根路径)>.json` | 钉（`DataRoot::locate().path()` 只拿来算哈希；文件在机器缓存目录） | 记上次的样子，按数据根的路径区分。数据根**路径本身**不变就不受影响 |
| `src/backstage.rs:22-24`、`src/notify/herdr.rs:184` | 核心程序 | 软：`MIYU_CORE_BIN`、`MIYU_PROGRAM` | 拉起 `miyu web`、写 herdr 的恢复命令 |
| `src/app/workspace.rs`、`src/core/connect.rs:126` 等 | 工作区 | 软：协议（`workspace.cwd`、`session.workspace_changed`） | 显示、换工作区 |
| `src/core/links.rs:142-144`、`src/crash.rs:14`、`src/avatars.rs:25` 等 | `<cache_root>/tui/...` | 不在数据根（机器缓存目录） | 链接卡片、崩溃记录、头像缓存 |

### 测试（测具自己造数据根，硬编码现有布局）

约 50 行、20 多个文件，全在 `tests/` 下：

| 文件:行（举例） | 拼的路径 |
|---|---|
| `tests/support/home.rs:41-50, 144, 189-193, 204` | `packages/miyu-tui`、`packages/<主题>`、`config.toml`、`personas/<编号>/prompts/persona.md` |
| `tests/history.rs:54, 62, 70` | `state/packages/miyu-tui/history.jsonl`、`state/tui/history.jsonl` |
| `tests/sent.rs:33-60`、`tests/widgets.rs:49, 71` | `blobs`、`packages/memes`、`extensions.json`、`state/providers/memes.json` |
| `tests/settings.rs:191, 272`、`tests/goal.rs:109, 133` | `secrets.toml`、`packages/miyu-base-goal.removed`、`sessions` |
| `tests/sessions.rs:246, 269`、`tests/levels.rs:101`、`tests/meters.rs:90`、`tests/notices.rs:21`、`tests/queued.rs:54`、`tests/reconnect.rs:27` | `sessions` |
| `tests/package_pages.rs:56-170`、`tests/packages.rs:53` | `packages/weather` 等 |
| `tests/oobe.rs:49, 364`、`tests/persona_edit.rs:17-122`、`tests/preset_edit.rs:14`、`tests/list_edit.rs:14`、`tests/seeds.rs:21, 70`、`tests/slash.rs:19, 57` | `personas/…`、`presets/…`、`commands`、`skills/…` |

（`tests/meters.rs:104` 的 `run`、`tests/herdr_resume.rs` 的 `config.toml` 是测具自己的假桌面、herdr 配置，不是数据根。）

### 文档里会过时的

| 文件:行 | 写的 |
|---|---|
| `README.md:9` | 装终端的包，「发版时核心照 `heads.toml` 放进 `packages/miyu-tui/`」 |
| `README.md:12, 20, 23-24, 35` | `MIYU_HOME=$HOME/.cache/miyu-tui-home` 的用法（只是变量，不随布局变；`/tmp` 的提醒仍对） |
| `AGENTS.md:15` | 「不碰 `~/.miyu`」 |
| `docs/blueprint/tui.md:49` | `MIYU_HOME` 照 `store.md` |
| `docs/blueprint/tui.md:587` | 截图不放数据根（`attachment_in_data_root`） |
| `docs/blueprint/tui.md:623` | 输入历史是 `<数据根>/state/packages/miyu-tui/` |
| `docs/blueprint/tui.md:1344` | `config.set` 写到 `system/config.toml`（**已经过时**，SU-5 以后没有 `system/`） |
| `docs/blueprint/tui.md:1348` | 「数据根只剩一层」：列了 `config.toml`、`secrets.toml`、`accounts.json`、`packages/`、`personas/`、`presets/`、`skills/`、`commands/`、`sessions/`、`blobs/` 直接在数据根下；说终端只有测具按路径读 |
| `docs/blueprint/tui.md:1501` | 输入历史放在数据根里 |

---

## miyu-web

### 产品代码

| 文件:行 | 拼的路径 | 方式 | 用途 |
|---|---|---|---|
| `crates/miyu-web/src/serve.rs:41-42, 211-212` | `<run()>/web.lock` | **硬**（常量 `LOCK = "web.lock"`，`root.run().join(LOCK)`） | 网页软件单实例锁 |
| `crates/miyu-web/src/serve.rs:417-418` | `<run()>/web.lock` | **硬** | `running()`：看锁有没有人拿着 |
| `crates/miyu-web/src/serve.rs:44-45, 135, 426` | `<run()>/web` | **硬**（常量 `ADDRESS = "web"`，`write_address(&root.run(), …)`） | 写网页地址 |
| `crates/miyu-web/src/serve.rs:439-440` | `<run()>/web` | **硬** | `address()`：读网页地址（`open` 用） |
| `crates/miyu-web/src/open.rs:59-60, 213-214` | `<run()>/open/<随机>.html` | **硬**（常量 `JUMPS = "open"`） | 带凭据的跳转页 |
| `crates/miyu-web/src/open.rs:287-290` | `<run()>/open/` | **硬** | `forget`：只删这个目录里的跳转页 |
| `crates/miyu-web/src/main.rs:124-129` | `<state()>/logs/web.log` | **硬**（`root.state().join("logs")`，文件名 `web`） | 网页软件的运行日志 |
| `crates/miyu-web/src/main.rs:50-60, 160` | 数据根 | 钉：`DataRoot::locate` / `prepare` | 起来、`forget` |
| `crates/miyu-web/src/open.rs:372` | 数据根路径 | 钉：`spawn_detached(…, root.path())`（工作目录是数据根） | 拉起 `serve` |
| `crates/miyu-webserve/src/ws.rs:228`、`crates/miyu-web/src/media/link.rs:194-205` | 套接字（不读令牌） | 钉：`connect_or_start_bare` | `/ws`、`/media` 转发到核心 |
| `web-demo/bridge/src/link.rs:126-136` | 数据根、套接字、令牌 | 钉：`DataRoot::locate`、`connect_or_start` / `connect` | 开发用的桥 |
| `crates/miyu-web/src/main.rs:108-123`、`src/settings.rs:16-37` | 网页包文件夹 | 软：`MIYU_PACKAGE_DIR`；程序在 `<x>/bin/` 里的取 `<x>`；都没有退回**资源目录**的 `packages/miyu-webui`（常量 `DIR`，在资源目录里，不是数据根） | 读 `web.json`、清单、页面 |
| `crates/miyu-web/src/main.rs:184-187` | 核心程序 | 软：`MIYU_PROGRAM`（桥 `web-demo/bridge/src/core.rs:37` 是 `MIYU_CORE_BIN`） | 拉起核心 |
| `crates/miyu-desk/src/reveal.rs` | 技能、脚本文件夹 | 软：协议（页面从 `skill.list`/`script.list` 的 `dir`，或 `from: "home"` 那几项的 `path` 倒推，`web-demo/packages/settings/tool-model.js:113-122`） | 「打开目录」 |
| `web-demo/src/model/notes.js:200-202` | 默认工作区 | 软：协议（`default_workspace`） | 「工作区：默认工作区」 |

matugen 的输出在机器缓存目录（`~/.cache/miyu/matugen.json`），不在数据根。

### 测试

约 11 行、5 个文件，多数经 `home.root.run()`：`crates/miyu-web/tests/jump.rs:9, 16`、`tests/open.rs:384, 407`（`run()/open`）、`tests/serve.rs:49, 213`、`tests/listen.rs:255`（断言文字「run/web」）、`tests/ws.rs:424`（「run/token」）、`tests/open.rs:115-128`（资源目录的 `packages/miyu-webui`，不是数据根）。
`web-demo/tests/*.test.js` 里有旧布局的字符串当夹具（`~/.miyu/home/admin/skills`、`system/config.toml`…，例如 `tests/packages/tool-pages.test.js:23, 81-83, 146`、`tests/notes.test.js:55-58`、`tests/packages/settings.test.js:30, 35`、`tests/packages/setup.test.js:78`）：只是协议数据的样子，不依赖真布局，改不改都不影响测试；`crates/miyu-desk/src/reveal.rs:122-123` 同理。

### 文档里会过时的

| 文件:行 | 写的 |
|---|---|
| `AGENTS.md:16` | 不碰 `~/.miyu` |
| `README.md:8` | 网页包装进资源目录的 `packages/miyu-webui/` 或从软件源装 |
| `docs/blueprint/web-module.md:46, 219, 255, 538, 637` | `home/<账号>/logins.json`（**已经过时**：store.md 现在是数据根顶层的 `logins.json`） |
| `docs/blueprint/web-module.md:217-218, 558, 637, 647` | `run/web.lock`、`run/web` |
| `docs/blueprint/web-module.md:295, 300, 540` | `blobs/tmp/`（还写着「这个账号的」） |
| `docs/blueprint/web-module.md:323, 479` | `connect_bare` 不读 `run/token` |
| `docs/blueprint/web-module.md:651`、`web-ui.md:36, 55` | `state/logs/web.log` |
| `docs/blueprint/web-ui.md:1, 15, 42-43, 65, 133, 135`（及 `serve.rs:1-11`、`open.rs:5` 的模块注释） | `run/web.lock`、`run/web`；`serve` 的工作目录是数据根 |
| `docs/blueprint/web-ui.md:28, 50` | 网页包装进数据根的 `packages/miyu-webui/` |
| `docs/blueprint/web-ui.md:67` | 跳转页 `run/open/<十六进制>.html` |
| `docs/blueprint/web.md:58, 844` | `system/secrets.toml`、写到 `system` 层（**已经过时**） |
| `docs/blueprint/web.md:1060` | 「同一用户的程序读得到 `run/token`」 |
| `docs/blueprint/webserve.md:78` | `run/web` |
| `miyu-webui/matugen/README.md:13` | 装好的网页包在资源目录或数据根下的 `packages/miyu-webui/` |
| `web-demo/README.md:15` | `system/config.toml`（**已经过时**） |

---

## miyu-desktop

产品代码里**没有**数据根路径：只经 `MIYU_PROGRAM`（`src/program.rs:26-53`）找主程序，跑 `miyu web --print --format json --ticket`（`src/program.rs:16`）拿网址；网页视图的数据放 Tauri 自己的数据目录。Windows 安装包（`packaging/windows/miyu.wxs:13-14`）跑 `miyu.exe core stop`，不碰路径。打包（`packaging/stage.py:4, 64`）放的是资源目录的 `packages/miyu-webui/`，不是数据根。

| 文件:行 | 写的 | 性质 |
|---|---|---|
| `AGENTS.md:16` | 不碰 `~/.miyu` | 文档 |
| `docs/blueprint/desktop.md:42` | 网页视图的数据不放进 Miyu 的数据根 | 文档，不随布局变 |
| `docs/construction/DT-3-外壳.md:27, 45` | 「`run/core.lock` 放了」 | 施工记录（历史），会过时但不必改 |

---

## miyu-im（最麻烦）

im 不读 `MIYU_PACKAGE_STATE`（整个仓库查不到这个名字），自己用 `root.state().join("packages").join("miyu-im")` 算状态目录；还在数据根**顶层**读两处 store.md 布局里没列出来的东西（`venues.d/`、`modules/miyu-im/`）；还按核心的约定读核心写的 `state/logs/miyu-im.stderr`。

补（2026-11-03，core1 数的）：im 的 main 调核心 `DataRoot` 的 `.state()`、`.cache()` 一共 36 处、28 个文件（代码加测试，多半是 `state/packages/miyu-im`、日志、统计）。FH-2上 起这两个方法去掉了，im 换钉以后编译就报出这些地方；要改成读 `MIYU_STATE_DIR`、`MIYU_LOG_DIR` 这些环境变量（FH-5上）。

### 产品代码

| 文件:行 | 拼的路径 | 方式 | 用途 |
|---|---|---|---|
| `crates/miyu-im/src/status_file.rs:37-40` | `<state()>/packages/miyu-im/` | **硬**（绕过 `MIYU_PACKAGE_STATE`） | im 的状态目录，下面几行都从它派生 |
| `crates/miyu-im/src/status_file.rs:29-30, 47-56` | `…/miyu-im/status.json` | **硬**（同上派生） | 状态文件：桥写；`miyu im status`（`control.rs:172`）、`miyu im web`（`web.rs:54`）读 |
| `crates/miyu-im/src/status_file.rs:32-45`、`serve.rs:239`、`site/builtins.rs:40`、`core/route/driven.rs:141` | `…/miyu-im/<平台>/`（如 `qq/`） | **硬**（派生） | 每个驱动自己的状态目录：`miyu-im-onebot/src/admin/records.rs:132` 的 `admin/<群号>.jsonl`、`admin/groups.json`、`listen/connection.rs:239` 的 `Seen` 等都在里面 |
| `crates/miyu-im/src/serve.rs:267-270` | `…/miyu-im/stats` | **硬**（`status_file::path().with_file_name("stats")`） | 今天的数 |
| `crates/miyu-im/src/serve.rs:272-274` | `…/miyu-im/recent.json` | **硬**（同上） | 最近判断 |
| `crates/miyu-im/src/serve.rs:276-278`、`site/records_page.rs:31` | `…/miyu-im/records.db`（`records.rs:27`） | **硬**（同上） | 聊天记录库 |
| `crates/miyu-im/src/serve.rs:372` | `…/miyu-im/` | **硬** | 交给路由（每个驱动的目录照它找） |
| `crates/miyu-im/src/rules/cards/store.rs:15-23` | `<state()>/packages/miyu-im/cards.json` | **硬**（自己再拼一遍，没走 `status_file::dir`） | 场所卡片 |
| `crates/miyu-im/src/site/home_cards.rs:17-22` | `<state()>/packages/miyu-im/home.json` | **硬**（自己再拼一遍） | 首页不放的插件卡片 |
| `crates/miyu-im/src/program.rs:219-224` | `<state()>/logs/miyu-im.log` | **硬** | 装运行日志 |
| `crates/miyu-im/src/logs.rs:37-41` | `<state()>/logs/miyu-im.log`、`<state()>/logs/miyu-im.stderr` | **硬**；`.stderr` 是**核心**写的（核心 `crates/miyu-endpoint/src/extensions/stderr.rs`），im 自己重算路径去读 | `miyu im logs [-f]` |
| `crates/miyu-im/src/site/methods.rs:72-77` | `<state()>/logs/miyu-im.log` | **硬** | 后台页「日志」 |
| `crates/miyu-im/src/rules.rs:56-57, 253-263` | `<数据根>/venues.d/*.toml` | **硬**（`root.path().join("venues.d")`） | 系统那一层的场所规则（管理员手写） |
| `crates/miyu-im/src/rules.rs:62-69, 285-288` | `<数据根>/modules/miyu-im/moderation.txt` | **硬**（`root.path().join("modules").join("miyu-im").join(…)`） | 系统的违规词表（后台页也写它） |
| `crates/miyu-im/src/rules.rs:290-293` | 上面两处 | **硬** | 变没变的戳 |
| `crates/miyu-im/src/program.rs:131-137` | 数据根 | 钉：`DataRoot::locate` / `prepare` | 起来 |
| `crates/miyu-im/src/serve.rs:164`、`control.rs:132`、`venue.rs:54`、`program.rs:181`、`web.rs:36`、`site/password.rs:27`、`site/ticket.rs:52` | 套接字、令牌 | 钉：`miyu_client::open::Core::connect` / `connect_running` / `connect_password` / `connect_login` | 连核心 |
| `crates/miyu-im/src/package.rs:17-40` | 包文件夹 | 软：`MIYU_PACKAGE_DIR`；没设退回**资源目录**的 `packages/miyu-im/` | 读 `data/`（出厂规则、`bridge.json` 等） |
| `crates/miyu-im/src/media.rs:270-285`、`miyu-im-onebot/src/tools/*.rs` | 工作区 | 软：`tool.call` 的 `cwd` | 下载平台的文件 |
| `crates/miyu-im-sponsor/src/main.rs:30-31, 54` | `sponsors.json`（相对路径） | 软：靠核心把**工作目录**设成包状态目录（核心 `supervise.rs:287` 注释：以 `MIYU_PACKAGE_STATE` 为准、工作目录照旧也是它） | 赞助账本 |

im **不直接读会话日志**（产品代码里查不到 `sessions/`、`.jsonl` 的会话读写；`.jsonl` 只有自己的 `admin/<群号>.jsonl`）；**不读别的包的状态目录**（插件的数据在 im 自己的目录下，`plugins/builtin.rs:31` 的 `store` 来自 `status_file::store`）。

### 测试

约 126 行、53 个文件（主要在 `crates/miyu-im-onebot/tests/`，`tests/rules.rs` 一个就 26 行）。常见的硬编码：`state/packages/miyu-im/{qq/admin,cards.json,…}`、`state/logs/miyu-im.{log,stderr}`、`venues.d`、`modules/miyu-im`；另有大量资源目录的 `packages/miyu-im/data/...`（那是资源目录，不随数据根变）。

### 文档里会过时的

| 文件:行 | 写的 |
|---|---|
| `docs/authors/README.md:115` | 给包作者：`<数据根>/state/logs/<你的包>.stderr`（**面向第三方作者**，要跟着改）；这一份也没提 `MIYU_PACKAGE_STATE` |
| `docs/blueprint/onebot.md:26, 271, 286, 851` | 状态文件 `state/packages/miyu-im/status.json`；工作目录是 `state/packages/miyu-im/` |
| `docs/blueprint/onebot.md:187, 310` | `state/logs/miyu-im.log`、`state/logs/onebot.stderr`（后者连包名都是旧的） |
| `docs/blueprint/onebot.md:638, 644-646, 651, 655, 899, 904, 906, 1354` | `venues.d/`、`modules/miyu-im/moderation.txt`（638、899 还写着 `system/modules/onebot/`，**已经过时**） |
| `docs/blueprint/onebot.md:1354` | 卡片在 `state/packages/miyu-im/cards.json` |
| `docs/blueprint/chat.md:586` | 违规词表在数据根的 `modules/miyu-im/moderation.txt` |
| `docs/designs/06-按人接入和两层功能.md:21, 76, 138` | `home/im/`（**已经过时**） |
| `docs/designs/07-多平台和Telegram.md:331` | 改名时搬 `state/packages/im/` → `state/packages/miyu-im/`、`modules/im/` → `modules/miyu-im/` |
| `docs/construction/*`（I-42、I-6下、I-73、I-86、I-8下、O-41、O-58、Q-2）、`docs/construction/README.md:160` | 各种旧路径（施工记录，历史，不必改） |
| `crates/miyu-im/src/status_file.rs:1, 13`、`rules.rs:4-8, 56, 62, 68, 285`、`rules/cards/store.rs:1`、`site/home_cards.rs:2`、`logs.rs:3-5`、`recent.rs:6` | 模块注释里的路径，随代码改 |

---

## miyu-memes

| 文件:行 | 路径 | 方式 | 用途 |
|---|---|---|---|
| `src/main.rs:2, 39` | `library.db`（相对路径） | 软：靠核心给的工作目录 = 包状态目录（没读 `MIYU_PACKAGE_STATE`） | 表情库 |
| `src/save.rs:34, 168-205` | 相对路径照 `cwd` 接 | 软：`tool.call` 的 `cwd` | 存本机文件 |
| `src/packs.rs:1` | 内容包 | 软：协议 `content.list` / `content.read` | 读表情内容包 |

文档：`docs/construction/施工顺序.md:11` 写「工作目录下 library.db」，不随布局变。没有硬依赖。

## miyu-memes-shorin-collection

只有清单 `package.toml:8` 的 `[content.miyu-memes] dir = "memes"`（包内相对路径）。没有依赖。

## miyu-embed

程序只认命令行 `--manifest <清单> --dir <目录>`（`src/main.rs:1, 19-27`），由核心交；测试的 `MIYU_EMBED_MODEL_DIR`、`MIYU_EMBED_BPE_DIR` 指模型文件。没有依赖。

## miyu-render

`src/tools/render_image.rs:1, 22-39`：存图照 `tool.call` 的 `pictures`（核心给的），没有的存 `<cwd>/rendered/`。软，没有硬依赖。
文档：`docs/prompt-ledger.md:8` 举例写 `/home/u/.miyu/home/owner/pictures/…`（**已经过时**，只是估算提示词长度的例子）。

## miyu-sdk

Python（`python/miyu_ext.py:89`）、Rust（`rust/src/reply.rs:314`）只转交 `tool.call` 的 `cwd`。查不到任何 `MIYU_*` 环境变量或数据根路径；**SDK 没有封装状态目录**（未查到 `MIYU_PACKAGE_STATE` 的读取），包作者要自己读变量或靠工作目录。

## miyu-scripts（25 个子包）

全部经环境变量，**没有硬依赖**：

| 变量 | 用到的包（文件:行） |
|---|---|
| `MIYU_SCRIPT_CACHE_DIR` | weather `scripts/weather.py:83, 97`；douyin-dl `douyin_dl.py:76`；bangumi `bangumi.py:72`；exchange-rate `exchange_rate.py:70`；web-image `web_image.py:240, 491`；juya-daily `juya_daily.py:50`；reddit-search `reddit_search.py:76`；game-compat `game_compat.py:109`；goofish `goofish.py:68`；bilibili-live-stream `bilibili_live.py:728`；aur-pkgbuild-review `aur_review.py:572`；travel-planner `hotel_deals.py:52`、`flight_deals.py:58`；ai-draw `ai_draw.py:470` |
| `MIYU_SCRIPT_STATE_DIR` | goofish `goofish.py:86`；bilibili-live-stream `bilibili_live.py:713`；travel-planner `hotel_deals.py:52`、`flight_deals.py:58` |
| `MIYU_PICTURES_DIR` / `MIYU_VIDEOS_DIR` | ai-draw `ai_draw.py:469`；web-image `web_image.py:490`；douyin-dl `douyin_dl.py:440` |
| `MIYU_SCRIPT_IMAGES_DIR` | ai-draw `ai_draw.py:491`；web-image `web_image.py:502`；douyin-dl `douyin_dl.py:546` |
| `MIYU_SETTINGS_JSON` | ai-draw `ai_draw.py:77` |
| `MIYU_CALLER_OWNER` | bilibili-live-stream `bilibili_live.py:1259` |

各包 `.github/workflows/release.yml` 只调 `miyu-agent/packages/.github/workflows/pack.yml@main`。README 里只写变量名，不写路径。

## packages（软件源仓库）

`.github/workflows/pack.yml:49-102`：装钉住的核心（`miyu-rev`），跑 `miyu pkg pack <暂存目录> --out $RUNNER_TEMP/out`，不设 `MIYU_HOME`、不碰数据根路径。`collect.yml` 只建索引。README 里的 `.miyu` 是包文件的扩展名。**没有依赖**。

---

## 跨版本（布局改了以后的连锁）

- 三个头（tui、web、im）都靠钉住的核心提交里的 `miyu_client` / `miyu_ipc` 找套接字（`run/core.sock` 或 `$XDG_RUNTIME_DIR/miyu-<指纹>/core.sock`，核心 `crates/miyu-ipc/src/place.rs:75-76`）和令牌（`run/token`，核心 `crates/miyu-ipc/src/files.rs:16-19`）。核心改了 `run/` 的位置，三个头都要换钉、重编、发版；`run/` 的位置不变，**已装的旧头照样连得上新核心**，旧头、旧核心还会照旧布局往新数据根里建 `state/` 这些：设计 55 改了标记文件名挡住它们（FH 审查第 2 条）。三个头现在钉的是三个不同的提交。
- 网页的 `run/web.lock`、`run/web`、`run/open/` 是网页自己定、自己读写的；核心的 `store.md` 也列了 `web.lock`、`web`。挪 `run/` 时要和网页一起挪。
- im 读核心写的 `state/logs/miyu-im.stderr`：核心挪扩展标准错误的位置（例如挪到 `var/log/`），im 的 `logs.rs` 要同步。
- memes、im-sponsor 靠「工作目录 = 包状态目录」：核心挪状态目录时只要继续把工作目录设成新位置就不受影响；更稳的是让它们改读 `MIYU_PACKAGE_STATE`。
