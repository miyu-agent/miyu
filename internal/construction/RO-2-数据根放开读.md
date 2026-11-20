## 施工单 RO-2：数据根两种模式都读得到

状态：做完，等合（分支 `step/RO-2`）。项目主人 2026-11-16 定。

### 目的

只读就是「完全放开减去写」：完全放开读得到的，只读都读得到，只读只多一条「不能写」。Miyu 的数据根（`~/.miyu`，连同存 key 的 `etc/private/secrets.toml`）在两种模式下都放开读，文件工具（`read`、`glob`、`grep`……）和命令都读得到；写数据根照旧哪一级都不行。项目主人在三个选项里明选了「文件工具也放开读」。

前一步 PV-RM 留下的「数据根哪一级都碰不到、只读的沙盒藏它」照新定的改。

### 摸底清单（动手前列的）

**一、挡读的：放开**

| 在哪 | 原来 | 现在 |
|---|---|---|
| 边界表 `miyu-fs/src/boundary.rs` 的 `Zone::Forbidden` | 「谁都不能碰」 | 改名 `Zone::DataRoot`：只说「这一处在数据根里」，读写怎么判由用它的一方定 |
| 权限策略判法表 `miyu-session/src/guard/levels.rs` 的 `mark` | 数据根读写都拒 | 读放行，写拒（和 `Shelf` 一样） |
| 只读的沙盒规格 `miyu-session/src/sandbox.rs` | `hidden` 只有数据根那一路（Linux 落在能写的里面不藏，macOS、Windows 照藏）；`reveal` 放开技能文件夹、附件目录；每条命令前补上 `run/`、`jobs/`（只为 Windows 只禁起来那一刻已有的） | `hidden`、`reveal` 都是空的；`hidden()`、`hides_inside()`、`keep_places`，`Sandbox` 的 `attachments`、`jobs` 和 `ToolKit` 的那两格随着删（只给它们用）。`Spec` 的 `hidden`、`reveal` 和各平台助手照旧（外部身份的会话还藏数据根） |
| 往下走目录的篱笆 `miyu-basesystem/src/walk.rs` 的 `Fence`（`glob`、`grep`） | 从上面走下来不进数据根；起点在数据根里也不进它的子目录（只有工作目录那一片照进），所以 `grep path=~/.miyu/etc` 搜不到 `private/` 里的 | 只挡从上面路过的数据根（同版本库目录一个道理，不算藏）；起点在数据根里（含等于）的照常往下走。`Fence::from` 只留落在起点下面的数据根，`Fence` 的 `workspace` 一格和 `held` 随着删（2026-11-16 主会话定，见下「`glob`、`grep` 为什么跳过路过的」） |

**二、挡写的：留着**

| 在哪 | 怎么挡 |
|---|---|
| 权限策略 `mark`：`(DataRoot \| Shelf, _, 写)` | 两种模式都拒，写给她 `forbidden.txt`（知识库的文档 `forbidden-kb.txt`） |
| 只读的沙盒：`write` 是空的 | 整盘只读，数据根也写不进 |
| 高危命令 `delete-data-root` | 完全放开下删数据根照旧直接拒 |
| 工具执行时只碰判过的位置（施工 5-18） | 判完以后链接被换成指进数据根的，照旧不碰 |

**三、不在这一步的（照旧挡数据根）**

- 头那一边的协议端点：`fs.list`、`fs.find`、`fs.read`（`path_forbidden`）、`blob.put` 读本机文件（`attachment_in_data_root`）。它们给人、给扩展用，不是她的文件工具；照 `Zone::DataRoot` 判，行为不变（见最后一节第 2 条）。
- 外部身份的会话（群、陌生人的私聊）：只碰得到自己的工作区，沙盒照旧藏数据根。
- 完全放开下命令本来就不进沙盒、读得到数据根（施工 LV-1），不变。

**四、给模型看的字、给人看的字**

