## 施工单 JT-1：后台任务回报里标题在前

### 为什么

后台命令、子代理结束时，她收到的回报标签是 `<command-ended job="j1" title="…" …>`，跟人说的时候常常照抄编号：「j1 已完成」，人不知道 j1 是哪个任务。项目主人 2026-11-26 定：不加给模型的字（担心提示词影响发挥），只把标题挪到编号前面，让她更容易照标题说。

### 改了什么

- `core/jobs/` 的四个标签：`command-open.txt`、`lines-open.txt`、`subagent-open.txt`、`subagent-message-open.txt`，`title` 挪到 `job` 前面。字一个没加，token 数不变。
- 登记簿（设计 26）四行的指纹和说明；`cargo xtask prompts` 重新生成蓝图 `prompts.md`。
- 测试和设计样本里写死的标签顺序跟着改；probe 存档用 `MIYU_PROBE_WRITE=1` 重写（请求的字节变了，存档里记的请求哈希跟着变）。蓝图 `kernel/request.md` 一处写法。

### 怎么验

- 请求的变化只是这四个标签里两格的顺序（重写的存档 diff 逐行核过）。
- miyu-assemble 整套测试；门禁。
- 这是一次计划内的缓存冷启动：已经化石化进历史的回报照原文回放，不变；只有改动以后新到的回报是新写法。
