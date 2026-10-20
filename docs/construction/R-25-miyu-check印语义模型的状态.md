## 施工单 R-25：miyu check 印语义模型的状态

状态：待合（2026-10-20 写、施工、自己验收）。项目主人 2026-10-20 问有没有 `miyu embed` 这类命令；现在用的是哪个语义模型、记忆和每个库补到了哪、后台还在不在补，只能翻运行日志。我推荐不另开命令，在 `miyu check` 里多印一段，项目主人同一天「按照你推荐的做」。

### 定了的

1. 协议 `check` 不写 `file`、核心接了照意思那一路（`Core::with_vectors`）的，回应另带 `embedding`（`protocol.md` 的 `check` 第 5 条），不进 `problems`、不算问题：
   - 用得上：`{"model", "filling", "memory": [{"persona", "have", "total"}], "turns": [...], "kb": [{"id", "name", "have", "total"}]}`。
   - 用不上：`{"model": null, "why": "off" | "not_installed"}`。
2. 只数不补：人格那一间照 `modules/memory/<人格>/` 目录（`MemoryLogs::personas`）、回合库的文件（`RecallIndexes::personas`，从 `forget_session` 拆出来）在的数，开的是核心一直开着的那一份（同 `forget_session`），没有的不建；知识库照 `index_of` 现有的数，不照文件补。计数走读的连接（`RecallIndex::coverage`，V-14 以后的 `reading()`）：向量跟着条目（拿掉条目时一起删，条目不在的不放），照模型数就是有向量的条数。
3. `Vectors` 多 `state`（用哪个模型、用不上的为什么）、`filling`（在补的名字集合不空）。
4. 命令行在合计后面印一段（`cli/check.md`「怎么走」第 4 条），退出码不变；`--format json` 带上；只查一份的不印。命令行只有中文、英文两种语言（`Language`），日文照英文。
5. 核心的主会话 2026-10-20 看了形状，没意见；提醒计数要走读的连接（V-14）。

### 不做什么

- 会话自己那一间（记忆范围 `session`）的记忆、回合库：少，散在会话目录里，不数。
- 补：不在这里起补；人搜、记的时候照旧补。

### 验收

1. 测试：端点 `check_embedding.rs` 三条（真小模型记一条、放一个库，补齐了的样子；关着、没装的为什么；没接照意思那一路的不带、只查一份的不带）；`miyu-store` 的 `coverage_counts_per_model`、`memory_personas.rs`；`miyu-session` 的 `memory_filling.rs`（起补的那一刻就算在补，补完不算）；命令行 `language/embedding.rs` 两条；真程序 `crates/miyu/tests` 的 config、login 两条跟着改（合计后面多一行，json 带上）。
2. 手写变异十个（关着当成没装、不看关着、一直不算在补、别的模型也算、拿掉的也算、只查一份也带、文字版不印、json 不带、补着不写、空的一类也写）：「拿掉的也算」漏了，查下来和原来等价（向量跟着条目），把多余的那一道去掉；其余九个全被逮住。`cargo xtask check` 八项；三台 CI。
3. 真程序：release 在沙箱数据根里装上内置语义模型，记几条记忆、放一个库，看样子；关掉 `models.embedding` 再看一次。验收手册 K-111。
