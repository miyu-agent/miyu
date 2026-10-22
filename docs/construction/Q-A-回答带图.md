## 施工单 Q-A：回答里贴图

状态：合了（2026-10-23）。终端会话转来（2026-10-22）项目主人的话：「为什么不让粘贴？得让粘贴才行。尤其是问问题工具，回答的时候用户需要图片辅助表达自己的信息的。」图纸 `docs/blueprint/protocol.md`「`session.answer`」、`docs/blueprint/kernel/events-bodies.md`。

### 定的（技术题，主会话定）

1. **图放在工具结果里**，不另起一条用户消息：一次调用一个结果，调用和结果挨着；三家驱动都会画工具结果里的图（openai-chat 照旧挪到紧跟的一条用户消息里），看不了图的模型照旧替看。
2. **协议**：`session.answer` 拒绝时可带 `attachments`；回答提问的每道题可带 `attachments`；写法同 `session.send`（`blob.put` 交回的那几格）。允许的带附件同带理由，`unexpected_reason`；回答提问带外面的 `attachments` 是 `bad_params`。附件先查、再找会话，`unknown_attachment`、`attachment_too_big` 照 `session.send`；找到会话以后照属主拷过去、没有本机路径的落成会话里的文件。
3. **记**：`tool.approval_decided` 多 `attachments`，`question.answered` 每道多 `attachments`，空的不写，以前的日志照读。
4. **给她**：拒绝的结果是理由那一句后面接图；`ask_user` 在附了图的那道回答后面写一句 `(attached below: N)`（`ask_user/attached.txt`，进登记簿），图照题目的先后跟在字后面；只附了图的那道不算没答。

### 做了什么

- 内核：`Response.attachments`、`ApprovalDecided.attachments`、`Answer::Approval.attachments`；拒绝的结果接上图。
- 基础系统：`ask_user` 照上面写。
- 端点：`session.answer` 的参数多 `attachments`（回答提问的每一项一份），照 `session.send` 那一套换成块。
- 测试：内核 `tests/approval/attached.rs`（拒绝带图进结果、允许带图拒）；`ask_user` 附图的那道写几个、图跟在后面；端点 `answer.rs` 拒绝带图、`bad_params`、`unknown_attachment`。
