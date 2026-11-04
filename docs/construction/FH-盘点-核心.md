# FH 盘点：核心仓库里数据根的每一处（2026-11-02，以代码为准）

设计 `55-数据根仿FHS.md`、施工单 `FH-数据根仿FHS.md` 的底稿。施工时照它逐项对，做完的不用回来改这份。

- 依据：`origin/main` = `64bc6ecd2`（R3-1 补）。下文路径都是相对仓库根的 `crates/...:行号`。
- 只读盘点，没有编译、没有跑测试。「未查到」就是没在代码/文档里找到，不是猜测。
- 对照的文档：`docs/blueprint/store.md`「数据根里有什么」（下称 **store.md 树**）。「文档」一列：✔ = 树里写了；◐ = store.md 别处或别的蓝图提到、树里没写；✘ = store.md 没写（别的蓝图可能有）。
- 性质：**真**＝用户数据，丢了就没了；**配**＝配置；**凭**＝凭据；**派**＝派生，可照真相重建；**缓**＝缓存，丢了重取；**日**＝日志；**运**＝运行时；**包**＝包的/装上的东西；**混**＝名义派生、实际含不可重建的。
- 「沙盒」一列说的是 Linux/macOS 的情况：数据根整条 `hidden`（`crates/miyu-session/src/sandbox.rs:202-226`），只透出 `write`/`reveal` 里的；Windows 的特殊情况统一放在第三节。

## 一、数据根下每一处

### 1.1 顶层标记、配置、凭据（数据根这一层的文件）

| # | 路径 | 是什么 | 写 | 主要读 | 性质 | 权限 | 文档 |
|---|---|---|---|---|---|---|---|
| 1 | `.miyu-root` | 数据根标记，一行 `This directory is a Miyu data root (layout 2).` | `miyu-store/src/root.rs:251-278`（`prepare`，只在空目录写） | 只看在不在：`root.rs:255`、`durable.rs:38,48`（`create_inside` 往上找它）、`config_file.rs:248`、`miyu-session/src/sandbox.rs:251`、`miyu-sandbox/src/install/files.rs:26` | 运/标记 | 默认 | ✔ |
| 2 | `config.toml` | 系统配置；核心新建时第一行写 `#:schema state/config/config.schema.json`（相对路径，`miyu-endpoint/src/config.rs:74`） | 核心 `miyu-endpoint/src/config.rs:105`（经 `config_file` 写，临时文件在 `run/`） | 同上；`check/unsandboxed.rs:22` | 配 | 替换时带原权限 | ✔ |
| 3 | `secrets.toml` | 密钥 | `miyu-store/src/secrets.rs:16,58`（`Mode::Private`）；路径 `miyu-endpoint/src/config.rs:110` | 核心起来读配置 | 凭 | 0600（临时文件建时就是） | ✔ |
| 4 | `trust.toml` | 项目配置的信任记录 | `miyu-endpoint/src/config/trust.rs:22`、`config/trusting.rs:99` | `config.rs:125,170`、`config/observe.rs:61,157` | 真（人的决定） | 带原权限 | ✔ |
| 5 | `journal.jsonl` | 日志：配置/密钥改动、信任、`usage.purged`、`usage.oneshot` | `miyu-store/src/journal.rs:26`；调用 `miyu-endpoint/src/config.rs:126`、`secrets.rs:349`、`login/files.rs:105`、`miyu-store/src/trash.rs:132`、usage oneshot | `miyu-store/src/usage/catch_up.rs:127,139` | 真（用量重建依赖它） | 默认 | ✔ |
| 6 | `extensions.json` | 扩展开关、批过的能力；注释明说「放数据根下、不放 `state/`：`state/` 是派生的」 | `miyu-store/src/extensions.rs:19`；`miyu-endpoint/src/extensions.rs:369` | 同 | 真/配 | 0600（private_json） | ✔ |
| 7 | `access.json` | 访问密码 argon2id 哈希 | `miyu-store/src/access.rs:22`；`miyu-endpoint/src/login/files.rs:19,50` | 同 | 凭 | 0600 | ✔ |
| 8 | `logins.json` | 设备令牌哈希 | `miyu-store/src/logins.rs:18`；`login/files.rs:111` | 同 | 凭 | 0600 | ✔ |
| 9 | `projects.json` | 项目列表（H-1，设计 38）：名称、说明、附加提示词、工作区；哪个会话属于哪个项目记在会话日志里 | `miyu-store/src/projects.rs:15,217`；`miyu-endpoint/src/projects.rs:233` | `miyu-session/src/open.rs:378`、`open/load.rs:459`、`miyu-endpoint/src/spawn.rs:163` | 真 | 0600（private_json） | ✘ |
| 10 | `terminal.json` | 终端会话指针（8-33、T-22）：在终端里说话进哪个会话，按终端键分，7 天没用清掉 | `miyu-store/src/terminal.rs:19,95`；`miyu-endpoint/src/terminal.rs:77,145` | 同 | 真（轻，丢了只是换新会话） | 0600 | ✘ |
| 11 | `avatar.{png,jpg,webp}` | 主人自己的头像（AV-1） | `miyu-endpoint/src/login/avatar.rs:20,28,49`（文件名表 `miyu-store/src/personas.rs:27`） | 同；握手带版本 | 真 | 默认 | ✘ |
| 12 | `kb.toml` | 包里知识库「自动想起」开关的人改值（`[auto]`） | `miyu-store/src/knowledge/place.rs:190,198`、`knowledge/manage.rs:184` | `place.rs` | 配 | 默认 | ✔ |
| 13a | `.<文件名>.edit-<随机>.toml`（顶层） | `miyu config edit` 给编辑器的副本，只许新建、Unix 0600；在本体旁边（数据根顶层），不走 `run/` | `miyu-cli/src/config/edit.rs:250-266` | 编辑器 | 运（含配置原文） | 0600 | ✘ |
| 13 | `.<名>.<pid>-<n>.tmp`（顶层，旧版残留） | R3-1 以前写配置/密钥的临时文件 | 现在不再写到这里 | `miyu-core/src/leftovers.rs:24` 起来时清（10 分钟以上的） | 运 | — | ◐ |

