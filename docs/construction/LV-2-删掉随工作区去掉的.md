## 施工单 LV-2：删掉随「工作区」一级去掉的

状态：已完成（2026-11-11）。设计 `11-权限与沙盒.md` A16（2026-11-09 项目主人定），接着 LV-1。

### 目的

LV-1 把级别收成只读、完全放开两级以后，几样东西还留在代码里却再也走不到：`shell` 和脚本的 `outside_sandbox` 参数、命令越过沙盒时「此目录不再询问」带的那几样、Windows 的沙盒用户。这一步把它们删掉。行为不变的只有一处例外：工具面少了一格参数，前缀的字节变一次。

### 蓝图改哪几节

- `tools/shell.md`：参数表、样本去掉 `outside_sandbox`；「命令前缀」一节、守着它的那两行去掉。
- `skills.md`：脚本「这一次在沙盒外跑」那一款去掉。
- `session/guard.md`：判法表里「要在沙盒外跑」「你在配置里放行了的」两行、下面讲它们的两条、守着它们的两行去掉。
- `session/tools.md`：`RunTool` 不再带 `grants`。
- `kernel/` 里写 `RunTool` 带 `grants` 的那一处。
- `cli/main.md`：命令表、帮助页样本、帮助页的页数去掉 `sandbox`；嵌着子命令的例子换成 `miyu service`。
- `sandbox/windows.md`：整页改成一句现在的样子（不装任何东西，沙盒用户撤了）；`README.md` 那一行跟着改。
- `store.md`：数据根里去掉 `sandbox-windows.json`、`windows-error.json`。
- `prompts.md`：`cargo xtask prompts` 重新生成。

### 删除清单

动手之前列的；清单外补的在最后一节标出。

**一、`outside_sandbox`（施工 D-4、D-4 补、5-14 改下）**

| 文件 | 删什么 |
|---|---|
| `resources/software/basesystem/tools/shell.json` | 参数 `outside_sandbox` 那一格 |
| `resources/software/basesystem/script/outside-sandbox.txt` | 整份（脚本登记时接进参数里的那一句） |
| `crates/miyu-basesystem/src/shell.rs` | `Args::outside_sandbox`、`Tool::outside_sandbox` 的实现 |
| `crates/miyu-basesystem/src/shell/script.rs` | 常量 `OUTSIDE`、字段 `Script::outside`、函数 `offer_outside`、`Script::passed`（参数原样交给脚本）、`Tool::outside_sandbox` 的实现、模块说明里那一条 |
| `crates/miyu-tool/src/lib.rs` | `Tool::outside_sandbox` |
| `crates/miyu-tool/src/testkit.rs` | 假工具的 `outside_sandbox` |
| `crates/miyu-basesystem/tests/shell.rs` | `shell_says_when_it_wants_out_and_what_to_show` 里查 `outside_sandbox` 的断言（问人时交什么的断言留着） |
| `crates/miyu-basesystem/tests/script.rs` | `a_script_can_ask_to_run_outside_the_sandbox_once`、`a_script_s_own_outside_sandbox_parameter_is_left_alone` |
| `docs/designs/26-提示词.md` 登记簿 | `script/outside-sandbox.txt` 那一行；`tools/shell.json` 那一行换指纹、token、为什么 |

旧会话日志里带这个参数的调用：内核的修正只动声明过的参数，多出来的原样留着；`shell` 读参数不认的格子不管。加两条测试守着（`shell`、脚本各一条）。

**二、命令的「此目录不再询问」（施工 5-15 给命令的那部分、SB-3）**

| 文件 | 删什么 |
|---|---|
| `crates/miyu-kernel/src/session/action.rs`、`session/tools.rs`、`session/approval.rs` | `Action::RunTool` 的 `grants`（执行器拿它往沙盒能写的里加目录，LV-1 起没人读） |
| `crates/miyu-session/src/actor/run.rs` | 拆 `RunTool` 时丢掉的那一格 |
| `crates/miyu-tool/src/lib.rs`、`testkit.rs` | `Tool::prefix` 和假工具的实现 |
| `crates/miyu-basesystem/src/shell.rs` | `Tool::prefix` 的实现 |
| `crates/miyu-policy/src/hazard/prefix.rs`、`prefix/tests.rs`、`hazard.rs` 里的导出 | 整个模块（取命令前缀，只给越过沙盒的规则用） |
| `docs/reference/sessions.md`、`sessions.en.md` | 「选 `session` 记住的是什么」里讲命令越过沙盒、`prefix` 的那几句 |

不删、还走得到的：文件工具的放行规则（`tool.approval_requested` 的 `rule`、内核的 `Grants`、`Action::GuardTool` 的 `grants`、`guard/asking.rs` 的 `granted`）。只读时读私密目录照旧问人，人选「本会话都允许」照旧记这条规则。

**三、`sandbox.tools`（施工 X-36、X-37）**

配置键、放行表、`miyu check` 的四种警告 LV-1 已经删了。这一步只剩文档：

| 文件 | 删什么 |
|---|---|
| `docs/authors/scripts-skills.md`、`.en.md` | 「非在沙盒外跑不可的脚本」一节 |
| `docs/reference/packages.md`、`.en.md` | `check` 条目里 `sandbox.tools` 那半句 |

老配置里留着 `sandbox.tools.<名字>` 的：照不在清单里的键报 `unknown_key`（警告，原样留在文件里，不进最终值）。加一条测试守着是警告不是错误。

