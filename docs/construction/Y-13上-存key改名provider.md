## 施工单 Y-13 上：存供应商 key 的命令改名 `miyu provider`

状态：主会话派分身做、主会话审。项目主人 2026-10-24 定：`miyu login`、`miyu logout`、`miyu whoami` 留给登录账号（设计 06 U26），存、列、删供应商 key 的命令挪到 `miyu provider` 下面，老写法不留兼容。登录账号（Y-13 下）和第二个本机入口（Y-1 下下）推到核心服务化（Z-1）一起做。

### 做了什么

- `miyu login [名字] [--system]` → `miyu provider login [名字] [--system]`；`miyu login --list [--format]` → `miyu provider list [--format]`；`miyu logout [名字] [--system]` → `miyu provider logout [名字] [--system]`。行为、输出、退出码不变。老写法现在是不认识的子命令（退出码 2）。
- 帮助页 `login.txt`、`logout.txt`（中英）并成 `provider.txt`，主帮助页跟着改；给人看的字里叫人去跑老写法的（命令行、核心的 `human` 三语）都改成新的。
- 蓝图 `cli/login.md`、`config.md`、`cli/main.md` 等，设计 07、14、15、22，作者说明，`docs/designs/samples/config/` 的样本。施工单（历史记录）不改。
- 顺手：「压缩」那一组挪到设置的「高级」页（网页 10-24 转项目主人，单独一个提交）。

### 验收

1. 命令行测试（`miyu-cli` 253 + 108）、`miyu` 真跑二进制的 login、provider、help 27 条全过；新加一条：老写法 `miyu login`、`miyu logout` 退出码 2。
2. 帮助页和 clap 定义一一对上（`help/tests.rs`），蓝图里的帮助页样本逐字节对上（门禁「文档」）。
3. `cargo xtask check` 全过。