### 1.2 你的、可覆盖出厂的几处（顶层目录）

| # | 路径 | 是什么 | 写/找 | 主要读 | 性质 | 文档 |
|---|---|---|---|---|---|---|
| 14 | `personas/<编号>/`（`persona.toml`、`prompts/persona.md`、`prompts/reminders.md`、`avatar.*`、`background.*`） | 你那一层的人格 | `miyu-store/src/personas.rs:117`；`miyu-endpoint/src/personas/write.rs`、`picture.rs:175` | 层叠：出厂 → 包 → 你 | 真 | ✔ |
| 15 | `presets/<编号>.toml` | 你那一层的预设 | `miyu-store/src/presets.rs:81,323`；`miyu-endpoint/src/presets/edits.rs:109,120` | 同 | 真 | ✔ |
| 16 | `skills/<名>/SKILL.md` | 你的技能；开着的技能文件夹做沙盒的 shelf/reveal | `miyu-store/src/skills.rs:74` | `skills.rs:94,158`、`miyu-session/src/skills.rs:119` | 真 | ✔ |
| 17 | `scripts/<文件>` | 你的脚本（被监视，`watch_dir`） | `miyu-store/src/scripts.rs:39`（`own_dir`） | `scripts.rs:225,266`、`miyu-core/src/lib.rs`（`watch_scripts`） | 真 | ✔ |
| 18 | `commands/<名>.md` | 你的斜杠命令模板 | `miyu-store/src/slash.rs:37` | `slash.rs:43,83` | 真 | ✔ |
| 19 | `prompts/` | 提示词覆盖层（`PROMPTS`，`miyu-store/src/resources/overlay.rs:17`） | `miyu-endpoint/src/presets/overrides.rs:65` | 资源目录叠加 | 真/配 | ✔ |
| 20 | `tools/<工具>.toml` | 工具说明的覆盖（OV-1，`miyu-store/src/overrides.rs:10`） | `miyu-endpoint/src/presets/overrides.rs:25,55` | 同；`check.rs:105` | 真/配 | ✔ |
| 21 | `packages/<编号>/` | 装上的包（整个文件夹） | `miyu-store/src/packages/install.rs:58-97`；`miyu-endpoint/src/packages.rs:79`；路径 `root.rs:111,126` | `miyu-store/src/packages.rs:120`、`miyu-core/src/leftovers.rs:15` | 包 | ✔ |
| 22 | `packages/.<编号>.new/`、`.<编号>.old/` | 装/升级的暂存与备份 | `install.rs:58,65` | `install::recover`（`leftovers.rs:15`） | 运 | ✘ |
| 23 | `packages/<编号>.removed` | 卸掉出厂包的记号（空文件） | `install.rs:14,147,156,168` | `packages.rs:203`（`removed()`） | 真/配（人的决定） | ✘（SU 文档、rename-migrate README 提到） |

### 1.3 会话、工作区、回收处、blob

| # | 路径 | 是什么 | 写 | 主要读 | 性质 | 权限 | 文档 |
|---|---|---|---|---|---|---|---|
| 24 | `sessions/<会话>/000000000001.jsonl …` | 会话日志（段，64 MiB 一段） | `miyu-store/src/log.rs`；目录 `root.rs:206` | 到处（`root.rs:188` 列会话；`miyu-endpoint/src/list.rs:262` 等） | **真** | 目录 0700 | ✔ |
| 25 | `sessions/<会话>/attachments/` | 没本机路径的附件落成文件；**绝对路径记进会话日志**，以后每次请求照它拼 | `miyu-store/src/attachments.rs:35`；`miyu-endpoint/src/attach/keep.rs` | 模型读；Guard 只读（`miyu-session/src/guard.rs:155-170`） | 真 | — | ✔ |
| 26 | `sessions/<会话>/jobs/<编号>.out` | 后台命令输出 | `miyu-store/src/jobs.rs:15`；每条命令前建（`miyu-session/src/sandbox.rs:254-257`） | 工具 | 真（可丢但无源） | 0700 | ✔ |
| 89 | `sessions/<会话>/imported.json` | 导进来的会话的标记：从哪个包导进来、是不是只读存档（施工 SE-1 下） | `miyu-store/src/bundle/import.rs`（`DataRoot::imported_file`） | `miyu-endpoint/src/import.rs`（拒改存档）、`list.rs`（`archived`）、`miyu-session/src/memory.rs`（存档不抽取）、导出 | 真 | — | ✔ |
| 27 | `sessions/<会话>/memory/log/`、`memory/turns.db`、`memory/memory.db` | 只在这个会话里的那间记忆：日志（真）＋回合库/记忆库（派生） | `miyu-store/src/recall/room.rs:80,88,96,103` | 记忆模块 | 真＋派 | — | ✘ |
| 28 | `workspace/` | 默认工作区根，核心起来时建 | `root.rs:139,171`；`miyu-core/src/start.rs:25` | `miyu-endpoint/src/hello.rs:315`、`files.rs:267`、`attach.rs:341`、`sessions/places.rs:99,149` | 真（主人的产出） | 0700 | ✔ |
| 29 | `workspace/<会话>/` | 每个会话的默认工作区（子会话/分叉沿用主会话的） | 用到时建：`miyu-session/src/default_workspace.rs:22-23`、沙盒 `workspace()` | `miyu-endpoint/src/list/defaults.rs:41`（协议 `default_workspace`） | 真 | — | ✔ |
| 30 | `trash/sessions/<会话>/`（多一个 `deleted_at`） | 删掉的会话 | `miyu-store/src/trash.rs:30-47` | `trash.rs:73`；`usage/catch_up.rs:409` | 真（限期） | — | ✔ |
| 31 | `trash/personas/<编号>.<毫秒>/`（带 `deleted_at`） | 删掉的人格 | `miyu-store/src/trash/personas.rs:23-29` | `personas.rs:53` | 真（限期） | — | ◐（只在方法表） |
| 32 | `blobs/<前两位>/<64 hex>` | 内容寻址的 blob | `miyu-store/src/blob.rs:61-114`；目录 `root.rs:221` | 到处（`miyu-endpoint/src/view.rs:77`、`attach.rs` 等） | **真** | 默认 | ✔ |
| 33 | `blobs/tmp/<pid>-<n>`、`blobs/tmp/upload-<编号>` | 存时临时文件、分块上传暂存 | `blob.rs:33,36,80,213` | `miyu-core/src/packages.rs:39`（起来清） | 运 | — | ✔ |

