## 施工 QU-2：DeepSeek 余额（接着 QU-1 的供应商额度）

状态：已完成（2026-11-16）。项目主人 2026-11-16 定：用 DeepSeek 官方供应商时，用量面板里显示余额。头画一行「余额 ¥12.34」，有赠送的后面暗字「含赠送 ¥2.00」，不可用时整行变红，不画条；头怎么画交给界面会话，这一步只做核心。

### 目的

DeepSeek 是充值扣费的，钱用完了请求就回 402。以前只能去官网看还剩多少。现在核心照会话用的那一家去问余额，头在用量框里画一行。

### 数据

`GET https://api.deepseek.com/user/balance`，头 `Authorization: Bearer <那一家的 key>`。官方文档 <https://api-docs.deepseek.com/api/get-user-balance>：

```json
{"is_available":true,"balance_infos":[{"currency":"CNY","total_balance":"12.34","granted_balance":"2.00","topped_up_balance":"10.34"}]}
```

金额是字符串，币种是 `CNY` 或 `USD`。

### 目录那一格

`resources/models/profiles.toml` 的 `[providers.deepseek]` 多一格，写法同 QU-1：

```toml
quota = { url = "https://api.deepseek.com/user/balance", format = "deepseek-balance" }
```

`QuotaFormat` 多一种 `deepseek-balance`，`read` 多一支（`miyu_models::quota::balance`）。`read` 交 `Reading { windows, balance }`：窗口、余额各是各的，一种格式只填其中一样。配置 `[providers.<id>.quota] url` 照旧能盖地址。

### 只认官方地址

经中转站跑 DeepSeek 模型的供应商（`catalog = "deepseek"`、地址指中转站），手里那把 key 是中转站的；拿它去问 `api.deepseek.com` 等于把中转站的 key 交给 DeepSeek（PK-1 补修过同一种错）。所以：

1. 目录里认得出官方地址的那一家（`provider_key::OFFICIAL`，现在只有 DeepSeek），配置里这一家供应商写的地址还得是官方的，`Provider.quota` 才有（`quota::settled` 多收一个 `catalog`）：没写地址（照档案就是官方的），或者字面地址的主机就是 `api.deepseek.com`。地址是环境变量引用的（查不出来）、别的地址一律没有，就不查、不发请求。
2. 配置改了查余额的地址也照这一条：只在供应商的地址是官方的时候查，不然官方的 key 会发去别处。
3. 判法和交 key 给扩展同一处：`provider_key` 里原来写在 `serving` 里的那段拆成 `official_host`、`official_address` 两个公开的函数，`serving` 和 `quota::settled` 都照它，只看主机，路径、查询、用户名里出现的不算。
4. 认不出官方地址的一家（opencode Go）不管地址，照 QU-1。

### 怎么查

和 QU-1 同一份代码（`miyu-session` 的 `route/quota.rs`），只是存的、交的从几个窗口换成 `Reading`：头 `Authorization: Bearer <key>`、`User-Agent` 是 `miyu/<版本>`；同一家 60 秒内照上一次的（查不到的也算一次）；会话每轮结束在后台查一次；查不到（401、500、连不上、超时、格式不对）当没有，记一行 `DEBUG quota unavailable`。

读（`quota/balance.rs`）：`is_available` 是真假，`balance_infos` 是数组，每一条四格都是字符串；金额、币种原样交，不换成数，不认死两种币。多出来的格不管；缺格、类型不对整份当没有。一种币都没有的照样交（`items` 空着），头至少能照 `available` 变红。

### 协议：`session.quota` 多一格 `balance`

```json
{"balance":{"available":true,"items":[{"currency":"CNY","total":"12.34","granted":"2.00","topped_up":"10.34"}]}}
```

