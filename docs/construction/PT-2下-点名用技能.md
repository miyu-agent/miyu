## 施工单 PT-2 下：人点名用技能，两个「谁能用」的字段

状态：写好了（2026-10-29）。主会话 2026-10-28 派：PT-2 差距报告（Agent Skills 接入对照）的 F1、F2。开工前报过要碰的文件，主会话认了（SU-5 上在同几个文件里删账号参数，后合的变基）。图纸 `docs/blueprint/skills.md`「技能」第 1、2、6 条，`protocol.md` 的 `command.catalog`。

### 目的

1. 人想「这次照这个技能做」只能自己写一句让她去读：agentskills.io 接入指南把「用户点名用技能」列为客户端该做的一条。
2. 别家技能常写 `disable-model-invocation`、`user-invocable`（Claude Code 定的两格），我们原来不认，作者以为生效了其实没有。

### 做了什么

1. `miyu-policy` `skill.rs`：读两格进 `SkillHead`（`model_invocable`、`user_invocable`；不是是非的照默认）；新的 `body()` 交回去掉开头的正文。
2. `miyu-session` `skills.rs`：system 里技能那一块不列只许人点名用的（文件夹照旧放开只读，人点名用了以后她读得到技能里写到的别的文件）。`path_text` 公开给端点用。
3. `miyu-endpoint`：
   - `commands/skills.rs`（新）：`command.run` 登记表、模板、扩展声明的都没有这个名字时照技能认（`templates::run` 最后一步）；`user-invocable: false` 的认不出。看预设（功能 `skill-<名字>`，关着的拒 `command_not_allowed`、说是哪个预设），读 `SKILL.md`，去掉开头，套外壳当人说的那一句发出去，`message.user` 的 `command` 是人敲的那一行。
   - `command.catalog` 列能点名用的技能，带 `"skill": true`；和命令撞名的、预设关着的不列。
   - `miyu check` 报能点名用却和命令同名的技能（`skill_command_clash`，三种语言的字）。
4. 外壳三份字（`core/skills/invoked-open.txt`、`invoked-close.txt`、`invoked-arguments.txt`），提示词登记簿第十节登记（28、3、11 token，整块 42 加正文），`prompts.md` 重新生成。
5. 图纸 `skills.md`、`protocol.md`，协议参考 `packages.md` 中英，作者文档中英。

### 不在这一步

- **点名用的那一次压缩以后不重读回来**：报告写的「这一次算模型读过这个技能（压缩时照 D1 保住）」没做，另开一步 **PT-2 补**（主会话 2026-10-29 定，排在 SU 之后）。压缩只认她用 `read` 读、效果里标着技能名的那一笔（`file.read` 的 `skill`）；点名用的技能是人说的那一句，要算进去，PT-2 补的范围：
  - `message.user` 另记一笔点名用了哪个技能（名字、`SKILL.md` 的路径），内核的事件格式加一格、读写和格式演进照 `03-事件模型.md` 第八节；
  - 样本（`kernel/events.md` 的样本块）、内核的随机测试跟着改；
  - 重建（`session/rebuild.rs` 的 `skills_before`）把这一笔和 `file.read` 的 `skill` 一样算进「用过的技能」，压缩以后照它重读回来；
  - 端点 `commands/skills.rs` 发的时候带上这一笔。
- **PT-2 末（技能列表的总预算）撤了**：和项目主人 2026-10-12 定的「多大都收」冲突（施工 X-12 下撤过一次预算，`skills.md`「做法上定的」第三条）；主会话 2026-10-29 照建议定了不做，不再去问。
- `argument-hint` 给技能：Claude Code 的技能也认这一格，菜单里名字后面跟什么的提示。报告没提，不做。
- 头的补全菜单：`command.catalog` 带了 `"skill": true`，头怎么显示由头定，已经通知终端、网页。

### 验收

1. `miyu-policy` `skill/tests.rs`：两格读法（默认、各自关、不是是非的照默认）；正文去掉开头和空行、BOM 和 `\r\n`、没有开头的、没收尾的。
2. `miyu-session` `skills/tests.rs`：只许人点名用的不列、文件夹照放开；全是这种的整块不放。
3. `miyu-endpoint` `tests/skill_invoke.rs`：模型收到的外壳、正文、`ARGUMENTS` 一行，时间线上是敲的那一行，没写参数的没有那一行；`user-invocable: false` 认不出、不进菜单；只许人点名用的点名用得了、不进 system；和模板命令撞名的照模板；菜单那一项；预设关着的拒、不列；`miyu check` 报撞名（内置、模板），不许点名的同名不报。
4. 原有的 `command_catalog.rs`、`memory_installed.rs` 比命令清单的三处：`command.catalog` 现在也列出厂 `authoring` 包带的两个技能，那几处只看命令，不算 `"skill": true` 的。
5. 手写变异 11 个全抓住（第一轮有一个变异脚本把源文件改坏没改回来，后面三个编不过被当成「活了」，改好重跑全抓住）。
6. `cargo xtask check`、Windows 版 clippy；三平台 CI。