### 1.4 记忆、知识库、索引

| # | 路径 | 是什么 | 写 | 主要读 | 性质 | 文档 |
|---|---|---|---|---|---|---|
| 34 | `modules/memory/<人格>/`（含 `_none`） | 跟着人格的记忆日志（段文件） | `miyu-store/src/memory.rs:129`、`recall/room.rs:95` | 记忆 | **真** | ✔ |
| 35 | `kb/<编号>/` | 知识库文档 | `miyu-store/src/knowledge/place.rs:115`；`knowledge/write.rs` | `place.rs:295`；边界表 `miyu-fs/src/boundary.rs:244-257` 认 `kb/<库>/…` | **真** | ✔ |
| 36 | `kb/<编号>/kb.toml` | 单个库的设置（挂进来的 `mount` 等） | `knowledge/write.rs:50` | `knowledge/catch_up.rs:212` | 配 | ✘ |
| 37 | `index/sessions.db`(+`-wal`/`-shm`) | 会话列表索引 | `miyu-store/src/index.rs:31` | `miyu-endpoint/src/list.rs:373`、`miyu-core/src/cache.rs:43` | 派 | ✔ |
| 38 | `index/recall/turns-<人格>.db`、`index/recall/memory-<人格>.db` | 回合库、记忆库 | `recall/room.rs:78,87,111` | `recall/indexes.rs:105` | **混**：`buried` 墓碑（删会话时写，「重建补不回来」，`miyu-store/src/recall.rs` 注释）；`vectors` 要重算 embedding | ✘ |
| 39 | `index/kb/<编号>.db`、`index/kb/packages/<包>/<编号>.db` | 知识库检索库 | `knowledge/place.rs:145,239` | 同 | 派 | ✘ |

### 1.5 照 XDG 给主人的几处、缓存

| # | 路径 | 是什么 | 写 | 主要读 | 性质 | 文档 |
|---|---|---|---|---|---|---|
| 40 | `pictures/` `videos/` `documents/` `downloads/` | `USER_DIRS`（`root.rs:24`），用到才建 | 脚本跑前建：`miyu-basesystem/src/shell/script/home.rs` | 握手 `host.dirs`（`miyu-endpoint/src/hello.rs:320`）；边界表 `own`（`miyu-session/src/guard.rs:155`） | 真 | ✔ |
| 41 | `cache/[@<场所>/]<来处>/<工具名>/` | 脚本缓存 `MIYU_SCRIPT_CACHE_DIR`，30 天清 | `miyu-store/src/scripts.rs:127`；建 `miyu-basesystem/src/shell/script.rs:190-210` | `miyu-core/src/cache.rs:31`、`miyu-store/src/cache.rs`、`miyu-basesystem/src/shell/script/sweep.rs:22` | 缓 | ✔ |
| 42 | `cache/[@<场所>/]<来处>/images-<16hex>/` | 脚本交图的临时目录（每次一个，读完删） | `miyu-basesystem/src/shell/script/images.rs:37-46`（在工具缓存目录的**上一层**） | 同 | 运 | ✘ |
| 43 | `cache/mermaid/` | mermaid 字体缓存 | `miyu-core/src/packages.rs:78` | `miyu-mermaid/src/lib.rs:94,130` | 缓 | ✘ |

### 1.6 `state/`（名义上「派生的全局索引、用量总表、运行日志」，`root.rs:229`；实际混了真相）

