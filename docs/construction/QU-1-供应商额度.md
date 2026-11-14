## 施工 QU-1：供应商的额度（第一家 opencode Go）

状态：已完成（2026-11-14）。项目主人 2026-11-14 定：usage 面板里显示 opencode Go 的三个额度窗口（5 小时滚动、每周、每月），只在当前会话用的是 opencode Go 时出现；头怎么画交给界面会话，这一步只做核心。

### 目的

opencode Go 是订阅制，额度按 5 小时、每周、每月三个窗口算，用完了要等重置。以前只能去官网看。现在核心照会话用的那一家去问额度，头在用量框里一行一个窗口画出来：名字、条、百分比、重置时刻。

### 数据

`GET https://opencode.ai/inference/go/v1/usage`，头 `Authorization: Bearer <那一家的 key>`。2026-11-14 实测回 200：

```json
{"usage":{"rolling":{"status":"ok","percent":4,"resetsAt":"2026-11-14T08:31:28.000Z"},"weekly":{...},"monthly":{...}}}
```

官方文档没写这个接口，旧地址 `/zen/go/v1/usage` 也还能用。`User-Agent` 要是非默认库的（Cloudflare 挡），客户端本来就是 `miyu/<版本>`；`x-opencode-session` 照档案另配的头带。

### 目录那一格：`quota`

`resources/models/profiles.toml` 的 `[providers.opencode-go]` 多一格：

```toml
quota = { url = "https://opencode.ai/inference/go/v1/usage", format = "opencode-go" }
```

- 名字用 `quota`：项目主人举的例子就是它；额度用完的出错分类本来就叫 `quota`（`[models.cooldown.quota]`），同一个「额度」。不叫 `usage`：核心里的 usage 是记账的 token 和金额，混在一起分不清。
- 写成 `{ url, format }` 而不是一个地址：以后别家有额度接口，回应的写法各不一样，照 `format` 挑读法（`miyu_models::quota::QuotaFormat`，现在只有 `opencode-go`，加一种多一支）。
- 配置能盖地址：`[providers.<id>.quota] url = "…"`（`QuotaSettings`），同 `base_url` 盖档案的地址；`headers`、`compat` 这些档案的格配置盖不了，地址是会挪的那一样（这一次就挪过一回）。格式只有档案知道：档案没写 `quota` 的一家，配置写了地址也不查。配置不开 `format`：选项类的配置项至少两个选项，等有第二种格式时再加。设置页不画（`hidden`）：只在供应商挪了地址、出厂的档案还没跟上时手写。
- 代码里不出现这个地址。

### 怎么查

`miyu-session` 的 `route/quota.rs`：

1. 只查档案写了 `quota` 的一家（经 `provider::provider` 合出来的 `Provider.quota`，配置盖过的地址在里面）；没有 key 的不查。
2. 头：`Authorization: Bearer <key>`（不照驱动的认证头：Go 上 Claude、MiniMax 走 `anthropic`，那是 `x-api-key`），档案另配的头照固定的种子 `quota` 换（额度不属于哪个会话，同一次性调用照用途），`User-Agent` 是客户端的。地址落在本机的不走代理，同拉列表。一次最多 10 秒，响应体最多 64 KiB。
3. 同一家 60 秒内照上一次的结果，查不到的也算一次；同一时刻只有一个在查，后到的等它查完照它答。只在内存里（`ModelData` 一份），核心重启就清零。
4. 会话每轮结束（这一批落盘的事件里有 `turn.ended`）actor 告诉路由（`ModelPort::turn_ended`，默认什么都不做），路由在后台查一次上一次解析出来的那一家（`tokio::spawn`，不挡回合）；还没模型的（`none`）不查。
5. 查不到、超时、回的不是 2xx、格式不对，一律当没有，记一行 `DEBUG quota unavailable`（`provider`、`error`），不报错、不告警。
6. 读（`miyu_models::quota::read`）：照 rolling、weekly、monthly 的先后，没有的窗口不交；`percent` 是数、原样；`status` 是字、原样；`resetsAt` 照 RFC 3339 读（`quota/instant.rs`：带不带小数秒、`Z` 还是 `+09:00` 这类偏移都认，日期和时刻之间认 `T`、`t`、空格，小数秒取到毫秒；主会话 11-14 定不只认一种），换成 UTC，交出去写成 `2026-11-14T08:31:28.000Z`。一个窗口都没有、有一个写错，整份当没有。

### 协议：`session.quota`

