## 施工单 WD-2：「不支持该系统」统一改成「不支持当前系统」

状态：做完，等验收（分支 `step/WD-2`）。

### 为什么

软件包不支持本机系统时，核心、网页界面和终端界面都会显示「不支持该系统」。`AGENTS.md`「文档的规矩」不使用「该」这种官腔，而这句话在三个仓库里是同一句，必须一起修改。施工 WD-1 因此没有修改它。界面会话确定了新的说法：中文「不支持当前系统」，英文 `Not supported on this system`，日文「このシステムには対応していません」。

### 改成什么样

- 命令行 `miyu pkg search` 一行后面的标记：`（不支持该系统）` 改成 `（不支持当前系统）`。英文 ` (not supported on this system)` 没有变化。
- 安装被拒绝时的原话（`wrong_target`）：`不支持该系统。` 改成 `不支持当前系统。`。英文 `Not supported on this system.` 没有变化。
- 中文字表里 `packages.other_systems` 的说明引用了这个标签，一起修改。日文字表里的引用从「このシステムには非対応」改成「このシステムには対応していません」，和界面的日文一致。
- 蓝图 `packages.md`、`cli/pkg.md`、`config.md` 里的引用，配置的两份中文样本，手册 `docs/zh/guide/` 里引用这句报错的六页，协议参考 `errors.md`、`packages.md` 和作者文档 `manifest.md` 一起更新。

### 不做什么

- 不修改施工单和设计文档里引用的旧文字，因为它们记录的是当时的样子。

### 验收

- 修改之前测试是红的。
- 门禁和三个平台的 CI 全部通过。

### 做下来的

- 改前红：新增 `miyu-cli` 的 `pkg::source::tests::a_package_for_another_system_is_marked_in_the_language`，并把 `miyu-endpoint` 的 `packages_systems` 里断言的原话换成新的一句。两条测试在修改之前的代码上都失败，输出的是「不支持该系统」。
- 没有实测：这一步只替换了一句文字，输出这句文字的两条路径（搜索的一行、安装被拒绝）都有测试直接断言。

### 怎么实测验收

1. 运行 `miyu core stop`，确认之后的命令由新版本的核心回答。
2. 在一台 Linux aarch64 的机器上运行 `miyu pkg install im`（`im` 没有这种系统的包），应该输出 `miyu: 不支持当前系统`。
3. 可选：运行 `miyu config set packages.other_systems true` 之后运行 `miyu pkg search`，不支持本机系统的软件包后面应该带 `（不支持当前系统）`。
