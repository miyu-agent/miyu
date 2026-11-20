## TX-2 常驻引导句

状态：已完成（2026-10-30）

### 目的

TX-1 留着的、每次请求都带的五句引导，照原来的剧本重测以后换成事实写法（设计 26 J13）。带着它们的会话每次请求再少几十个 token。

### 蓝图改哪几节

- 设计 `26-提示词.md`：第三节 J13 的「例外」写明哪些留着（一次性提示，记忆抽取、整理，调工具的程序那一段）、哪些 TX-2 换了；第十节登记簿五行换指纹、token、为什么（附重测的数）。
- 蓝图 `agents.md` 第九条第 3 条、`tools/goal.md`：照新的说明改说法；`tools/show.md`、`kernel/request.md` 引的原文换成新的。
- `prompts.md` 重新生成。

### 拍板了的（2026-10-30 项目主人）

- 做常驻的五条：`show` 的 system 一句、本机路径一句、子会话场所说明的末句、`remember` 的「只记以后用得上的」、`goal` 说明里的两句。
- 不改：记忆的抽取、整理（含人卡、写关键词），调工具的程序那一段和 `miyu call` 的说明，压缩：都多轮测过。一次性的提示照 J2 留着。

| 文件 | 改前 | 改后 | token |
|---|---|---|---|
| `software/basesystem/show/system.txt` | handing it over with show lets the user see it right away, rather than just a path | Files passed to show open in the user's interface. A path in a reply is only text to them. | 45 → 21 |
| `core/local-paths-rule.txt` | When a reply links or embeds a local file, write its absolute path. … | Relative paths in links and embeds in a reply resolve against the session working directory. | 24 → 16 |
| `core/jobs/subagent-venue.txt` | …When the task is done, give that answer and stop, without checking back or waiting. | 删掉末句 | 60 → 42 |
| `software/memory/tools/remember.json` | …Only what will matter later. | 删掉末句 | 说明 44 → 38 |
| `software/goal/tools/goal.json` | Set one only for work that takes many rounds … End it with complete once the result is verified … | While it runs, the session starts a new round by itself whenever it is idle, until it is marked complete or blocked. | 说明 61 → 37 |

### 不做什么

- 一次性提示、辅助请求、人格侧的字。

### 验收

1. `cargo xtask check` 全过。
2. 重测（基准测试仓库 `runs/2026-10-30/tx2/`，容器里 `miyu ask`，常规开发，OpenCode Go 的 `deepseek-v4.1-flash`，同一个程序只换那一份，A 是原句、B 是新写法，成对交替）：

   | 条 | 剧本 | A | B |
   |---|---|---|---|
   | `show` | D-5 补二的四个场景（画正弦曲线、下示例视频、写介绍猫的网页、做月度销售表）各 3 次 | 主动用 12/12 | 12/12 |
   | 本机路径 | 2-7 补的剧本：画 SVG 存到工作区外、回答里显示，8 次 | 每次都调 `show`，回答里嵌图的全是绝对路径 | 同左 |
   | 子会话 | 派子代理数文件行数，6 次 | 回头问、发消息、干等 0 | 0 |
   | `remember` | 三件长期的事混三件一次性的小事，6 次 | 记长期的 17 条、小事 0 | 15 条、小事 0 |
   | `goal` | 三件一步能做完的小事各 2 次 | 乱开目标 0/6 | 0/6 |

### 风险

- 每条只重测了一个剧本、次数不多：以后真实使用里看到哪一条的老毛病回来了，照登记簿里写的剧本补回原句、写明。
