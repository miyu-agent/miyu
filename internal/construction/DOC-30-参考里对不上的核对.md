## 施工单 DOC-30：协议参考和教程里前后对不上、中英对不上的 14 处，回代码核对后改正

状态：待验收（2026-11-20）。DOC-29 改写时子代理报了一批「原文自己前后对不上」「中文、英文两份对不上」的地方，改写那一步只换写法，没有逐条核实。这一步逐条回代码查清实际行为，文档错的照代码改。

### 怎么核的

派一个 opus 子代理只读核对（不改文件、不编译、不运行）：每一条给出实际行为、`文件:行号` 和那几行代码或测试的原文、判断是哪一边错。回来以后我抽读了其中六处代码（第 1、5、6、7、8、11 条），和它的结论一致，再动手改文档。

### 改了什么

| 位置 | 原来 | 代码里实际是 | 证据 |
|---|---|---|---|
| 会话页 `session.create` 出错表（中英） | `memory`「不是那三种取值之一」 | 只接受 `persona`、`off` | `miyu-policy/src/memory.rs` 的 `MemoryScope::parse` |
| 软件包页 `package.list`（中英） | 顶层 `state` 写成「状态」；`mcp` 一格先说只有 `name`、`state`，又说也没有 `state` | 顶层 `state` 是状态目录的真实位置；后一个 `state` 说的是它，不是服务的状态 | `miyu-endpoint/src/packages.rs` 的 `OWNER_ONLY`、`packages/item.rs` |
| 软件包页驱动自查的内置样本（中英） | 中文把私有数据只挂在思考上，英文只挂在工具调用上；说「各一份」六样 | 一共五份；带私有数据的那一份里，思考、供应商侧工具、工具调用三样都带 | `miyu-session/src/route/extension/check.rs` 和它的测试 |
| 扩展页环境变量、`pictures`（中英） | 「`MIYU_PICTURES_DIR` 这一类的四个目录」没说是哪四个；英文说在主人的家目录里 | `MIYU_PICTURES_DIR`、`MIYU_VIDEOS_DIR`、`MIYU_DOCUMENTS_DIR`、`MIYU_DOWNLOADS_DIR`，在数据根的 `root/` 下 | `miyu-store/src/root.rs` 的 `USER_DIRS`、`program.rs` |
| 扩展页 `sent` 的语音上限（中英） | 上限写 10 MiB，给 AI 的话写成「超过 10 MB」（不是原话） | 上限 10 × 1024 × 1024 字节；给 AI 的原话是 `The audio was not delivered: it is larger than 10 MB.` | `miyu-endpoint/src/provide/voice.rs`、`resources/core/tool-results/voice-too-big.txt` |
| 记忆页 `who`（中文） | `memory.remember` 其他类写了「也不接受」 | `memory.remember` 忽略；`memory.import` 才拒绝（写 `null` 等于没写） | `memory/write.rs`、`memory/import.rs` |
| 记忆页一条记忆的字段（中英） | 列表里没有 `people`、`place` | 回应里有这两格 | `memory/shown.rs` |
| 场所页 `venue.session` 的 `cwd`（英文） | 不写时用 the owner's default workspace | 用这个会话自己的默认工作区 | `sessions/places.rs` 的 `opening_cwd` |
| 连接页订阅流（英文） | `prefs` 一行没写推的是什么；`notices` 一行写 `claimed: true` | 推 `prefs.changed`；回应是 `{"claimed": true}` | `prefs.rs`、`subscriptions/listed.rs` |
| 配置页 `applies`（英文） | 五个取值只解释了一个 | 五个各是什么意思（照中文补全） | `miyu-config/src/item.rs` 的 `Applies` |
| 「写扩展」改文件的工具（中英） | 「Miyu 自己的数据目录在任何级别下都拒绝；私密目录在只读下读取要问人」 | 数据根不另外保护，按权限级别判；读取两种级别都放行；只有关在自己工作区里的场所会话，工作区以外一律拒 | `miyu-session/src/guard/levels.rs`、`guard.rs` 的 `paths`、`miyu-endpoint/tests/provide_paths.rs` |
| 「接 MCP 服务」第一段（英文） | 少一句只给网址的服务（Streamable HTTP） | 支持，写 `url` | `miyu-config/src/package/mcp.rs`、`miyu-endpoint/tests/mcp_url.rs` |
| 「写一个软件包」编号（中英） | 「最多 64 个」没有单位，没说不能用的名字 | 64 个字符（只许 ASCII）；不能是 Windows 设备名，不能是数据根里保留的 19 个名字 | `miyu-store/src/layers.rs`、`miyu-config/src/package/reserved.rs` |
| 界面设置页 `prefs.json`（中英） | 「头自己不要用这个名字」，没说后果 | 硬冲突：头写进去的会被当成设置读出，格式不对的会被下一次 `prefs.set` 盖掉 | `miyu-store/src/prefs.rs` |

### 交主会话核对以后补进来的

核对时另外看到两处，文档定不了，交主会话派人查（查了代码并在沙箱里实测），结论补进这一步：

- 清单的 `prefix`：明写时最多 32 个字符；不写时前缀就是包的编号，这时不查 32 个字符（编号最长 64 个字符，实测 40 个字符的编号带设置项正常）。清单页 `prefix` 一格写明。代码不改。
- 记忆回应里的 `by` 只给种类，一共四种：`tool`、`module`、`person`、`harness`（原来只写了 `person`、`tool` 两种）。记忆页那一格补全。
- 「写扩展」改文件的工具：场所会话里真实位置不在工作区的读写一律拒绝，数据根也在其中（另有防御测试守着），那一句写明。

### 核实

- 文档线的检查脚本问题 0 个（中英 62 页一页对一页，1560 个链接都指得到）。
- 这一步只改文档；`manifest.md`、`mcp.md`、`scripts-skills.md`、`errors.md` 这四页有测试读，改动没有碰测试核对的字段名和 `reason`。
- 没有运行程序。

### 验收

1. 上表每一行，文档现在的说法和「证据」那一列的代码对得上。
2. `cargo xtask check` 全过，三台 CI 全绿。
