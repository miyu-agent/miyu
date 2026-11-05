## XG-1上 扩展工具声明路径、交效果、声明后台参数

状态：施工完，待 core1 审（2026-11-06）。设计 `63-扩展工具的效果和声明.md`（core1 2026-11-05 定五条：1A、2A、3A、4A、5A），这一步是
5A 拆出来的上半：`paths`、`effects`、`background_parameters`，e0 的红测试转绿。`runs_commands`、能力 `tools.commands`、通道是 XG-1下。
蓝图 `providers.md`（规格两格、`tool.call` 的 `checked`、「声明了 `paths` 的」「扩展交的效果」、出错、守着它的、还没有的）、
`session/guard.md` 第四条、`kernel/events-bodies.md`「效果」、`tools/interface.md`、`protocol.md` 的 `bad_tool`、`extensions.md`「还没有的」。

### 目的

扩展经 `provide` 登记的工具交不了效果：第三方的 `edit` 改了文件，时间线上没有增删行数、`view.detail` 没有差异，她先读过的文件被它
改了，接着用内置 `edit` 被拦「读过以后又被改了」。它也不报路径，权限链照「不报路径的调用」放行读写：工作区一级下第三方写工具改
工作区外的文件不问人，内置 `edit` 会问（这是要补的安全口子）。`background_parameters` 只有内置能声明。设计 00 第四节「内置功能不开
后门」。

### 做了什么

- **规格两格**（`miyu-endpoint/src/provide/declared.rs`）：`paths` 是 `[{"parameter", "write"?}]`，参数要是 `input_schema.properties`
  顶层的、类型是字或字的数组，`write` 的要 `access` 是写，不重复、不多格；`background_parameters` 是顶层的、不在 `required` 里的参数名，
  不重复。写错回 `bad_tool`，`problem` 是格名。不写的规格、工具面、登记缓存逐字节和以前一样（缓存照 `skip_serializing_if`）。
- **报路径**（`provide/remote.rs`）：`RemoteTool::targets` 照 `paths` 从参数取（数组每项一条），权限链照内置 `edit` 那张表判；
  `background_parameters` 照登记的交。`Tool::background_parameters` 的签名从 `&'static [&'static str]` 改成 `Vec<&str>`（扩展的名字是
  登记时读进来的），`shell`、测试的 `Fake`、`agents.rs` 跟着改，行为不变。
