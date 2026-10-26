## BS-1 上中：基础系统拆成十八个包

状态：主会话做。设计 44（基础系统拆包）第三节第 1、2 条，项目主人 2026-10-26 定：基础系统默认装、能卸、能换、能关；一件工具一个包，`sessions`、`packages` 两对各合一个；清单的 `required` 删掉（出厂带什么是打包时挑的事）。

### 一、做了什么

1. **清单**：`resources/packages/basesystem/` 拆成十八份 `resources/packages/<编号>/package.toml`（`read`、`write`、`edit`、`glob`、`grep`、`trash`、`shell`、`jobs`、`subagent`、`ask_user`、`todowrite`、`history`、`session_usage`、`sessions`、`packages`、`show`、`project-docs`、`clicall`）。每份写 `groups = ["basesystem"]`；带工具的写一个同编号的功能、写明工具，装别的包时照它查工具撞名。名字、说明三语。
2. **组**：清单多 `[package] groups`（只有内置包能写，写法同包的编号、不重复，错的 `bad_group`）；组的名字、说明在 `resources/packages/groups.toml`。找预设时把包到组的对照放进 `PresetFile::groups`（`miyu_store::presets::Presets::with_groups`，端点照出厂清单算 `packages::groups`），不进指纹；`[software]` 写了组名的，组里没单独写的包照组的（`decided`）。`preset.get` 不把组名当「写了没装的包」列。
3. **删 `required`**：清单这一格（写了是 `unknown_key`）、拒绝 `package_required`、`package.list` 的 `required`、关包那一道（`config.set` 拒、`switching_off` 滤掉）、起来时的 `WARN required package missing` 都去掉。
4. **登记**：`miyu_basesystem::packages` 照 `package_of` 把工具分到各包，核心装了哪个登记哪个。
5. **跟着走的行为照包判**：后台运行看包 `jobs`（`miyu_tool::BACKGROUND` 改成 `"jobs"`），脚本调工具看包 `clicall`，项目指令的功能编号本来就是 `project-docs`；展示那一句的开关从 `basesystem.show.system_hint` 改成 `show.system_hint`，挂在包 `show` 那一组。
6. **出厂预设**：常规开发照旧写 `basesystem = true`（照组展开）；极简开发改成 `[software]` 开 `read`、`write`、`edit`、`shell`，不再一件件关。
7. **字**：「关掉这个包」的说明去掉「必需的包关不了」；沙盒放行那一项里出厂 `shell` 的来处从 `basesystem` 改成 `shell`；作者说明 `manifest.md` 中英两份的 `required` 换成 `groups`。

### 二、不做什么

- 装卸整组（`miyu pkg install basesystem`）、`package.list` 带组、斜杠命令的拥有者，是 BS-1 下。
- `packages`、`package` 两件不上工具面、会话里 `miyu pkg` 走通道，是 BS-1 中。

### 三、怎么验

1. 配置：`links/tests.rs` 的 `only_builtins_have_groups`（只给内置包、写法不对和重复报 `bad_group`、`required` 不认）；`features/tests.rs` 读出组。
2. 预设：`preset/tests.rs` 的 `a_group_switches_its_packages_unless_one_is_written`（照组开、单独写的照自己、不进指纹）、`packages_switched_off_are_withheld`（原来必需的包也关得掉）；`presets/tests.rs` 出厂三份。
3. 核心：`tools.rs`（卸掉 `shell` 只少它，`sessions` 带着 `send_message`；整组都卸只剩别的包的工具；每件工具归它的包）。
4. 端点：`preset_minimal`、`preset_face`、`preset_background`、`presets`、`package_off`（原来基础系统的包对自己关掉就没了）、`package_switch`、`packages_*`、`widgets`、`pkg`（命令行）跟着改；配置样本照新的生成。
5. 手写变异 10 个全抓到：组不展开、组表不放进预设、端点不算组、组许非内置包写、组许重复、send_message 不归 sessions、没装的也登记、预设页把组名当没装的包列、后台运行不看 jobs、极简开发多开一个包。
6. 整个工作区测试、本机门禁。
