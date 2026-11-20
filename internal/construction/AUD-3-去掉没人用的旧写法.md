## 施工单 AUD-3：去掉没人用的旧写法

状态：做完，等合（分支 `step/AUD-3-去掉没人用的旧写法`，叠在 AUD-2 上）。来源：项目主人 2026-11-13 定「还没发版，旧兼容的都去掉」。AUD-2 去掉了头四样；它的施工单「不做什么」里列着还留着的，这一步去掉其中不牵动别的仓库、主人的数据里又一处都没有的六样。

### 目的

去掉六样只为以前的文件、以前的会话留着的认法。去掉以后同别的写错的一个待遇。

| | 去掉的 | 原来 | 现在 |
|---|---|---|---|
| 一 | 工具改名留的旧名字 | `subagent` 以前叫 `agent`、`send_message` 以前叫 `message_agent`：改名以前造的会话快照里冻着旧名字，工具目录照旧名字也找得到（`Tool::formerly`），给人看的显示名、那几句结果两个名字各留一份 | 只认现在的名字。照旧名字调是调一件没有的工具 |
| 二 | 预设、人格里撤掉的两个键 | `[preset]`、`[persona]` 里写着 `base`、`[preset]` 里写着 `default_persona` 的当没写，下一次写这份文件时顺手去掉；`preset.set` 写 `default_persona` 回 `bad_params` | 同别的不认识的键：文件报 `unknown_key`，`preset.set`、`persona.set` 回 `preset_invalid`、`persona_invalid` |
| 三 | 把写着 `kind` 的清单改成新写法的那一段 | `miyu_config::package::without_kind`，调它的 `miyu-store` 的 `migrate` 早搬走了，全仓库只剩它自己的测试在用 | 删掉（47 行加 40 行测试） |
| 四 | 清单 `[theme] file` | 读成 `[theme.tui] file` | `unknown_key` |
| 五 | 命令行装包前那句 `notice` 整个是一个字符串的 | 也认 | 只认数组（核心给的一直是数组，命令行和核心一起发） |
| 六 | 请求外壳里的 `scope` | 先摘掉再往下认 | 不摘：外壳里多写的格本来就不看，行为不变 |

第一样连带去掉给人看的字里三个没人用的显示名（`agent`、`message_agent`，和施工 FL-1 就没了的 `load_tools`）、`message_agent/*` 五句，三种语言各一份；测试工具 `miyu_tool::testkit::Renamed`。

### 蓝图改哪几节

- `tools/subagent.md`「以前的名字」、`tools/send_message.md`、`tools/interface.md`（工具的接口去掉 `formerly()`，「登记」去掉以前的名字那两条）、`session/tools.md`「怎么走」第 1 条、`core.md`、`agents.md`、`providers.md`、`cross-session.md`：照现在的写，注明施工 AUD-3 去掉了。
- `presets.md`「预设文件」第 2 条、`personas.md`、`protocol.md` 的 `preset.set`：撤掉的键不再另外认。
- `packages.md`「外观包」第 1 条和出错表：`[theme]` 下只有 `tui`、`web`。
- `protocol.md`「请求」：认请求的表里去掉 `scope` 那一行（那一行本来就和代码对不上：施工 SU-1 起是照收不理）。
- 设计 26 的登记簿里 `send_message.json` 那一行「什么时候加进来」去掉「旧会话冻着 `message_agent`」，`prompts.md` 跟着改。

### 影响面（2026-11-13 只读查的）

主人的两份数据：先前日常用的 `~/.cache/miyu-tui-home`（1627 个会话，最早的 10-25）、现在在用的 `~/.miyu`（492 个会话）。

- 工具的旧名字：日志里没有一次调用叫 `agent`、`message_agent`、`load_tools`，没有一句 `message_agent/*`；两份数据 4394 个 blob 里的会话快照，工具面上都是 `subagent`、`send_message`，没有旧名字。
- 预设、人格：三份人格文件、两份预设文件里没有 `base`、`default_persona`；出厂的和工作区里各包仓库的也没有。
- 清单：装着的 41 个包、官方软件源的 44 个、社区的 4 个都没有 `kind`、`[theme] file`；终端的主题包发出去的那一版已经是 `[theme.tui]`。
- 两个头现在的代码不发外壳里的 `scope`。

真碰上了会怎样：改名以前造的会话再打开，她照旧名字调会被拒（工具面是快照里冻着的，这个会话得重开）；写着撤掉的键的预设、人格在列表里带一条 `problem`，删掉那一行就好；写着 `[theme] file` 的主题包装不上，报错里写着是哪个键。

### 不做什么

AUD-2 施工单里列着、这一步还是不动的：

1. 起脚本、扩展时的旧环境变量名：等软件包改名字（设计 55 的 FH-6）。
2. `session.create` 的 `chosen`：终端、网页现在还在发，等头先改。`/goal edit`：是照旧能用的命令，只是头不再提，不算旧兼容。
3. 权限级别 `workspace`（日志、协议、配置、命令行别名是同一个读法）：主人的两份数据里 1615 个、358 个会话的日志记着它，去掉以后这些会话会按只读开。要项目主人定。
4. 预设的 `[software]`、清单的 `workers`：出厂的文件自己还在用，要先改出厂的。`Features::read_legacy`、「以前造的快照」的别的读法：要一样样对着数据查是哪一步以前造的会话，另算。

### 验收

1. `cargo xtask check` 全过；分支推上去三平台 CI 全绿。
2. 改了说法的测试：
   - `miyu-tool` 的 `tools_remember_which_package_they_came_in`：`agent` 在目录里找不到。
   - `miyu-policy` 的 `a_base_is_an_unknown_key_like_any_other`、`the_keys_taken_out_earlier_are_unknown_like_any_other`。
   - `miyu-endpoint` 的 `a_broken_preset_is_listed_with_its_problem_and_is_not_written`、`a_broken_persona_is_listed_with_its_problem_and_is_not_written`；`preset_set.rs`「写错的什么都不写」里加了写 `preset.default_persona` 的一项。
   - `miyu-config` 的 `the_spelling_before_the_split_is_an_unknown_key`。
   - `miyu-cli` 的 `the_notice_is_the_core_words_as_they_are`：一个字符串的不认。
3. 去掉的测试都是专门测旧写法的：`spawn/renamed.rs` 整份、`orphans.rs` 和 `spawn/recorded.rs` 里「改名以前」的几条、目录和架子的三条、显示名「两个名字一样」的一条。
4. 测试数据里还写着旧写法的跟着改：命令行等子代理的那几条、视图照种子造日志的那一份，调用名从 `agent` 改成 `subagent`（它们靠旧名字的显示名）；`miyu-store` 预设测试的夹具去掉 `default_persona` 那一行。

### 风险

改名以前造的会话打不开旧名字的工具。查过主人的数据里没有；别人手里没有发过版的 Miyu。