- **`tool.call` 带 `checked`**：声明了 `paths` 的才带，照报的先后配上 `Call::checked`；交出去之前去掉 Windows 的 `\\?\`（`miyu_fs::usable`）。
- **改前、效果**（`provide/effects.rs`）：发之前在阻塞线程里把要写的那几条各读一份（`open_file`，至多 20 MiB）；回应的 `effects` 在阻塞
  线程里查：路径照工作目录、家目录换真实位置，要是判过的（改、删的要判了写），`file.changed` 改后现读、和改前一样的丢、带的哈希原文
  对不上的丢，`file.trashed` 要真不在，`file.read` 哈希照现读的，`todo.written` 只查写法、出错时不收；别的种类整条丢。读成的做成
  `miyu_tool::Effect`，接在 `state` 前面交给执行器，往下和内置的分不出来。丢的记 `WARN tool effect dropped tool kind why`。
- **测试扩展**（`src/bin/miyu-test-extension.rs`）：多一步 `effects:<JSON>`，答 `ext_fake*` 时原样交这一份效果、不改文件。
- **文档**：蓝图见上；协议参考 `extensions`（中英）：`provide` 两格、`bad_tool` 多两种、`tool.call` 的 `checked`、回应的 `effects` 和「效果」
  一节；作者文档 `extensions`（中英）：两格和「改文件的工具」一节；设计 10 第五节那一句改成「diff 显示、她看过的、压后重读」（撤销照
  U-1 不改回文件）、设计 12 R8 指到契约测试、设计 44 第三节「以后加」指到设计 63；设计 63 跟着提交。

### 定的技术题

1. `checked` 只给声明了 `paths` 的：别的工具的 `tool.call` 一个字节不变。
2. 交给扩展的 `checked` 去掉 `\\?\`（交给别的程序用），效果里记的路径照核心的真实位置，和内置工具一样（Windows 上内置的也带前缀）。
   e0 的红测试原来拿去掉前缀的写法比效果的路径，Windows 上内置的那一边也对不上，改成照内置的写法比。
3. 同一种同一个文件在一次回应里报第二遍的丢（`shape`），改后同一个文件只读一次：防一条回应里重复上万条让核心反复读大文件。
4. `file.read` 的 `lines` 要从 1 数起、从不大于到，不然 `shape`。
5. 改后不在、不是普通文件、读不了的 `file.changed`，改前读不了的，都算 `mismatch`（设计列的六个 `why` 里最近的一个）。
6. 出错的回应里的 `todo.written` 不收、不记 `WARN`：同 `state`、`detail`、`data` 出错时不收也不记。
7. `effects` 不是数组的整份不收，记一行（`kind=-`、`why=shape`）；`null` 当没写。
8. 改前改后照「安全地打开」读（`miyu_fs::open_file`）：不跟最后一层链接、FIFO 不卡住，同内置 `write`、`edit` 读原文。

### 不做

- `runs_commands`、能力 `tools.commands`、`tool.call` 的 `channel`、`miyu call` 那一段挪进核心、人跑命令只认内置：XG-1下。
- 开发包 `miyu-sdk`（另一个仓库）的 `Tool` 三个设置、`Reply` 的四个帮手、`Call.checked`、`FakeCore`：不在这个仓库，另做。
- 给模型看的字：没加。

### 怎么验

- `provide/tests/declared_tests.rs`：两格的好写法、十一种坏写法各回对的 `problem`；照 `paths` 报路径（数组每项一条、缺的、不是字的、
  参数不是 JSON 的不出）；没声明的不报、不交后台参数；不写两格的登记缓存逐字节和以前一样，写了的原样存。
- `provide/tests/effects_tests.rs`：改前改后核心读、相对路径照工作目录换；带的哈希、原文对不上的丢；没改的丢；新建的；新建却带改前的丢；
  太大的、改后不在的丢；没判过的（工作区别的文件、私密文件）、只判了读的改和删、没声明的都不收；删了的要真不在、`trash` 超长的丢；
  不收的种类、写法不对的、重复的丢；出错时待办不收、文件照收；`effects` 不是数组的、`null` 的。
- `tests/provide_effects.rs`（e0 的红测试，去掉 `ignore`）：第三方 `edit` 和内置 `edit` 并排比效果、`view.detail` 的差异和增删行数，
  `tool.call` 的 `checked`，接着用内置 `edit` 不用重读，撤掉那一轮两边一样被拦、文件不动。
- `tests/provide_paths.rs`：工作区一级写工作区外的问人，人点头以前扩展收不到 `tool.call`、点了头收到的 `checked` 是那个位置，工作区里的
  不问；后台运行关着的会话工具面上没有声明的后台参数、开着的有，不写新格的参数格式逐字节照登记的。
- `tests/provide_effects_log.rs`（单独一个测试程序，装全局日志订阅者）：报假的六种各自丢、`WARN` 照 `why` 数对、结果照常；私密文件的
  路径不进会话日志、不进运行日志；没声明 `paths` 的一条文件效果都不收。
- 受影响的 crate（`miyu-tool`、`miyu-session`、`miyu-basesystem`、`miyu-endpoint`）测试全过；门禁 11 项（`xg1a`）；三平台 CI。

### 变异

手写 15 个，见下表（「抓住的」是变红的测试）。

| 改坏 | 抓住的 |
|---|---|
| 扩展的工具不报路径（不过链） | `provide_paths` 问人那条、`provide_effects`、`provide_effects_log`、`declared_tests` 报路径 |
| 信未声明的路径（不查是不是判过的） | `effects_tests` 只收判过的、`provide_effects_log` |
| 改前不读 | `provide_effects`、`provide_effects_log` |
| 改前在发之前不读（当空的） | `provide_effects`、`provide_effects_log` |
| 改前挪到回应以后才读 | `provide_effects` |
| 哈希、原文对不上的不丢 | `effects_tests` 改前改后 |
| 没改的不丢 | `effects_tests` 改前改后、`provide_effects_log` |
| 出错时也收待办 | `effects_tests` 种类和写法 |
| `tool.call` 不带 `checked` | `provide_effects`、`provide_paths` |
| 不交后台参数 | `provide_paths` 后台参数、`declared_tests` 报路径 |
| 只读工具也能声明写路径 | `declared_tests` 两格的写法 |
| 说删了不查还在不在 | `effects_tests` 删了的、`provide_effects_log` |
| 只判了读的也收改、删 | `effects_tests` 只收判过的 |
| 太大的改前当新建 | `effects_tests` 改前改后 |
| 同一种同一个文件报两遍不丢 | `effects_tests` 种类和写法 |
