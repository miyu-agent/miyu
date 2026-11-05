## XG-2 工具职能：照声明认，不照名字

状态：XG-2上 施工完，待 core1 审（2026-11-05，sub1）；XG-2下 未开工。设计 `64-工具职能.md`（core1 2026-11-05 定）；蓝图 `tools/interface.md`「规格」、
`session/tools.md`「工具面」、`skills.md`；设计 26 第十节登记簿。

### 目的

核心有几处照工具的名字做决定：只有叫 `read` 的在才列技能，名字在常量表里的那几件才只给本机会话。第三方的同类工具因此不被认，违反
设计 00 第四节「内置不开后门、加东西只登记」。改成工具在规格里声明职能（`role`）和范围（`scope`），核心照声明认。

### XG-2上（这一步）

- **内核**（`miyu-kernel/src/tool.rs`）：`Role`，封闭的表，现在只有 `read_file`。认不得的读不进。规格和快照两边都要，所以放在这里；
  内核自己不看它。
- **规格**（`miyu-tool`）：`Spec` 多 `role: Option<Role>`、`scope: Scope`。`Scope` 有 `any`（默认）、`local`、`local_main`、`local_main_attended`、
  `spawn` 五个取值，也是封闭的。说明文件（`SpecFile`）认 `"role"`、`"scope"` 两个键，写了认不得的值整份读不进。原来 120 处 `Spec`
  字面量补上这两格；测试用的 `Fake::scoped`。
- **自带的说明文件**写上：`read` 是 `read_file`；`send_message`、`session_usage`、`todowrite` 是 `local`；`sessions`、`goal` 是 `local_main`；
  `ask_user` 是 `local_main_attended`；`subagent` 是 `spawn`。设计 26 登记簿这 8 行换指纹，备注写明加的键不发给模型、token 不变；
  `prompts.md` 照它重新生成。
- **工具面**（`miyu-session/src/agents.rs` 的 `face()`）：原来照七件的名字特判，改成照声明的范围筛。范围只多拿掉，原有的限制照旧算：
  提供者的 `venues`、子代理的层数。记忆、知识库那两组照定的不改。
- **快照**（`miyu-policy` 的 `ToolEntry`）：多 `role`，没有的不写（`skip_serializing_if`），老快照字节不变；发给模型的 tools 数组只有
  名字、说明、参数三格，不受影响。
- **技能列表**（`miyu-session/src/skills.rs`）：工具面上有一件声明 `read_file` 的才列，好几件只看有没有。快照里一件都没写职能的
  （XG-2 以前建的）照名字 `read` 认（`LEGACY_READ`，只这一处）。

### 怎么验

- `skills` 单元：第三方的读文件工具叫什么都行、好几件只看有没有、没有读文件的不列；老快照照名字 `read` 认。原来几条照旧过。
- `miyu-tool` 的 `load`：写了的职能、范围读进规格，没写的没有职能、范围是 `any`；认不得的职能 `write_file`、范围 `everywhere` 整份读不进。
- `miyu-policy` 的 `tools`：没有职能的不写这一格、老的读得懂；有的读回来一样；拆给模型的那一份里没有。
- 集成（`miyu-session/tests/tool_scope.rs`，真会话）：
  - 声明 `local`、`local_main` 的在本机主会话有、在群会话里没有；
  - 第三方声明 `any`、`venues` 只给本机的写工具，群会话里照旧没有（core1 要的「范围不放宽」）；
  - 自带那几件的说明文件写的范围、职能对得上；
  - `tests/skills.rs`：工具面上没有叫 `read` 的、只有第三方声明 `read_file` 的 `open_file`，技能照样列进 system。
- 受影响的 crate 测试全过，clippy 两种目标都没有警告。

### 变异

手写 12 个，都抓住：
- 老快照不照名字认；
- 一律照名字认；
- `local` 当 `any`；
- `local_main` 当 `local`；
- 不看有没有人；
- `spawn` 当 `local`；
- 组工具面时不把职能写进快照；
- 说明文件的范围不读；
- 说明文件的职能不读；
- 没有职能也写这一格；
- `read` 不声明职能；
- `subagent` 不声明范围。

「组工具面时不把职能写进快照」第一次没抓住：测试的工具面里都有叫 `read` 的，新快照没写职能就落到老快照那条退路，照名字认到了。补了
`skills.rs` 那条第三方 `open_file` 的真会话测试，抓住了。原来守着名字特判的几条测试（`sessions`、`questions`、`messages`）照旧守着
换成范围以后的样子。

### XG-2下（下一步）

- 扩展经 `provide` 交的工具（`ToolParams`）、包里的脚本工具（注释头）认 `role`、`scope`，认不得的拒 `bad_params`、跳过并记一行。
- 参考 `docs/reference/`（中英）、作者文档 `extensions`、`scripts-skills` 写怎么声明。
- 设计 15 第二节、蓝图 `models.md` 第八条写 Zen 的占位为什么照旧照名字。

### 坑

- **两棵工作树共用 target，同名 crate 的测试程序会互相盖**：同时开着 RL-5，在一棵里跑 `cargo test -p xtask` 跑到的是另一棵编的。跑之前
  `touch` 一下这棵的源文件再编，核对通过的条数。
- **职能现在只有一个**：写了职能的工具面一定有读文件的那件，所以「快照里一件都没写职能」等于「XG-2 以前的快照，或者这个会话
  没有读文件的工具」。后一种照名字 `read` 再找一次也找不到（自带的 `read` 写了职能），只有第三方一件不写职能、却叫 `read` 的会被当成
  读文件的：和以前照名字认一样，不比以前差。
