## 施工单 DOC-22：手册里引的界面文字、程序输出对各仓库现在的字表核一遍

状态：待验收（2026-11-19）。DOC-21 里发现终端界面页一句英文报错抄的是旧字（程序早改了）。手册里用反引号引的界面文字、程序输出有一千三百多处，程序那边改了字、手册没跟上的不会只有这一处，所以全量核一遍。

### 怎么核的

三个脚本（文档线的工具，不进仓库）：

1. 把各组件仓库 `origin/main` 上的字表、包清单、源码导出成底库（只读，`git archive`）：通讯平台、网页界面、终端界面、系统托盘、精简版、迁移包、表情包、渲染、文字识别、语义模型、语音、官网、开发包、包模板，加上核心自己的 `resources/`、`crates/`、`packaging/`。
2. 手册每一页里写在反引号里、像界面文字或程序输出的句子（中文 621 处、英文 771 处），把占位、数字、路径去掉以后到底库里找原文；源码注释里的话不算。
3. 找不到的（中文 63 处、英文 109 处）各列出底库里最像的两句，逐条人工看：大部分是带了示例值的正常引用（`这份 Miyu 由 AUR 安装…`）、程序自己加的前缀（`提示：`、`沙盒不可用：`）、命令示例；确实对不上的改掉。

### 改了什么

| 手册原来写的 | 程序现在的字 | 出处 |
|---|---|---|
| `不支持本机的系统。`（7 处）、`Not available for this system.` | `不支持该系统。`、`Not supported on this system.` | 核心 `refusal/message.rs` 的 `wrong_target` |
| 终端界面首次引导的 `AI 用什么语言回复`、`AI 怎么称呼你` | `回复语言`、`称呼`（英文 `Reply language`、`Preferred name`、`Additional details (optional)`） | 终端界面字表 |
| `首页显示形象`、`侧边栏显示形象` | `首页形象`、`侧边栏形象`（英文 `Home screen figure`、`Sidebar figure`） | 终端界面包清单 |
| 网页界面的 `允许局域网访问`、`AI 输出样式` | `局域网访问`、`输出样式`（英文 `Local network access`、`Output style`） | 网页界面设置包的清单 |
| 英文确认框的 `Allow this time`、留言开关 `…outside this project` | `Allow once`、`…outside the project` | 网页界面字表 |
| 英文 `Scheduled task done`、`stopped when the core restarted` | `Scheduled task finished`、`stopped by core restart` | 终端界面字表 |
| 应急模式输入框上的 `应急 · 不落盘`、`rescue · nothing saved` | `应急模式`、`rescue mode` | 精简版字表 |
| 安全页的 `谁是你本人` | 后台页的 `设置 → 绑定 → 本人的平台 ID` | 原来指的是通讯平台页的小节名，不是界面上的字 |
| 子代理报告里的 `你在它做事时跟它说过话` | 英文原话 `The user also talked to this subagent during the task.`，后面括号里给中文意思 | `resources/core/jobs/subagent-person.txt` |
| 默认预设不存在的告警末尾多一个句号 | 去掉 | 核心字表 `config/unknown-preset` |

另外两处不再当原话引：预设没打开循环时 `/loop` 的回答（`没有这条命令`，字表里没有这一句，改成「命令列表里不会列出它」）、网页界面的 `打开文件夹`（字表里没有叫这个名字的按钮，改成「打开文件夹的按钮」）。

应急修复页跟上精简版现在的命令行：`litemiyu provider import`、`litemiyu doctor` 已经没有了（命令行只剩全屏、`ask`、`config`、`help`）。「以后不想手动输入」那一句改成在 `litemiyu config` 的配置界面里选 `从 Miyu 导入…`；「LiteMiyu 的日常用法」里的命令示例和后两条说明跟着改。

### 核实

- 每一条都在对应仓库 `origin/main` 的字表、清单或源码里看到了现在的写法再改，中英一起。
- 改完重跑扫描：剩下找不到的都逐条看过，是示例值、前缀、命令示例这三类。
- 文档线的检查脚本问题 0 个。
- 没有运行程序：这一步是拿手册对字表，界面上真看一眼要等界面实测。

### 留下的

- 这种核对要在每次发版前重跑一遍（底库导出加扫描一共两三分钟）。
- 扫描只认反引号里的句子：没加反引号的界面文字、表格里当数据的界面名不在内。
- 工具页说提问时网页界面显示 `… 提问`，这一处仍然没在字表里找到对应的句子，等界面实测。

### 验收

1. 全仓库文档里搜不到 `不支持本机的系统`、`provider import`、`litemiyu doctor`。
2. 文档线的检查脚本问题 0 个。
