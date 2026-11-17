## 施工 EN-2：核心起不来的各步照代码说

状态：做完，等验收（分支 `step/PRE-A`）。来源：EN-1 实测查出来、主会话 2026-11-17 交来：核心起不来的原因还是中文（`miyu: the core couldn't start: 运行日志写不了：File exists (os error 17)`），命令行和迁移包连不上时都照它说。

### 查到的

- 核心起不来时写一行 `error [<代码>: ]<原因>` 交给拉起它的头（`miyu-ipc` 的 `Ready::Failed`）。只有数据根认不出的三种带代码（施工 LC-1），别的十步（找数据根、拿锁、装运行日志、找资源目录、建运行时、开套接字、建工作区、读模型和工具的资源）照中文整句写原因，头只能照原样印。
- 原因里多数是下层的 `Display`（`运行日志写不了：…`、`MIYU_RESOURCES 指的 … 不是一个目录`），整句写死中文。

### 怎么改

1. 核心起不来的每一步都带代码（`miyu-core` 新增 `not_started.rs`，`failed`、`say` 从 `lib.rs` 挪过去，`lib.rs` 原来 499 行）：原因只写会变的那一段（系统的原话、路径），整句的字放在内核 `human/` 三种语言的 `start/<代码>` 里，`{reason}` 换成原因。代码共二十二个（`CODES`）：`miyu_home_relative`、`no_home`、`no_local_app_data`（同 EN-1），`data_root_not_ours`、`data_root_old_layout`、`data_root_other_layout`（LC-1，原因改成数据根的位置，字里照旧是 `{path}`），新增 `data_root_failed`、`lock_failed`、`log_failed`、`resources_relative`、`resources_missing`、`resources_not_found`、`resources_unknown`、`runtime_failed`、`socket_too_long`、`socket_not_private`、`socket_occupied`、`pipe_occupied`、`socket_failed`、`home_failed`、`models_failed`、`tools_failed`。
2. `StartError::said`：带代码的照 `start/<代码>`，原因同时给 `{reason}`、`{path}`；没代码的照旧 `start/refused`。取不到字的 `Display` 写「核心起不来（<代码>）：<原因>」。
3. `miyu-client` 的 `open::Core::connect` 这几个连核心的口子：连不上、拉不起来的 `Refused::message` 照这台机器的语言（`locale`：`zh`、`ja` 开头的照它，别的英文）从资源目录取字，取不到的照原话。迁移包、网页软件、接入通讯平台的程序经它连核心，换钉到这一版以后不用改代码就照语言说。
4. 命令行照 EN-1 的 `startup.rs`，不用改。

### 蓝图改哪几节

`ipc.md`「拉起核心」那一行的写法、「出错」；`core.md` 起不来的那一条；`client.md` 的 `open`。

### 验收

1. 改前红（换回 main 的 `miyu-core/src`、`miyu-ipc/src` 跑）：
   - `miyu` 的 `early_words::a_core_that_cannot_start_is_said_in_the_interface_language`：运行日志的目录、工作区是个文件，英文界面 `miyu ask hi` 印 `miyu: the core couldn't start: cannot write its log: …` / `… cannot create folders in the data root: …`，没有一个汉字。
   - `core::a_core_that_cannot_start_says_why`：资源目录不在的带代码 `resources_missing`，原因是那个位置。
   - `core::a_data_root_it_does_not_know_comes_with_a_code`：原因是数据根的位置。
2. `miyu-core` 的 `not_started::tests`：每个代码三种语言都有字；几种原因只写会变的那一段。`miyu-client` 的 `open::tests::an_unreachable_core_is_said_in_the_machine_language`。
3. 门禁全过等（同本批）。实测见下。

### 风险

- 那一行的原因换了意思（带代码的只剩会变的一段）：头经 `miyu-ipc` 读，照 `said()` 说；只照原样印原因的头会少前半句。发版前不做旧兼容。
- 迁移包要换钉到含这一步的核心才照语言说。
