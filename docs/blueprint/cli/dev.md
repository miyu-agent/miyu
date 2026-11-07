## `miyu dev`

### 是什么

给写软件包的人用的几样（施工 MD-3）。现在只有 `driver-check <包编号>`：查这个包提供的模型驱动编码稳不稳（设计 `66-扩展提供模型驱动.md` 第七节第 2 条）。连上核心（没在跑就拉起），发 `driver.check`，把每个驱动、每份样本过没过印在标准输出上。比法、样本、重启都在核心（协议参考 `packages.md`「`driver.check`」），命令行只印。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `dev`、`dev driver-check`；换上帮助页 |
| `crates/miyu-cli/src/dev.rs` | 参数、找数据根、连核心、握手、发、排成几行（`report`）、退出码 |
| `crates/miyu-cli/src/language/dev.rs` | 给人看的字 |
| `crates/miyu-cli/src/help/{zh,en}/dev.txt` | 帮助页（`cli/main.md`「帮助页」），`dev`、`dev driver-check` 的 `-h` 都印它 |

### 对外的样子

| 参数 | 做什么 |
|---|---|
| `driver-check <包编号>` | 查这个包的模型驱动：每份样本编两次、重启扩展再编一次、末尾加一条消息再编一次。**会重启扩展**；有会话正经它的驱动调模型时不查（核心回 `driver_busy`） |

**给人看的字**：一个驱动一行（`<包>/<驱动名> (<缓存类型>) passed|failed|skipped`，中文是「通过」「没通过」「跳过」），下面一份样本一行（`ok plain`、`failed pair: <原因>`，中文是「通过」「没通过」）；原因是核心的英文原话，接在冒号后面。结果行不带句号。

**退出码**：都过了、只有跳过的 0；有没通过的 1；被拒（没装、没声明驱动、没在跑、正在用）照核心的原话，1。

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-cli/src/dev/tests.rs` | 排成的几行（中英）、原因接在后面；有红的不算过，只有跳过的算过 |
| `crates/miyu-endpoint/tests/extension_driver_check.rs` | 核心那一头：编得稳的都过、扩展真重启了；放时间戳的、重启以后才变的、稳定区之前就变的各红各的；没实现的 `contract` 红、别的跳过；正在用的 `driver_busy`、不重启 |
