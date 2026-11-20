## 施工 MD-SNAP：出厂的目录快照换成现在的

状态：已完成（2026-11-17）。核心主会话 2026-11-17 定（GO-1 交回时提的拍板项）。

### 为什么

安装包带的 models.dev 目录快照（`resources/models/models-dev.json`）还是施工 8-7 拉的 2026-10-01 那一份。opencode 2026-11 起按协议分地址（GO-1），离线的机器、后台更新关着的、第一次起来还没拉到新目录的，照快照走的都还是旧地址；快照里也没有这一个半月新出的模型。

### 改了什么

1. `models-dev.json` 换成 2026-11-17 01:25:54 UTC 从 `https://models.dev/api.json` 拉的原样字节（`ETag` `e11d25c0950a068c237c3922462221be`）：5.3 MB → 6.7 MB，225 家 → 324 家，8341 个模型 → 9922 个。去掉了 6 家（`crof`、`iflowcn`、`lilac`、`llama`、`tempr`、`v0`），多了 105 家。`models-dev.meta.json` 的 `fetched` 跟着换。许可证（MIT）没变。
2. 快照里 opencode Go 这一家的地址是 `https://opencode.ai/inference/go/openai/v1`，每个模型写着自己那条（GO-1 照它走）；Zen 这一家是 `https://opencode.ai/inference/openai/v1`，档案钉在 `/zen/v1`（GO-1）。
3. 照快照写的测试只改期望值：`miyu-core` 的 `the_bundled_catalog_and_vendor_table_read` 里 DeepSeek 官方 `deepseek-flash` 的窗口 1 000 000 → 1 048 576（目录里改了写法），断言的意思没变。别的测试一条没动，意思变了的没有。
4. 蓝图 `models.md` 第二条第 1 条写上新快照的大小、家数、模型数。

### 验收

1. 整个工作区的测试全过（125 个测试程序）；门禁全过。
2. 离线实测（沙箱数据根，`XDG_CACHE_HOME` 指到空目录、没有缓存的目录，`models.catalog.url` 指到 `http://127.0.0.1:9/api.json` 连不上）：
   - 核心起来记 `catalog loaded source=snapshot fetched=2026-11-17T01:25:54.000Z providers=324 models=9922`，接着 `WARN catalog refresh failed error="…Connection refused…"`，照快照用。
   - `model.list`：`opencode-go` 的 `base_url` 是 `https://opencode.ai/inference/go/openai/v1`、42 个模型；`opencode` 是 `https://opencode.ai/zen/v1`（档案钉的）、140 个模型。
   - 拿一把假 key 真发：`opencode-go/minimax-m3`、`deepseek-v4.1-flash`、`gpt-5.6-luna`，`opencode/minimax-m3-free`、`deepseek-v4.1-flash-free` 都回 `HTTP 401: Invalid credential`。对照（同一把假 key 直接 curl）：`…/inference/go/anthropic/v1/messages`、`…/zen/v1/messages` 回 401，`…/inference/go/openai/v1/messages`、`…/inference/openai/v1/messages` 回 404，所以 Go 的 minimax-m3 是照它自己的 `…/anthropic/v1` 发的，Zen 是照 `/zen/v1` 发的。

### 风险

- 快照大了约三成，整份常驻时内存跟着多（施工 V-25 下以后平常只常驻记下的答，起来时读一次整份约 0.37 秒，本机调试版）。
- 去掉的 6 家是 models.dev 自己删掉的：配置里照编号写了它们、又没写地址的，以前离线时还认得出，换了以后要手写地址（联网的机器后台拉到新目录以后本来就认不出了）。