| # | 路径 | 是什么 | 写 | 主要读 | 性质 | 权限 | 文档 |
|---|---|---|---|---|---|---|---|
| 44 | `state/`（目录本身） | 骨架，`prepare` 建 | `root.rs:43,279` | — | — | 0700 | ✔ |
| 45 | `state/logs/core.log`、`core.log.1…5` | 运行日志（10 MiB 换，留 5 份） | `miyu-core/src/lib.rs:90`、`miyu-log/src/lib.rs:38-41,115` | `miyu-perf`、人 | 日 | 文件 0600、目录 0700（`miyu-log/src/rotate.rs:151,163`） | ✔ |
| 46 | `state/logs/memory.jsonl`(+`.1`) | 核心自己的内存账 | `miyu-core/src/memlog.rs:23,127` | 人 | 日 | 同上 | ✘ |
| 47 | `state/logs/<扩展实例>.stderr` | 扩展的标准错误 | `miyu-endpoint/src/extensions/stderr.rs:24` | 软件包列表给最后几行 | 日 | — | ✘ |
| 48 | `state/logs/mcp-<服务>.stderr` | MCP 服务标准错误 | `miyu-endpoint/src/mcp/launch.rs:140-144` | 同 | 日 | — | ✘ |
| 49 | `state/config/config.schema.json`、`reference.toml` | 生成的 Schema、参考文件 | `miyu-core/src/settings.rs:115,203,280` | 编辑器（`config.toml` 首行引用 `state/config/config.schema.json`） | 派 | 默认 | ✔ |
| 50 | `state/usage.db`(+wal/shm) | 用量汇总 | `miyu-store/src/usage.rs:46,112` | 同 | 派（见第五节） | 默认 | ✔ |
| 51 | `state/rates.db` | 汇率缓存（UC-1） | `miyu-store/src/usage/rates/cache.rs:14`；`usage.rs:114` | 换算 | 缓（要上网重取） | 默认 | ✘ |
| 52 | `state/schedules.db` | 唤醒表；注释：「它是真相，不是派生的…坏了不删不重建」 | `miyu-store/src/schedules.rs:199` | `schedules/open.rs:100`（只读开） | **真** | 默认 | ✔ |
| 53 | `state/log-salt` | 运行日志里记忆短号的盐（32 字节） | `miyu-store/src/recall/salt.rs:18,43-52` | 同 | 真（丢了旧日志短号对不上，不致命） | 0600 | ✔ |
| 54 | `state/models/learned.json` | 用出来的上下文窗口 | `miyu-session/src/route/shared.rs:287` | `shared.rs:387` | 派（观察所得，不可从别处重建；丢了重新学） | 默认 | ◐ |
| 55 | `state/models/providers/<供应商>.json` | 拉到的供应商模型列表 | `shared.rs:300`、`shared/renamed.rs:30,34` | `shared.rs:391` | 缓 | — | ◐ |
| 56 | `state/models/pools.json` | 池的指针 | `shared.rs:340` | `shared.rs:406` | 派/状态（丢了从头轮） | — | ◐ |
| 57 | `state/models/renamed.json` | 供应商改名记录（8-34） | `shared/renamed.rs:28` | `shared.rs:413` | 配/状态 | — | ✘ |
| 58 | `state/models/catalog.json` | 目录答记下的一份（`RECALL_FILE`，`miyu-session/src/route/shared/recall.rs:35`） | `shared/recall.rs:377` | `miyu-core/src/models/recall.rs:109` | 缓 | — | ✘ |
| 59 | `state/providers/<包>.json` | 包 `provide` 登记的工具原文缓存 | `miyu-endpoint/src/provide/cache.rs:16-26` | 同 | 缓 | — | ✘ |
| 60 | `state/mcp/<服务>.json` | MCP 工具目录缓存 | `miyu-endpoint/src/mcp/cache.rs:50-54,106` | 同 | 缓 | — | ✘ |
| 61 | `state/mcp/<服务>/` | MCP 服务（命令型）的工作目录 | `miyu-endpoint/src/mcp/launch.rs:148-150` | 服务进程自己 | **真/包**（服务可能放登录态） | — | ✘ |
| 62 | `state/personas/pictures.json` | 人格图片版本/主色的缓存 | `miyu-endpoint/src/personas/glance.rs:19,131,142` | `personas.rs:142,192`、`write.rs:158` | 缓 | — | ✘ |
| 63 | `state/sandbox-probe.json` | 探到的沙盒能力 | `miyu-core/src/sandbox/remembered.rs:17,78-90`；调用 `miyu-core/src/lib.rs:373` | `remembered.rs:68` | 缓 | 默认 | ✘ |
| 64 | `state/sandbox/windows.json` | Windows 沙盒用户的安装记录：登录名、SID、**DPAPI 加密的密码** | `miyu-sandbox/src/install/record.rs:11,96`；逐层不跟链接建 `install/files.rs:41-47` | 卸载、doctor、5-9 登录 | **凭** | Windows ACL 仅本人+SYSTEM | ✘ |
| 65 | `state/sandbox/windows-error.json` | 提升过的安装进程失败原因 | `install/report.rs:10,59` | 同 | 运 | 同上 | ✘ |
| 66 | `state/packages/<包>/` | 包的状态目录，`MIYU_PACKAGE_STATE`，也是扩展的工作目录 | 路径 `miyu-store/src/packages.rs:122,210`；扩展起之前建 `miyu-endpoint/src/extensions/supervise.rs:276` | 包自己 | **真/包**（包的登录态等） | — | ✔ |
| 67 | `state/packages/.local/<包>/{desc,files}` | 本地库（照 pacman `local/`） | `miyu-store/src/packages/local.rs:17,101-113,186` | `local.rs:122-136`；`miyu-endpoint/src/packages/item.rs`（`installed_at`） | 真（可大致照 packages/ 重算，但装的时刻、来源丢） | — | ✘ |
| 68 | `state/packages/.leftovers.json` | 卸了的包留下的人格/预设改动是谁的 | `miyu-store/src/packages/leftovers.rs:15,51,86` | `miyu-endpoint/src/packages/left.rs:24-131` | 派/提示（坏了当没有） | — | ✘ |
| 69 | `state/packages/.sync/{index.json,index.json.sig,fetched}`、`.sync/logos/` | 官方软件源索引缓存 + 图标 | `miyu-endpoint/src/packages/source/feeds.rs:66`、`sync.rs:31-33,282`、`logo.rs:42` | 同；**命令行直接读** `miyu-cli/src/packages/offered.rs:17-26` | 缓 | — | ✘ |
| 70 | `state/packages/.sources/<源名>/…` | 配置里加的第三方源，同 `.sync` 结构 | `feeds.rs:74-79` | 同；`offered.rs:23` | 缓 | — | ✘ |
| 71 | `state/packages/.cache/<包文件>.miyu`、`.<名>.part` | 下载的包文件 | `source/download.rs:25-62` | `source/fetch.rs:132`、`clean.rs:26` | 缓 | — | ✘ |
| 72 | `state/packages/.unpack/<pid>-<纳秒>/` | 装 `.miyu` 时的解包暂存 | `miyu-endpoint/src/packages/archive.rs:55,94` | 同 | 运 | — | ✘ |
| 73 | `state/scripts/[@<场所>/]<来处>/<工具名>/` | 脚本持久状态 `MIYU_SCRIPT_STATE_DIR` | `miyu-store/src/scripts.rs:44,114`；建 `miyu-basesystem/src/shell/script.rs` | `forget_states`（`scripts.rs:202`，卸包时）；`miyu-endpoint/src/check/skipped.rs:30` | **真**（登录态） | 0700 | ✔（`@场所` 一层 ✘） |
| 74 | `state/perf-cache` | 只有性能测试工具用 | `miyu-perf/src/sandbox.rs:91` | 同 | 缓（工具专用） | — | ✘ |

### 1.7 `run/`

