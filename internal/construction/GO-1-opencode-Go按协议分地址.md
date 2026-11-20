## 施工 GO-1：opencode Go 按协议分地址

状态：已完成（2026-11-17）。发版前核对真端点的一批，核心主会话派。

### 查到的

- **官方现在的写法**（2026-11-17 查）：`https://opencode.ai/docs/go` 的端点表、`https://opencode.ai/docs/console/harnesses` 的「Base URLs」一节。根是 `https://opencode.ai/inference`（按量付费，原来的 Zen）、`https://opencode.ai/inference/go`（Go），后面照协议接：Responses `/openai/v1/responses`、Chat Completions `/openai/v1/chat/completions`、Anthropic Messages `/anthropic/v1/messages`、Gemini `/google/v1beta/models/<模型>:generateContent`。端点表里 Grok、GPT、Muse Spark 走 Responses，Claude Haiku、MiniMax M3、Qwen 几个走 Messages，Gemini 走 Google，别的走 Chat Completions，和目录里各模型的包名一致。官方文档不提旧地址 `/zen/go/v1`。
- **models.dev 跟着改了**（`https://models.dev/api.json`，2026-11-17）：`opencode-go` 这一家的 `api` 是 `https://opencode.ai/inference/go/openai/v1`，每个模型在 `provider.api` 写自己那条（`…/openai/v1`、`…/anthropic/v1`、`…/google/v1beta`）；`opencode` 同样（`https://opencode.ai/inference/openai/v1` 等）。
- **出厂快照**（`resources/models/models-dev.json`，2026-10-01 拉的）还是旧的：这一家 `https://opencode.ai/zen/go/v1`，模型只写包名、不写地址。
- **问题**：核心只认这一家的地址，模型照包名换驱动、地址不换。快照里旧地址一条走三种协议，没出事；后台拉到新目录以后（`catalog.update` 默认开），Go 里走 anthropic 的模型（MiniMax M3、Qwen 3.7 Plus、Qwen 3.8 几个）发到 `…/inference/go/openai/v1/messages`，404。ST-1 当时只看到「列不到模型」。
- **不带 key 的探测**（2026-11-17，`curl -A miyu-probe/0`，带 `x-opencode-session`）：

  | 地址 | 回 |
  |---|---|
  | `/zen/go/v1/chat/completions`、`/messages`、`/responses` | 401 `OpenCode Go requires an API key` |
  | `/inference/go/openai/v1/chat/completions`、`/responses` | 401 同上 |
  | `/inference/go/anthropic/v1/messages` | 401 同上（Anthropic 的错误格式） |
  | `/inference/go/openai/v1/messages` | 404 |
  | `/inference/go/anthropic/v1/chat/completions` | 404 |
  | `/inference/go/v1/chat/completions` | 401（文档没写） |

  旧地址还答，新地址按协议分、走错协议的 404。没有能用的 Go key，没发真请求。

### 怎么改

1. **目录读模型的 `provider.api`**（`catalog.rs`）：`CatalogModel::api`，和 `npm` 一样照原文、同样的字共用一份。整理好的一份（`catalog/digest.rs`）的一行多一格，写法的版本 2 → 3。记下的查目录的答（`catalog/recall.rs`）带着整条模型，版本不动：以前写的没有这一格的照没有读，换了程序版本串本来就变了、整份重记。
2. **合资料带上它**（`facts.rs`）：`Wire::api`，和包名一样只取第 1、2 层对上的。
3. **发给模型时照它换地址**（`provider.rs` 的 `Provider::for_model`）：驱动、地址都不是手写的（新的一格 `Provider::base_url_written`：配置、档案写的算，照目录这一家推出来的不算），模型写了自己的地址的照它。手写的地址、手写的驱动照手写的：中转、自己配的地址不跟着跳。
4. **发的地方照换的地址**：路由先照这一家算好地址和 key（`route/choice.rs`），挑定了模型时（`route/base.rs` 的 `ready`）照 `Provider::moved_address` 只换地址，key、头照旧；`provider.test` 发的那一句同样（`route/probe.rs`），列模型照这一家的。`miyu rescue` 的直接连（`direct.rs`）本来就照 `for_model` 的结果。
5. **Zen 钉在旧地址**（核心主会话 2026-11-17 定：免费档在新地址上实测出字才跟）：档案 `[providers.opencode]` 写 `base_url = "https://opencode.ai/zen/v1"`，档案写了地址的不跟模型自己的地址。实测见下面「Zen 免费档」。
6. **出厂快照不换**：全量换快照牵动八千多个模型和一批照快照写的测试，另作一步，等拍板。快照里旧地址还答，离线的机器照旧用得了。
7. QU-1 的额度地址 `https://opencode.ai/inference/go/v1/usage` 不动。

