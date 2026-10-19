## 施工单 R-18：删掉 bge

状态：做好了（2026-10-19 写、施工、自己验收）。项目主人 2026-10-19：「bge可以直接删除了，不要做成软件包了」。出厂的内置语义模型 R-5 七补已经换成 granite，bge 只剩门槛表里的三行、文档里「bge-m3 以后当可选的包」的说法和核心仓库里的一份 Release。剩下的是技术细节，照施工方案第一节第 1 条直接做。

### 定了的

1. **门槛表**：`resources/core/memory/recall.toml`、`core/knowledge/recall.toml`、`core/knowledge/search.toml` 拿掉 `local:bge-small-zh-v1.5` 那一行；注释只留 granite 的量法，提一句原来 bge 的数是多少、什么时候拿掉的。装着旧 bge 包的人照「表里没有的模型」办：联想不带、自动想起只走标题名、搜的下限照 0.40。
2. **测试**：查出厂表的几条改成查 bge 不在了。
3. **文档**：蓝图里说现状的几处（向量表模型编号的例子、包「只放 bge」、知识库门槛、守着它的表）改成 granite；量过的数（测评集上 bge 那几行）是历史，照留；`memory/eval.md`「bge-m3 以后当可选的包」改成不做。
4. **Release**：核心仓库的 `models-bge-small-zh-v1.5`（预发布）删掉。删以前查过没有代码、脚本、CI 再取它：`miyu-embed` 仓库切词的标准答案是提交在仓库里的文件，不下载。
5. **WordPiece 切词留着**：核心测试用的手算小模型（`crates/miyu-session/tests/fixtures/embed-tiny/`）照它切，清单的 `wordpiece` 这一种也留着。`miyu-embed` 仓库里那一份（和 bge 的词表测试数据）归 miyu-core 那边定，已经告诉他们。

### 不做什么

- 不动供应商的 bge-m3 那些远程模型名字（`resources/models/embedding.toml` 的 `starts = ["bge"]`、配置的例子）：那是别家的 API 认得出能出向量的，不是我们的包。
- 不改历史施工单。

### 验收

`cargo xtask check` 八项；三台 CI。