| # | 路径 | 是什么 | 写 | 主要读 | 性质 | 权限 | 文档 |
|---|---|---|---|---|---|---|---|
| 75 | `run/`（目录） | 骨架；每条沙盒命令前补（`keep_places`）；Windows 沙盒里整条禁 | `root.rs:37,43`；`miyu-session/src/sandbox.rs:254`；`config_file.rs:251` | — | 运 | 0700 | ✔ |
| 76 | `run/core.lock` | 单实例锁 | `miyu-ipc/src/lock.rs:17,48,78` | 头、rename-migrate | 运 | — | ✔ |
| 77 | `run/spawn.lock` | 拉起核心的锁 | `miyu-ipc/src/start.rs:21,100` | 头 | 运 | — | ✔ |
| 78 | `run/token`（`token.tmp`） | 本机令牌 | `miyu-ipc/src/files.rs:16,30,77-88` | 头（经 `miyu-client`） | **凭**（运行时） | 0600 | ✔ |
| 79 | `run/socket`（`socket.tmp`） | 套接字实际位置 | `files.rs:19,52,61` | 头 | 运 | 0600 | ✔ |
| 80 | `run/core.sock` | 没有可用 `XDG_RUNTIME_DIR` 时的套接字 | `miyu-ipc/src/place.rs:76` | 头 | 运 | — | ✔ |
| 81 | `run/.<文件名>.<pid>-<n>.tmp` | 写数据根这一层配置/密钥/信任/access/logins/extensions/projects/terminal 时的临时文件（R3-1） | `miyu-store/src/config_file.rs:241-256`（`temp_dir`） | `miyu-core/src/leftovers.rs:24` 清 | 运（含密钥原文） | secrets 0600 | ✔ |
| 82 | `run/web.lock`、`run/web` | 网页软件的锁和地址 | 核心仓库代码里**未查到**写入方（store.md 说是 web-ui.md / W-9，网页包自己写） | 网页包 | 运 | — | ✔ |

### 1.8 数据根以外（同一个布局决策会牵扯的）

| # | 位置 | 是什么 | 代码 |
|---|---|---|---|
| 83 | `<数据根>-sandbox-tmp`（数据根的兄弟目录） | 数据根落在临时目录里时，沙盒自己的 TMPDIR | `miyu-session/src/sandbox.rs:149-153,292-300`（0700、查属主） |
| 84 | `$XDG_RUNTIME_DIR/miyu-<数据根路径 SHA256 指纹>/core.sock`；放不下时 `$TMPDIR/miyu-<uid>/<指纹>.sock` | 套接字 | `miyu-ipc/src/place.rs:5-7,57,75` |
| 85 | `$TMPDIR/miyu-call-<pid>/s/<hex>.sock`、`…/bin/miyu(.exe)` | 脚本调工具的通道 | `miyu-basesystem/src/shell/channel/unix.rs:39,111-121,168`；`windows.rs:96-104` |
| 86 | `$TMPDIR/miyu-scripts/<工具名>` | 会话外调用脚本时的缓存（退路） | `miyu-basesystem/src/shell/script.rs:198-201` |
| 87 | `<项目>/.miyu/config.toml` | 项目配置 | `miyu-endpoint/src/config/project.rs:12`、`miyu-cli/src/config/paths.rs:62`、`check.rs:227`；`.miyu` 在出厂只禁写名单 `miyu-fs/src/readonly/shipped.rs:9` |
| 88 | 服务单元（`~/.config/systemd/user/miyu.service`、autostart、LaunchAgents、`AppData\Local\Miyu\miyu-task.xml`），里面写 `MIYU_HOME=<数据根>` | 开机自启 | `miyu-cli/src/service/plan.rs:87-150` |

### 1.9 机器缓存目录 `cache_root`（`root.rs:333-342`：Linux `$XDG_CACHE_HOME/miyu` 或 `~/.cache/miyu`；macOS `~/Library/Caches/Miyu`；Windows `%LOCALAPPDATA%\Miyu\cache`）

| # | 路径 | 是什么 | 代码 | 性质 |
|---|---|---|---|---|
| 89 | `models/models-dev.json`、`models-dev.meta.json`（ETag） | 拉的 models.dev 目录 | `miyu-core/src/models.rs:281-288`、`models/catalog.rs:18,21` | 缓 |
| 90 | `models/models-dev.digest.json` | 整理好的目录（V-5） | `miyu-core/src/models/digest.rs:16` | 缓 |
| 91 | `models/logos/<编号>.svg`、`<编号>.color.svg`、`fetched`；暂存 `.logos.new` | 供应商图标 | `miyu-core/src/models.rs:262`、`models/logos.rs:36,213` | 缓 |
| 92 | `sandbox/{cargo,npm,pip,go/mod,go/build,xdg,firefox}`；`sandbox/cargo/config.toml`（拷来的） | 沙盒里的工具链缓存，写名单 | `miyu-core/src/sandbox.rs:105-123`、`miyu-session/src/sandbox/caches.rs:13-75` | 缓（0700、查属主） |

另：`miyu-wallpaper/src/matugen.rs:50-54` 写 `<XDG_CACHE_HOME>/miyu/matugen.json`，不经 `cache_root`（自己拼的同一个目录）。

**合计**：数据根内 83 处（#1–#82 加 #13a），数据根外相关 6 处（#83–#88），机器缓存目录 4 处（#89–#92）＋壁纸 1 处。

## 二、DataRoot 的方法与常量（`crates/miyu-store/src/root.rs`）

| 名字 | 路径 | 行 |
|---|---|---|
| `OWN_PLACES` | `scripts` `skills` `commands` `packages` | 20 |
| `USER_DIRS` / `user_dir()` | `pictures` `videos` `documents` `downloads` | 24-30 |
| `CACHE` / `cache()` | `cache` | 33,165 |
| `RUN` / `run()` | `run` | 37,235 |
| `PRIVATE`（`prepare_private`） | `trash` `blobs` `modules` `index` `kb` | 40,149 |
| `SKELETON`（`prepare`） | `state` `run` | 43,279 |
| `MARKER` / `MARKER_TEXT` | `.miyu-root` / `layout 2` | 47,50 |
| `packages()` / `real_packages()` / `own_places()` | | 110-127 |
| `workspace()` / `session_workspace()` / `prepare_workspace()` | `workspace/[<会话>]` | 139,170,177 |
| `sessions()` / `session_dir()` / `sessions_dir()` / `imported_file()` | `sessions/<会话>`、`sessions/<会话>/imported.json`（施工 SE-1 下） | 187,205 |
| `trashed_sessions()` / `trashed_personas()` | `trash/sessions`、`trash/personas` | 210,215 |
| `blobs()` / `index()` / `state()` | | 220,225,230 |
| `create_inside()` | 往上找到标记才建 | 292 |
| `cache_root()` | 机器缓存 | 333 |

