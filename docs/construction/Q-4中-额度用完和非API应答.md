## 施工单 Q-4 中：额度用完单独一类，2xx 非 API 应答照样换人

状态：核心的分会话做。设计 `43-旁路请求和档位池.md` 第三节第 5 条第 3 款，第五节第 4 条（Q-4 拆三步的中间一步，主会话 2026-10-27 认可）。主会话补的两条照做：额度用完照供应商给的重置时刻、封顶 24 小时，冷却时记一行 `WARN` 写明哪个供应商、到几点；2xx 非 API 应答只在驱动确实拿不到可解析的应答时算，流里已经吐了一半再断的照原来的断流办，不换成员重来。

### 为什么

额度、余额用完（402、各家的余额码）原来归在认证失败里，冷却照认证失败的 10 分钟起翻倍：供应商明说了几点重置也不照；日志里只有一行和网断了一样的 `INFO endpoint cooling`，人看不出是没钱了。

### 做了什么

1. 错误分类多 `quota`（`miyu-kernel` 的 `event/model.rs`）。驱动分类（`miyu-drivers` 的 `classify.rs`）：402、额度的说法（`insufficient_quota`、余额不足、`FreeUsageLimitError` 这些）先认成 `quota`，401、403 照旧 `auth`。三家驱动共用这一份，`anthropic` 的 402 `billing_error`、`openai-responses` 的 429 `insufficient_quota` 跟着变。内核照旧不重试它（同 `auth`）。
2. 冷却（`miyu-models` 的 `cooldown.rs`）：`Rules` 多 `quota`，配置 `[models.cooldown.quota]`（`base`、`max` 默认都是 30 分钟）只管供应商没说什么时候重置的；说了的（`retry-after` 这些）照说的，短的也照，最多一天（`QUOTA_SAID_MOST`，主会话定的封顶）。单位照旧是这个模型：同 `auth`，中转站会把上游的额度错透传过来，整家停会连累别的上游（项目主人 2026-10-19 为 401 定的那条照用）。
3. 记冷却时额度用完另记一行 `WARN provider out of quota provider=… model=… until=<时刻>`（`route/ended.rs`）。
4. 旁路请求（一次性入口的 `call_side`）遇到额度用完照样退到会话的模型；会话端口那一条照「记了冷却」退，本来就算它。
5. 命令行出错的人话、视图的 `view/error/class/quota`、配置项两项，三语；认证失败冷却那两项的说明改对（不再写「额度用完」「整家供应商」）。配置样本重生成，图纸里嵌的样本同步。
6. 2xx 非 API 应答：查下来三家驱动原来就是这样——一页 HTML、空的响应体、不是应答的 JSON 里都没有一个能解的事件，解码到结尾没有应答、没有结束标记，报 `retryable`，池里照样记冷却、换下一个；已经收到过增量再断的，路由那一层照「收到过增量」不换（`route/ended.rs` 原来的规矩）。没改代码，补了三家驱动、三种回应的测试守着。

### 不做的

- 全冷却只探一个：Q-4 下。
- 额度用完整家停：没做，理由见第 2 条；要整家停另说。

### 验收

1. `miyu-drivers` 的 `classify/tests.rs`：402、429 带额度说法、401/403 带额度说法、Anthropic 的 `billing_error`、OpenCode Zen 的免费档用完都是 `quota`，401、403 照旧 `auth`；修以前 4 条红。
2. `miyu-models` 的 `cooldown/tests.rs`：额度用完没说的 30 分钟、连着也是，说了五小时照五小时，说一分钟照一分钟，说三天封顶一天；配置照 `[models.cooldown.quota]`。
3. `miyu-session/tests/quota_log.rs`：池里 402 换到别的成员，冷却照供应商说的两小时，记 `WARN provider out of quota` 带供应商、模型、冷却到的时刻；修以前红在 `WARN` 那一行。`once_side.rs` 的 `an_out_of_quota_normal_pool_falls_back_too`：一次性旁路请求退到会话的模型。
4. `miyu-session/tests/route_non_api.rs`：三家驱动、三种 200 的非应答都报可重试、换到下一个。
5. 手改都红：分类不认额度、额度不记冷却、说了的不照、说了的不封顶、不记 `WARN`、旁路不认额度、驱动把「流完了没应答」分成 `bad_stream`。
6. `cargo xtask check` 全过、Windows 版 clippy、三平台 CI（碰驱动）。
