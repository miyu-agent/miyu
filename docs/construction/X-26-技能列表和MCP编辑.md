## 施工单 X-26：技能列表和 MCP 编辑

状态：2026-10-20 合了（16d076f3）。网页的会话要的（项目主人要设置里加技能、脚本、MCP 三页）。

### 起因

网页的设置页要加「技能」「MCP」两页（「脚本」那一页以后另做，这一步不碰脚本）：

- 技能那一页要列出装了的技能：叫什么、给人看的名字和说明、从哪来、文件在哪。协议上没有列技能的方法，`preset.get` 只在一个预设的功能里带技能的名字，不说从哪来、在哪。
- MCP 那一页要能加、改、删个人设置里的服务，还要看到每个服务有哪些工具。`mcp.md` 第 2 条写着「头要做编辑页的另给方法，随那一步」，就是这一步；`mcp.list` 原来不带工具。

### 做了什么

1. **`skill.list {}`**：照造会话的找法（`miyu_store::skills::find`）列，同名的只列核心实际用的那一份，照名字排。一项 `{"name", "display_name"?, "summary"?, "from", "package"?, "path"}`：给人看的两格照连接的语言挑（同 `package.list`），一种都没写的不带；`from` 是 `home`（管理员自己放的）、`shipped`（出厂的包带的）、`package`（管理员装的包带的），随包来的另带包的编号；`path` 是 `SKILL.md` 的位置，家目录下的写成 `~/…`（同项目配置给头的写法）。代码在 `crates/miyu-endpoint/src/skills.rs`。
2. **`mcp.list` 每项多 `tools`**：这个服务这时在工具目录里的工具名，来源同 `preset.get` 里 `mcp-<名字>` 那个功能的工具（起好列完的、核心起来时照目录缓存先登记的都算）；还不知道的不带这一格。
3. **`mcp.set {name, command, args?, env?, venues?, timeout_ms?, need_load?}`**：在管理员个人设置里加上一张 `[mcp.<名字>]`，或者整条换掉同名的那一张，回应 `{"name"}`。
   - 写之前把要写的那一张单独照读服务表的规矩读一遍（`personal_mcp`），写错的拒：`mcp_invalid`，`data.problem` 用装包时清单写错的那一句（`package-problems/bad_mcp`，中、英、日三种）。
   - 写法照 `config.set`：新加 `config/rewrite.rs`，拿着配置服务的锁先重读、带版本替换、撞上手改从头再来，写成了记日志、推 `config.changed`（`keys` 是空的）。`config/set.rs` 里「文件改不了」那一段挪成函数 `broken`，两边共用。
   - 只动那一张的字（`mcp/written.rs`，同 `miyu_config::edit` 按位置换的做法）：新的接在末尾；换掉的放在原来表头的位置，子表（`[mcp.<名字>.env]`）一起换掉，写成点号、行内表的删掉那几行、新的接在末尾。`mcp` 写成值、行内表、`[[mcp]]` 的放不进去，回 `config_file_broken`。
   - 个人设置里没写这个名字、有包带着同名服务的拒：`server_from_package`（新的原因码，中、英两句，同现有原因码）。已经写了的同名服务照常换（它本来就盖住了包里的）。
   - 写了以后正在跑的服务照现有「个人设置变了」那条路换（`follow_home_mcp` 看配置服务换上的新一份），不另造。
   - `need_load` 交了的原样写进去，没交的不写；读服务表时 `need_load` 的默认、读法都没动。
4. **`mcp.remove {name}`**：删掉那一张（连同子表、后面的空行；在文件末尾的连前面的空行），回应 `{}`；没写这个名字的 `unknown_server`。
5. `mcp.set`、`mcp.remove` 只给人（`connection/people.rs`）；`skill.list` 同 `package.list`，扩展也能调。三个方法从 `packages/methods.rs` 分（方法表到了行数上限）。
6. 文档：协议参考 `packages.md`、`errors.md` 中英两份，图纸 `protocol.md`、`mcp.md`、`skills.md`。

### 不做什么

- 脚本那一页（`script.list`）、脚本相关的一切：以后另做。
- `need_load` 的默认、读法：另一步在改。
- `mcp.set` 的 `command` 收一个程序、`args` 另给，回应、`mcp.list` 里的 `command` 照旧是整串（程序在前）。

### 验收

- 端点测试 `crates/miyu-endpoint/tests/skill_list.rs`：
  - `skills_are_listed_once_each_with_where_they_come_from`：家目录里的、出厂包带的、管理员装的包带的各一个，盖住出厂同名的只列一次，照名字排，没写给人看的两格的不带，路径写成 `~/…`，日文没写的照英文、说明只有中文的照中文。
  - `names_follow_the_connection_language`：英文、中文照语言挑。
- 端点测试 `crates/miyu-endpoint/tests/mcp_edit.rs`：
  - `a_set_server_is_written_followed_replaced_and_removed`：新加的写进个人设置、别的字不动、服务起来；整条换掉（没交的格子没了）、服务换进程；删掉以后字回到原样、列表空了；再删 `unknown_server`。
  - `bad_servers_and_package_ones_are_refused_and_nothing_is_written`：名字、空命令、会话种类、时限、环境变量写错的 `mcp_invalid`（话照连接的语言），不认识的参数 `bad_params`，随包来的 `server_from_package`、删它 `unknown_server`，拒了的什么都不写；个人设置里已经写了的同名服务照常换。
  - `listed_servers_carry_their_tools_once_known`：起好列完的带 `tools`，没起过、没缓存的不带；服务起不来时照缓存登记的照样带。
- 单元测试 `crates/miyu-endpoint/src/mcp/written/tests.rs`（六条）：写出来的样子、加在末尾和换行、换在原处连同子表、点号和行内表的、删掉连同空行、放不下的样子不改。
- `tests/mcp_settings_status.rs` 照旧过（等状态的两个函数挪进 `support/mcp.rs` 共用）。
- `cargo xtask check` 八项。

### 结果

- 手写变异 10 处全抓住：`skill.list` 不带 `package`、出厂的也写成 `package`（`skills_are_listed_once_each_with_where_they_come_from` 红）；`mcp.set` 不查写法、不挡随包来的、改之前不重读，`mcp.remove` 没写的也当删了（`bad_servers_and_package_ones_are_refused_and_nothing_is_written` 红，最后一处连 `a_set_server_is_written_followed_replaced_and_removed` 一起红）；`mcp.list` 不带 `tools`、不知道的也带（`listed_servers_carry_their_tools_once_known` 红）；换掉的不放回原处、删掉不带空行（`written` 的单元测试红：端点测试里那一张正好在末尾，看不出来）。改回来以后照原样全绿。
