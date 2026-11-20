## 施工单 X-23：model.call 照用途挑模型

状态：施工中（2026-10-16）。项目主人 10-16 验表情包后台页时要「重新描述」：看图模型把描述、用法写好填进框里。扩展说不出人配了哪个模型（`models.vision` 在人的配置里），`model.call` 只认具体的模型、池。

### 这一步做什么

1. `model.call` 多一个参数 `use`：`chat`、`vision`，照这一刻的配置用 `models.chat`、`models.vision`；和 `model` 至多写一个，一起写的、不认识的 `bad_params`。
2. `use` 是 `vision`、`models.vision` 没配的：`no_model`，原话同替看图那一句（`no vision model configured: set models.vision`）。

### 施工时定的（主会话，技术题照推荐）

- 只开这两个用途：别的用途（`embedding`）不是对话模型，`model.embed` 另有方法。
- 不另设能力：同 `model.call`。

### 验收

- 真核心（`tests/model_call.rs` 的 `use_picks_the_configured_model_for_that_purpose`）：`vision` 发给 `models.vision` 那个、`chat` 发给 `models.chat` 那个；一起写、写错的 `bad_params`、一次都没发；没配 `models.vision` 的 `no_model` 和原话。
