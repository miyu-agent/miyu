## 施工单 FX-4：写文档时查出来、人会碰到的几处小毛病

状态：做完，等合（分支 `step/FX-4` 上）。来源：主会话写文档时查出十条，2026-11-14 交来，后来又加两条（第十一、十二条）；每条先查清现象是真的再修，第 4 条的确认、第 6 条要拍板，没做。

### 目的

人照着报错、帮助页、设置说明去做，做得通；给人看的字不带施工号、不说「她」；两处测试守得住它们该守的东西；核心 CI 在往 main 提的 PR 上也跑。

### 蓝图改哪几节

- `store.md`「出错」`PrepareError` 的 `NotOurs`、`OldLayout` 两行。
- `cli/setup.md`「样子」「给人看的字」「核心看不到」那一行取舍；`models.md`「还没有的」那一句。
- `cli/memory.md`「对外的样子」`clear me` 一行和两份帮助页样本。
- `cli/check.md`「怎么走」第 4 条、「守着它的」两行；`protocol.md` `check` 第 5 条；`config.md` 语义模型的说明；协议参考 `packages.md`、`packages.en.md` 的 `check`。
- `life.md` 命令表和第 2 条的回执原文。

### 一、碰到旧数据根的报错

**证实**：`PrepareError::OldLayout` 的原话是「…旧数据照施工单 FH-7 自己搬」，命令行第一个词不是内置命令时照原话印（`miyu-cli` 的 `packages::old_layout`），各命令建骨架出错也照原话印。再往下查：`OldLayout` 认的是 `.miyu-root`，那是重制版布局 1、2 的标记；0.7 版的数据目录（也在 `~/.miyu`）两种标记都没有，落在 `NotOurs`（「里面有别的东西，认不出」），原来一句怎么搬都不说。

**修法**：`NotOurs` 的原话和三种语言的 `start/data_root_not_ours` 加一句：是 0.7 版的数据目录的话，先 `mv <数据根> <数据根>-0.7`，再 `miyu pkg install migrate`、`miyu migrate <数据根>-0.7`（照迁移程序的说明），不是的话把 `MIYU_HOME` 设成空目录。`OldLayout` 去掉施工号；三种语言的 `start/data_root_old_layout` 去掉「旧数据用迁移程序搬」（迁移程序只认 0.7 版的）。

测试 `miyu-store` 的 `human::old_data_roots_say_how_to_move_in_plain_words`：原话和三种语言都说怎么搬、都不带施工号。改前红。

### 二、核心起不来叫人看的日志

**证实**：`StartError::Silent` 写死「看数据根的 state/logs/core.log」，日志 FH-3 起在 `var/log/core.log`（蓝图 `ipc.md` 早是对的）。

**修法**：`miyu-store` 加 `CORE_LOG`（装日志用的名字）和 `DataRoot::core_log()`，核心装日志、`miyu core stop` 的提示、测试夹具都照它；`Silent` 的原话照 `DataRoot::at("").core_log()` 写相对位置。

测试 `miyu` 的 `core::a_core_that_leaves_without_a_word_is_reported` 多一段：原话里说的那个文件，真核心起来一次以后在数据根里真有。改前红（`state/logs/core.log` 不存在）。

### 三、`miyu setup` 说核心看不到环境变量

**证实**：提示「核心空闲退出后重新运行 'miyu setup'」；核心 Z-1 上起默认常驻（`core.idle_exit` 默认 0），等不到。

**修法**：提示改成「先运行 'miyu core stop'，再重新运行 'miyu setup'」（英文 `run 'miyu core stop', then 'miyu setup' again`）。`miyu setup` 照 `connect_or_start` 连核心，停了以后由这个终端拉起，新核心带着这个终端的环境。装成服务的核心正常停下不会被拉回来（systemd 是 `Restart=on-failure`，launchd 是 `KeepAlive` 的 `SuccessfulExit = false`）；下次开机由服务起的核心照旧看不到这个终端的环境，要长期用的还是贴 key。测试 `miyu-cli` 的 `setup` 照新字改；只是换字，没有改前红的测试。

### 四、`miyu memory clear me`

**证实**：协议 `memory.forget` 的 `clear: me` 是整间（`memory.md`「协议」，「清空」第 5 条：不带会话的「那以前的全都清掉」；整理第 15 款：「整间清空连卡一起拿掉」），代码照它做；帮助页写「清除关于用户本人的全部记忆」，说窄了。照蓝图改帮助：「清除全部记忆，包括人物卡」（英文 `Clear all memories, person cards included`），`cli/memory.md` 那一行和样本跟着改。只是换字，没有改前红的测试。

**先确认：没做，要拍板**。`cli/memory.md`「出处」和施工单 `R-3-命令行（再补）.md` 写着「清空不问」是项目主人 2026-10-07 定的。要不要改成像 `miyu pkg remove` 那样问「…？[Y/n]」、`--yes` 跳过，等项目主人定。

### 五、换成远程的语义模型以后不联想

**证实**：设计如此。联想、知识库的自动想起都照模型的门槛表（`core/memory/recall.toml`、`core/knowledge/recall.toml`），表里只有内置的模型，别的不联想（2026-10-10 项目主人定：没量过的不知道多像才算像）。搜索照常（下限表没有的照 `FLOOR`）。只是哪里都不说。

**修法**：`check` 回应的 `embedding` 多一格 `no_recall`：这个模型不自动联想的几样（`memory`、`kb`），`miyu check` 末尾印一行「  不自动联想：记忆、知识库（这个模型没有测过门槛，搜索照常）」。设置项 `models.embedding` 的说明后面加一句「只有内置模型会自动联想记忆和知识库，换成别的模型后只在搜索时用到。」（三种语言；这句说明 R-5 再补时是项目主人定的，这次只在后面加）。

