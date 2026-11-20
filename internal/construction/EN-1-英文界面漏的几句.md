## 施工 EN-1：英文界面漏的几句

状态：做完，等验收（分支 `step/PRE-A`）。来源：写文档的会话查出来、主会话 2026-11-17 交来（发版前「字和检查」一批）；FX-4「不做什么」第三条记下的「命令行建骨架出错照中文原话印」一并做掉。

### 查到的

- **连不上核心的八句只有中文**：`miyu-ipc/src/error.rs` 的 `ConnectError`、`StartError` 照 `Display` 写死中文（核心没在跑、套接字目录不是自己的、命名管道另一头不是自己的、核心起不来、没起来也没说为什么、等太久、别的程序在拉起、拉不起核心）。命令行各命令都是 `error.to_string()`，英文界面也印中文。
- **数据根出错只有中文**：`miyu-store/src/root.rs` 的 `RootError`（`MIYU_HOME` 不是绝对路径、找不到家目录、找不到 `LOCALAPPDATA`），`root/prepare.rs` 的 `PrepareError`（认不出的、旧版的、别的布局的）照原话印。后三种的三语字 `start/data_root_*` 早有（施工 LC-1、FX-4），命令行没用。
- **`miyu check` 帮助的例子少一层 `root/`**：人格在数据根的 `root/personas/`（`DataRoot::personas`），帮助写 `~/.miyu/personas/…`。日文没有帮助页。
- **`miyu schedule --cwd` 的帮助说默认家目录**：代码（`schedules/methods.rs` 的 `target`）没写的照 `sessions::roots` 的第二个，是数据根的工作区，开会话时每个会话在里面有自己的一格（手册 `automation.md` 早写对了）。

### 怎么改

照分层：`miyu-store`、`miyu-ipc` 是第三层（执行器），拿不到界面语言，只交**说法**（`miyu_kernel::event::Said`：哪一句、换进去的字段），字放在内核的 `human/` 三种语言里，头照自己的语言取（同 LC-1 的 `start/<代码>`）。

1. 新增三种语言的 `start/miyu_home_relative`（`{path}`）、`start/no_home`、`start/no_local_app_data`、`start/not_running`、`start/not_private_socket`（`{path}`）、`start/not_private_pipe`（`{path}`）、`start/refused`（`{reason}`）、`start/silent`（`{log}`）、`start/timeout`、`start/busy`、`start/spawn_failed`（`{error}`）。
2. `RootError::said`、`PrepareError::said`（认得出的三种，照现成的 `start/data_root_*`）、`ConnectError::said`、`StartError::said`：交 `core/start/…` 的说法；读写出错的交空（照原话，原话本来就是系统给的）。`Display` 不动：日志、没有资源目录时照它。`miyu-ipc` 多依赖 `miyu-kernel`（第一层，层序允许）。
3. 命令行新增 `startup.rs`：`spoken` 照握手以前的界面语言（系统的语言，`language::current`）从资源目录的字里取，取不到的照原话。十八个命令找数据根、建骨架、连核心、拉起核心出的错都经它（原来各自 `error.to_string()`）。
4. 帮助页：`check.txt` 两种语言的例子改成 `~/.miyu/root/personas/…`；`schedule.txt` 两种语言 `--cwd` 的默认改成「新会话自己的工作区」，蓝图 `cli/schedule.md` 的两份样本跟着改。

报错照设计 47 经 `diagnostic::fail` 收拾（去句号、首字母小写，见 TD-1）。

### 蓝图改哪几节

`store.md`「出错」；`ipc.md`「出错」；`cli/main.md` 文件表；`cli/schedule.md` 帮助页两份样本。

### 验收

1. 改前红：
   - `miyu-ipc` 的 `error::tests::start_and_connect_errors_have_words`：八句都交说法、三种语言取得出字，英文是 `The core isn't running` 这样（改前没有 `said()`，编不过）。
   - `miyu-cli` 的 `startup::tests::data_root_and_connect_errors_are_said_in_english`：`RootError`、`PrepareError`、`ConnectError` 的英文那句（`MIYU_HOME must be an absolute path, got rel/path`）；`errors_without_words_keep_their_own`：读写出错的照原话。
   - `miyu` 的 `early_words::a_relative_miyu_home_is_said_in_the_interface_language`：英文界面下 `MIYU_HOME=rel/path miyu check` 印 `miyu: MIYU_HOME must be an absolute path, got rel/path`、退出码 1，中文界面照中文（换回 main 的 `miyu-cli/src/config.rs` 跑，英文那条红）。
2. 门禁全过等（同本批）。

### 风险

- 资源目录找不到的（装坏了的）照中文原话印，同改前。
- 头（终端界面、网页）可以照 `said()` 说同样的字；不用也照旧。
