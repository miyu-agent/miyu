## 施工单 AUD-2：去掉旧兼容

状态：做完，等合（分支 `step/AUD-2-去掉旧兼容`）。来源：审计（AUD-1）查出核心里有一批只为「以前写的照旧读得进」留着的代码。项目主人 2026-11-13 定：还没发版，旧兼容的都去掉。这一步去掉不牵动别的仓库的四样；牵动软件包、两个头的列在「不做什么」，各要另排一步。

### 目的

去掉四样只为以前的写法、以前的日志留着的东西。去掉以后，旧写法同别的写错的一个待遇：不认识的格拒，不认识的键报，不认识的事件跳过。

| | 去掉的 | 原来 | 现在 |
|---|---|---|---|
| 一 | 方法 `tool.set_loading` | 施工 FL-1 起工具一律全量，它照旧收、照旧查参数，回 `{}`，什么都不改 | 没有这个方法：`unknown_method` |
| 二 | `need_load` 那一格 | 工具说明文件、扩展 `provide`、`mcp.set`、清单的 `[[mcp]]`、系统配置的服务都照旧收，不看 | 说明文件读不进；`provide`、`mcp.set` 回 `bad_params`；清单、系统配置里是 `unknown_key` |
| 三 | 「退役的键」那张表 | 配置里写了 `providers.<id>.compat.strict_tools`、`loading.tools.<name>`、`loading.mcp.<id>`、`memory.merge_sessions` 的不报 | 同别的不认识的键：`unknown_key`，警告，不进最终值 |
| 四 | 事件 `files.restored` 的类型 | 施工 U-1 起不再产生，类型留着读以前的日志 | 内核不认识这一种：读日志时同别的不认识的种类一样跳过 |

脚本开头的 `# Need load:` 本来就没有专门的代码（不认识的行不管），只改了注释和测试的名字。

### 蓝图改哪几节

- `protocol.md`：方法表去掉 `tool.set_loading`；`mcp.*` 一节 `mcp.set` 的参数去掉 `need_load`，写明 `tool.set_loading` 拿掉了、为什么。
- `mcp.md`：「系统配置里的一张表」、清单的 `[[mcp]]` 去掉 `need_load`；「守着它的」「来历」跟着改。
- `providers.md`「登记」、`extensions.md`、`skills.md`：规格、说明文件不再有这一格。
- `config.md`：读配置第 3 条去掉「退役的键不报」；`models.md`、`memory.md` 提到那几个键的照现在的写。
- `kernel/events.md`、`kernel/events-bodies.md`：去掉 `files.restored` 那一行、那一节（事件的种类数照代码是 41）。
- `kernel/history.md`、`kernel/session.md`、`kernel/blocks.md`、`kernel/request.md`、`tools/history.md`、`view.md`：讲改回文件的结局怎么记、怎么读的几段去掉，留一句「施工 U-1 拿掉了改回文件，施工 AUD-2 拿掉了类型」。`kernel/history.md` 里「改回的几步」一节（施工 U-1 起就没有对应的代码）跟着去掉，蓝图索引那一行照改；顺手补回「撤掉压缩」的小标题（施工 4-7 再补插「重做」一节时带丢了，页里几处照这个名字指它）。
- 协议参考 `packages.md`（那一节只剩 `feature.set`，锚点跟着改）、`extensions.md`，作者文档 `extensions.md`、`mcp.md`、`manifest.md`、`scripts-skills.md`，中英两份：去掉 `need_load`、`Need load`、`tool.set_loading`。
- 设计 02、03、04、10、25：原文不动，在讲到的那一句后面注明 2026-11-13 改了什么。设计 03 的事件样本 `samples/events/files.restored.jsonl` 删掉。

### 要拍板的（已定）

1. 协议 1 的方法清单（`docs/reference/methods-v1.txt`）写着只加不改不删，这一步从里面删了 `tool.set_loading`。依据是项目主人 2026-11-13 定的「还没发版，旧兼容的都去掉」：冻结是给发了版以后的头和扩展看的，现在外面没有照它写的程序。两个头的代码里已经一处都不调它（审计时查过 `miyu-tui`、`miyu-web` 的 main）。门禁照旧拿清单对代码，两边一起删所以不红。
2. 清单的 `[[mcp]]` 里写了 `need_load` 的包，以前装得上，现在读清单就拒（`unknown_key`）。照查过的影响面（下一节）没有这样的包。

### 影响面（2026-11-13 只读查的）

- 本机 14 个有会话的数据目录（主人先前日常用的那份 1627 个会话、现在在用的那份 492 个，别的是验收、演练留下的）：日志里没有一条种类是 `files.restored` 的事件（照 `"kind":"files.restored"` 搜；连备份、演示用的一共 142 个会话文件夹、6511 个会话再搜一遍也是 0）；配置里没有那四个退役的键，没有 `need_load`。搜的时候 `.` 要转义：现在在用的那份里有一个会话，模型的一条回复正文里有相近的字样，不转义会搜到它，它不是这种事件。
- 本机 305 份扩展登记缓存（`provide` 的原样）：没有带 `need_load` 的。真有的话照缓存登记不上，记一行 `WARN`，等扩展自己登记，不坏事。
- 官方软件源 44 个包、社区的 4 个包、工作区里各包仓库的清单和说明文件：没有 `need_load`。`miyu-sdk` 的 Rust、Python 两个开发包原来留着 `need_load` 这个参数（不传就不写进规格），同一天在它的仓库里去掉了（SDK-6，三平台 CI 过了，已在 `miyu-sdk` 的 main）。
- 日志里真有 `files.restored` 的：照常载得进。那一条原样留在日志里，载入时跳过，别的照常；它本来就不进上下文、不属于哪一轮。那一次撤销照样能恢复，接着说话照常答（`miyu-session` 的 `an_old_log_with_a_files_restored_line_still_loads_and_goes_on`：真会话撤销以后停下，照 U-1 以前的样子往日志里接一条，再载入）。