测试：`miyu-endpoint` 的 `check_embedding::it_says_which_model_and_how_far_each_index_is_filled`（测试的小模型不在表里，两样都列）改前红；`check::embedding::tests::only_a_measured_model_recalls_on_its_own`（出厂的表：内置的两样都联想、远程的都不）；`miyu-cli` 的 `a_model_without_a_threshold_says_it_does_not_recall`。

### 六、`miyu -h` 里列着 render、memes、sponsor

**证实**：是核心定的规矩。清单 `[process]`（核心拉起的扩展）要有 `[command]`（`needs_command`），程序名只能写在那里；`[command]` 的 `about` 必写，有 `[command]` 的就是一条 `miyu <名字>` 子命令，`miyu -h` 照它列、照它转交。这三个包只想让核心拉起 `serve`，照规矩不得不写一条子命令。只靠核心挑不出来：`miyu im` 也是 `[process]`，它的子命令是真的。

**没改，要拍板**：怎么改要核心和包一起动。推荐让 `[process]` 自己能写 `program`、`[command]` 不再是必写的（`[command]` 只给真有子命令的包），这三个包去掉 `[command]`；另一条路是 `about` 改成可以不写、不写的不列进 `miyu -h`（还能转交），包去掉 `about`。

### 七、人生会话的回执说「她」

**证实**：`commands/life-about`、`life-awake`、`life-started`、`life-renewed` 中文写「她」，英文写 she、her；日文没有。

**修法**：改成中性写法（「这个人格」、不提主语），蓝图 `life.md` 的原文、`life_command` 测试跟着改。新测试 `miyu-store` 的 `human_languages::nothing_for_people_calls_the_ai_she`：内核和每个软件包的 `human/` 三种语言，中文不写「她」，英文不写 she、her、hers、herself，日文不写「彼女」。改前红。

### 八、人格的 `Code::ALL`

**证实**：少了 P-7 加的 `bad_background_opacity`、`bad_background_focus`、`bad_background_blur`。

**修法**：补上。新测试 `miyu-policy` 的 `persona::tests::all_lists_every_code_in_order`：一个穷尽的 `match` 给每一种排位，加了代码不补这里编不过，补了 `ALL` 不跟着补就红；改前红。另照预设、软件包的写法加 `miyu-store` 的 `human::every_persona_problem_is_said_in_three_languages`。

### 九、`others_get_the_table_while_a_large_batch_retires` 偶发红

**证实**：原来断言卸会话期间拿表的锁最久等不过 1 秒，macOS 的 CI 忙时等过 1.19 秒。

**修法**：不比时长，比看到了什么：卸的期间一直有人排队要表的锁（拿完马上排回去，不睡），表的锁先排先得，一批批放锁的，排着的那一个在批与批之间拿得到，看得到卸了一部分的表（既不是 200 也不是 0）；卸完整批才放锁的只看得到 200 和 0。断言至少看到一次卸了一部分的。把 MEM-5 的分批临时退回（`BATCH` 改成 100000，一批卸完全部）跑：红。

### 十、核心 CI 不在 PR 上跑

**证实**：`ci.yml` 只在推 `main`、`step/**` 时跑，别人从分叉提的 PR 一项都不跑。

**修法**：加 `pull_request`，只对往 `main` 提的，不跑的文档照推送那一份。从 `step/` 分支提 PR 的会跑两遍（推送一遍、PR 一遍），可以接受。

### 十一、扩展能力 `search` 的名字

**证实**：NF-1 把设置「搜索服务供应商」改叫「联网服务」，能力 `search` 的说明跟着改了，名字还是「当搜索服务供应商」（英文 `Be a search provider`，日文「検索プロバイダーになる」），装包、升级时确认的那一屏上看到的就是它。

**修法**：三种语言照设置的新名字改：「提供联网服务」、`Be a web provider`、「ウェブプロバイダーになる」；协议参考 `extensions.md` 中英两份的能力表、蓝图 `cli/pkg.md` 的样本、命令行两处测试的假数据跟着改。只是换字。

### 十二、协议参考里新加的「她」

**证实**：VW-1 到 LF-10 这几步在 `docs/reference/` 加的行里，`life.md`（LF-10）、`packages.md`（LO-1 的 `/loop`）、`view.md`（HT-3 改过的 `tool` 一行）、`venues.md`（QT-1 的 `quoted`、回顾的回应）写了「她」，英文那几行写了 she、her。协议参考给人和扩展作者看。

**修法**：改成「模型」「会话日志」或者不提主语，中英两份一起改。更早的行、给模型看的提示词、人格侧的字不在这一次。

### 不做什么

- 第 4 条的确认、第 6 条的规矩：等拍板。
- 协议参考里更早写下的「她」（第十二条只管最近几步加的行）。
- 命令行建骨架出错照中文原话印（各命令都是 `error.to_string()`），英文界面也是中文：原来就这样，这次不动，记下。

### 验收

1. `cargo test -p miyu-store --test all human`、`cargo test -p miyu-policy --lib persona`、`cargo test -p miyu-endpoint --test all check_embedding:: life_command:: memory_pressure:: config::`、`cargo test -p miyu --test all core::`、`cargo test -p miyu-cli`。
2. `cargo xtask check` 十一项、`cargo test -p miyu-kernel random:: -- --include-ignored`、Windows 目标 clippy、`cargo xtask prompts`、`cargo xtask logs`；配置样本重新生成。

### 风险

- `check` 回应多一格：头照字段取，不认识的不管。
- 设置说明改了：生成的配置样本跟着变。
