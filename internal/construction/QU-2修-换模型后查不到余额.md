## 施工 QU-2 修：换了模型，额度照换成的那一家查

状态：待验收（2026-11-16）。界面会话 11-16 实测报来（核心 main 44385d35f），主会话转来。

### 查出来的

沙箱里配 DeepSeek 官方供应商（假 key），`[providers.deepseek.quota] url` 指本机假余额服务器：

1. 直接用 `deepseek/deepseek-v4-pro` 造的会话，`session.quota` 回 `balance`，假服务器收到请求。
2. 先用别的模型开会话，再 `session.configure` 换到 `deepseek/deepseek-v4-pro`：之后 `session.quota` 一直回 `{}`，假服务器一个请求都没收到。
3. 重启核心以后，那个换过模型的会话查得到。

### 根因

`miyu-endpoint` 的 `quota.rs` 照 `found.handle.next().model` 的 `endpoint` 查。`Next` 是路由上一次解析的那一个（`miyu-session` 的 `shown.rs`，`Next::of(port)`，actor 的 `hand()` 写），而换模型照设计下一个回合开始才重新解析（`models.md`「怎么走」第六条第 3 条，`Actor::begin` 里叫 `ModelPort::turn` 再 `hand`）。`session.configure` 只叫内核记一条 `session.policy_changed`，不碰路由，所以换了、还没开回合的，`next` 还是换之前的那一家（中转站，没有 `quota`），回 `{}`、不发请求。

重启就好：载入会话时路由照日志里换过的引用造，`next` 当场就是 DeepSeek。说完一轮也会好（回合开始重新解析），界面会话那次没说话。

`subscribe` 的 `model`、`model.changed` 照同一份 `Next`，换了模型下一个回合开始才变：这是 8-10 定的（`models.md`「怎么走」第六条第 5 条，`protocol.md`「换模型」第 4 条：下一个回合开始时生效，生效的那一刻推 `model.changed`），不改。opencode Go（QU-1）走同一条路，同样坏。

### 做什么

`miyu-endpoint` 的 `quota.rs` 多一个 `upcoming`：接下来请求的那一家照会话记着的引用定，同 SM-1 补 `session.configure` 的 `effort`（`models/effort.rs`）：

1. 记着的引用：`Handle::current` 的 `model`（内核照日志算的，刚换了、还没开回合的也是换成的），没记的跟着这一刻的 `models.chat`。
2. 和路由上一次解析的（`Next::reference`）一样的：照路由的 `Next::model`，钉住的池照钉着的成员，轮换的池没有。
3. 不一样的（换过了、下一个回合开始才重新解析）：模型照引用写的那一家；这一家配置里没有、改过名的照新编号（`ModelData::renamed`，同 `route/turn.rs`）。换成了池的：钉哪个成员要下一个回合开始才定，这时没有，回 `{}`。
4. 会话停了、问不到 `current` 的照路由的。

协议形状不变，`methods-v1.txt` 不动。蓝图 `protocol.md`「`session.quota`」第 1 条、`models.md`「怎么走」第十七条第 5 条和「守着它的」两行，协议参考 `sessions.md`、`sessions.en.md` 各补一句。

### 不做什么

- 换模型当场重新解析路由、当场推 `model.changed`：8-10 定了下一个回合开始生效，头上显示的模型照旧下一轮才换。
- 换成池的当场猜钉哪个成员：要动池的指针，留到回合开始。

### 验收

1. 测试（改前红）：
   - `miyu-endpoint` 的 `tests/session_balance.rs` 新一条 `switching_a_session_to_official_deepseek_gets_its_balance_before_the_next_turn`：中转站开的会话换到官方的，没开回合就交 `balance`、假服务器收到一次；换回中转站的回 `{}`、不再发。
   - `tests/session_quota.rs` 新一条 `switching_a_session_to_go_gets_its_windows_before_the_next_turn`：别的一家开的会话换到 opencode Go，没开回合就交窗口、假服务器收到一次。
   - 退回改前的 `quota.rs`，两条都红（`balance` 是 `null`、`windows` 是 `null`）。
2. 实测：沙箱数据根起核心，照「查出来的」三步走真协议。

### 施工结果

1. **测试**：新两条过，`miyu-endpoint` 的 `session_` 一共 164 条照旧过。
2. **实测**（沙箱数据根，`relay` 中转站开会话，DeepSeek 官方假 key、查余额的地址指本机假服务器）：
   - 开会话问 `session.quota`：`{}`；`subscribe` 的 `model` 是 `relay/deepseek-v4.1-flash`。
   - `session.configure` 换到 `deepseek/deepseek-v4-pro`：`subscribe` 的 `model` 照旧是 `relay`（下一轮才换）；`session.quota` 回 `{"balance":{"available":true,"items":[{"currency":"CNY","granted":"2.00","topped_up":"10.34","total":"12.34"}]}}`，再问一次一样；假服务器收到 1 次（`/user/balance`、`Bearer`、`miyu/2026.11.0`）。
   - 换回 `relay`：`{}`，假服务器还是 1 次。再换到 DeepSeek、重启核心：`subscribe` 的 `model` 是 `deepseek/deepseek-v4-pro`，`session.quota` 回同一份 `balance`，假服务器 2 次（重启清了 60 秒复用）。
