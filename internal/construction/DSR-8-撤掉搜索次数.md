## 施工单 DSR-8：撤掉用量里的搜索、抓取次数

状态：做完了，等合并（分支 `step/DSR-8`，从 main 开）。设计 `72-DeepSeek内置联网工具.md` 第二节第 4 条。

### 为什么

施工 DSR-4 把 DeepSeek 报的搜索、抓取次数记进了每次请求的用量，施工 DSR-7 又把它记进用量汇总，准备给界面显示「搜索 N 次」。项目主人 2026-11-20 定：界面不显示这个数，用不上的代码撤掉。界面那边没有合并过任何用到它的改动。

### 改成什么样

- **内核**：`Usage` 去掉 `searches`、`fetches` 两格，`plus` 不再加它们。`crates/miyu-kernel/src/event/model.rs` 恢复成施工 DSR-4 之前的内容。
- **驱动**：`openai-responses` 不再读 `usage.server_tool_use`。
- **用量汇总**：表 `spent` 去掉两列，写入和查询恢复成施工 DSR-7 之前的样子。表的版本从 4 升到 5，旧库按派生库的规矩删掉重建（设计 07 第六节），不留兼容。
- **协议参考**：`usage.query`、`model.call`、扩展驱动的 `usage` 说明里去掉这两格，中英文四页恢复原样。
- **蓝图、设计**：`drivers/openai-responses.md` 的用量表、`kernel/events-bodies.md`、`models.md` 跟着恢复；设计 72 第二节第 4 条改成「不记」，「决定」一节加一条。
- 施工 DSR-4、DSR-7 的施工单保留，开头注明被这一步撤掉的部分。

### 不变的

- 供应商侧工具块（`deepseek_web_search`、`deepseek_read_url`）的解码、日志、界面显示、下一轮写成文字带回，都不变。
- 设计 71 第三节第 8 条「供应商报的次数不另记，回复里这种块的个数就是」重新成立。

### 已经写出去的日志

施工 DSR-4 到这一步之间，带内置搜索的请求在会话日志的 `model.called` 用量里写过这两格。用量的读法不拒绝不认识的格，所以这些日志仍然读得进，多出来的两格被忽略。`crates/miyu-kernel/src/event/model/tests.rs` 里加了一句断言守着这一点。

### 验收

- 这一步是撤掉功能，没有改前红的测试。去掉的测试：`crates/miyu-store/tests/usage_searches.rs`（3 条）、内核里次数的读写和相加（1 条）、驱动测试里关于次数的两处断言。
- 代码里除了那一句兼容断言以外，不再出现用量的 `searches`、`fetches`。
- 门禁、Windows 版 clippy、三个平台的 CI 全部通过。

### 做下来的

- 内核的 `event/model.rs`、驱动的 `openai_responses/usage.rs`、蓝图 `kernel/events-bodies.md` 和协议参考里扩展驱动的 `usage` 说明，恢复成施工 DSR-4 之前的内容，逐字相同。
- 用量汇总的三个文件恢复成施工 DSR-7 之前的内容，只有版本号从 3 改成 5。
- 代码里 24 处用量的字面量各去掉两行。
- 设计 72 第二节第 4 条改成「不记」，「决定」一节加一条。

### 怎么实测验收

2026-11-20 在沙箱里用编出来的核心实测（`MIYU_HOME` 和 `HOME` 都指向临时目录；模型是中转站上的 `deepseek-v4.1-flash`，上限 2 次请求，实际 2 次）：

1. 启动核心，提问一次。`usage.query`（参数 `{"group":["session"]}`）回出一行，`usage` 里只有四项 token。库的版本是 5，没有 `searches` 这一列。
2. 停掉核心，把现场改成上一版留下的样子：用 `sqlite3` 给 `spent` 加上那两列并填上次数，版本写成 4；把会话日志里那一条 `model.called` 的用量加上 `"searches":3,"fetches":1`。
3. 再启动核心并查询：回出的那一行和第 1 步相同，没有那两格。
4. 接着这个会话再提问一次：回答正常，说明带着那两格的日志仍然读得进。查询里这个会话变成 2 次请求。
5. 停掉核心查看：库的版本是 5，没有 `searches` 这一列，有 2 行；运行日志里有一行 `usage index rebuilt`。

自己验收时可以调用 `usage.query`，`usage` 里不应该出现 `searches`、`fetches`。
