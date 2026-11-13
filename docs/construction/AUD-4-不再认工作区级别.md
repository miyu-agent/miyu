## 施工单 AUD-4：不再认「工作区」这个权限级别

状态：做完，等合（分支 `step/AUD-4-去掉工作区级别`）。来源：AUD-2、AUD-3 的施工单「不做什么」里留着的那一条。项目主人 2026-11-13 定：记着这个级别的旧会话直接删，核心不留读它的那一行。

### 目的

权限 A16 起只剩「只读」「完全放开」两级（施工 LV-1），以前的「工作区」`workspace` 一直照「完全放开」读，是为旧会话、旧配置、没跟上的头留的。这一步去掉，`workspace` 从此同别的不认识的取值一个待遇。

| 在哪 | 原来 | 现在 |
|---|---|---|
| 日志、快照里的级别（`Level` 的读法） | `workspace` 读成 `full` | 读成不认识的取值（`Level::Other`），原样留着、原样写出去；用到它的地方按只读算 |
| 权限那一块事实认上一块说的是哪一级 | `read_only`、`workspace`、`full` 三种都认 | 只认 `read_only`、`full` |
| 协议 `session.set_permission_level` 的 `level` | `"workspace"` 照 `"full"` 收 | `bad_params` |
| 配置 `permission.level` | 选项是 `full`、`workspace`，新会话开局照它（A16 起写什么都是完全放开） | 整项去掉：写着的照不在清单里的键报 `unknown_key` 警告（同施工 LV-2、LV-5 拆掉的 `sandbox.*`）；三种语言的字表去掉这一项，参考文件、Schema 的样本重新生成；开局级别直接写完全放开，`opening_level` 去掉 |
| 命令行 `miyu ask --permission` | `workspace` 是 `full` 的别名 | 没有这个别名 |
| 协议 `session.create` 的 `chosen` | 照收不看（施工 5-14 起就不看了） | 参数里没有这一格；头再发的，照「参数里不认识的格不理」（`protocol.md` 外壳那一节）不理，回应、行为都和不写一样，不回 `bad_params` |

`chosen` 一并去掉：两个头都不发了（网页 W-1178 去掉，终端早就不发，界面的会话 2026-11-13 说）。

### 蓝图改哪几节

- `kernel/events-bodies.md`、`kernel/blocks.md`：`level` 的取值。
- `kernel/request.md`「事实」认上一块的那一条；顺手把那一页里 `permission-rule.txt` 的原文照资源改对（施工 LV-1 改了资源，这一行没跟，还写着工作区那一句）。
- `session/guard.md`「实际生效的那一级」和测试表。
- `protocol.md` 的 `session.set_permission_level`、出错表、测试表；协议参考 `sessions.md` 中英两份。
- `config.md`：`permission.level` 那一条改成「没有这一项、写着的报警告」，三种语言的字的表去掉它，参考文件的样本块照重新生成的文件换。`protocol.md` 文件表里 `sessions/opening.rs` 那一行。
- 设计 11 A16 的批注。
- `cli/ask.md` 的 `--permission`。
- `protocol.md` 的 `session.create` 参数表、协议参考 `sessions.md` 中英两份：去掉 `chosen` 那一行。`fs.md` 挑工作区那一句还写着施工 9-7 的「人明着选的照用」，照施工 5-14 改对。

### 先后（都办了）

1. **数据**（2026-11-13，项目主人定全删，走 Miyu 自己的删除、进回收处 7 天）：主人的两份数据里记着 `workspace` 的会话，先前日常用的那份 1615 个、现在在用的那份 353 个进了回收处；现在在用的那份还剩 5 个 9、10 月从旧版导进来的群会话没删成（所在的群没有机器人号接着：核心不让接入程序以外的连接碰群会话，接入程序只删它认得的），没人会打开它们。别的十来个验收、演示留下的数据目录没管。
2. **迁移程序**：它拼 `session.created` 写的是 `workspace`，不改的话以后导进来的会话全是只读。`miyu-migrate` 的 MG-20 改成写 `full`，0.1.3 当天发了（项目主人同意）。
3. **两个头、接入程序**：现在的代码都不发 `workspace`（终端读核心回的级别时还认它，是它自己的事，已告诉界面的会话）。

### 影响面

- 日志里最后记的级别是 `workspace` 的会话，换上这一版以后是只读的：文件类工具写不了、命令在只读的沙盒里跑。会话照常载得进；切一次级别（发 `session.set_permission_level {"level": "full"}`，界面上就是点一下权限）就回到完全放开。
- 这样的会话再往后发的请求，前缀里以前那几块权限事实不变（原文记在日志里）；下一个边界照现在生效的只读再写一块。
- 配置里写着 `permission.level` 的（不管写什么）：报一条 `unknown_key` 警告，核心照常起，新会话照旧完全放开开局。主人的配置里没有。
- 终端读首页的开局级别时 `config.get` 带着 `permission.level`：这一版以后整个调用回 `unknown_config_key`，首页停在默认的完全放开（本来就是它）。终端改成只读 `permission.start_read_only`，由主会话告诉界面的会话。

### 不做什么

- 确认时选「此目录不再询问」的 `workspace`（`Decision::Workspace`）是另一回事：那是授权记到工作区上，不是权限级别。
- 5 个没删成的群会话、别的数据目录里的旧会话：不为它们留读法。

### 验收

1. `cargo xtask check` 全过；分支推上去三平台 CI 全绿。
2. 改了说法的测试：
   - `miyu-kernel` 的 `the_old_workspace_level_is_unknown_like_any_other`：读成 `Other`、原样写出去、按只读算。
   - `miyu-session` 的 `the_level_in_effect`：`workspace` 和别的不认识的级别一样是只读。
   - `miyu-endpoint` `permission.rs` 的 `values_that_do_not_fit_are_bad_params`：`"level": "workspace"` 是 `bad_params`。
   - `miyu-core` `settings.rs`：三种语言的字和清单对得上，生成的参考文件、Schema 和样本逐字节一样。
   - `miyu-endpoint` `check.rs` 的 `keys_removed_with_the_workspace_level_only_warn`：配置里写着 `permission.level` 的报 `unknown_key` 警告（改前红）；`config.rs` 的 `new_sessions_open_at_full`：写着它也照样完全放开开局。
3. `chosen`：`miyu-endpoint` `workspace.rs` 带 `chosen` 的那一次照旧成、说的一样（不认识的格不理）；别的测试里的 `chosen` 去掉。`miyu-cli` 的 `new_session_options_are_for_a_new_session_only`：`--permission workspace` 读参数时就拒。
4. 去掉的测试：`an_old_workspace_block_is_followed_by_the_switch_to_full`（事实认旧的一块）。测试数据里把 `workspace` 当级别写的十三个文件改成 `full`（它们测的是别的事，原来读出来就是完全放开）。

### 风险

换上这一版时数据里还有记着 `workspace` 的会话没处理的，那些会话变成只读。主人的两份数据处理过了；真碰上了切一次级别就好，不丢东西。