参数 `{"session"}`，回应 `{"windows": [{id, label, percent, status, resets_at}…]}`，没有的 `{}`。`id` 是 `rolling`、`weekly`、`monthly`；`label` 照连接的语言（三语字表 `quota/window/<编号>`：5 小时、每周、每月 / 5 hours、Weekly、Monthly / 5 時間、週間、月間）。照会话接下来请求的那一家（`subscribe` 的 `model.endpoint`）；轮换的池没有。

选方法、不随用量推送：

- 额度是一家供应商的，不是会话的：推进 `view.status` 要每个会话的状态多一格，查完了（在后台、晚到）还要另找时机重推整份状态，状态那边也不知道连接的语言。
- 头的用量框本来就是打开时问几次 `usage.query`（终端 `app/usage.rs`），多问一次最顺；网页同理。
- 每轮结束核心在后台查过一次，60 秒内照它答，打开用量框多半不用等；过了 60 秒的当场查，在后台答，不挡这个连接别的请求。

权限同 `session.usage_parts`：扩展要 `events.read`；关着的场所会话人的头只读地也能问（`sessions/peek.rs` 的 `READS`：只读，不改会话）。登记进 `methods-v1.txt`。

### 蓝图、参考改哪几处

- `models.md`：「在哪」加 `quota.rs`、`route/quota.rs`，`profiles.toml` 那一行；「协议」加一句；「怎么走」第十七条；「给人看的字」额度窗口的名字；「守着它的」三行。
- `protocol.md`：方法表一行，`session.quota` 一节（形状、为什么选方法）。
- `config.md`：清单加 `providers.<id>.quota.url`，名字和说明三语一行。
- 协议参考 `sessions.md`、`venues.md`（中英）。
- 资源：`profiles.toml`、`core/human/{zh,en,ja}.json`（配置项的名字说明、三个窗口的名字）；配置的生成样本重写。

### 不做什么

- 头怎么画（界面会话做）。
- 不推送、不进 `view.status`。
- 不在额度见底时自己换模型、不告警：冷却照旧只看供应商回的出错分类。
- 别家供应商：等有了实测的接口再加 `format`。

### 验收

1. `miyu-models`（`src/quota/tests.rs`）：`the_profile_takes_a_quota_with_its_format`、`the_config_moves_the_address_but_needs_the_profile`、`the_go_reply_reads_into_three_windows_in_order`、`a_missing_window_is_left_out_and_a_broken_one_spoils_the_reply`、`reset_times_read_any_rfc3339_form_into_utc`（带不带毫秒、大小写、空格、正负偏移都换成同一个 UTC 时刻；不带时区、`+0900`、越界的偏移、不存在的日子、闰秒都读不出）。
2. `miyu-session`（`tests/route_quota.rs`，额度地址指着单独的一台假服务器）：
   - `the_go_quota_reads_three_windows_with_the_key_and_is_reused_for_a_minute`：三格、`GET /v1/usage`、`Authorization: Bearer sk-go`、`x-opencode-session` 照种子 `quota`、`User-Agent` 是 `miyu/`；60 秒内第二次不再发；
   - `a_provider_without_a_quota_is_not_asked`：档案没写的，配置写了地址也不发；
   - `the_config_moves_the_address`：配置盖地址；
   - `a_failed_or_garbled_reply_is_no_quota_and_not_asked_again_at_once`：404、500、乱码、写错的一格都当没有，紧接着再问不再发；
   - `a_turn_ending_asks_once_in_the_background`：没说话不查，说完一轮后台查一次，再问照它答。
3. `miyu-endpoint`（`tests/session_quota.rs`，真核心、真路由）：
   - `a_go_session_gets_three_windows_checked_after_its_turn`：说完一轮后台查过，`session.quota` 交三格（中文名字），不再发；
   - `no_quota_interface_or_a_failed_one_is_an_empty_answer`：档案没写的不发、回 `{}`；500、404、乱码回 `{}` 不报错；
   - `an_unknown_session_or_bad_params_are_refused`。
4. 改前红：见提交说明。
5. 门禁全过；Windows 版 clippy 过。

### 风险

- 接口没有文档，opencode 改了地址、改了写法的：改地址照配置盖、出厂档案跟上；改了写法的当没有、记 `DEBUG`，用量框里那一块不出来，不影响别的。
- 每轮结束都可能发一次 GET：同一家 60 秒内只发一次；不是 opencode Go 的会话一次都不发。