| 说法 | 原来 | 现在 |
|---|---|---|
| `core/permissions/forbidden.txt` | `"{path}" is inside Miyu's own data, which no tool can read or change.` | `"{path}" is inside Miyu's own data, which no tool can change.` |
| `core/permissions/forbidden-kb.txt` | `… Read and change it with the knowledge base tools: kb_read, kb_edit, kb_delete.` | `… Change it with the knowledge base tools: kb_edit, kb_delete.`（读放开了，只在写时出现） |
| `core/permissions/forbidden`（人） | 这是 Miyu 自己的数据，谁都不能碰 / Miyu's own data, off limits / 誰も触れられません | 这是 Miyu 自己的数据，不能改 / Miyu's own data, can't be changed / 変更できません |

两句都只在被拒时出现、不进前缀；token 各少 2、5 个（o200k），登记簿 `prompts.md` 跟着改。

### `glob`、`grep` 为什么跳过路过的（2026-11-16 主会话定）

照「放开读」的定义本该不跳过；动手前在本机真实的 `~/.miyu` 上量过，从上面走进去有明显问题，停下报告：

- 本机数据根约 3.6 GB，会话日志 208 MB；`grep TODO` 多出 33 个文件（会话日志 5 个、运行日志 1 个、blob 1 个，别的是工作区、回收站里的）。
- 她这个会话的日志里先记下这次调用（连同搜的字）才跑，从家目录搜什么都回回搜到自己的日志。
- `glob` 新的在前：核心一直在改的索引、通讯平台的库、日志排到最前面。

三个选项：A 不跳过；B 从上面路过时跳过、起点在数据根里照常搜；C 照旧全挡（和「读放开」对不上）。主会话定 B：同默认不进 `.git`、`target` 一个道理，不算藏，点名读、在数据根里起搜都照常。

### 照「读放开」认下的连带（2026-11-16 主会话认）

- 本机令牌 `run/token` 读得到。只读的命令在 Linux（Landlock 第 9 版以上或 seccomp）、macOS 上连不上核心的套接字，拿到也用不上；Windows 的命名管道没量过；完全放开下命令本来就连得上。
- 预设里关掉的技能文件夹读得到（只是不列）；别的脚本的状态、缓存目录本机只读时读得到（写不进）；别的会话的附件读得到。
- `audit_ext_commands.rs` 里「脚本通道里读被拒（`code=1`）」那一格没了：本人的会话里读不再有被拒的，读数据根改成 `code=0`。
- 头那一边的 `fs.read`、`blob.put` 照旧不给数据根（远程的头经网页连进来，不顺手读到密钥）。

### 蓝图改哪几节

- `session/guard.md`：边界表 `DataRoot` 一行、先后第 0、2、3 条，判法表数据根拆成读、写两行，拒绝时写给她的那一句，给人看的字，测试表。
- `fs.md`：`Zone`、边界表先后第 2 条和下面两条、太宽那一条的「数据根靠沙盒藏住」，测试表。
- `session/tools.md` 第 1a 条（只读什么都不藏；补 `run/`、`jobs/` 那一步去掉）、文件表、测试表。
- `sandbox.md` 的 `hidden`、测试表；`sandbox/linux.md`、`sandbox/macos.md`、`sandbox/windows-run.md` 里说「藏数据根」的几句。
- `kernel/tools.md` 的两行；`prompts.md` 的两条登记；`protocol.md` 两处措辞。
- `tools/glob.md` 第 4 条、测试表，`tools/grep.md` 测试表。
- `tools/read.md`、`tools/packages.md`、`tools/send_message.md`、`tools/show.md`、`skills.md` 里「数据根碰不到」的几句。
- 协议参考 `sessions.md`（中英）：`session.set_permission_level` 写明两种模式读得到的一样多、数据根写不进。
- `docs/guide`、`docs/dev` 由写文档的会话另改，这一步不碰。

### 验收