- `available`：官方的 `is_available`；`items` 照官方的先后，`total`、`granted`、`topped_up` 是 `total_balance`、`granted_balance`、`topped_up_balance` 原样的字符串。
- 没查到的不带这一格；`windows`（QU-1）和 `balance` 各是各的，有了哪一格不影响另一格，都没有的照旧 `{}`。
- 形状 11-16 已发给界面会话。没有新方法、没有新权限，`methods-v1.txt` 不动。

### 蓝图、参考改哪几处

- `models.md`：「在哪」`quota.rs` 那一行、加 `quota/balance.rs`；「怎么走」第十七条加第 8 条；「守着它的」三行。
- `protocol.md`：方法表那一行，`session.quota` 一节加第 4 条（形状表、例子）。
- `config.md`：`providers.<id>.quota.url` 的说明三语补一句。
- 协议参考 `sessions.md`、`sessions.en.md`。
- 资源：`profiles.toml`、`core/human/{zh,en,ja}.json` 配置项的说明；配置的生成样本重写。

### 不做什么

- 头怎么画（界面会话做）。
- 不推送、不告警；余额见底照旧只看供应商回的出错分类（402 记 `quota` 冷却）。
- 不替经中转站的 DeepSeek 查余额，也不另开「这把 key 是官方的」的开关：要查就把地址写成官方的。

### 验收

1. `miyu-models`（`src/quota/tests/deepseek.rs`）：
   - `the_profile_takes_the_deepseek_balance_format`；
   - `the_balance_is_asked_only_where_the_provider_talks_to_the_official_address`：没写地址、字面官方地址（大小写、带路径）的有；中转站、路径里藏官方地址、环境变量地址的没有；改了查的地址，官方的照改、中转站的照旧没有；认不出官方地址的一家照 QU-1；
   - `the_reply_reads_into_a_balance_with_the_amounts_as_written`：两种币照先后、金额原样、多的格不管、一种币都没有的照交；
   - `a_broken_balance_spoils_the_reply`：不是 JSON、缺 `is_available`、类型不对、金额是数、缺格、别的格式的回应都不要。
2. `miyu-session`（`tests/route_balance.rs`，查余额的地址指着本机的假服务器，不打真网）：
   - `the_official_provider_reads_its_balance_with_the_key_and_reuses_it_for_a_minute`：`GET`、`Authorization: Bearer sk-ds`、读成余额，60 秒内第二次不再发；
   - `a_literal_official_address_and_a_moved_balance_address_are_asked`：字面官方地址加配置改的查余额地址照查；
   - `a_relay_or_an_env_address_is_never_asked_even_with_a_moved_balance_address`：中转站、路径里藏官方地址、环境变量地址、写了 catalog 指中转站、写了 catalog 地址是环境变量的，配置改不改查的地址都一次不发；
   - `a_refused_failed_or_garbled_reply_is_no_balance_and_not_asked_again_at_once`：401、500、乱码、缺格当没有，紧接着再问不再发。
3. `miyu-endpoint`（`tests/session_balance.rs`，真核心、真路由）：
   - `an_official_deepseek_session_gets_its_balance_as_written`：`session.quota` 交 `{"balance": …}`（不带 `windows`），金额、币种原样，key 在 `Authorization`；
   - `a_deepseek_session_through_a_relay_has_no_balance_and_sends_nothing`：经中转站的回 `{}`，配置改了查的地址也不发。
4. QU-1 的测试照旧过（`read` 改交 `Reading`）；PK-1 补的 `provider_key` 测试照旧过（判法挪成公开函数）。
5. 实测：沙箱数据根起核心，配置改查余额的地址指本机假服务器，看 `session.quota` 回应原文；真模型经网关开一个会话，`session.quota` 不带 `balance`、日志里没有发往 `api.deepseek.com` 的请求。
6. 门禁全过；Windows 版 clippy 过。

### 风险

- 官方改了写法的：当没有、记 `DEBUG`，用量框里那一行不出来，不影响别的。
- 主人自己写了官方地址的变体（`https://api.deepseek.com/v1` 之类）：只看主机，照样认；写成环境变量的不查，要看余额就写字面地址。
