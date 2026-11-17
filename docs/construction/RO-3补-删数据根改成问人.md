## 施工单 RO-3补：删整个数据根改成问人，不再直接拒

状态：做完，等验收（分支 `step/OA-1`）。项目主人 2026-11-17 定（主会话转来，照选项字面）：「删整个数据根这类危险命令照旧弹确认（问你，不是拦）」。

### 目的

RO-3 照「照旧」没动 `delete-data-root`：它从施工 5-17 起是灭顶的、直接拒，人点了也不跑；RO-3 还给 `trash` 加了「不删数据根」。项目主人选的是「弹确认」，这一步照字面改：

1. `shell` 认出删数据根（`rm -rf ~/.miyu`、删数据根里的文件、删装着它的目录，认法不变）改成问人，归「受保护的命令」的删除那一类（`permission.confirm.delete`），人点了允许就跑；关掉那一项的不问。
2. `trash` 删数据根自己、装着它的，同样报高危 `delete_data_root`、归删除那一类，问人；RO-3 加的「不许删」退回。数据根里面的照常删、不问。
3. 被人拒的照受保护命令现有的说法（内核的 `denied`），不另写。

### 改了什么

- `miyu-policy`：`Kind::refused` 去掉 `DeleteDataRoot`，`Kind::group` 归 `Group::Delete`。
- `miyu-basesystem`：`trash` 实现 `Tool::hazard`；`protected` 去掉数据根；`shell` 只给直接拒的读拒绝那一句，`hazard/delete-data-root.txt` 和给人看的 `shell/hazard-delete_data_root` 删了。
- 给模型看的字：删 `software/basesystem/shell/hazard/delete-data-root.txt`；`trash/protected.txt` 退回 RO-3 以前的原文。登记簿跟着改。
- 给人看的字：「受保护的命令」那一组的说明里，永远拒绝的不再列「Miyu 的数据」；「递归删除」的说明加上「删 Miyu 的数据」（三语）。配置样本重写。

### 蓝图改哪几节

- `session/guard.md` 判法表下那段、第四条末（灭顶的、种类、十一种）。
- `tools/shell.md` 两张字表；`tools/trash.md` 开头、报给权限策略的高危、第 4 条、测试表；`config.md` 名字表里两句说明、嵌的样本。
- 协议参考 `sessions.md`（中英）那一句。

### 验收

1. 改前红：`miyu-policy` 的 `deleting_the_data_root_asks_as_a_delete`（删数据根不再 `refused`、归 `Group::Delete`），在 RO-3 上跑是红的；`miyu-basesystem` 的 `deleting_the_data_root_is_reported_as_a_hazard`（RO-3 的 `the_data_root_itself_is_not_deleted` 换掉）。
2. 门禁十一项、长跑随机、Windows 目标的 clippy、`cargo xtask prompts`、`cargo xtask logs`。

### 不做什么

- 不改认法：`shell` 删数据根里的单个文件照旧认成 `delete_data_root`（现在是问人）；`trash` 只在删数据根自己、装着它的时问。
