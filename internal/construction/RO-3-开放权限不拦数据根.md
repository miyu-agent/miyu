## 施工单 RO-3：完全放开下文件工具不再拦数据根

状态：做完，等验收（分支 `step/OA-1`）。项目主人 2026-11-17 本机验收第 1 条定。派工时叫 DR-1，和「整理会话记下结果」撞号，接 RO-2 叫 RO-3。

### 目的

项目主人：完全放开下拦数据根只拦得住自带的文件工具，拦不住命令，等于没保护；只读下的保护是对的。定「拿掉工具拦截，留删除确认」：

- 完全放开下 `write`、`edit`、`trash` 这些文件工具写、改、删数据根里的东西同别的目录一样，不再拒。
- 只读照旧哪儿都不写。
- 删整个数据根照旧拦：`shell` 认出的高危命令 `delete_data_root` 不变；`trash` 补上「数据根和装着它的不删」（原来靠权限策略拒写数据根顺带挡着）。
- 只碰得到自己工作区的会话（群、陌生人的私聊）照旧碰不到数据根，头那一边的 `fs.*`、`blob.put` 照旧不给。

### 摸底

| 在哪 | 原来 | 现在 |
|---|---|---|
| 权限策略判法表 `miyu-session/src/guard/levels.rs` 的 `mark` | `(DataRoot \| Shelf, _, 写)` 拒 `Forbidden`，别的照级别 | 不看在哪一片：只读的写拒，别的放行。`Mark::Forbidden` 删 |
| `guard.rs` 照路径判 | `Forbidden` 写给她 `forbidden.txt`，知识库的文档写 `forbidden-kb.txt` | 两句都不用了；边界表只给只碰得到自己工作区的会话判 |
| 给模型看的字 `core/permissions/forbidden.txt`、`forbidden-kb.txt` | 拒写数据根、知识库文档时的两句 | 删。知识库那一句原是为了把她从 `trash` 指到 `kb_delete`（2026-10-23 实测），现在 `trash`、`edit` 改知识库文档照常成，索引照文件的指纹补（`knowledge.md`「怎么走」第三条），没有单独留它的理由 |
| 给人看的 `core/permissions/forbidden`、`forbidden-kb`（三语） | 两句 | 删 |
| 策略快照 `PermissionTexts` 的 `forbidden`、`knowledge`，`GuardTexts` 的两个方法 | 两格两句 | 删；以前造的快照里有这两格，读的时候跳过（serde 不认的字段不拦） |
| 边界表 `Places::knowledge`、`Boundary::knowledge` | 判是不是知识库文档，只给上一行用 | 删，`Places::here` 只收数据根 |
| `trash` 不许删的 | 工作目录和上级、家目录、根目录 | 加上数据根和装着它的（照这次调用带的数据根的真实位置） |
| 高危命令 `delete-data-root` | 灭顶的，直接拒 | 不动 |

### 给模型看的字

| 文件 | 原来 | 现在 |
|---|---|---|
| `core/permissions/forbidden.txt` | `"{path}" is inside Miyu's own data, which no tool can change.` | 删 |
| `core/permissions/forbidden-kb.txt` | `"{path}" is a knowledge base document. Change it with the knowledge base tools: kb_edit, kb_delete.` | 删 |
| `software/basesystem/trash/protected.txt` | `… the home directory, or the root.` | `… the home directory, Miyu's data or a folder holding it, or the root.`（o200k，路径按 `~/x`：27 → 37）|

都只在被拒时出现，不进前缀。登记簿 `prompts.md`、设计 26 第十节跟着改。

### 蓝图改哪几节

- `session/guard.md`：文件表、边界表 `DataRoot` 一行、先后第 0、2、3 条，判法表（两行）和下面那段，拒绝时写给她的、给人看的字，测试表。
- `fs.md` 的 `Zone`、边界表第 1 条；`tools/trash.md` 开头、第 4 条、两张字表、测试表；`skills.md` 两句。
- `kernel/tools.md`、`policy.md`、`store/resources.md` 里 `forbidden` 的几行；`prompts.md` 登记。
- 协议参考 `sessions.md`（中英）：`session.set_permission_level` 那一段。
- `docs/guide` 由写文档的会话另改，这一步不碰。

### 验收

1. 改前红（在 `origin/main` fcfa565b8 上跑，四条红）：
   - `miyu-session`：`guard.rs` 的 `the_data_root_is_read_at_every_level_and_written_at_full_access`、`guard/knowledge.rs` 的 `a_knowledge_base_document_is_written_like_any_file_at_full_access`、`secrets_readable.rs` 的 `file_tools_write_the_data_root_only_at_full_access`（完全放开 `write` 建新文件、`edit` 改密钥都成，只读都拒、文件不动）。
   - `miyu-basesystem`：`trash.rs` 的 `the_data_root_itself_is_not_deleted`。
2. 照旧的：只碰得到自己工作区的会话碰不到数据根（`guard_confined.rs`、`sandbox_member.rs`）、链接调包（`guard_checked.rs`）、删数据根的高危命令（`hazard.rs`）照旧绿；以前造的快照带着那两格照读（`miyu-policy` 的 `a_snapshot_with_the_data_root_texts_still_reads`）。
3. 门禁十一项、`cargo test -p miyu-kernel random:: -- --include-ignored`、Windows 目标的 clippy、`cargo xtask prompts`、`cargo xtask logs`。

### 不做什么

- 高危命令 `delete-data-root` 不改：照旧灭顶直接拒（见下「拿不准的」）。
- 只读的沙盒、外部身份的会话、头那一边的端点不动。

### 拿不准的

- 项目主人定的是「留删除确认」，派工时说「`delete-data-root` 照旧弹确认」；代码里它从施工 5-17 起就是灭顶直接拒（人点了也不跑，2026-10-20 项目主人定），不是问人。这一步照「照旧」不动它，比确认更严。要改成问人（同删工作目录，归「受保护的命令」的删除那一类）另说。
