## CC-1：不带会话的命令菜单照默认预设

状态：主会话做，一步，2026-10-28 做完。10-27 终端头撤回 `/loop` 报错时留下的小项。

### 一、为什么

终端首页（还没开会话）问 `command.catalog` 不带会话，核心照「包装没装」列全部。跟着包的命令（`/loop`、`/goal`、`/todo`）在默认预设没开那个包时也列出来。人敲了，开出来的会话里没有那件工具，回 `unknown_command`。

### 二、定下的（技术题，主会话定）

1. 不带会话时，跟着包的核心命令照替谁干活的那个人的默认预设（`preset.default`，个人设置压着系统配置）筛：照 `presets::package_on`（定时任务到点判包开没开用的同一个），没开的不列。
2. 只管跟着包的核心命令。模板、扩展声明的命令照每条命令的功能（`command-<名字>`）判，预设关了敲出来是 `command_not_allowed` 带原因，不是这次的毛病，照旧都列。
3. 带会话的不变。

### 三、做了什么

1. `presets::default_of`：从 `resolve` 挪出来。
2. `commands/catalog.rs` 不带会话那一支照它和 `package_on` 筛。
3. 蓝图 `protocol.md`、协议参考 `packages.md` 中英两份。

### 四、怎么验

1. 端点 `command_catalog.rs` 新一条：个人设置默认预设是「极简开发」（`minimal`），不带会话列不出 `goal`、`loop`、`todo`，`clear`、`stop`、`workspace` 照列。原来「全部功能」那条一字不变。
2. 手写变异（不筛）这条红。
3. 本机门禁全过。
