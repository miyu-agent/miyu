## 施工单 AG-1：中转转来的拒绝说明不当她说的话

状态：核心的分会话做。onebot 报（2026-10-27）：项目主人那份部署上，`antigravity/gemini-3.8-flash` 碰到 Google 的内容政策，中转把 Google 的拒绝说明（「The prompt could not be submitted. The prompt contains sensitive words that violate Google's Generative AI Prohibited Use policy …」）当正文流回来，接着断开；核心记成一条被打断的 `message.assistant` 带着那段字，`model.called` 记 `retryable`「Antigravity: the reply ended before it was complete」，终端、网页当成她说的显示，im 发到了 QQ（im 那边先改成被打断的回复不发，I-52）。主会话派：认出这类中转报错，整轮按出错处理，不落正文。方案 B 主会话、项目主人定了（magpie 不是项目主人的程序，网关那边改不了；C 不做）。

### 查下来的

取证只读：会话日志拷一份到 `~/.cache` 再看，没有用主人的数据根起核心。

1. 那条线不是专门的驱动：端点是 `openai-chat`，地址是局域网里的 magpie 网关，`antigravity/gemini-3.8-flash` 是网关的模型名。
2. 日志：seq 3061 `message.assistant`（`interrupted: true`，一个正文块就是 Google 那段话）；3062 `model.called`（`first_token_ms` 1、`duration_ms` 46、一个块，`error` 是 `retryable`、原话「Antigravity: the reply ended before it was complete」）；3063 内核补的 `reply_cut`。
3. 那句原话 Miyu 里没有，是网关写的；它进 `model.called` 的路子只有解码器的「流里报的错」（`event: error`，或者带 `error` 的一段），分类认不出就是 `retryable`。所以原始流是：网关先把拒绝说明当一段普通的 `delta.content` 流出来，再发一个出错事件。解码器把前一段当正文交出去，内核照「说到一半断了」留成被打断的回复。
4. 一段正文分块本身有没有别的记号（`finish_reason`、`id`、`model`）没法从日志看：日志不存原始流，网关不是我们的程序。重放原来那条请求不做（会把那段内容再发一次）。

### 做了什么

1. `openai_chat/refusal.rs`：认 Google 拒绝说明，开头那一句「The prompt could not be submitted.」、政策链接 `policies.google.com/terms/generative-ai/use-policy` 两样都在才算；照 `content_policy` 出错，原话是拒绝说明（最长 500 个字）。
2. `openai_chat/decode.rs`：回复的第一段正文（还没有别的块、这一段没带 `finish_reason`）是拒绝说明的，先扣住不交。紧跟着来的是出错事件、流断了：整段丢掉，这一次照 `content_policy` 出错；来的是带内容的一段（正文、思考、工具调用、`finish_reason`）或者 `[DONE]`：照原样交出去，只晚一段；只报用量的一段不算。
3. `classify.rs` 的内容策略说法加「prohibited use policy」：中转把拒绝说明放进出错事件时也认得。
4. 内核、端点、头不动：出错没有半截，内核照出错结束这一轮（`content_policy` 不重试），给人看的走现成的「内容被拦截」（`view/error/class/content_policy`）。
5. 图纸 `drivers/openai-chat.md`：解码第 11 条、收尾的表、内容策略的说法、样本、测试表。

### 误杀面、漏认面

- 误杀：一个真模型的回复第一段正好同时有这一句和这个链接、紧接着流又出错，这一段才丢；那一轮本来就是出错。没出错的只晚一段。
- 漏认：Google 换了说法、别的语言、拒绝说明分几段到、别的中转换了包法（例如放在 `finish_reason` 前一段之外的地方），都认不出，照旧落成被打断的半截。
- 依据只靠文字，是因为结构上能用的只有「出错事件紧跟在第一段正文后面」，单靠它分不清真断流。

### 验收

1. 修前：换回 main 的解码器，`relay-refusal-error` 那份样本解出来是一段正文加 `retryable`（和现场一样），对不上。
2. 新加的五份流样本：紧跟着出错、断了、夹着只报用量的一段再出错的没有正文、`content_policy`；照常说完的、后面还有正文的照原样交出去。`refusal/tests.rs` 两条，分类一条。
3. 手写变异 6 个都红：从不认、只看开头那一句、只报用量的也放出、出错时不丢、流断时照放出、分类没有这句说法。
4. `cargo xtask check` 全过、Windows 版 clippy、三平台 CI。
