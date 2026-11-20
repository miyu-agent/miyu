## 施工单 test-once：剧本的一次性入口

状态：2026-10-20 合了（f94ac37a）。同一天项目主人同意（终端的会话提的）。

### 起因

测试核心的请求模型是剧本（`miyu_session::testkit::Script`），它没有一次性入口（`Models::one_shot` 交 `None`），核心不交抽取、整理要的几样：`/dream`、`memory.dream` 一律回 `memory_unavailable`。终端的端到端测试因此只测得到被拒那一条路，整理时画什么只能靠单元测试和真核心自测。

### 做了什么

1. `Script::once(答案)`：排了的剧本有一次性入口，照先后答正文；答完了的再问交 `no_model`。没排的照旧没有（和真核心没配模型一样），原来的测试都不变。
2. `Script::asked()`：交给一次性入口的问，照先后。
3. `OneShot` 里照剧本答的那一种只在 `testkit` 开关里有（`OneShot::scripted`）：不经路由、不记账，正文一整段交给边收边看的。

### 验收

- `memory_dream::a_scripted_core_dreams_with_the_answers_it_was_given`：剧本排了整理的三段答，`memory.dream` 交回和真路由一样的数，问的记下了；没排的剧本回 `memory_unavailable`。让剧本永远没有一次性入口就红。
- `model_call::no_model_says_why_and_a_scripted_core_has_no_one_shot` 照旧过：没排的剧本没有一次性入口。
- 本地门禁八项。
