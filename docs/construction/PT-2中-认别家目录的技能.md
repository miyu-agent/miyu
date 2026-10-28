## 施工单 PT-2 中：认别家共用目录里的技能

状态：写好了（2026-10-28）。主会话 2026-10-28 派：PT-2 差距报告（Agent Skills 接入对照）的 A1，SU-3 下（技能只认主人家目录和包带的）合了以后做。只做 `~/.agents/skills/`；报告里待问主人的三件（`~/.claude/skills/`、项目目录里的技能、`allowed-tools`）不做。`miyu-session` 的 `open`、`miyu-endpoint` 的 venues、connection/checks、attach/reach、sessions 不碰（SU-4 中在改）。图纸 `docs/blueprint/skills.md`。

### 目的

别的客户端（Codex、opencode、Claude Code 新版）照 agentskills.io 的约定把技能装进 `~/.agents/skills/`。Miyu 原来只认自己的目录，那些技能要人手动拷进来。

### 做了什么

1. `miyu-store`：
   - `DataRoot` 记下找数据根时家目录下的 `.agents/skills/`（`shared_skills()`），没有家目录的没有。放在数据根上是因为找技能的几处（含 `miyu-session` 的 `open`）手上都只有数据根，不改 `skills::find` 的参数、不碰 `open`。测试的数据根没有家目录，碰不到本机真的 `~/.agents/skills/`。
   - `skills::find` 最后找它：自己放的、包里的、别家共用的，同名的先找到的赢，被盖住的照旧记下被谁盖住（`miyu check` 照旧报）。链接的围栏照自己放的那一处（数据根外面的都认，数据根里只认自己放东西的几处）。找到的标着 `shared`。
2. `miyu-endpoint`：
   - `skill.list` 的 `from` 多一种 `agents`，不带包。
   - `skill.remove` 删别家共用目录里的拒 `skill_shared`（带 `data.name`）：别的客户端也在用，到那边删。
3. 图纸 `skills.md`（顺手把「找」那一行里施工 SU-3 下拿掉了的系统区去掉），协议参考 `packages.md`、`errors.md` 中英，作者文档中英。

### 不在这一步

- `~/.claude/skills/`、项目目录里的 `.agents/skills/`、`allowed-tools`：等主人定（PT-2 报告 A1 后半、A2、D 节）。
- 改动监视不盯 `~/.agents/skills/`：技能那一块钉在快照里，列表变了走回合尾巴（施工 PT-2 上把技能拿出了改动监视）。

### 要知道的

- 拉起真核心的测试（`miyu`、`miyu-cli`）照真的家目录找，开发机上有 `~/.agents/skills/` 的会多认几个技能。现在这些测试都不看技能、不比 system 的字节，没有影响；以后有了要比的，在测试里给核心换家目录。
- 头（网页、终端界面）要认 `from: "agents"`、`skill_shared`：不认的照未知的来处、未知的拒绝显示。

### 验收

1. `miyu-store` `skills/tests.rs` 的 `the_shared_agents_folder_comes_after_miyu_s_own`：共用目录里的认、标着；同名的自己放的、包里的赢，共用那一份记下被谁盖住；没有家目录的不找。
2. `miyu-endpoint` `tests/skill_list.rs` 的 `skills_in_the_shared_agents_folder_say_where_they_come_from`（`from` 是 `agents`、不带包、同名的照自己放的）；`tests/skill_remove.rs` 的 `a_skill_in_the_shared_agents_folder_is_not_removed`（拒 `skill_shared`、文件还在）。两处修前红。
3. 手写变异 6 个全抓住：不找共用目录、共用目录排最前、不标共用、设了 `MIYU_HOME` 就不找、列表不写 `agents`、删的时候不拦。
4. `cargo xtask check`、Windows 版 clippy；三平台 CI。
