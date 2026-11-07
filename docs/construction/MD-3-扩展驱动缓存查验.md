## MD-3 扩展提供的模型驱动：缓存的查和记

状态：施工完，待 core1 审（2026-11-07）。设计 `66-扩展提供模型驱动.md` 第七节第 2、3 条，第十三节第 3 条。接在 MD-2（通道）后面。

### 定了的

1. **照设计**：`driver.encode`（参数同 `driver.call`，回 `{"body"}`，不真发）；`miyu dev driver-check <包编号>`：每份样本编两次、重启扩展再编
   一次，三次逐字节相同；只在末尾加一条消息的两份，稳定区结束之前相同；`contract` 的必须实现 `driver.encode`。`sent` 同一个请求第二次
   不一样的记一行 `WARN`，每个驱动每次核心运行一次。
2. **在哪一头查（A，core1 11-07 照推荐定）**：只给人的核心方法 `driver.check {"package"}`，核心调扩展、中途重启、比字节，交回每个样本过没过、
   哪里不对；`miyu dev driver-check` 只调它、印出来，有红的退出码 1。没选的：命令行自己经 rpc 一步步调（比法分在两处）。
3. **样本从哪来（A）**：核心里用代码造（`docs/designs/samples/drivers/` 是各家线上的字节、不是统一的请求；统一的请求读不回来；发行版
   不带 `docs/`），照金样的几种：纯文字、工具面、带私有数据的思考和工具调用、看不了图的转述、续写，各一份；再加只在末尾多一条消息的一对。
4. **稳定区怎么查（A）**：最后一条稳定区消息里放记号字 `miyustableend7f3a`（纯 ASCII 字母数字，JSON 里不会被转义），两份编出来的字节里
   找到它，到它为止逐字节相同；找不到的照「没法查稳定区」报红、写明原因，不当过（core1 补）。
5. **正在用的不重启**（core1 补）：查之前、重启之前，这个扩展有没答完的 `driver.call` 的，回 `driver_busy`（`data.running` 是几次），
   不重启；没有的才重启，重启完照常拉起。参考写明「会重启扩展」。
6. `contract` 的没实现 `driver.encode`（回 JSON-RPC 的 `-32601`）算没通过，别的算跳过（core1 同意）。

### 做了什么

- **样本和比法**（`miyu-session/src/route/extension/check.rs`，对外是 `miyu_session::driver_check`）：`samples`、`pair`（参数照
  `wire::params` 写，编号固定 `check`，不交 key、地址、图）、`first_difference`、`stable_prefix`、`STABLE_MARK`。
- **`sent` 对不上**（`route/extension/wires.rs`）：模型资料里的 `Wires`，每个驱动留最近 64 次「请求的哈希、报的 `sent`」，同一个请求
  `sent` 不一样、这个驱动这次运行还没说过的交回上一次的；`extension.rs` 收到回答时比，记 `WARN driver encoded the same request differently`。
- **核心这一头**（`miyu-endpoint/src/models/check.rs`）：`driver.check`：查装没装、有没有驱动、连没连上、有没有没答完的调用；头一轮每份编两次；
  `extensions::restart_process` 重启、等新连接握手（30 秒）；再编一次、比；编那一对、比稳定区；排成回应。`Peer::streaming` 数没答完的；
  新的拒绝 `no_drivers`、`driver_busy`（中英的说法在 `refusal/message.rs`）；能调什么的表加 `driver.check`（只给人）、`driver.encode`
  （核心发的）；分派表加 `driver.check`。
- **命令行**（`miyu-cli/src/dev.rs`、`language/dev.rs`、`help/{zh,en}/dev.txt`，主程序登记 `dev`）：`miyu dev driver-check <包>`，
  一个驱动一行、一份样本一行，原因接在后面；总帮助页加一行。
- **测试扩展**：`driver:<怎么答>+<怎么编>`，编法 `steady`、`stamped`（放时间戳）、`pid`（进程号，重启以后才变）、`count`（前面写消息条数，
  稳定区之前就变）、别的回「没有这个方法」；`driver.encode` 和 `driver.call` 走同一个分支（主文件到了行数上限）。
- **文档**：协议参考 `packages.md`（中英）`driver.check` 一节、`extensions.md`（中英）`driver.encode` 一节和 `sent` 对不上的一句、
  `errors.md`（中英）两行；蓝图新页 `cli/dev.md`，`cli/main.md`、`README.md` 各一行，`extensions.md`「扩展提供的模型驱动」第 6、7 条，
  `log.md` 一行；作者文档 `manifest`（中英）`cache` 那一格；设计 66 第七节第 2 条写明怎么做、谁定的。

### 测试

- `miyu-session` `route/extension/check/tests.rs`：五份样本各不相同、都带记号字、私有数据记成这个驱动的；那一对只差末尾一条；头一处不同；
  稳定区到记号字为止比，前面变了的说第几个字节，记号字被转义了的报「没法查稳定区」。`wires/tests.rs`：不一样的说一次、一样的不说、一次
  运行只说一次、别的驱动各说各的、挤出去了的不再比。`route/extension/tests/wired.rs`：回答里的 `sent` 交给模型资料记着，同一个请求换了字节查得出上一次报的。
- `miyu-endpoint` `tests/extension_driver_check.rs`（真核心、真扩展进程）：编得稳的六份都过、扩展真重启了（进程号变了）；**故意放时间戳的
  当场红**（同一个请求两次编得不一样）；只在重启以后才变的、末尾加一条消息稳定区之前就变的，各红各的；没实现的 `contract` 红、
  `best_effort` 跳过；有会话正经它调模型的 `driver_busy`、`running` 是 1、进程号没变；没装的 `unknown_package`。
- `miyu-cli` `dev/tests.rs`：排成的几行（中英），有红的不算过，只有跳过的算过。

### 手搓的变异

13 个，全红：不比两次编的、不比重启以后的、不比那一对；不看有没有在跑的调用；没实现的一律跳过；稳定区比空的；一份是另一份开头的当相同；
样本不放记号字；每次都说、留着不挤；回答的 `sent` 不交给模型资料记；命令行把跳过当没过；不重启。

### 不做的

- `driver.models`、`miyu-perf` 两项、作者文档「写一个模型驱动」：MD-4。
- 官方软件源收包时照它查：软件源的规矩另写（设计 66 第七节第 2 条）。

### 坑

- 测试扩展的主文件到了 500 行，新答法都放在 `miyu-test-extension/drivers.rs`。
