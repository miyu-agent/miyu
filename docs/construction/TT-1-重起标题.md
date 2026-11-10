## 施工单 TT-1：重起标题

状态：做完，等合。

### 目的

`miyu rename` 不带标题时，以前是把空标题交给 `session.set_meta`，被核心拒掉（或者清掉标题）。改成用一次模型请求更新标题，不是清掉标题，用的模型同自动起标题的那一个（2026-11-11 项目主人定）。做成以后协议多一个 `session.retitle`，头也能做「让 AI 重起一个标题」。

### 蓝图改哪几节

- `protocol.md`：方法表多 `session.retitle`；`session.set_meta` 后面多 `session.retitle` 一节（参数、回应、八条规矩）；「在哪」「守着它的」各多一行。
- `kernel/request.md`：`Assembler` 的表多 `retitle`；「起标题的请求」多第 7 条；「守着它的」多一行。`docs/designs/26-提示词.md` 第十节：起标题那一行补重起时喂什么，量了三轮的例子（`prompts.md` 跟着重新生成）。
- `kernel/session.md`：命令表多 `Retitle`；原因码表多 `nothing_to_title`；「起标题」多第 9 条「人要求重起」；封存了照收的命令多它；「守着它的」多一行。
- `kernel/events-bodies.md`：`session.meta_changed` 内核起的那一种，注明人要求重起的也是它。
- `cli/rename.md`：标题可以不写、第 5 到 7 步、帮助页样本、退出码、守着它的。`cli/main.md`：主帮助页样本的 `rename [<标题>]`。
- `docs/reference/sessions.md`、`sessions.en.md`：方法表、谁能调、`session.retitle` 一节；`methods-v1.txt` 多一行。
- `docs/designs/04-核心协议.md` 第九节：方法表、「自动起标题」那一条补一句。

### 怎么改的

- 喂什么（2026-11-11 项目主人定：重起只喂最近几轮，不喂第一轮）：组装器多 `retitle`（`crates/miyu-assemble/src/title.rs` 的 `recent`）。指令、上限（连指令 1024 个 token）、标签、截法和自动起的同一份；对话换成最近答过的至多 3 轮，每轮只有人这边的话和她最后一条有正文的回复，还没走完的那一轮（没有 `turn.ended`）不带；放不下先整轮去掉最老的、最前写回顾的那一行省略了，只剩一轮还放不下的两段各截中间。只答过一轮的和自动起的逐字节一样。
- 内核（`crates/miyu-kernel/src/session/title.rs`）：命令 `Retitle`。起标题的账 `Naming` 多一格「人要了」（只在内存里）。原来 `title_up` 里「怎么起」的那一段拆成 `title_now`，自动起和人要求的共用；人要求的组装走 `Assembler::retitle`，自动起的照旧走 `Assembler::title`，用途、模型池、记用量都没动。人要求的不看「标题动过」「说了不自动起」「一次性的会话」「子会话」「试过几次」；有一次在路上的并进去；组装不出来、回合正在进行的等这一轮的 `turn.ended` 落了盘；空闲着也组装不出来的拒 `nothing_to_title`。说完了，「人要了」的盖得掉动过的标题；标题一动（起好了、人又改了）或者没起成就放下。
- 端点（`crates/miyu-endpoint/src/meta.rs`）：`session.retitle`，权限同 `session.set_meta`（`sessions.drive`）；内核的 `nothing_to_title` 换成 `bad_params`（不带 `data`，照 `session.set_meta` 标题不合时的先例）。
- 命令行（`crates/miyu-cli/src/rename.rs`）：标题一个词都没写的发 `session.retitle`，写了的照旧；帮助页中英两份、主帮助页那一行。

### 不做什么

- 不改提示词文件，不动自动起的那一份请求（照旧只喂第一轮，一个字节不变）。
- 轮数 3 写在组装器里（`RECENT_TURNS`），没进快照：进快照要改快照的字节和旧快照的读法，等真要调它再说。
- 不让命令行等模型：请求发出去就回，标题过一会儿到；模型没写成的，命令行不知道。
- 核心的命令目录里没有 `/rename`（斜杠命令是头自己做的），不加。
- 回应不附事件、两条事件不带 `cause`：和自动起的写法一样，头照 `by` 是内核认。

### 验收

1. `cargo test -p miyu-assemble title`：`title/recent_tests.rs` 五条（五轮喂最后三轮、没有第一轮；只答过一轮的和自动起的逐字节一样；带工具的轮只带人话和正文、没答完的不带；超长的先丢最老的一轮再截；没有回答的没有）；`probe_title.rs` 后三条是真内核、出厂的字（五轮、只答过一轮的和自动起的相等、超 4096 字节丢最老的一轮）；`title/tests.rs`、`probe_title.rs` 的存档原样过：自动起的没变。
2. `cargo test -p miyu-kernel --lib retitle`：六条场景（五轮的会话要的是最近三个回答的那一份、盖掉人起的；一次性的、人起过名的要了照起；没东西可起的拒、拒了不留着；在路上的不再发；第一轮没答完的等这一轮完；要了以后人又改的不盖、没起成的不重试）。
3. `cargo test -p miyu-endpoint --test all retitle::`：五条（四轮的会话请求里是最近三轮、没有第一轮，推来新标题、不算人起的；人起过名的换掉；空会话和没写 `session` 的 `bad_params`；连要两次只发一次；`titles: false` 的照起）。改之前五条都红：`unknown_method`。
4. `cargo test -p miyu-cli`、`cargo test -p miyu --test all rename::`：不带标题的走 `session.retitle`，模型收到一次起标题的请求，日志里一条内核起的；没东西可起的印「参数错误」、退出码 1（改之前是退出码 2）。
5. 手测：`miyu ask "帮我看看这个仓库的测试怎么跑"`，再 `miyu rename`：什么都不印、退出码 0；过一两秒会话列表里那一项有了标题。再 `miyu rename 我起的`，再 `miyu rename`：标题换成模型起的。新开的空会话 `miyu rename -s <编号>`：「参数错误」，退出码 1。
6. 提示词文件没变：`git diff --stat main -- resources` 是空的；登记簿（`26-提示词.md` 第十节）起标题那一行写了重起时喂什么，量了三轮的例子。

### 风险

- 人要求的那一次没起成也算「试过一次」（日志里多一条 `purpose` 是 `title` 的 `model.called`）：没起名的会话上连着失败两次以后，自动的那一路也不再试。可以接受：人再要一次就是。
- 「人要了」只在内存里：第一轮没答完时要的，核心在这一轮结束前重启就丢了，不补。
