## 施工单 CX-2：上下文明细照真数据修正

状态：合了（2026-10-23）。项目主人 2026-10-23 看了 `session.context` 在真会话上的样本，说「修」；主会话同意插在 V-12 前，估算器的改法审过。

### 目的

拿终端验收数据根里最长的三个真会话跑 `session.context`，查出两处不对：旧核心造的会话 system 拆不开（人设也混进「其余说明」）；子代理、后台命令的回报整块没算，明细里没有它、估算也少。这一步把两处修好。

### 查下来的

- 旧核心造的会话：核心说明那几行后来改过，照现在的料重拼和快照逐字节对不上，CX-1 下就退回只拆单存的几块。
- 回报：`estimate::event` 对 `child.reported`、`job.reported` 回 0，组装时正文却原样进请求。`01a150de` 清空后 7 份子代理回报 61,238 字节，约 15.3k token。
- 剩下的差距不归估算器（不改除数，主会话定）：
  - `01a150de` 走 magpie 的 openai-chat 驱动，Opus 的思考不回到日志里，代理却接回请求里，多报的量和上一次回复的输出量对得上。锚含输出，所以 `used` 准，只是摊的时候这部分平均摊到各块。
  - Opus 在英文、代码上少估约 1.3 倍，DeepSeek 在中文上多估约 1.2 倍，都是分词器的差别。
- 「压缩 2 次、用量 0」不是错：那是两次发出去、又被打断的手动压缩，供应商没报用量。

### 蓝图改哪几节

- `protocol.md`「`session.context`」：system 逐块认、`messages.reports`。
- `compaction.md` 第一条第 2 条：本地估算算上回报。
- `docs/reference/sessions.md`、`sessions.en.md` 跟着改。

### 做了什么

1. 和快照对不上的，逐块到快照的 system 里照先后找原文，找得到的各成一块。块的先后是定的：夹在两块中间、又正好是没找到的那块该在的位置的那一截，是它的旧字（旧核心的说明），归给它；没有块该在那儿的（以前多拼的一行），并成一块「其余」。只有人设文件没了、拼不出来的，才退回 CX-1 上的拆法。
2. 估算器算回报：子代理的照正文，后台命令的照带回来的最后一段，外面的开头、收尾不算。只影响锚之后那一截，朝宁多勿少的方向；锚之前的照旧由供应商报的数盖住。
3. 明细多一类：回报，`messages.reports`，三种语言的名字。
4. 新会话预览写 `preset: null` 的是无工具（终端验收撞见：被当成了默认预设）：`session.context`、`session.tools` 照 `session.create` 的认法，没写的照默认，写 `null` 的无工具。

### 真数据前后（同一份验收数据根副本，CX-1 下的核心对这一步的核心）

| 会话 | 改前 system | 改后 system |
|---|---|---|
| 01a14c61（有人格，DeepSeek） | 技能 160 · 知识库 59 · 关于你 27 · 其余 185 | 人设 13 · 关于你 27 · 核心说明 130 · 技能 162 · 知识库 58 · 风格锁 41 |
| 01a14aca（无人格，Opus） | 技能 209 · 知识库 44 · 关于你 35 · 其余 171 | 关于你 34 · 核心说明 170 · 技能 211 · 知识库 44 |
| 01a150de（清空过，派过子代理） | 技能 348 · 关于你 59 · 其余 326 | 关于你 47 · 核心说明 236 · 技能 290 · 其余 35（旧核心多拼的一行） |

01a150de 的对话多出「子代理和后台命令的回报」27,786，本地估的和从 74,101 到 89,413（供应商报 162,253；剩下的是代理藏的思考和分词差）。`session.context {"preset": null}` 改前带内置工具约 3.8k，改后只有 system 122、示范对话 13。

### 不做什么

- 不改估算器的除数，不按模型调；看不到的内容（代理藏的思考）不靠猜补。
- 不改用量库里压缩的记法。

### 验收

- `miyu-kernel`：`estimate/tests.rs` 的 `reports_count_by_the_text_they_carry`，`session/tests/scenario/reports.rs` 的 `reports_are_their_own_part_of_the_context`。
- `miyu-session` 的 `actor/persona/context_tests.rs`：`an_older_session_is_still_laid_out_block_by_block`（核心说明改过、末尾多一句：人设、技能照样认出，中间那截归核心说明，末尾那句归其余，连起来是整段）、`stretches_of_the_rest_are_one_part`、`a_block_the_old_text_never_had_claims_nothing_later`（没找到的块旧的里本来就没有，不认领后面多出来的字）。
- `miyu-endpoint` 的 `tests/session_context.rs` 的 `a_preview_with_a_null_preset_has_no_tools`（`session.tools` 一起）。
- 真数据：见上面「真数据前后」。
- 手写变异 9 个都红（中途一个没抓住，补了上面最后那条才抓住）；`cargo xtask check`、Windows 版 clippy；推 step 分支过三台。

### 风险

- 逐块找原文时，很短的块（例如一行人设）可能在更前面的地方碰巧出现。照先后从上一块的末尾往后找，减小这种情况；找错了也只是归类不准，总数不变。