1. 改前红（在 `origin/main` 0feda574c 上跑，十一条红）：
   - `miyu-session` `secrets_readable.rs`（PV-RM 的 `secrets_unreachable.rs` 改名改写）：`the_read_only_spec_hides_nothing`、`file_tools_read_the_secrets_at_either_level`、`file_tools_cannot_write_the_data_root_at_either_level`（拒的话只说改）。
   - `sandbox.rs`：`each_level_gets_its_own_spec`、`commands_really_run_in_the_sandbox`（只读的命令读得到数据根）、`a_read_only_command_reads_the_secrets_but_cannot_write_the_data_root`（真经助手：`cat` 密钥读得到，`>>` 密钥、`touch` 数据根里的新文件都写不进）。
   - `guard.rs` 的 `the_data_root_is_read_at_every_level_but_never_written`；`guard/knowledge.rs` 的 `a_knowledge_base_document_reads_like_any_file`、`a_knowledge_base_document_is_refused_with_the_kb_tools_named`（新的那一句）。
   - `miyu-basesystem` `walk_home.rs` 的 `searching_inside_the_data_root_finds_the_secrets`（起点 `data/etc`，`private/secrets.toml` 那一行搜得到）、`glob.rs` 的 `a_workspace_inside_the_data_root_is_searched_all_the_way_down`（起点是数据根照常走）：在旧的 `walk.rs` 上跑是红的。新加的 `searching_from_above_passes_the_data_root_by`（从上面搜下来不出数据根里的）守的是不变的那一半，改前改后都绿。
2. 写照旧挡：`secrets_readable.rs` 完全放开、只读下 `write`、`edit` 写数据根都拒、文件不动；`guard_checked.rs` 的链接调包、`hazard.rs` 的删数据根照旧绿。
3. 门禁十一项、`cargo test -p miyu-kernel random:: -- --include-ignored`、`cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings`、`cargo xtask prompts`、`cargo xtask logs`。

### 实测（真模型，沙箱数据根）

环境：`cargo xtask dev-home` 造的数据根（`MIYU_HOME`、`HOME` 都指草稿目录里的沙箱），中转站的 `relay/deepseek-v4-1-flash-261017`，`etc/private/secrets.toml` 放假 key `FAKE-KEY-RO2-SANDBOX-0001`；这一版 `cargo build` 出来的 `miyu`、`miyu-sandbox`。工具结果原文（会话日志里的）：

| 级别 | 叫她做的 | 结果 |
|---|---|---|
| 只读 | `read` 读 `secrets.toml` | ok，`1\tdeepseek = "FAKE-KEY-RO2-SANDBOX-0001"` |
| 只读 | `shell` 跑 `cat` 它 | ok，`deepseek = "FAKE-KEY-RO2-SANDBOX-0001"` |
| 只读 | `shell` 跑 `touch <数据根>/etc/ro2-note.txt` | error，`touch: 无法 touch '…/data/etc/ro2-note.txt': 权限不够`、`Exit code 1` |
| 只读 | `write` 写同一个文件 | denied，`Not run: the session is read-only.` |
| 完全放开 | `edit` 改 `secrets.toml` | denied，`"…/data/etc/private/secrets.toml" is inside Miyu's own data, which no tool can change.`；界面上的说法「这是 Miyu 自己的数据，不能改」；文件没变 |
| 只读 | `grep` 起点 `<数据根>/etc` 搜 `FAKE-KEY`（`content`；这一趟数据根放在沙箱 `HOME` 里的 `.miyu`，假 key 是 `…-0002`） | ok，`…/home/.miyu/etc/private/secrets.toml:1:deepseek = "FAKE-KEY-RO2-SANDBOX-0002"` |
| 只读 | `grep` 起点沙箱 `HOME`（包住数据根）搜同一个词 | ok，`No matches found`；这时她这个会话的日志里已经有 4 处 `FAKE-KEY`（这次调用的参数），路过的数据根照旧不进，所以没搜到自己的日志 |

测完按记下的进程号停掉自己起的核心；key 只放在命令的环境变量里，草稿目录、工作树里数过没有。

### 不做什么

- 头那一边的端点不改（上面第三节）。
- 外部身份的会话照旧藏数据根。
- `Sandbox` 的 `workplace`（施工 LV-1 起就没人读）不在这一步删，另列。

### 拿不准、要拍板的

- 没有了：`glob`、`grep` 那一条主会话定了 B，连带的几条照「读放开」认下（上面两节）。
