## 施工单 AUD-6：不认识的键的值写成行内表时，只报这个键一次

状态：已完成（2026-11-19）。

### 为什么

LiteMiyu 审计时根据代码判断出一条问题：「退役的配置键的值写成一张表（`key = { secret = … }`）时，读配置的代码会进入这张小表，把表里的键报成不认识的键」。因为 LiteMiyu 的 `retired.rs` 是从 Miyu 搬过去的，所以这一条转给 Miyu 核实。

核实的结论有两条：

- **审计描述的问题在 Miyu 上不成立。** Miyu 已经没有「退役的键」这个概念。施工 AUD-2 删除了 `crates/miyu-config/src/retired.rs` 和退役键的那张表：因为还没有发版，所以不保留旧兼容。以前登记过、后来拆掉的键现在就是不认识的键，读配置时同样报警告。因此没有「先认退役键」这一步可以修。
- **核实时发现了一个来源相同的问题。** 在沙箱里用 main 的程序运行 `miyu check` 可以看到：如果一个不认识的键的值写成行内表，警告里的键名会多出一层，而且表里有几项就报几条。
  - 在 `[providers.dev]` 下写 `kee = { secret = "dev" }`，警告是「没有 providers.dev.kee.secret 这一项」；而写成 `kee = "plain"` 时，警告是「没有 providers.dev.kee 这一项」。
  - 在 `[loading]` 下写 `mode = { secret = "x", extra = 1 }`，警告有两条：`loading.mode.secret` 和 `loading.mode.extra`。

  这些都只是警告，程序仍然忽略这一行，其他键不受影响。问题在于警告里的键名不是你写的那个键。引用密钥、环境变量正好都写成行内表，所以键名拼错时，你看到的是一个自己没有写过的名字。

核心主会话决定修第二条。

### 改成什么样

读配置的代码走到不认识的路径上时，如果遇到的值是**行内表**（`key = { … }`，写在一行里的一个值），就把它当成一个值：只报这个键一次（`unknown_key`，位置是这个键），不再进入表的内部。

以下行为保持不变：

- 标准的表头（`[weird]`）和用点号连着写的键（`uii.language = 1`），仍然逐层进入，每一项报一条；键名拼错时仍然提示最接近的键名。
- 如果一组认识的键写成行内表（`ui = { language = "zh" }`），程序仍然读取表里的每一项。
- 清单里值本身就是行内表的项（例如引用密钥），原本就不会进入表的内部，这一步没有修改它们。

### 验收

- 修改之前测试是红的。
- 门禁、Windows 目标的 clippy、三个平台的 CI 全部通过。
- 实测：在沙箱里写几种配置，查看 `miyu check` 输出的警告。

### 做下来的

- 改前红：`parse::tests::inline::an_unknown_key_holding_an_inline_table_is_reported_once_as_written` 在修改之前的代码上报出 `ui.nosuch.secret`（第 3 行第 12 列）、`ui.nosuch.extra`、`weird.thing.env` 三条，而正确的结果是 `ui.nosuch`（第 3 行第 1 列）和 `weird.thing` 两条。另外两条测试守着保持不变的行为（认识的一组键写成行内表时仍然读取每一项；表头和点号连着写的键仍然每一项报一条），它们在修改前后都是绿的。
- 修改之后，`miyu-config` 这个 crate 的 351 条测试全部通过。
- 实测（2026-11-19，数据根和家目录都在沙箱里，没有使用模型）：对同一份配置，用修改之前的程序和这一步的程序各运行一遍 `miyu check`。
  - 修改之前的程序输出 `etc/config.toml:4:9 警告：没有 providers.dev.kee.secret 这一项`、`7:10 … loading.mode.secret`、`7:24 … loading.mode.extra`，加上其他三条，一共 6 处警告。
  - 这一步的程序输出 `4:1 警告：没有 providers.dev.kee 这一项` 和 `7:1 … loading.mode` 各一条；`[weird]` 下的两项仍然每一项报一条，`ui.langauge` 仍然带着「是不是想写 ui.language？」；一共 5 处警告。
  - 第一遍对比时，两个程序的输出完全相同。原因是前一条命令启动的旧核心还在运行，`miyu check` 问到的是旧核心。运行 `miyu core stop` 之后，这一步的程序自己启动了核心，才得到上面的结果。

### 怎么实测验收

1. 先运行 `miyu core stop`，否则 `miyu check` 问到的还是旧核心。然后运行 `miyu core status`，确认输出的是新版本。
2. 把 `MIYU_HOME` 指到一个空目录，作为试验用的数据根（不要使用真实的配置），在它的 `etc/config.toml` 里写入：

   ```toml
   [providers.dev]
   driver = "openai-chat"
   base_url = "http://127.0.0.1:9/v1"
   kee = { secret = "dev" }

   [loading]
   mode = { secret = "x", extra = 1 }

   [weird]
   thing = 1

   [ui]
   langauge = "en"
   ```

3. 运行 `miyu check`，应该看到 4 处警告：
   - `…:4:1 警告：没有 providers.dev.kee 这一项。已忽略此行。`（键名不是 `….kee.secret`）
   - `…:7:1 警告：没有 loading.mode 这一项。已忽略此行。`（只有一条，不是 `.secret`、`.extra` 两条）
   - `…:10:1 警告：没有 weird.thing 这一项。已忽略此行。`
   - `…:13:1 警告：没有 ui.langauge 这一项。是不是想写 ui.language？已忽略此行。`
