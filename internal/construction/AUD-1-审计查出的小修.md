## 施工单 AUD-1：审计查出的小修

状态：做完，等合（分支 `step/AUD-1-审计小修`）。来源：项目主人 2026-11-13 让审计会话把工作区的仓库过一遍，找坏的、对不上的、白费的、不再要的。核心这边确信的几样在这一步修；要商量的、量大的写进审计报告交核心主会话排。

### 目的

1. Windows、macOS 的 CI 不再被两条偶发红的测试带红（main 最近 60 次里它们红了 3 次）。
2. 依赖表、蓝图、打包说明和现在的代码对得上。

都不改行为、不改协议、不改给模型看的字。

### 蓝图改哪几节

- `tools/shell.md`「脚本调工具」第 2 条：Windows 端点的两个空着的实例在开端点时就建齐。
- 指到没了的文件的几行照现在的写（下面第四节的表）。

### 一、`miyu-call` 的 Windows 端点：空着的实例开的时候就建齐

**现象**：`windows::tests::a_second_client_right_after_the_first_is_not_busy` 在 Windows CI 上两次红（main 的 42792498336、43187728678，两次改的都和它无关），都是「第 0 对连不上：Some(231)」。

**原因**：`Channel::open` 只同步建第一个实例，第二个空着的实例在 `tokio::spawn` 出去的 `accept` 任务里才建。测试开完端点马上连两个：`accept` 还没跑到，系统里只有一个实例，第二个拿到 `ERROR_PIPE_BUSY`。XG-1补2 说的「一直留两个空着的」在开端点到任务跑起来之间不成立。真命令起一个进程要几毫秒，平时撞不上；扩展作者自己写的客户端不一定重试，所以照根上修，不改测试。

**修法**：`open` 里把 `IDLE` 个实例都建好再交给 `accept`；`accept` 只管等它们、连上一个补一个。第二个建不成的，以前是记一行警告、端点悄悄没了（环境变量已经交给命令），现在 `open` 当场交回出错，照「建不成的交回出错，命令照旧跑，只是没有通道」走。

### 二、`route_extension_driver` 的两条测试照用途筛主请求

**现象**：`in_a_pool_the_extension_member_is_skipped` 在 macOS CI 上红过一次（main 的 43211309235）：日志里两条 `model.called`，第一条主请求成了，第二条 `purpose: Title`、`Connection refused`。

**原因**：测试拿 `calls.last()` 当主请求。这一轮说完以后核心另发起标题的旁路请求，假服务器只备了一份回复，旁路请求连不上；它赶在测试读日志以前落盘就排在最后。同文件 `an_absent_driver_is_no_model_and_a_pool_moves_on` 的后半段是同一种写法。同目录别的测试（`route_pools.rs`、`route_failover.rs` 这些）都先 `.filter(|call| call.purpose.is_none())`。

**修法**：加一个 `main_request`（最后一条不带用途的），两处断言照它。代码不动。

### 三、`miyu-basesystem` 去掉 `grep-matcher`

`crates/miyu-basesystem` 里一处都没用到 `grep_matcher`（搜内容用的是 `grep-regex`、`grep-searcher`，它是这两个的间接依赖，照旧在依赖图里）。`Cargo.toml` 去掉这一行，`Cargo.lock` 里 `miyu-basesystem` 的依赖表少一行；第三方许可证清单不变。

### 四、指到没了的文件、旧路径的

把蓝图、协议参考、作者文档里反引号写的仓库路径（3595 处）对着仓库查了一遍，指不到的这几处：

| 在哪 | 原来写的 | 现在 |
|---|---|---|
| `recall.md` 第 183 行 | `crates/miyu-embed/tools/bpe_files.py` | `miyu-agent/miyu-embed` 仓库的 `tools/bpe_files.py`（S-7 搬走了） |
| `protocol.md`、`mermaid.md`「守着它的」 | `crates/miyu-endpoint/src/queries/tests.rs` | `crates/miyu-endpoint/src/queries.rs` 里的测试 |
| `slash.md`「守着它的」 | 接入QQ 仓库的 `crates/miyu-onebot/tests/group.rs` | `miyu-agent/miyu-im` 的 `crates/miyu-im-app/tests/group.rs` |
| `sandbox/linux.md`「守着它的」 | `crates/miyu-sandbox/src/bin/miyu-sandbox/linux/tests.rs` 那一行 | 删掉：5-15 上起助手不再查规格、这份测试没了 |
| `http.md`「在哪」「数值」 | `crates/miyu-session/src/http.rs` | `crates/miyu-session/src/route.rs`（8-6 起，`IDLE` 在它里面） |
| `tools/write.md`、`tools/interface.md`、`protocol/undo.md`「守着它的」 | `crates/miyu-session/tests/restore.rs` | `crates/miyu-session/tests/undo_files.rs`；`write.md`、`undo.md` 那两行的说法跟着 U-1 改（撤销、恢复不改回文件） |
| `venues.md`、`README.md` | `miyu-agent/miyu-onebot` | `miyu-agent/miyu-im`（原名 `miyu-onebot`） |
| `packaging/README.md` | 运行日志在 `state/logs/`（两处）；「第二个本机入口（`run/public.sock`，施工 Y-1 下）做好以前」 | `var/log/`（设计 55）；第二个本机入口随多用户一起不做了（施工 SU） |
| `xtask/src/logs.rs` 三句理由 | `state/logs`、`state/models/`、`state/models/catalog.json` | `var/log`、`var/lib/models/`、`var/cache/models/catalog.json` |

`onebot.md`、`chat.md`、`packages.md` 里写着「搬走了」的那几句是当时的记录，不动。

### 不做什么

- `protocol/undo.md`、`tools/write.md` 别的行里和 U-1 有没有对不上的，没有逐句核，只改指到没了的文件的那几行。
- 代码注释里还写着旧布局路径的（`state/models/pools.json`、`state/usage.db` 这些，68 行），清单在审计报告里，交核心主会话排。
- 设计文档的索引缺行、施工单的状态没跟着改，同上。

### 验收

1. `cargo test -p miyu-session --test all route_extension_driver` 连跑不红。
2. `cargo clippy --target x86_64-pc-windows-gnu -p miyu-call --all-targets -- -D warnings` 过；分支推上去三平台 CI 全绿，Windows 那台的 `miyu-call` 测试过。
3. `cargo xtask check` 全过。

### 风险

Windows 端点只能在 Windows 上测：本机只过得了 Windows 版 clippy，行为靠 CI 那台。`open` 出错的路以前也有（第一个实例建不成），调的一方照旧处理。
