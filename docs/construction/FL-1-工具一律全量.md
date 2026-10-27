## FL-1：工具一律全量

状态：主会话做，分两步。2026-10-27 项目主人定（设计 `25-工具加载.md` W10，W9 作废）。

### 一、为什么

主人那份部署用 magpie 的 clinepass/cline-pass/deepseek-v4.1-flash，这一家做约束解码：要先加载的工具在请求里只有空外壳，模型只能发 `{}`。`game_compat` 连着五次 `{}`，`load_tools` 成功以后照样 `{}`；技能带的脚本 `aur_review` 也一样。那份配置没写 `compat.strict_tools`，出厂档案里也一家都没标。项目主人比过「加载了才换成完整格式」（每加载一次工具列表就变一次，整段缓存不命中）和全量（多的是固定一块，每次都吃缓存），定了全量，再定把懒加载整套拆掉。

### 二、上：做了什么

1. 拼工具面那一处（`miyu-session` 的 `agents.rs`）一律当完整格式发：脚本、技能的脚本、MCP、扩展登记的都一样，登记成要先加载的、人改成先加载的也一样；工具面上不再有 `load_tools`，请求不另带完整格式。
2. 不留「超过多少件退回外壳」的兜底。技能正文照旧用到再读。
3. 测先加载那几条的改成断言全量（`scripts/loading.rs`、`script_pages.rs`、`tools.rs`、`strict_tools.rs`）；`member_config.rs` 原来拿「改成先加载」当成员设置生效的记号，换成他把 `read` 这个包对自己关掉。

### 三、上：怎么验

1. 上面几条改过的测试：开头写了 `# Need load: yes` 的脚本、登记成先加载的工具，工具面上都是完整参数、没有 `load_tools`；标没标 `strict_tools` 都一样。
2. 本机门禁。

### 四、下：要做的

拆整套：`Spec.need_load`、`load_tools` 和它的字（`core/loading/`）、`miyu-policy` 的外壳、内核的 Loading/Contract、个人设置 `loading.tools.*`、`loading.mcp.*`、`compat.strict_tools`、L-4 照实测学约束解码。已发的包清单、脚本开头、MCP 设置里写了这些的照旧读得进，忽略不报错。`tool.set_loading` 是协议 1 冻结的方法，留成空操作；两个头的技能、脚本、MCP 页去掉加载开关。登记簿里 `load_tools` 那几条字跟着撤。
