## 施工单 Q-W：不打断，退回排队的消息

状态：本机门禁过了，待合。终端会话转来项目主人定的界面（2026-10-22）：点排队的那条就撤回；`/undo` 有排队的时候先逐条撤，最新的先撤。原来只有 `session.interrupt` 带 `queued: "return"` 能退回，但它会打断这一轮。图纸 `docs/designs/02-内核.md` 第六节「排队的消息」第 3 条、`docs/blueprint/kernel/session.md`「排队的消息」第 6 条、`docs/blueprint/protocol.md`「`session.withdraw`」。

### 做了什么

- 内核：命令 `Withdraw { message }`。队里有写的那一条（不写的是最新的一条）的，从队里拿掉，追加只有它的 `message.withdrawn`（`by` 是退的人，`cause` 是这个命令），回合照跑，结束时由剩下排着的接着开。写了、不在队里的拒绝 `already_heard`（请求带上了、由它接着开了一轮、退过了）；没写、队里是空的（闲着的也算）拒绝 `nothing_queued`。封存了照收（同打断）。
- 「还没听到」照内核的队：发主请求那一刻队里的就挪进听到过的（施工 4-15 补），所以请求在路上时，它带着的那几句已经退不回了，之后来的退得回。
- 协议：`session.withdraw {session, message?}`，回 `{"events": [<message.withdrawn 的序号>]}`，落了盘才回；权限同 `session.interrupt`（`sessions.drive`）。推送、视图（`withdrawn: true`）和打断退回的一样，头不用另认。拒绝的中英两句进 `refusal/message.rs`。
- 测试：内核 `tests/queue/withdraw.rs` 三条（退最新的、回合照跑、剩下的接着开；点名退；不在队里、退过的、空队、闲着的拒）；随机测试多一种输入 `Withdraw`、两条路（「不打断退回一条」「退回时不在队里」），看守照内核的队判该退哪条、该拒什么；端点 `tests/withdraw.rs` 真的核心里排两句、一句句退、拒的两种、都退完了打断「接着发」不再开一轮。变异 4 条全杀，长跑随机两万例过。
