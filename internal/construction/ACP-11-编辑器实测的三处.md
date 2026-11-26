## 施工单 ACP-11：`miyu acp` 在 Zed 实测里撞到的三处

设计 73 第五节第 2、3 条、第八节。项目主人在 Zed 里实测 ACP-6、ACP-9 撞到的三处（2026-11-26）。

### 为什么

1. **编辑器拉起的核心用编辑器的环境**（中）：Zed 拉起 `miyu acp`，核心没开就由它拉起，核心继承的是 Zed 的环境（这次 Zed 的 `HOME` 是沙箱），
   之后终端界面、网页也连这个核心，扩展驱动里的 Claude Code 因此找不到登录、报「要先登录」。查下来所有的头都一样：`miyu ask`、终端界面、
   托盘、网页、`miyu core start` 都照 `miyu_ipc::connect_or_start` 直接拉起 `miyu core`，核心继承拉起它的进程的环境；`miyu service enable`
   装的服务由服务管理器拉，环境是服务管理器给的（服务文件里只带 `MIYU_HOME`，手册「服务和环境变量」写着读不到 shell 里的变量），核心空闲
   退出、人停了以后照样由下一个头拉起。从程序坞、开始菜单打开的编辑器（还有托盘），`PATH` 是图形会话的，macOS 上只有
   `/usr/bin:/bin:/usr/sbin:/sbin`，没有 shell 配置文件里加的目录和变量。
2. **回答到一半核心停了，Zed 只显示 `restarted`**（低）：有计划的重启（`miyu core stop`、换核心）先把正在进行的那一轮收好，记一条原因是
   `restarted` 的收尾再断开。`miyu acp` 照收尾回出错，话照 `explain`、错误原话找，都没有就写原因的原文。网页同一件事显示「已重启」。
3. **重放时后台任务叫醒的那一轮放错了位置**（低）：`session/load` 重放时，叫醒的那一轮的开头那一句（ACP-6 补的 `user_message_chunk`，
   `messageId` 是 `turn-<那一轮>`）交回的是另一种要做的事（`Action::Woken`），`attach` 把要做的事分成「现在报的」（只有 `Action::Update`）
   和「回了编辑器再办的」，开头那一句被分进了后一半：排在回应以后、整段对话最后，下面没有回答；那一轮的回答前面没有开头，Zed 把它并进了
   前一条助手消息。核心导出的记录先后是对的。

### 改了什么

- `acp/setup.rs`：`attach` 分的时候开头那一句（`Action::Woken`）同 `Action::Update` 一起照先后现在报，只有在等人的、配置项变了的回了编辑器
  再办。
- `acp/session.rs`：`Stop` 多一种 `Ended(EndReason)`：出错收尾、核心既没给说明也没有错误原话的，交回怎么结束的，不再写成原因的原文。
- `acp/server.rs`：回 `session/prompt` 时 `Stop::Ended` 照界面语言说（`language/acp.rs` 的 `acp_ended`）：`restarted` 是
  `核心重启了，这一轮没有做完`，`aborted` 是 `核心意外退出了，这一轮没有做完`，`error` 是 `这一轮出错了`，别的是 `这一轮没有做完`；英文各一份。
- 第一处不改代码，写进手册：`guide/editors.md`（中英）「需要知道的」说清核心用启动它的程序的环境、编辑器先起的就是编辑器的，建议先在终端里
  `miyu core start`，已经由编辑器起了的先 `miyu core stop` 再在终端里起；装服务的那条照实写（登录就起、不再由编辑器起，但同样读不到 shell
  的变量）；「出了问题」加一条找不到程序、扩展驱动要登录的怎么办。
- 测试夹具：`tests/commands.rs` 的 `until_go` 改成 `pub`，`tests/acp/sessions.rs` 也用。
- 文档：蓝图 `cli/acp.md`（对外的样子、一轮结束、一轮怎么翻、给人看的字、测试表、还没有的）、设计 73 第七、八节、手册。协议没变，协议参考
  不用改。

### 定下的

- 第一处只写手册、不改「核心怎么起」：
  - 装了用户服务的走服务管理器拉（`systemctl --user start`、`launchctl kickstart`）不解决 `PATH`：systemd 用户管理器、launchd 给的也不是
    shell 的环境，launchd 的 `PATH` 同程序坞打开的程序一样短；只解决「编辑器自己设的环境漏进核心」（这次的沙箱 `HOME`、编辑器设置里的
    `env`），还要给每种服务管理器各写一条路、所有头一起改才一致。
  - 真正补上 `PATH` 的是核心起来时读一次登录 shell 的环境（Zed、VS Code 的做法），这是改所有头拉起核心的规矩，有产品影响（启动多一次
    shell、shell 配置文件出错时怎么办）。
  - 两样都等项目主人定，记在蓝图「还没有的」、设计 73 第八节。
- 出错的话不带原因的原文：编辑器里给人看，原文在核心的日志、会话记录里都有。

### 怎么验

- `crates/miyu-cli/tests/acp/sessions.rs` 多一条 `load_replays_a_woken_turn_where_it_happened`：第一轮把等 `go` 的命令放到后台，放了 `go` 以后
  命令结束、叫醒她说「跑完了。」；另一个编辑器载入：回应以前依次是人说的、第一轮的回答、开头那一句（`turn-` 打头，`后台任务「等 go」已完成`）、
  「跑完了。」（另一个 `messageId`）。修之前红：开头那一句不在回应以前。
- `crates/miyu-cli/tests/acp/relink.rs` 的 `after_a_restart_the_running_turn_fails_and_the_next_requests_go_on` 多一句：回的出错是
  `核心重启了，这一轮没有做完`，或者没来得及收好就断开的 `与核心的连接已断开`。修之前红：回的是 `restarted`。
- `crates/miyu-cli/src/acp/view/tests.rs` 多一条：等的那一轮以 `restarted` 收尾、没有说明，交回 `Stop::Ended`，中英两句。
- 门禁。
