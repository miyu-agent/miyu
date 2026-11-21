## 施工单 UP-3：用量的用途有给人看的名字

状态：做完，等验收（分支 `step/CB-2`）。项目主人第 39 条（截图：终端 `/usage`「主对话 94% · judge 5% · memory 1% · compaction 1% · 起标题 <1% · join-approval <1%」中英混杂）。派工时叫 UP-1，和已有的「花费分四种」撞号，改叫 UP-3。

### 目的

`usage.query` 照 `purpose` 分组时回应里只有 `purpose`（自由字符串），头只能照原样印或者自己猜。改成每一行带 `purpose_name`（照连接的语言）：核心自己的照字表，扩展写的照包清单声明的，都没有的不带。

### 蓝图改哪几节

- `models.md`「协议」`usage.query`：多一条 `purpose_name`，例子带上；「守着它的」加一行。
- `packages.md`：清单样子多 `[[purpose]]`；`process` 能写的表；出错码 `bad_purpose`；放在哪、守着它的各一行。
- `protocol.md`：`usage/names.rs` 一行。
- 协议参考 `config.md`（中英）`usage.query` 的回应；包作者文档 `manifest.md`（中英）多 `[[purpose]]` 一节。

### 做法

1. 核心的名字（字表 `core/human` 三语的 `usage/purpose/<用途>`，照 UX-1 写软件说法的名词短语）：代码里实际写得出来的用途是 `recap` 回顾、`title` 起标题、`compaction` 压缩、`memory` 记忆整理（内核的整理会话、记忆模块的一次性调用）、`vision` 看图、`embedding` 向量化；`purpose` 是 `null` 的那一行（主请求，连同子代理的、当场压缩的）照 `main`「主对话」。有哪几种就看字表里有哪几条，代码里不另列名单。`subagent`、`other` 只在 `use` 分组里出现，这一步不给 `use` 带名字，不写；`judge`、`join-approval` 是 im 的，核心不替它起名。
2. 扩展的名字：清单多一种表 `[[purpose]]`（照 `[[side]]` 的写法，一段一条）：`id` 同 `model.call` 的 `purpose`（1 到 32 个小写字母、数字、`-`，两处共用 `miyu_config::package::purpose_written`），`name` 分语言。只有扩展包能写，不另要能力；写错报 `bad_purpose`。几个包声明同一个的照包的编号排第一的，核心自己有的扩展改不了名。
3. `miyu-endpoint`：`usage/names.rs` 照这一刻装着的包、连接的语言给名字；只在照 `purpose` 分组时读字表。

### 给 im、头的

- im 清单：
  ```toml
  [[purpose]]
  id = "join-approval"
  name = { en = "Join review", zh = "入群审核" }

  [[purpose]]
  id = "judge"
  name = { en = "Judge", zh = "判官" }
  ```
- 头：`usage.query {"group": ["purpose"]}` 每一行多 `purpose_name`（字符串，照连接的语言），没有的不带，照 `purpose` 原样印；`purpose` 是 `null` 的那一行带「主对话」。

### 验收

1. 改前红：`miyu-endpoint` 的 `usage_names::rows_by_purpose_carry_a_name_in_the_connection_s_language`（把写名字的那一句拿掉，每一行的 `purpose_name` 都是 `null`）。另加 `miyu-config` 的 `package::purpose::tests` 三条。
2. 实测（沙箱数据根，真模型是中转站的 `opencode-go/deepseek-v4.1-flash`）：一个声明了 `join-approval` 的假扩展照 `join-approval`、`plain-test` 各调一次 `model.call`，本机会话说一轮（自动起标题），中、英、日三个连接 `usage.query {"group":["purpose"]}` 看 `purpose_name`。
3. 门禁十一项、长跑随机、Windows 目标的 clippy、`cargo xtask prompts`、`cargo xtask logs`。

### 不做什么

- 不给 `use` 分组带名字（没要；要的话同一张字表加 `subagent`、`other` 两条就成）。
- 核心不替扩展起名：im 的 `judge`、`join-approval` 由 im 清单声明。