**不经 DataRoot 方法、直接拼字符串的地方**（布局改时要逐一跟）：`schedules.rs:199`（`join("state").join("schedules.db")`）、`scripts.rs:44`（`join("state").join("scripts")`）、`scripts.rs:127`、`memory.rs:129` 与 `recall/room.rs:95`（`modules/memory`）、`knowledge/place.rs:115,190`（`kb`、`kb.toml`）、`skills.rs:74`、`slash.rs:37`、`presets.rs:81`、`personas.rs:117`、`miyu-sandbox/src/install/files.rs:43-45`、`record.rs:96`、`report.rs:59`（`state/sandbox`）、`miyu-endpoint/src/config.rs:74`（`"state/config/config.schema.json"` 字符串）、`miyu-policy/src/hazard/paths.rs:93-94`（`format!("{}/workspace/", data_root)`）、`miyu-fs/src/boundary.rs:255`（`"kb"`）、`miyu-cli/src/packages/offered.rs:17-26`（`state/packages/.sync|.sources/*/index.json`）。

## 三、硬编码耦合点

### 3.1 沙盒规则

- **Linux/macOS**（`miyu-session/src/sandbox.rs:125-226`）：`hidden` = 数据根（工作目录是 `~` 且只有 Landlock 时不藏）＋私密目录；`write` = 工作区（默认 `workspace/<会话>/`，在数据根里）、加进来的目录、放行过的、临时目录、`cache_root/sandbox`；脚本另加 `cache/…/<工具>`、`state/scripts/…/<工具>`、`pictures` 等四处、`images-*`（`miyu-basesystem/src/shell/script.rs` 头注释）；`reveal` = 开着的技能文件夹（`skills/<名>` 或 `packages/<包>/skills/<名>`）、本会话 `attachments/`、脚本文件本身。
- **关起来的会话**（`sandbox/confined.rs:109-145`）：`hidden: [数据根]`，`read` 只有系统程序目录（`PROGRAMS`/`ETC`）、工作区、技能文件夹。
- **数据根在临时目录里** → 改用 `<数据根>-sandbox-tmp`（兄弟目录，布局外）。
- **出厂私密清单**写死 `~/.miyu`（`miyu-fs/src/private/shipped.rs:12,67,100,158`，「旧版 Miyu 的数据」），出厂只禁写名单写死工作区里的 `.miyu`（`readonly/shipped.rs:9`）。

### 3.2 边界表（`miyu-fs/src/boundary.rs`）

- `data_root` → `Zone::Forbidden`；工作区、`own`（`USER_DIRS`）落在数据根里的照工作区算（:200-216）。
- `shelves`（开着的技能文件夹，真实位置）→ `Zone::Shelf`，只读（:194）。由 `miyu-session/src/skills.rs:119` 给、`guard.rs:180-190` 装。
- `knowledge()`：`<数据根>/kb/<库>/<…>` 的形状写死（:244-257）。
- `blocks_descent`：走目录的工具穿过数据根只为到达工作区/`own`（:262-275）。
- 附件目录：本会话 `attachments/` 只读，别的会话的照数据根禁（`guard.rs:166-170`）。
- `Confines`（放行出沙盒的工具的文件必须落在）= `own_places()`＋`real_packages()`＋资源目录（`miyu-session/src/unsandboxed.rs:62-71`；`miyu-store/src/links.rs:44-50` 同一组）。

### 3.3 Windows 沙盒（MXC）与「命令起来前必须已经存在」

机制（`miyu-sandbox/src/carve.rs:1-40`、`docs/blueprint/sandbox/windows-run.md` 第 4 步）：MXC 的禁止压过嵌在里面的放开，所以数据根里有要透出的（`write`/`reveal`）时不整条禁数据根，而是**一层层禁旁边那几项**；路上那几层落在整盘只读里，**命令跑着时在路上那几层新建的东西读得到**；只禁命令起来那一刻已有的。数据根落在能写的里面（工作目录 `~`）时整条禁（SW-9）。

**必须事先存在的**（代码里实际保证的）：

| 什么时候建 | 建什么 | 代码 |
|---|---|---|
| 第一次用数据根（`prepare`） | `.miyu-root`、`state/`、`run/` | `root.rs:251-283` |
| 核心起来（`start.rs`） | `workspace/`、`trash/`、`blobs/`、`modules/`、`index/`、`kb/` | `miyu-core/src/start.rs:24-29` → `root.rs:139,149` |
| 每条沙盒命令写规格前（`keep_places`，有标记才建） | `run/`、本会话 `sessions/<会话>/jobs/` | `miyu-session/src/sandbox.rs:250-259` |
| 每条命令/脚本前 | 本会话默认工作区 `workspace/<会话>/`；脚本的 `cache/…`、`state/scripts/…`、四个 XDG 目录、`images-*` | `sandbox.rs:262-268`；`miyu-basesystem/src/shell/script.rs:190-215` |

**整条禁的**：`run/`（从不透出东西，`config_file.rs:243-247` 注释）；`hidden` 的私密目录（去掉被独占打开的文件，SW-1）；数据根顶层命令起来时已有、又不通往透出项的每一项（`.miyu-root`、`config.toml`、`secrets.toml`、`trust.toml`、`journal.jsonl`、`extensions.json`、`access.json`、`logins.json`、`projects.json`、`terminal.json`、`kb.toml`、`avatar.*`、`trash/`、`blobs/`、`modules/`、`index/`、`kb/`、`personas/`、`presets/`、`commands/`、`prompts/`、`tools/`…）。

**按代码推断的暴露面**（未实测，只从 carve 逻辑推）：会透出东西的「路上那几层」里新建的读得到——`workspace/`（工作区）、`sessions/` 与 `sessions/<本会话>/`（附件目录存在时；会话日志换段的新段、第一次建的 `memory/` 在这一层）、`skills/`、`packages/`、`packages/<包>/`、`scripts/`、`cache/`、`state/`、`state/scripts/`（脚本跑时）。`state/` 是路上那一层时，命令跑着时第一次建的 `state/rates.db`、`state/providers/`、`state/mcp/` 等读得到。

