## 施工单 DOC-6：照手册把命令真敲一遍，改掉对不上的

状态：待验收（2026-11-16）。项目主人 2026-11-16 立的规矩：改动除了过代码测试，交之前要当用户真用一遍。DOC-1 到 DOC-5 的手册只对过代码，这一步补上实测。

### 实测做了什么

在另一台机器上编当前 main 的 `miyu`，照 `dev/contributing` 的「编和跑起来」造一个沙箱数据根（家目录、缓存、运行目录都指到临时目录，不碰谁的 `~/.miyu`），把手册代码块里的命令照原样敲：

- 不要模型的约 110 条：`miyu -h`、`check`、`config get/set/explain`（连写错的）、`provider login`、`kb new/mount/add/search/read/edit`、`memory add/list/search/edit/forget`、`schedule add/pause/resume/rm`、脚本工具、`pkg list/info/remove`、`service status`、`rescue`、`core stop`。
- 要模型的约 35 条（项目主人给的测试端点，串行、每句只要一句话的回答，没有遇到 429）：`ask`、`ask -c`、从管道和 `--file` 带内容、`rename`、`recap`、`undo`、`restore`、`redo`、`compact`、`session export/import`、`--permission read-only`、让它跑 `git push --force`、`--format jsonl`、默认预设和默认人格写错时、英文界面下同一批报错。

### 看到了什么

和手册一致的（原来标着没实测的几样）：

- PowerShell 用的那种列表写法 `"['a', 'b']"`（外面双引号、里面 TOML 单引号）程序照收，存下来的和 bash 写法的一样。
- 编号 `con` 被拒，提示和手册引的一字不差；`sandbox.private.add '["~"]'`、`log.level verbose`、`--every 30s` 的报错原文同手册。
- 只读下让 AI 建文件：没建，AI 照实说是只读。让 AI 跑 `git push --force`：没跑，末尾印「警告：1 步需要确认，已跳过」，退出码 `4`。
- `miyu undo` 以后那一轮建的文件还在，同手册「撤销只退对话」。
- 密钥文件 `etc/private/secrets.toml` 是 `-rw-------`，目录是 `drwx------`。

对不上的，这一步改了（中英两份）：

1. `guide/personas-presets`：写的是默认预设指着没有的预设时「`miyu check` 说 …」。实测 `miyu check` 说「没有问题」，`miyu ask` 报「预设不存在」。那句警告只对软件包的设置项有。
2. `guide/cli`、`guide/memory` 英文页：命令行印核心的拒绝语时去掉句末的句号、英文句首改小写（`miyu: nothing to undo`），英文页引成了带句号、大写开头的；中文页「记忆不可用。」多了句号。
3. `guide/cli`：`session import` 导入这个数据根里早就有的会话什么都不印，手册没说。
4. `guide/cli`：`compact` 没东西可压时报「没有可压缩的内容」，退出码 `1`，手册没说。
5. `guide/config`：带引号的键在 PowerShell 里原来写「改用手改」。实测键里的引号换成单引号程序照收，补上这个写法。

另跟一处刚合的修正（没实测，照代码 `crates/miyu-models/src/provider_key.rs`）：`guide/web-search` 里「模型那边的 DeepSeek 供应商」怎么认，补上「地址还得是官方的才借它的 key」，写了 `catalog = "deepseek"`、地址指到中转站的不借。扩展拿 key 照同一条认法，软件包页原来就指到这一段。

### 没实测的

- Windows、macOS 上的一切（没有这两个系统的机器在手边）；PowerShell 的写法只验了程序收不收那个值，没在 PowerShell 里敲。
- 终端、网页、托盘、通讯平台的界面（开发用的数据根里没装这几个包）；`miyu service enable`、`miyu upgrade`、`miyu pkg install <软件源里的包>`、`miyu shellhook`、`miyu web`、`miyu im`。
- 斜杠命令、推演人生、长期目标、循环。

### 蓝图改哪几节

不改。只改文档。

### 验收

1. `cargo xtask check` 全过（见 CI）。
2. 改过的每页中英行数、标题层级、表格行列、代码块个数相等；英文页的链接和锚点都指得到。
3. 改的每一处有实测的输出对着（输出留在写文档的会话的缓存目录里）。

### 风险

- 实测只在 Linux 上、只走了命令行。