### 不做什么

**还留着的旧兼容**（牵动别的仓库，或者要先查数据、先商量，清单交核心主会话排）：

1. **起脚本、扩展时的旧环境变量名**（`miyu-store/src/program.rs`：`MIYU_PACKAGE_DIR`、`MIYU_PACKAGE_STATE`、`MIYU_SCRIPT_STATE_DIR`、`MIYU_SCRIPT_CACHE_DIR`、`MIYU_SCRIPT_IMAGES_DIR`，设计 55 的 FH-6 留的）：官方的 13 个脚本、技能包和 `miyu-render`、`miyu-web`、`miyu-im` 现在只认旧名字。要先把这些包改成新名字（`MIYU_APP_DIR`、`MIYU_STATE_DIR`、`MIYU_CACHE_DIR`、`MIYU_IMAGES_DIR`）、发了版、软件源收了，核心才能去掉旧名字。
2. **头的旧参数**：`session.create` 的 `chosen`（照收不看；终端、网页现在还在发）、订阅参数的 `scope`（照收不理；两个头已经不发）、配置 `permission.level` 的值 `workspace`（照收、读成 `full`）和 `miyu` 命令行 `--permission workspace` 这个别名、命令行装包前那句 `notice` 写成一个字符串的旧写法、`/goal edit`（照旧认、头不再提）。头还在发的要等头先改。
3. **旧数据、旧写法**（下面的数是 2026-11-13 在主人的两份数据里数的：先前日常用的那份 1627 个会话，现在在用的那份 492 个）：
   - 日志里以前记的权限级别 `workspace`（读成完全放开，`event/session.rs`）：1615 个、358 个会话的日志里有。去掉的话这些会话再打开会被当成只读，所以要先定它们怎么办。
   - 工具改名留的旧名字（`agent`、`message_agent`，`Tool::formerly`）：两份数据里都没有会话用过。
   - 预设里撤掉的两个键（`presets/write.rs` 的 `retired`）：没有预设文件写着。预设的 `[software]`：主人的预设里没有，出厂的 `dev.toml`、`minimal.toml` 自己还在用，要先改出厂的。以前的快照里照包编号记的没开的软件（`Features::read_legacy`）：没数。
   - 清单的 `workers`：装着的包里没有，出厂的 `base-memory` 自己还在用。`[theme] file`、写着 `kind` 的清单（`package/upgrade.rs` 改它）：装着的 41 个包里都没有。

**不算旧兼容、不动的**：

4. 事件里「以前的日志没有这一格」的可选格：格式往前长的正常写法。
5. 以前造的快照里工具带着 `need_load` 的（`miyu-policy/src/tools/tests.rs` 那一条测试）：快照本来就留着不认识的格，没有专门的代码，测试留着。

**这一步没补的文档欠账**：

6. 蓝图里还写着「正在改回文件」的二十来行（`compaction.md`、`cross-session.md`、`kernel/session.md`、`tools/sessions.md`）：施工 U-1 起没有这个状态了，只剩读回日志。这一步只改了 `kernel/history.md` 和别的页里提到 `files.restored` 的几句。
7. 蓝图 `kernel/events.md` 的种类表缺九种事件的行（`goal.*` 四种、`job.lines`、`package.state`、`shell.ran`、`todos.cleared`、`turn.joined`），这一步只把种类数改对。

### 验收

1. `cargo xtask check` 全过；分支推上去三平台 CI 全绿。
2. 改了说法的测试：
   - `miyu-endpoint` `script_pages.rs` 的 `set_loading_is_gone_and_tools_come_whole`：调 `tool.set_loading` 是 `unknown_method`，配置一个字不写，工具面照旧全量。
   - `miyu-tool` 的 `a_descriptor_with_need_load_is_refused_like_any_unknown_field`、`miyu-endpoint` 的 `need_load_is_no_longer_a_field_of_the_spec`、`mcp_edit.rs` 里交 `need_load` 的那一段、`miyu-config` 的 `need_load_is_an_unknown_key_now`。
   - `miyu-config` 的 `keys_taken_out_earlier_are_unknown_like_any_other`：四个退役的键照 `unknown_key` 报。
   - `miyu-kernel` 的 `files_restored_is_just_an_unknown_kind_now`：读得进、落得了盘，不当撤销的结局认。`miyu-session` 的 `an_old_log_with_a_files_restored_line_still_loads_and_goes_on`：带着这一条的日志载得进、能恢复撤销、接着用。
3. `grep -rn "set_loading\|_retired_need_load\|RestoreAction" crates/*/src` 只剩说明它拿掉了的注释。

### 风险

- 清单里写了 `need_load` 的包装不上了。查过的都没有；真碰上了报错里写着是哪个键、哪一行，删掉那一行就行。
- 协议 1 的清单少了一个方法：外面要是有照旧清单调它的程序，会拿到 `unknown_method`。现在没有发过版，两个头都不调。