**已知、先不处理**（`windows-run.md:56` 原话要点）：`secrets.toml`、`access.json`、`logins.json` 第一次新建的那一下——在数据根顶层、命令跑着时才第一次建出来的，不在禁止的里面，这条命令跑完以前读得到（密钥原文、访问密码的 argon2 哈希、设备令牌的哈希）；已经有的改写不碰（改名盖上的照路径禁、读不到，11-02 虚拟机上量过）；只有命令正在沙盒里跑着时第一次设密钥（第一次 `miyu provider login`、MCP 第一次 OAuth、第一次 `secret.set`）、第一次设访问密码、第一次登记设备才会碰到。同理（推断）同样适用于其他第一次新建的顶层文件：`config.toml`、`trust.toml`、`extensions.json`、`projects.json`、`terminal.json`、`journal.jsonl`、`kb.toml`、`avatar.*`。

另：Windows 沙盒安装读 `state/sandbox/`，逐层查不是链接（`install/files.rs:41-75`）。

### 3.4 程序目录、资源目录在沙盒里只读

- 没有专门「资源目录只读」的规则：它们不在 `write` 里，沙盒整体可读，所以只读（Linux/macOS）；Windows 是「每个固定盘的盘根 reveal 进只读路径」（`windows-run.md` 第 4 步）。
- 关起来的会话只给 `PROGRAMS`（Linux `/usr /bin /sbin /lib /lib32 /lib64`）＋`ETC` 几样（`miyu-session/src/sandbox/confined.rs:19-75,85-100`），资源目录不在 `/usr` 下时读不到——未查到专门处理。
- 文件工具的边界表：资源目录不在 `readable`，只有系统目录（`boundary.rs:54-78`）。`Confines` 把资源目录算作「主人管得住的」（`unsandboxed.rs:62-71`）。
- 助手在主程序旁边（`miyu-sandbox/src/locate.rs:1-12`）。
- R3S-6 审计原文：仓库里**未查到**（只在 `docs/construction/README.md:824` 提到「审计 R3S-6 的小项：找 shell 时 PATH 里相对路径的那几项不看」）。

### 3.5 核心起来时建的、迁移、标记

- 建的：见 3.3 第一张表；另 `leftovers::tidy`（`miyu-core/src/leftovers.rs`）看 `packages/` 和 `run/`＋顶层清临时文件；`packages::clear_uploads` 清 `blobs/tmp/upload-*`；`trash::purge`、`cache::sweep`。
- **标记版本**：写 `layout 2`（`root.rs:50`），注释「现在只认文件在不在」；仓库里**没有读版本号的代码**（`grep` 只在 `root.rs:50,268` 和 `root/tests.rs:334`）。3-1 施工单写「以后布局要变，再定怎么读它」。
- **迁移代码**：产品与仓库里都没有（SU 施工单：「产品里不认旧布局，也不自己挪」「没发过版，不为旧布局写搬家」）。仓库外有两个一次性工具：`~/.cache/miyu-remake/su-migrate`（layout 1→2，写新标记 `main.rs:238`）、`~/.cache/miyu-remake/rename-migrate`（场所改编号/包改名，README 列了它碰的 20 来处路径，可当清单参考）。xtask 里没有 `su-migrate`。SQLite 库都是「版本不对删掉重建，不写迁移」（`sqlite.rs:3`、`index.rs:3`、`recall.rs:43`），`schedules.db` 除外（真相）。

### 3.6 测试夹具的量级（粗计，`grep` 测试文件里 `join("<名>"…` / `"<名>/` 的次数；`packages`/`skills`/`tools`/`prompts` 还混着资源目录的）

| 名字 | 次数 / 文件数 | 名字 | 次数 / 文件数 |
|---|---|---|---|
| packages | 339 / 110 | state | 83 / 41 |
| scripts | 182 / 24 | kb | 65 / 28 |
| skills | 161 / 41 | run | 59 / 15 |
| tools | 106 / 60 | workspace | 31 / 14 |
| personas | 101 / 36 | sessions | 27 / 17 |
| prompts | 99 / 22 | blobs | 26 / 9 |
| config.toml | 93 / 34 | cache | 23 / 9 |
| presets | 70 / 36 | kb.toml | 22 / 11 |
| commands | 68 / 12 | recall | 22 / 17 |
| secrets.toml | 18 / 11 | logs | 18 / 10 |
| trash | 17 / 7 | index | 13 / 7 |

测试文件共 1708 个 `.rs`（含 `tests.rs`、`tests/`、`testkit`）。

### 3.7 其他写死的

- `miyu-client` 把 `DataRoot`、`cache_root` 再导出给头（`crates/miyu-client/src/lib.rs:55`、`open.rs:111-158`）：头（web、tui 等，heads.toml 钉版本）编进了 `run/token`、`run/socket`、`run/core.lock`、`run/spawn.lock` 的布局。改 `run/` 要连头一起换。
- 套接字指纹用数据根路径算（`miyu-ipc/src/place.rs:57`），数据根挪位置指纹就变。
- `config.toml` 第一行的 `#:schema state/config/config.schema.json` 已经写进主人的文件里。
- 会话日志里记着绝对路径：附件 `path`（`attach/keep.rs`）、工作目录（默认的就是 `<数据根>/workspace/<会话>`）；数据根改位置/改布局时这些不会跟着变。

## 四、对外暴露数据根内部路径的地方

