## FL-1：工具一律全量

状态：主会话做，分两步。2026-10-27 项目主人定（设计 `25-工具加载.md` W10，W9 作废）。

### 一、为什么

主人那份部署用 magpie 的 clinepass/cline-pass/deepseek-v4.1-flash，这一家做约束解码：要先加载的工具在请求里只有空外壳，模型只能发 `{}`。`game_compat` 连着五次 `{}`，`load_tools` 成功以后照样 `{}`；技能带的脚本 `aur_review` 也一样。那份配置没写 `compat.strict_tools`，出厂档案里也一家都没标。项目主人比过「加载了才换成完整格式」（每加载一次工具列表就变一次，整段缓存不命中）和全量（多的是固定一块，每次都吃缓存），定了全量，再定把懒加载整套拆掉。

### 二、上：做了什么

1. 拼工具面那一处（`miyu-session` 的 `agents.rs`）一律当完整格式发：脚本、技能的脚本、MCP、扩展登记的都一样，登记成要先加载的、人改成先加载的也一样；工具面上不再有 `load_tools`，请求不另带完整格式。
2. 不留「超过多少件退回外壳」的兜底。技能正文照旧用到再读。
3. 只管新开的会话：已经开着的会话，快照里冻着的工具面每件带着 `need_load`，预设换了重拼时以前就有的那几件也照快照原样（`actor/persona/face.rs` 的 `refaced`），照旧是外壳。要等下一步把内核那一层的外壳拆掉才变成全量。
3. 测先加载那几条的改成断言全量（`scripts/loading.rs`、`script_pages.rs`、`tools.rs`、`strict_tools.rs`）；`member_config.rs` 原来拿「改成先加载」当成员设置生效的记号，换成他把 `read` 这个包对自己关掉。

### 三、上：怎么验

1. 上面几条改过的测试：开头写了 `# Need load: yes` 的脚本、登记成先加载的工具，工具面上都是完整参数、没有 `load_tools`；标没标 `strict_tools` 都一样。
2. 本机门禁。

### 四、下：做了什么

把整套懒加载拆掉，以前写过的照旧读得进。

1. 内核：`load_tools` 当场答的那一套（`tool::Loading`、`Contract`）、请求另带的 `full_parameters`、策略里的 `loading` 都拆了。以前日志里调过 `load_tools` 的照原样回放；她要是再调，照不认识的工具答。
2. 策略：外壳那一段（`miyu-policy/src/loading.rs`，`stub`、`LoadingTexts`、`LOAD_TOOLS`、`STUB`、`Offered`）拆了，`ToolEntry` 去掉 `need_load`。以前造的快照里工具带 `"need_load":true`、冻着先加载用的字的，照旧读得进，造策略时不认，照完整格式发：老会话下一次请求就是全量（缓存掉一次），这一步才让已经开着的会话也好起来。`session.tools` 照 `sent_names` 列名字。
3. 资源：`resources/core/loading/` 四份字撤了，登记簿、`prompts.md` 跟着撤。人看的字里 `loading/loaded`、`loading/bad-args` 留着：旧日志里那几条照它显示。
4. 驱动、模型：`compat.strict_tools`（驱动的开关、档案的格、手写的配置项 `CompatSettings`、`model.list` 那一格）拆了。L-4 没落地过，没东西拆。
5. 登记来源：`Spec.need_load` 拆了。说明文件、扩展 `provide`、`mcp.set` 照旧收这一格（这三处拒收不认识的格，老的写法要读得进），不看、不写；脚本开头的 `# Need load:`、包清单 `[[mcp]]` 的 `need_load` 照别的不认识的写法不管。
6. 配置：个人设置 `loading.tools.<name>`、`loading.mcp.<id>` 两项、`providers.<id>.compat.strict_tools` 拆了；配置那一层多一张退役的键（`miyu-config/src/retired.rs`）：写过的照旧读得进，不进最终值，也不报 `unknown_key`。`miyu-session/src/loading.rs` 只剩「脚本里能不能调」那两项。
7. 协议：`tool.set_loading` 是协议 1 冻结的方法，照旧收、照旧查参数，回 `{}`，什么都不写；`mcp.list`、软件包列表上的服务、`script.list` 不再带 `need_load`；`session.tools` 不再出现 `loaded: false`；`session.context` 没有 `tools.deferred` 那一块。

### 五、下：怎么验

1. `miyu-config` 的 `retired_keys_are_read_quietly`：三样退役的键读得进、不报、不进最终值，拼错的照旧报；`retired::tests` 认得那三样、不认别的。
2. `miyu-policy` 的 `an_old_snapshot_with_tools_to_load_is_sent_whole`：以前的快照带 `need_load` 和先加载的字，造策略发的是完整参数，没有 `load_tools`。
3. 照旧收的几处：`miyu-tool` 的 `a_descriptor_with_a_retired_need_load_is_still_read`、端点 `provide` 的 `a_retired_need_load_is_accepted_whatever_it_says`、`mcp_edit.rs` 交 `need_load` 不写进服务表、`script_pages.rs` 的 `set_loading_is_accepted_and_changes_nothing`、`miyu-config` 的 `a_retired_need_load_is_read_and_ignored`（包清单）、`miyu-policy` 的 `a_retired_need_load_line_is_ignored`（脚本开头）。
4. 测先加载的测试删了（驱动 `openai_chat_strict.rs`、模型 `provider/tests/strict.rs`、端点 `models_strict.rs`、内核的 `load_tools` 那几条），配置样本照 `MIYU_SAMPLE_WRITE=1` 重写，蓝图里的样本块照它重填。
5. 请求字节：出厂会话本来就没有要先加载的工具，`request_sample` 和各驱动的逐字节测试照过。
6. 手写变异 5 个全杀：解析不认退役的键、退役表少一项、`provide`、`mcp.set`、说明文件不再收 `need_load` 那一格。拆掉的代码没法变异，守它的是第 2 条（旧快照照全量发）。
7. 本机门禁。
