## 施工单 ENV-1：新内置包「注入环境信息」

状态：主会话 2026-10-26 审过草稿、定了四条，扩展线做。前提：U23 末下合了。

### 口径（项目主人 2026-10-26，主会话转）

1. 新内置包，编号 `env`，三语名：注入环境信息 / Environment info / 環境情報。不是必需的：整包关掉就是不注入任何环境信息。
2. 基础系统里的七个功能挪进去：`env-time`、`env-workdir`、`env-permission`、`env-session`、`env-model`、`env-harness`、`env-machine`。功能编号不改，预设、个人设置里写的 `env-*` 照认。
3. 再加一个功能「用户身份」：现在的「关于你」那一块（称呼、更多描述、回复语言，`core/profile.txt`），关了就不拼。
4. 出厂预设跟着核对，开着的环境信息和挪之前一样。
5. 前缀：开着、关着的字节都和挪之前一样。
6. 不并进这一步：「偏好思考语言」（要先实测怎么写才有约束力，单独一步）；基础系统别的功能拆包是 BS-1（设计 44，主会话做）。还没发版，不做迁移。

### 为什么

C-11 把环境那几块拆成了能开关的功能，但它们挂在必需的基础系统下面：想一点环境信息都不给（群聊）只能在预设里逐个关，「关于你」还关不掉。拆成一个不必需的包，整包关掉就干净。

### 定论（主会话 2026-10-26）

1. 「用户身份」编号 `env-profile`，和七个同一个前缀，`miyu_policy::shown` 一处管；不叫 `profile`（会和配置段名、`miyu_policy::profile` 混）。
2. 开不开改成「装着、并且没被关」：原来是「快照里记的没开的功能（`PresetPin::off`）里没有它就开」，整包关掉以后功能不在装着的清单里，照旧会算成开着。
3. 「用户身份」关了整块不拼，回复语言那一行一起。
4. 整包关掉时 system 里跟着这几块的那几行（`shown` 管的）也一起不拼：和八个都关逐字节一样。
5. 设置页、预设页上这几个开关显示在「注入环境信息」下面：`preset.get` 的每个功能加一格 `package`（协议只加不改）；顶上不带包名表，头照 `package.list` 的名字查，BS-1 给 `package.list` 加组时再看要不要带。

### 做了什么

- 新包 `resources/packages/env/package.toml`：`[builtin]`、不写 `required`；七张 `[features.env-*]` 原样从基础系统挪过来，加 `[features.env-profile]`（用户身份 / User identity / ユーザー情報）。基础系统那七张删掉。`miyu-core` 的 `built_in()` 加 `env`。
- `miyu_policy::shown`：加 `PROFILE`、`PACKAGE`、八个的 `ALL`、`absent(features)`（八个里没装的）；`Shown` 多一格 `profile`。`Shown::of` 照快照的 `off` 和 `absent` 两样，都不在才算开。
- 快照 `PresetPin` 多 `absent`（空的不写，以前的快照、以前的程序读写一字不变；算进 `means_the_same`）。`Chosen` 造的时候照装着的功能算好，钉快照时带上。整包关掉（`package.disable env`、个人 `packages.env.off`）以后装着的功能里没有这八个，全记进 `absent`。
- 「关于你」：造会话拼它时看 `Shown::of(preset).profile`，关着就不拼（快照里就是没有，同没填任何一项）。会话中途照旧不改。
- 出厂预设：`dev.toml` 的 `[software]` 加 `env = true`；`minimal.toml` 的 `[features]` 加 `env-profile = true`；「全部功能」`unlisted` 开着，不用动。
- `preset.get` 的每个功能多一格 `package`：同预设 `[software]` 认的那个编号（机主私人的脚本、MCP 服务、自己放的模板命令是 `~home`），不归包的技能、没装的没有。
- 配置样例重新生成（多一组 `[packages.env]` 的个人关掉开关，U23 末上带来的）。
- 文档：图纸 `protocol.md` 的 `preset.get`、协议参考 `personas.md` 中英两份，图纸 `presets.md`「环境那几块」、`packages.md` 内置包那一行，设计 30 第三节第 6 条、设计 26 第四节第 2 块和登记簿 `core/profile.txt` 一行，`prompts.md` 重新生成。

### 不做的

- 头按 `package` 分组画：合了以后告诉终端、网页两个头，它们自己改。设置页「软件包」那一页本来就按包分组，`env` 那一组是 U23 末上的「把这个包关掉」。
- 迁移：家目录里自己写了 `unlisted = "off"`、`[software] basesystem = true` 的预设，环境那几块原来跟着基础系统开，现在要另写 `env = true` 才开（「用户身份」同样）。还没发版，照规矩不做迁移，写进图纸 `presets.md`「环境那几块」。
- `request_shape_probe` 的五张脸都是全部功能，没碰；「整包关」和「八个都关」逐字节一样由 `env_package.rs` 用真核心比快照的 system 证明。

### 验收

1. `crates/miyu-policy/src/shown/tests.rs`：`absent` 里的和 `off` 里的一样算关；`absent()` 只列八个里没装的。
2. `crates/miyu-endpoint/tests/env_package.rs`：全部功能的快照 system 有「关于你」；只关「用户身份」的只少那一块；八个都关的连环境那几行也没有；`package.disable env` 以后新会话的 system、`Shown` 和八个都关一字不差；`preset.get` 里八个的 `package` 是 `env`、`files` 是 `basesystem`，没装的没有这一格。
3. `crates/miyu-store/src/presets/tests.rs`、`crates/miyu-endpoint/tests/presets.rs`、`preset_memory.rs`：出厂三份预设开着的环境信息和挪之前一样。
4. `crates/miyu-assemble/tests/probe*.rs` 存档一字不变。
5. 变异各红一次：`Shown::of` 不看 `absent`；「关于你」不看开关；`dev.toml` 少 `env = true`；`absent()` 列错；`means_the_same` 不比 `absent`；`preset.get` 不写 `package`。
6. `cargo xtask check` 全过，Windows 版 clippy 过。
