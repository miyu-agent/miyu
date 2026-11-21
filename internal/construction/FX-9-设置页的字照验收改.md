## 施工单 FX-9：设置页的字照验收改：联网服务去掉三处说明，「看图的模型」改叫「多模态模型」

状态：做完了，等合并（分支 `step/FX-9`，从 main 开；主会话 2026-11-21 派工）。蓝图 `config.md`「给人看的字」的 `models.vision`、`deepseek-responses` 两行和参考文件的样子，`tools/web_search.md`「守着它的」。

### 目的

项目主人 2026-11-20 验收设置页时要改的几处字：

1. 「联网服务」的选项 `deepseek-responses` 的说明去掉后一句「搜索更深入，但每次搜索会多用约一万个 token。」，前一句留着。
2. 「联网服务」下拉里各家后面的「需要密钥」「需要地址」不再写。冷却中的那一家照旧写「冷却到 15:30」。
3. 默认模型里的「看图的模型」改叫「多模态模型」，说明改成「在对话模型不支持多模态时提供多模态能力。」。英文、日文同义：`Multimodal model`、「Provides multimodal abilities when the chat model doesn't support them.」；`マルチモーダルモデル`、「会話のモデルがマルチモーダルに対応していないとき、マルチモーダルの機能を提供します。」。配置键 `models.vision`、协议里的 `vision` 都不改。

### 改了什么

- 字表 `resources/core/human/{zh,en,ja}.json`：上面三处；去掉 `config/option-needs-key`、`config/option-needs-address` 两句。
- `miyu-endpoint` 的 `OptionState` 去掉 `NeedsKey`、`NeedsAddress`，`miyu-core` 的 `Kit::option_state` 只说冷却；`config.schema` 的选项不再带这两种 `note`。
- 手册、协议参考里给人看的叫法：中文「看图的模型」「看图模型」改成「多模态模型」，英文 `vision model`、`the model that looks at images` 改成 `multimodal model`；反引号里照抄的程序原话不改（例如表情包后台的 `未配置看图模型`，那是表情包自己的字）。`models.md`、`config-keys.md` 里 `models.vision` 的说明照新说明写。联网服务那一页去掉「需要密钥」「需要地址」的说法。
- 配置的样本（`internal/designs/samples/config/`）照新字重新生成。
- 主会话 2026-11-21 审时定的两件：用量里用途的名字 `usage/purpose/vision` 也改成「多模态」（`Multimodal`、`マルチモーダル`）；`miyu_net::missing`（「这一家少配了什么」，原来只给下拉里那两句用）连同 `Missing` 和它的测试删掉。

### 不做什么

- 接入通讯平台、网页、表情包这些仓库自己写的「看图模型」不在这个仓库里。

### 验收

- 改之前是红的：`crates/miyu-core/src/kit/tests.rs` 的 `sources_missing_a_key_or_an_address_say_nothing` 期望没配 key、没配地址的什么都不说，旧代码交回 `NeedsKey`、`NeedsAddress`；`the_search_source_option_tells_when_a_limited_source_comes_back` 开头一句同样改了期望。
- `crates/miyu-core/tests/settings.rs` 的 `generated_files_match_the_samples_byte_for_byte` 照新样本通过；`miyu-store` 的 `human_languages` 三种语言对得上。
- 本机门禁 12 项全过，三平台 CI 全绿。
