## 施工单 PT-2（上）：宽松地读技能，改技能不再误问「前缀要变」

状态：写好了（2026-10-28）。主会话 2026-10-28 照 PT-2 报告（Agent Skills 规范对照 Miyu 技能）拆的头一步。图纸 `docs/blueprint/skills.md`「技能」第 1 条、`docs/blueprint/protocol.md`「`session.send`」第 9 条。

### 目的

1. 别家写的技能（Codex、opencode、Claude Code 新版都认同一个格式）常有小毛病，我们原来一律不收：
   - 名字和文件夹不同名（从 zip 解开、手改了文件夹名）；
   - YAML 里没加引号、值里带冒号（`description: Use when: …`，最常见）。
   agentskills.io 的接入指南建议「警告、照收」，只有真读不成的才跳过。
2. PT-2 报告里读代码推断的一条误报（OV-1 交过来）：改了技能，声明了 `prefix_ask` 的头会收到「前缀要变了」，可技能那一块钉在快照里、中途变了走尾巴注入，前缀其实不变。

### 做了什么

1. `miyu-policy` 的 `skill.rs`：读开头改成宽松的，照收的记 `SkillWarning`（`SkillHead::warnings`），六种：
   - 名字和文件夹不同名：照写的名字用（`skill_name_mismatch`）。
   - 名字没写、写错：照文件夹的名字用（`skill_name_from_folder`）；两个都不合写法的才不收。
   - YAML 读不懂：把顶格、没加引号、值里带「冒号加空格」或以冒号结尾的那几行当一整句字重读（`skill_yaml_lenient`），还读不懂才不收。
   - 说明超过 1024 个字截掉（`skill_description_cut`）。
   - `compatibility` 不是字或超过 500 个字（`skill_compatibility`）。
   - `metadata` 不是字到字的表（`skill_metadata`）。

   不收的只剩：没有开头那一段、宽松重读也读不懂、两个名字都不合写法、没有说明。`SkillProblem::NameMismatch` 拿掉。
2. `miyu-endpoint`：
   - `miyu check` 一种警告一条，码同上，话照字表 `package-problems/<码>`（中英日三份）。
   - `skill.list` 每项多可选的 `warnings: [{"code", "message"}]`，话照连接的语言，只加不改。
3. 误报：先写测试证实（改一份已有的技能，头收到 `prefix_changing`），再把 `skills/` 从改动监视的几棵树里拿掉（`presets/edits.rs`）。修完测试守着：改技能不问、号不动、两轮的 system 一字不差。
4. 原来拿「名字不同名」「说明里带冒号」当坏例子的几条测试换成仍然不收的「没有说明」。
5. 图纸 `skills.md`、`protocol.md`，作者文档 `scripts-skills` 中英，协议参考 `packages`、`sessions` 中英，OV-1 施工单加注。

### 不在这一步

- 认 `~/.agents/skills/`：PT-2 中（等 SU-3 并好技能的层）。
- `/技能名`、`disable-model-invocation`、`user-invocable`：PT-2 下。
- 列表总预算：PT-2 末。（撤：和项目主人 2026-10-12 定的「多大都收」冲突，施工 X-12 下已经撤过一次预算；主会话 2026-10-29 照这个定了不做，不再去问。）
- `package-creator` 那份 `SKILL.md` 还写着 `package` 工具（PT-2 报告第六节第 4 条）：给模型看的字，改了要重量，另做。

### 验收

1. `miyu-policy`：`skill/tests.rs` 的 `small_slips_are_kept_with_warnings`（不同名、没写名字、名字写错、名字不是字、冒号带引号带反斜杠原样、只改读不懂的那几行、说明上限正好和多一个字、`compatibility` 超长和不是字、`metadata` 不是字到字、合格式的没警告）；`wrong_ones_say_what_is_wrong` 改成两个名字都不合的才不收；说明截了要说。
2. `miyu-store`：不同名的照收、照写的名字、文件夹照真实的；没有说明的跳过、说为什么。
3. `miyu-endpoint`：
   - `tests/prefix_ask.rs` 的 `an_edited_skill_does_not_ask_because_the_prefix_stays`：修之前是红的，`prefix_changing` 带着那份 `SKILL.md`。
   - `tests/skill_list.rs` 的 `slips_in_a_skill_come_with_warnings_in_the_people_language`：`skill.list` 带两条警告、话是中文，没警告的不带；`check` 报同样的两个码。
4. 手写变异 12 个：头一轮「说明的上限放宽一个字」「已加引号的值也套引号」两个没抓住，补了上限正好、差一个字两头，和「同一段里一行要宽松重读、另一行本来就加了引号」以后全抓住。
5. `cargo xtask check`、Windows 版 clippy；三平台 CI。

### 顺带看到的（没修）

改动监视在**新建的子文件夹**里写文件时没看到（测试头一回在新建的 `skills/notes/` 里写，号没变；改已有的文件就变）。照这个推断，`prompts/` 下新建子文件夹里放覆盖，也可能不问。交主会话定要不要单查。
