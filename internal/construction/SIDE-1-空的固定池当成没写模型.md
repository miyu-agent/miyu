## 施工单 SIDE-1：明写的固定池是空的时，按没有写模型处理

状态：做完了，等合并（分支 `step/SIDE-1`，从 main 开）。设计 `43-旁路请求和档位池.md` 第三节第 3 条、第六节第 5 条。

### 为什么

项目主人本机的通讯平台从 11-18 12:27 起，群聊的判官一直被核心拒绝，原因码是 `unknown_model`，一共 3948 次，群里没有任何提示。

查下来不是核心没有退回。核心的规矩原来是这样的：旁路请求登记的档位池是空的时，使用会话的模型；但是明写的引用解析不出时一律拒绝。那个群的卡片上单独配了判断模型 `@lite`，通讯平台这时发的是明写的 `model`，不是旁路请求；lite 池是空的，所以每一次都被拒绝。

2026-11-20 用同样的池子（`[pools.lite] models = []`，`normal`、`flagship` 各一个成员）在 rc.2.3（`fcfa565b8`）和 main 上各运行了同一个探针，结果逐项相同：旁路请求带会话、不带会话都能成功，`model.call` 写 `@lite`、`session.create` 写 `@lite` 都是 `unknown_model`。

项目主人 2026-11-20 定：三个固定池永远存在、可以是空的，「把一处的模型指到固定池，后来又把它清空」是正常的操作，这时按没有写模型处理，自动退回。

### 改成什么样

明写的引用是三个固定池之一（`@lite`、`@normal`、`@flagship`），这一轮池里又没有认得出的成员时：

| 在哪里写的 | 原来 | 现在 |
|---|---|---|
| `model.call` 的 `model` | `unknown_model` | 使用 `models.chat` |
| 旁路请求退到的会话模型（`session` 那个会话这一刻的引用） | `unknown_model` | 使用 `models.chat` |
| `session.create` 的 `model` | `unknown_model`，不创建 | 按没有写处理，记下这一刻的 `models.chat` |
| `session.configure` 的 `model` | `unknown_model`，不更换 | 和写 `null` 相同，回到跟随 `models.chat` |

- 核心在运行日志里记一行 `INFO empty pool fallback pool=<池名>`，方便以后查。不告诉模型，不在界面和群里提示。
- 没有变化的：人自己建的池是空的、池不存在、引用写错，仍然是 `unknown_model`；`vision`（看图模型）写了空的固定池在这一步仍然是 `unknown_model`（施工 SIDE-2 改成同一个规矩）；会话已经记下的引用在回合开始时解析不出，仍然按原来的规矩退回 `models.chat` 并记 `session.policy_changed`。

蓝图 `models.md`（旁路请求第 2 条、`model.call` 第 2 条、出错的两张表、测试表）、`protocol.md` 的原因码表，设计 43 第三节和第六节，协议参考 `config.md`、`sessions.md` 中英文一起修改。

### 不做什么

- 不修改通讯平台：卡片上配了判断模型时它仍然发明写的 `model`，这一步之后核心会自己退回。
- 不处理看图模型：它解析不出时有自己的退路（图片按没有转成文字处理）。

### 验收

- 改之前是红的测试：
  - `crates/miyu-endpoint/tests/model_call_side.rs` 的 `an_explicitly_empty_fixed_pool_counts_as_no_model_written`：真的路由、扩展的连接，池子按上面的配置。
  - `crates/miyu-session/tests/once_side.rs` 的 `a_fallback_that_is_an_empty_fixed_pool_goes_to_the_chat_model`。
- `crates/miyu-models/src/reference/tests.rs` 加一条：只有没有成员的固定池算。
- 门禁、Windows 版 clippy、三个平台的 CI 全部通过。

### 做下来的

- `crates/miyu-models/src/reference.rs`：新加 `empty_fixed_pool`，判断一个明写的引用是不是空的固定池。
- `crates/miyu-session/src/route/once.rs`：一次性入口拿到的模型引用是空的固定池时，按没有写处理。`model.call` 的 `model` 和旁路请求退到的会话模型都经过这里。
- `crates/miyu-endpoint/src/models.rs`：新加 `record_or_none`，`session.create`、`session.configure` 改用它；看图模型仍然用原来的 `record`。
- 两条测试在修改之前是红的：建会话写 `@lite` 得到 `unknown_model`；旁路请求的后备是空的 `@normal` 时得到 `pool "normal" has no models`。修改之后通过。

### 怎么实测验收

2026-11-20 在沙箱里用编出来的核心实测（`MIYU_HOME` 和 `HOME` 都指向临时目录；模型是中转站上的 `deepseek-v4.1-flash`，上限 5 次请求，实际 2 次）。配置里 `models.chat` 是那个模型，`[pools.lite] models = []`。

1. `model.call`，`model` 写 `@lite`：回答 `ok`，回应里的模型是 `models.chat` 那一个。修改之前是 `unknown_model`。
2. `model.call`，`model` 写 `@nope`（没有的池）：仍然是 `unknown_model`，「模型或模型池不存在。」
3. `miyu ask --model @lite`：会话创建成功并回答 `ok`。会话日志的 `session.created` 里记的模型是 `models.chat` 那一个，说明按没有写处理。
4. `session.configure`，`model` 写 `@lite`：回应 `{}`。
5. 运行日志里有三行 `INFO empty pool fallback pool=lite`：一行来自一次性入口（第 1 步），两行来自协议端点（第 3、4 步）。

自己验收时可以把某个群卡片上的判断模型配成 `@lite`、让 lite 池保持空的，判官应该照常工作，核心的运行日志里出现 `empty pool fallback pool=lite`。
