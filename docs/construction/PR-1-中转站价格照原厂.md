## 施工单 PR-1：中转站的模型价格照原厂

状态：做完，等验收（分支 `step/CB-2`）。项目主人 2026-11-17 定「借原厂价、不标」。

### 目的

模型对目录的第 3、4 层（同名、规整后最长前缀）几家同名、挑不出认出的那一家和原厂的，条目照字节序取第一家，价格一概不借，主人看不到中转站模型的价。例：magpie 网关的 `marvis/deepseek-v4-1-flash-261017` 对到 byteplus 的「DeepSeek V4.1 Flash 1017」，没有价格。改成：条目照旧，价格照原厂那一家里同一个模型的借，借不到才没有；`price_multiplier` 照乘；用量里不另标估算。

### 蓝图改哪几节

`models.md`：「模型的资料」下面第 3、4 层那一条；「怎么走」第二条第 6 条第 3 款（挑法写清）、第 7 条例子表（`newapi` 那一行改、加 `marvis` 一行）、第 8 条；`model.called` 的 `cost.source`；`catalog.json` 的写法版本；「守着它的」两行改、一行加；「起草时定的」第 8 条改写并写明主人定的。

### 做法

1. 挑法（技术细节照推荐定，写进蓝图第 6 条第 3 款）：原厂照 `vendors.toml` 同第 2 款认；在原厂那一家里只挑有价格的，先找规整名是条目规整名 N 或它最长前缀的（同第 4 层，单段通用名只在正好相等时算），再找家族一样、和 N 从头数相同的段最多的（一样多的取字节序在前的）。
2. `miyu-models`：`Matched::price` 从「借不借」改成「借哪一条」（`Priced`）；新文件 `matching/borrow.rs`；`ModelSide` 带借的那一条的价格；`facts` 照它取、来源写借的那一条（`Source::priced`）；`RECALL_VERSION` 加到 2（记下的答多了一格，旧的一份整份重算）。

### 拿不准、照推荐定的

- 主人举的例子说借「DeepSeek 官方 `deepseek-v4.1-flash`」，可 2026-11-17 的目录（随包的快照和 models.dev 现在的）里 DeepSeek 官方只列 `deepseek-flash`、`deepseek-v4-flash`、`deepseek-v4-flash-vision-exp`、`deepseek-v4-pro`，没有 V4.1。照家族那一步借到 `deepseek-v4-flash`（四项和 byteplus 这一条一样：0.15 / 0.6 / 缓存读 0.003 美元）。哪天官方列了 V4.1，照前缀那一步先借到它。

### 验收

1. 改前红：`miyu-models` 的 `matching::borrow::tests::a_relay_model_borrows_the_vendor_s_same_model`（例子借到 `deepseek/deepseek-v4.1-flash`）、`without_the_version_the_family_member_sharing_most_segments_lends_it`，`matching::tests::each_row_of_the_example_table`（`newapi` 那一行），`facts::tests::name_matches_borrow_all_and_a_relay_takes_the_vendor_s_price`：照字节序挑的那一支改回不借，四条都红。
2. 实测：沙箱配一家认不出的中转 `marvis`（地址 `https://relay.example.invalid/v1`，`price_multiplier = 1.5`，模型 `marvis/deepseek-v4-1-flash-261017`，不发请求），`model.list` 看价格和出处。
3. 门禁十一项、长跑随机、Windows 目标的 clippy、`cargo xtask prompts`、`cargo xtask logs`。

### 不做什么

- 不改挑条目的先后（认出的那一家、原厂、字节序），能力、窗口照旧借挑到的那一条。
- 不在用量、界面上另标「估算」（主人定「不标」）。