| 渠道 | 名字 | 值 | 代码 |
|---|---|---|---|
| 扩展环境变量 | `MIYU_HOME` | 数据根 | `miyu-endpoint/src/extensions/supervise.rs:290` |
| 扩展/包程序环境变量 | `MIYU_PACKAGE_STATE` | `state/packages/<包>/` 真实位置（扩展的 cwd 也是它） | `supervise.rs:293`；`miyu-cli/src/packages.rs:211` |
| 同上 | `MIYU_PACKAGE_DIR` | 包文件夹（装的在 `packages/<包>/`，出厂的在资源目录） | `supervise.rs:292`；`miyu-cli/src/packages.rs:210` |
| 同上 | `MIYU_RESOURCES`、`MIYU_PROGRAM` | 资源目录、主程序 | `supervise.rs:283,291` |
| 脚本环境变量 | `MIYU_SCRIPT_STATE_DIR` | `state/scripts/[@场所/]<来处>/<工具>/` | `miyu-basesystem/src/shell/script.rs:82`；`miyu-store/src/scripts.rs:102-115` |
| 同上 | `MIYU_SCRIPT_CACHE_DIR` | `cache/[@场所/]<来处>/<工具>/` | `script.rs:76`；`scripts.rs:117-132` |
| 同上 | `MIYU_SCRIPT_IMAGES_DIR` | `cache/…/<来处>/images-<hex>/` | `script.rs:79`；`script/images.rs:37-41` |
| 同上 | `MIYU_PICTURES_DIR` `MIYU_VIDEOS_DIR` `MIYU_DOCUMENTS_DIR` `MIYU_DOWNLOADS_DIR` | 数据根顶层四处 | `miyu-basesystem/src/shell/script/home.rs:15-18` |
| 命令/脚本环境变量 | `MIYU_SESSION_LOG` | `sessions/<会话>/`（没关起来的会话；沙盒里其实读不到） | `miyu-basesystem/src/shell/env.rs:46,59`；`miyu-session/src/open.rs:168` |
| 握手 | `host.workspace`、`host.dirs.{pictures,…}` | 真实位置 | `miyu-endpoint/src/hello.rs:304-327` |
| 协议 | `skill.list` 的 `dir` | `skills/` | `miyu-endpoint/src/skills.rs:78` |
| 协议 | `script.list` 的 `dir` | `scripts/` | `miyu-endpoint/src/scripts/list.rs:49` |
| 协议 | 软件包项的 `state` | `state/packages/<包>/` | `miyu-endpoint/src/packages/item.rs:50` |
| 协议 | `package.local` 的 `dir` | 包目录 | `miyu-endpoint/src/packages/local.rs:193` |
| 协议 | 会话列表 `default_workspace` | `workspace/<主会话>/` | `miyu-endpoint/src/list/defaults.rs:41` |
| 协议 | 配置/检查的 `file`：数据根里的写成相对数据根（`config.toml`、`secrets.toml`、`trust.toml`、`journal.jsonl`、`tools/x.toml`…），命令行再接回 | `miyu-endpoint/src/check.rs:354`；`miyu-cli/src/config/paths.rs:1-40` |
| 会话日志/协议块 | 附件的绝对 `path` | `sessions/<会话>/attachments/<名>` | `miyu-endpoint/src/attach/keep.rs`、`miyu-store/src/attachments.rs:35` |
| 技能 shelf | 开着的技能文件夹真实位置进权限策略、沙盒 `reveal` | `skills/<名>/`、`packages/<包>/skills/<名>/` | `miyu-session/src/skills.rs:119` |
| 命令行直接读 | `state/packages/.sync/index.json`、`.sources/*/index.json` | | `miyu-cli/src/packages/offered.rs:17-26` |
| 服务单元 | `MIYU_HOME=…` | | `miyu-cli/src/service/plan.rs:100-122` |
| 外部包（不在本仓库） | `run/web.lock`、`run/web` | 网页包自己写（store.md 说） | 未查到（核心仓库） |

## 五、`state/usage.db` 能不能完全重建

- 代码注释（`miyu-store/src/usage.rs` 头部）：派生的；真相是会话日志的 `model.called` **加上** `journal.jsonl` 里的 `usage.purged`、`usage.oneshot`；补的时候读 `journal.jsonl`、数据根里的会话、**回收处里的会话**（`usage/catch_up.rs:105,127,139,405-409`）；空表照这样补满就是重建。
- 所以：删掉的会话在回收处真删以前，用量照日志重建；真删以前往 `journal.jsonl` 写 `usage.purged`（一小时一个（供应商、模型、用途）的合计，`trash.rs:132`），之后只能照这一条重建（细到每次请求的丢了，只剩小时合计）。一次性调用（`miyu ask` 之类不落会话的）只记在 `journal.jsonl` 的 `usage.oneshot`。**结论：能完全重建的前提是 `journal.jsonl` 在；`journal.jsonl` 本身不可重建。**
- 汇率 `state/rates.db` 是单独的缓存（`usage/rates/cache.rs`），重建要上网重取。
- 文档里的「用量账本」：只有作为说法出现——`docs/designs/45-定时和循环.md:27`（「照用量账本的写法开库」）、`docs/blueprint/models.md:938` 和 `docs/construction/8-34-供应商改名.md:11`（「用量账本照日志重算，不动」）、`docs/reviews/2026-10-10-0.0.1差距清单.md:46`（「思考 token 进用量账本」推荐放到预览版以后）。
- **UP-1**：`origin/main` 的 docs、施工 README、各远端分支的 docs 里都**未查到**。

## 六、store.md 树里没写到的（汇总）

`projects.json`、`terminal.json`、`avatar.*`、`.config.toml.edit-*.toml`（`miyu config edit` 的副本）、`packages/<编号>.removed`、`packages/.<编号>.new|.old`、`trash/personas/`（只在方法表）、`sessions/<会话>/memory/{log/,turns.db,memory.db}`、`index/recall/turns-*.db|memory-*.db`、`index/kb/…`、`kb/<编号>/kb.toml`、`cache/…/images-*`、`cache/mermaid/`、`state/logs/memory.jsonl`、`state/logs/*.stderr`、`state/logs/mcp-*.stderr`、`state/rates.db`、`state/models/renamed.json`、`state/models/catalog.json`（`learned.json`/`providers/`/`pools.json` 只在下面的说明里）、`state/providers/<包>.json`、`state/mcp/<服务>.json` 与 `state/mcp/<服务>/`、`state/personas/pictures.json`、`state/sandbox-probe.json`、`state/sandbox/windows.json|windows-error.json`、`state/packages/.local/`、`.leftovers.json`、`.sync/`、`.sources/`、`.cache/`、`.unpack/`、`state/scripts/@<场所>/` 一层、`state/perf-cache`（测试工具）、`run/token.tmp|socket.tmp`、数据根外的 `<数据根>-sandbox-tmp`；机器缓存目录里的 `models-dev.digest.json`、`models/logos/`。
