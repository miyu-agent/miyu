## UC-3：替扩展换算金额

状态：做完了。项目主人 2026-11-10 定：接入通讯平台（im）的使用限制页、群里按人查用量要带金额。每次请求的原币种金额 `model.called` 的 `cost` 里本来就有（施工 8-15），im 自己照人、照日子记得下来，缺的只是换算：汇率只有核心取、存（施工 UC-1）。汇率照天取，不写进事件。协议的形状和 im 对过。

### 一、做了什么

1. 协议多一个方法 `usage.convert`（图纸 `models.md`「协议」`usage.convert`、「怎么走」第九条第 8 条，`protocol.md` 方法表）：
   - 参数 `{"groups": [[{"day","currency","amount"}, …], …]}`：一组是要合成一个数的那几笔，`day` 是 UTC 日子（`2026-11-10`），`currency` 是三个英文字母的币种代码，`amount` 是数。组可以是空的。最多 1000 组、合计最多 20000 笔。
   - 回应 `{"currency", "converted"}`：`currency` 是这一刻的 `usage.currency`；`converted` 照组的先后一组一项，和 `usage.query` 一行里的 `converted` 是同一个函数写的，一笔都没换成的（连同空的组）是 `null`。
   - 几组的汇率只找一次（`UsageIndex::convert_all`），缺的当场取、最多等两秒，同 `usage.query`。
   - 写错的、超上限的回 `bad_params`。
2. 谁能调同 `usage.query`：人、扩展都能调；不在后台答。
3. 代码在 `crates/miyu-endpoint/src/usage/convert.rs`；存储层没动。
4. 协议参考 `docs/reference/config.md`、`config.en.md` 多 `usage.convert` 一节，方法快照 `methods-v1.txt` 登记。

### 二、不做什么

- 不替扩展记账：谁用了多少由扩展自己记。
- 不给 `converted_parts`：扩展没有分四种的账。
- 汇率不写进事件，不给扩展直接读汇率的方法。

### 三、怎么验

1. 端点 `usage_convert_method.rs`（先写的，没有这个方法时四条都红在 `unknown_method`）：
   - 两组（一组两天两种币、一组同币种）换出来的数对，空的组是 `null`，再问不再取汇率。
   - 取不到汇率的那一笔在 `rest`，一笔都没换成的组是 `null`。
   - 回应的 `currency` 跟着配置，用不着汇率的不去取。
   - 日子写坏、金额不是数、币种不是三个字母、超上限的回 `bad_params`；正好在上限上的照办。
2. `robust_methods.rs` 的方法表加上它。
3. 本机门禁。
