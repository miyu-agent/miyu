## 施工单 test-once（补）：剧本的一次性入口能卡住不答

状态：2026-10-20 合了（81beead9）。终端的会话要的。

### 起因

剧本的一次性入口（`Script::once`）当场答完，终端想测「整理（`/dream`）中途叫停」（施工 R-22）来不及叫停：合并那一问一回就答完了。

### 做了什么

1. `miyu_session::testkit::Once`：`Says(正文)` 整段答，`Holds` 卡住不答，直到调的一方放弃（叫停时请求被丢掉）。`&str`、`String` 都能转成 `Once`，原来 `.once(["…"])`、`.once([String])` 的写法不用改。
2. `Script::once` 的参数改成 `impl IntoIterator<Item = impl Into<Once>>`。照剧本答的那一种（`route/once.rs`）交回 `Said::Text` 或 `Said::Holds`，遇到卡住就永远等下去。卡住的那一问照样记进 `Script::asked()`。
3. 端点测试放在新文件 `memory_dream_hold.rs`，不动 `memory_dream.rs`。

### 验收

- `memory_dream_hold::a_held_one_shot_is_dropped_when_the_dream_is_stopped`：会话里 `/dream`，合并那一问卡住，`/dream stop` 以后两条回应都在 60 秒内到，回执是「已停止整理。」和「整理已中断，下次再整理。」。让卡住的一支立刻答空字符串就红。
- `memory_dream::a_scripted_core_dreams_with_the_answers_it_was_given` 和 `cargo test -p miyu-session` 照旧过：原来用 `Script::once` 的写法行为不变。