**四、Windows 的沙盒用户（施工 5-8）**

| 文件 | 删什么 |
|---|---|
| `crates/miyu-sandbox/src/install.rs`、`install/` 整个目录（`dpapi`、`elevate`、`files`、`password`、`quote`、`record`、`report`、`user`、`winsys`、`testkit`、各自的测试） | 整个模块 |
| `crates/miyu-sandbox/src/lib.rs` | `pub mod install` 和说明里那一条 |
| `crates/miyu-sandbox/Cargo.toml` | 只给装沙盒用的依赖：`miyu-kernel`、`miyu-store`、`getrandom`、`base64`、Windows 的开发依赖 `miyu-pipe`；`windows-sys` 只给它用的功能 |
| `crates/miyu-cli/src/sandbox.rs`、`sandbox/flow.rs`、`sandbox/tests.rs`、`language/sandbox.rs` | `miyu sandbox setup`、`remove` 和它说的话 |
| `crates/miyu-cli/src/help/zh/sandbox.txt`、`help/en/sandbox.txt`、`help.rs` 的 `Page::Sandbox`、`help/tests.rs` 里查这一页的 | 帮助页 |
| `crates/miyu-cli/src/help/zh/miyu.txt`、`en/miyu.txt` | `sandbox` 那一行 |
| `crates/miyu-cli/src/lib.rs`、`language.rs` | 模块和导出（`Sandbox`、`SandboxAction`、`OwnerArgs`、`sandbox()`） |
| `crates/miyu-cli/src/misuse.rs`、`misuse/tests.rs` | 注释、测试里拿 `miyu sandbox` 当嵌着子命令的例子的，换成 `miyu service`（说的话不变） |
| `crates/miyu/src/main.rs` | `Command::Sandbox`、帮助页的接线 |
| `crates/miyu/tests/commands.rs` | `elsewhere_the_sandbox_needs_no_setup`、`the_sandbox_help_is_its_page`；另两条拿 `miyu sandbox` 当例子的换成 `miyu service` |
| `crates/miyu-store/src/root/places.rs`、`places/tests.rs` | `sandbox_record_file`、`sandbox_report_file` |

不动：助手在 Windows 上照 MXC 收紧的那一路（`src/bin/miyu-sandbox/windows.rs` 和它下面的、`tests/windows.rs`、`busy`、`carve`），核心探测、`lifeline`。

### 不做什么

- 不补 LV-1 没改的蓝图：`session/guard.md`、`session/tools.md` 里讲工作区那一级的段落只删这一步点到的几处。
- 不动 `Tool::file`（施工 X-37 补，现在只有一条测试读它）：留给以后清无用代码。
- 不动设计文档的旧段落、以前的施工单：设计 11 开头已经写明以 A16 为准。
- 不动头：终端、网页的「此目录不再询问」按钮归 LV-3。

### 验收

- `cargo test --workspace --no-fail-fast` 全过；`cargo clippy --workspace --all-targets -- -D warnings`、Windows 目标的 clippy、`cargo fmt --check`、`cargo doc` 不报。
- `cargo xtask prompts` 以后没有没提交的差别。
- 新测试：`shell` 带着 `outside_sandbox` 的旧调用照常跑；脚本带着它的照原样交给脚本；老配置里的 `sandbox.tools.<名字>` 只报警告。
- `miyu sandbox` 报不认识的命令。

### 风险

- 工具面的字节变一次：所有会话的缓存前缀冷一次。token 照本机 tiktoken `o200k_base` 量，真端点的数待主会话补。
- Windows 上装过沙盒用户的机器：账号 `miyu-sandbox`、数据根里的 `etc/private/sandbox-windows.json` 留着，Miyu 不再管。蓝图 `sandbox/windows.md` 写了手工怎么撤。
- Windows 的代码本机编不了运行：只过 Windows 目标的 clippy，三平台 CI 由主会话推。

### 清单外补的

- `crates/miyu-basesystem/src/load.rs` 的 `raw`：只有读 `outside-sandbox.txt` 用它，删了那一份就没人调。
- `crates/miyu-cli/src/misuse.rs` 的 `ErrorKind::MissingSubcommand` 那一支（「'<命令>' 需要子命令」）：只有 `miyu sandbox` 走得到（它关了「没写参数就印帮助」），别的嵌着子命令的命令少了子命令照旧说用法。蓝图 `cli/main.md`「参数写错时」那一行跟着去掉。
- `crates/miyu-cli/src/language/sandbox.rs` 没有整份删：`miyu ask` 说沙盒用不上的那几句也在里面，只删了 `miyu sandbox` 说的话。
- `crates/miyu/tests/commands.rs` 的夹具 `listing`：只有删掉的那条测试用它。
- `crates/miyu-cli/src/packages/tests.rs`：总表里最后一条命令原来是 `sandbox`，接软件包那一节的断言换成照现在的最后一条（`upgrade`）。
- 包编号的保留名 `sandbox` 不动（`miyu_config::package::RESERVED`）：装过沙盒用户的机器上 `var/log/sandbox/` 还在。`miyu-store` 对这张表的测试里单独写上它。
- `crates/miyu-store/src/root.rs`、`crates/miyu-sandbox/Cargo.toml`、`crates/miyu-cli/Cargo.toml` 里讲装沙盒的注释跟着改。
- 蓝图 `protocol.md`、`kernel/asking.md`、`kernel/session.md`、`sandbox.md`、`sandbox/windows-run.md`、`store.md` 里点到这几样的句子。
