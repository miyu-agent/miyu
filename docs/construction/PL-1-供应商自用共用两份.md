## 施工单 PL-1：供应商自用、共用两份都能用

状态：主会话做。项目主人 2026-10-24 定：同一个编号的供应商系统配置、个人设置都配了时两份都要能用（不同意改编号：模型池、冷却、用量都按编号记）；形状「`<编号>` 自用、`<编号>@system` 共用原样」项目主人确认。

### 为什么

配置照层叠，同一个编号个人设置逐格盖过系统配置：配了自己的 key，共用的那份就用不上了。

### 做了什么

- `miyu-config` 的 `merge`：系统配置、个人设置都写了的供应商编号，另照只有系统配置的那几层合一次，键里的编号换成 `<编号>@system`（`merge/system_copy.rs`）；它引用的 `{ secret }` 换成新的 `Reference::SystemSecret`。只在最终值里有，文件里写不出。
- `key::system_copy`、`of_system_copy`、`valid_provider`：模型引用（`miyu-models` 的 `reference`、配置里的 `kind`）认 `<编号>@system/<模型>`。
- 端点 `Config::secret`：`SystemSecret` 只从系统那一份密钥文件取。
- `model.list` 每家多 `layer`（`personal` 自用、`system` 共用）。`config.get` 不列派生的那份（它对不上清单里的项）。
- 冷却、粘住、模型清单的缓存、用量本来就照编号记，两份自然分开。
- 图纸 `models.md`「两种写法」，协议参考 `config` 中英。

### 不做的

- 界面上的「自用」「共用」两组：网页、终端照 `layer` 画（10-24 已经说过形状）。
- 成员：只继承系统层，他自己也配了同编号的，同样出两份（同一套规矩）。

### 验收

1. `miyu-config` 单元测试两条：两层都写的出自用、共用两份，共用的只照系统配置、密钥换成只从系统那份取；只有一层写的不另出。
2. `tests/provider_copies.rs`：系统配置、个人设置都配了 `relay`，各存一个 key（个人那份里另存一个同名的 `relay`）：`relay/m` 发出去带自用的 key，`relay@system/m` 带共用的 key；`model.list` 两份、`layer` 对；`config.get` 不列 `@system`。
3. 变异 6 个全抓到：不另合、密钥不换成只从系统取、取密钥先找自己的、引用不认 `@system`、`layer` 一律写 system、两层只要有一层写就另合。
4. `cargo xtask check` 全过。

### 修（2026-10-26，网页实测报的两处）

1. **共用那份对不上目录**：`deepseek@system` 拿带后缀的编号去对档案、目录，没有地址、`needs base_url`，名字直接是「deepseek@system」。改成照原编号对（`miyu-models` 的 `provider`），配置照它自己的那份读；没有名字可用的照原编号叫（`models/entry.rs` 的 `name`）。
2. **删了自用那份，共用的引用失效**：人选过 `relay@system/m`，后来删掉个人设置里的 `relay`，就只剩一份 `relay`，`relay@system` 没了，报 `bad_reference`。改成只有系统配置写了的那一家，`<编号>@system` 记成它本身的别名：合的时候（`merge/system_copy.rs` 的 `unfold`）最终值里的引用、池成员换成 `<编号>/<模型>`，查文件里的引用在不在照别名认（端点的 `missing_in`），临时传来的照 `Values::alias` 认（`provider`、`reference::record`）。
3. 测试：`provider/tests.rs` 的 `a_shared_copy_is_matched_by_its_original_id`（档案、目录各一家）；`tests/provider_copies.rs` 的 `a_shared_reference_still_works_after_my_copy_is_gone`（不报错、用共用的 key、`uses` 是 `relay/m`），原来那条加断言两份都照原编号叫。两条修之前各红在报告的那一处。
4. 变异 10 个全抓到：对档案、对目录不换原编号（两处）、`provider` 不认别名、合的时候不换引用、端点查引用不认别名、`record` 不记本身、名字照带后缀的编号、两层都写了的也记别名、只换列表不换单个引用、`values()` 不带别名。
