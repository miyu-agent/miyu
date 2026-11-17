## 施工单 DOC-10：手册跟上软件包的「启用」开关和设置项改名

状态：待验收（2026-11-17）。跟核心的 UX-1、UX-1补（软件包只留一个「启用」开关，一批设置项改成软件术语），和同一天合的 RO-3、RO-3补（开放权限不拦数据根，删整个数据根改成先问）、VS-1（包的命令认 `--version`）、NS-1（网络搜索）。

### 改了什么

中文页、英文页同一处一起改。

- `guide/packages`：「开和关」写成每个包都有一个「启用」开关，关掉以后它带来的工具、技能、脚本、命令、预设、知识库、内容、外观都不在，再开就回来；表里加命令行一行；「只对自己关掉一个包」一节换成「用命令行开关」（`packages.<编号>.enabled`）；`miyu pkg list` 关着的接「（已关闭）」；「不能启用或停用」那一行报错去掉（这个拒绝原因没有了）。
- `guide/config-keys`：`packages.<编号>.off` 一行换成 `packages.<编号>.enabled`；「关于你」一节改叫「个人资料」。
- `guide/tools`、`guide/web-search`：关掉整个包的示例换成 `…enabled false`。
- 改了名的设置项，手册里引到的几处：`guide/service`「空闲退出时间」，`guide/sessions`「预压缩」，`guide/heads/tui`、`guide/heads/web` 设置页那一行的「个人资料」和里面的项，`guide/models` 网页池卡片上的「子代理可用」。

- `guide/permissions`、`guide/security`、`guide/troubleshooting`、`guide/knowledge`（RO-3）：开放权限下文件工具写数据根同别的目录；只读照旧哪里都不写；删整个数据根、数据根里的东西、装着它的目录归「递归删除」先问你，不再直接拒；「这是 Miyu 自己的数据，不能改」那一句和它的排错行去掉；知识库的文档开放权限下文件工具也改得了。
- `guide/cli`（VS-1）：只敲 `miyu <包的命令> --version`、`-V` 的印「miyu <命令> <包的版本>」；包的程序拿得到 `MIYU_HOME`。
- `guide/web-search`、`guide/config-keys` 和提到包名的几页（NS-1）：出厂包「联网搜索」改叫「网络搜索」；联网服务多 `exa`、`anysearch` 两个选项，多一项 `net.search.anysearch.key`。

没改的：终端首次引导里的「AI 怎么称呼你」那几句、终端模型池窗口里的「子代理能选」，是终端自己的字，没变；网页首次引导的同理。UX-1 改名的四十来项里别的，手册没有引它们在设置页上的名字（配置键表只列键、类型和说明）。

### 实测

沙箱数据根，当前 main 编的 `miyu`，Linux，中英界面各一遍：

- `miyu config set packages.base-net.off true`：「没有 packages.base-net.off 这一项」，退出码 1；配置文件里手写的，`miyu check` 警告「没有 packages.base-jobs.off 这一项。已忽略此行。」。
- `miyu config set packages.base-net.enabled false`：「已设置 packages.base-net.enabled = false（立即生效）」；`miyu config get` 回 `false`；`miyu pkg list` 里这一行接「（已卸载）」（出厂的内置包关掉就是卸掉）；`etc/config.toml` 没有多出东西。写 `true`：装回来，标记没了。
- `miyu config explain` 看几项的名字：空闲退出时间、预压缩、称呼、使用权限、整理间隔（轮），英文 Idle exit time、Pre-compaction、Preferred name、Access、Organizing interval (turns)。

- `miyu pkg list`、`miyu pkg info base-net`：名字是「网络搜索」；`miyu config set net.search.source exa`、`anysearch` 都收；`miyu config explain` 看得到 AnySearch 那一项和 DeepSeek 那一项的说明。
- 照 `dev/head` 里那个小包的清单装一个带命令的包：`miyu tiny --version`、`-V` 印「miyu tiny 0.1.0」；`miyu tiny hello` 交给包的程序，它的环境里有 `MIYU_HOME`（没设这个变量、照默认位置的也有）和 `MIYU_PROGRAM`；`miyu tiny config --version` 照旧交过去。`miyu ask --version` 印「miyu 2026.11.0」。

没实测：

- RO-3 那几条（开放权限下文件工具写数据根、删整个数据根先问）：要模型才测得了，测试端点上那个模型当时被关着（网关回 HTTP 404），四次请求都没发成，这一步照 RO-3、RO-3补 的施工单和代码写。DOC-11 换了模型补测过，见 `DOC-11-数据根实测.md`。
- 人停的核心头不再自己拉起（CS-5）、DeepSeek 的 key 在设置页显示「已继承」、额度写明供应商、群聊会话显示发言人、戳一戳：要头那一边画出来才看得到，这一步没写，等头合了再跟。
- 不是出厂的包关掉以后 `miyu pkg list` 接「（已关闭）」、它带的技能和脚本不在：开发用的数据根里没装这样的包，照 `crates/miyu-cli/src/language/pkg.rs` 和 UX-1 的施工单写。
- 设置页上这几项的名字：要在终端、网页里看；网页池卡片上的「子代理可用」照网页的代码是取核心的名字来画，没开窗看过。
- Windows、macOS。

### 蓝图改哪几节

不改。只改文档。

### 验收

1. `cargo xtask check` 全过（见 CI）。
2. 手册里搜不到 `packages.<编号>.off` 和「把这个包关掉」。
3. 改过的每页中英行数、标题层级、表格行列、代码块个数相等；英文页的链接和锚点都指得到。
