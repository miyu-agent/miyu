## 施工单 PF-3：回合开始的挂接点分段计时

状态：已完成（2026-11-13）。PF-2 待拍板第 1 条（★ 推荐的「先补量」），主会话派。

### 为什么

PF-2 照 bm「组合加码」的日志量到：CPU 被抢时每轮 `run_turn_start_hooks` 93 / 609 / 321 ms，第 4 档 5.5 秒。`actor/model.rs` 的 `turn_start` 串着好几跳（冻结配置 → 刷新人格和找技能并着 → 压缩线 → 记忆、知识库、扩展、项目指令一起等最慢的那个），每跳至少一趟阻塞线程来回，日志里没有分段，看不出慢在哪一跳、是排队还是做活。要量清了再定 PF-2 第 2、3 条做不做。

### 目的

会话来源调到 `DEBUG` 时，每个回合开始的挂接点等完、交给内核之前记一行 `turn start timed`：每一段用了多少微秒，每一段里阻塞线程来回排队等了多少。只加日志，注入、交给内核的一个字节不变；`INFO` 时不读时钟。

### 蓝图改哪几节

- `session/actor.md`「运行日志」加 `turn start timed` 一行（键、每一格量的是哪一段），「守着它的」加 `tests/turn_start_log.rs`。
- `log.md` 第 8 条：这一行也是微秒、键带 `_us`；「守着它的」加一行。
- `session/actor.md`「在哪」加 `actor/model/hooks.rs`、`timing.rs`；`knowledge.md`、`memory.md`、`session/actor.md` 里「`actor/model.rs` 的 `hooks`」改成 `actor/model/hooks.rs`。

### 做法

- `timing/turn.rs`：`TurnStart` 记几段，`Lap::run` 套着一段量整段用时。`DEBUG` 开着时，每段套一个 task-local 的计数，`blocking.rs` 看到它就记下这一趟来回等的：交出去到阻塞线程开始做，加上做完到这一头接着跑，不算做活的时间（`_wait_us`）。不在量的照原样，不读时钟。
- `actor/model.rs` 的 `turn_start` 每一段套上；记忆、知识库两个挂接点挪到 `actor/model/hooks.rs`（`model.rs` 放不下了），里面的摘要、卡、联想、知识库各套一个。
- `turn_start` 的 future 放到堆上（`Box::pin`）：每段多套一层以后它变大，原样嵌在 actor 跑的 future 里，debug 编的 `memory_lanes` 几个测试在 2 MiB 的线程栈上溢出（实测，三遍都溢出；放到堆上以后过）。一个回合多一次分配。

### 怎么读那一行（给 bm）

```
DEBUG session  <会话> turn start timed turn=12 config_us=310 config_wait_us=120 persona_us=900 persona_wait_us=200 skills_us=700 skills_wait_us=180 line_us=3 line_wait_us=0 memory_us=1500 summary_us=600 summary_wait_us=90 card_us=400 card_wait_us=80 recall_us=800 recall_wait_us=150 knowledge_us=500 knowledge_wait_us=60 extensions_us=40 project_us=300 project_wait_us=70 total_us=3200
```

- 都是微秒。串着的是 `config_us` → `max(persona_us, skills_us)` → `line_us` → `max(memory_us, knowledge_us, extensions_us, project_us)`；`total_us` 减去它们是几段之间同步的活。
- `<段>_wait_us` 大、`<段>_us` 减它小的，是排队（被抢 CPU）；反过来是那一段自己做活慢（读盘、算向量）。
- 记忆、知识库的几格问了才有；各件含 300 毫秒的限时，过了的另有 `turn start hook timed out`。
- 开法：起核心时 `MIYU_LOG=debug`，或者系统配置 `log.level = "debug"`（运行中改也当场换，`log.md` 第 3 条），同 YC-14 的 `request timed`；每个回合开始一行，`grep 'turn start timed'`。

### 不做什么

- 不改回合开始的串法（PF-2 第 2 条，等这一行的数）。
- 不量扩展进程那一头、记忆里派到别的任务的活：只算这一段自己等的阻塞线程。

### 验收

- `tests/turn_start_log.rs`：`INFO` 时跑一轮没有这一行；调到 `DEBUG` 再跑两轮，各一行、回合编号往前走，每个回合都有的十三格和记忆的七格都在、没装知识库的没有那两格，每一格都是数，`config_wait_us` 不比 `config_us` 大。先红：`turn_start` 里不记时没有这一行。
- `cargo test -p miyu-session --no-fail-fast`；clippy 两份（本机、`--target x86_64-pc-windows-gnu`）、`cargo fmt --check`、`cargo doc --workspace --no-deps`（`-D warnings`）、`cargo xtask prompts`、`cargo xtask logs`。

### 风险

- `blocking` 每次多问一回 task-local（不在回合开始量的时候只是一次线程局部的查找），开销可以忽略。
- 量的时候每段多套一层 future、多读几次时钟，`DEBUG` 才有；量出来的含这一点，微秒级。