### Zen 免费档（2026-11-17 实测）

照档案给 Zen 发的头（`User-Agent: opencode/2.0.21`、`x-opencode-client: cli`、`x-opencode-project: global`、`x-opencode-session: ses_<倒序时间 12 位 + 种子哈希 14 位>`），不带 key，流式，工具面带 `read`、`shell` 两件占位，说一句「Reply with the single word: pong」：

| 模型（目录里标免费的） | `https://opencode.ai/zen/v1` | 新地址 |
|---|---|---|
| `deepseek-v4.1-flash-free`（chat） | 429 `FreeUsageLimitError` | `…/inference/openai/v1`：429 `FreeUsageLimitError` |
| `big-pickle`（chat） | 429 `FreeUsageLimitError` | 同上 429 |
| `minimax-m3-free`（anthropic） | 400 `Upstream request failed: Model is unavailable.` | `…/inference/anthropic/v1`：同样的 400 |
| `hy3-free`（chat） | 400 `Model is unavailable.` | 同样的 400 |

429 的原话：`Free usage limit reached. Free models work in OpenCode (https://opencode.ai); subscribe to OpenCode Go to keep going: https://opencode.ai/go`。带 `Authorization: Bearer public` 一样；对照组（`User-Agent` 换成别的、不带 `x-opencode-*` 头）也是这一句 429，所以这次 429 说明不了过没过客户端识别的闸，只是这台机器的免费额度用完了。新旧地址都出不了字，照定的不跟，限流不反复试。

### 蓝图改哪几节

`models.md`：第八条「opencode Zen」开头的地址那一段。`profiles.toml` 的注释。

### 验收

1. 改前红：
   - `miyu-models` 的 `direct::tests::a_model_s_own_address_follows_its_package_unless_written_by_hand`：照 models.dev 现在的写法裁一份目录，`opencode-go/minimax-m3` 要 `…/inference/go/anthropic/v1`、anthropic；改前是 `…/inference/go/openai/v1`。DeepSeek、GPT 照 `…/openai/v1`；手写地址的中转、手写驱动的照手写的。
   - `miyu-session` 的 `route_zen::a_go_model_with_its_own_address_is_sent_there`：假服务器上真发两次，路径是 `/anthropic/v1/messages`、`/openai/v1/chat/completions`，`x-api-key`、`x-opencode-session` 照带；改前第一条是 `/openai/v1/messages`（和真端点 404 的那条一样）。
   - `catalog::tests` 多两句：模型的 `provider.api` 读得到，整理好的一份读回来带着它。
2. 门禁全过；`cargo test -p miyu-kernel random:: -- --include-ignored`；整个工作区 Windows 版 clippy；`cargo xtask prompts`、`cargo xtask logs`。

### 风险

- 手写了旧地址 `https://opencode.ai/zen/go/v1` 的配置，新目录里照地址认不出是 opencode Go（目录里的地址换了），照编号认得出的不受影响。
- Zen 钉着旧地址：opencode 哪天下掉 `/zen/v1` 就断，到时候先在新地址上实测免费档出字，再去掉档案那一行。
