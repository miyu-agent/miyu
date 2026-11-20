## 施工单 RM-1：移除推演人生（核心）

状态：已完成（2026-11-18）。

### 为什么

项目主人 2026-11-18 定（核心主会话转达）：**彻底移除推演人生**，施工 LF-1 到 LF-15 做的全部拆掉，连 LF-11 的「后天」。这个功能横切了定时表、循环、会话的生命周期、记忆和三个头，到处是「是人生的话另走一条」的特判；而在聊天框里定时自言自语，成不了「让 AI 有自己的人生」。拆净以后核心回到正交的底座。

同时定的两条：

1. 「后天」（`note_change`、记忆里的那一类、人格页和记忆页的「后天」）一起删。
2. 旧数据**不做兼容**（项目主人 2026-11-11：发版前不做旧兼容）：核心里不写「读到人生留下的东西就绕过去」的分支。主人本机的人生会话、定时表里的人生行、人格文件里的 `[life]` 这些，部署前由部署脚本删；这一步交出要删的清单（下面「部署前要清的」）。

拆的顺序：核心先拆（这一步），合了以后终端、网页、托盘照新的核心跟（另外的会话做）。

### 拆掉的

- **命令和协议**：斜杠命令 `/life`；方法 `life.status`、`life.start`、`life.pause`、`life.resume`、`life.kill`、`life.new`、`life.list`、`life.seen`；`command.run` 回应里的 `session`、`persona`、`lives`；通知 `life.ended`、`life.seen`；`session.list` 的 `life`；原因码 `life_schedule`、`no_life`、`life_exists`、`life_ended`、`bad_life_context`。
- **内核**：事件 `life.started`、`life.ended`；人生结束了就不再开轮、封存的那一层（封存本身是施工 C-8 的，留着）；回合开始挂接点的 `opener`；`By::is_lived_turn`。
- **定时表**：第三种循环 `life`（`Loop::Life`、`by` 里的 `"loop":"life"`）；没定下一次照默认间隔再跑、核心起来补一次、`stop` 不收、通用入口改不了删不掉；行上的 `persona`、`rounds`、`busy`；定时库里的小表 `doing`、`rounds`、`life_unseen`。
- **每一轮那段话**：标签 `<life-round>`、推演方法（出厂的 `software/life/method.txt`、人格的 `prompts/life.md`）、她留给下一轮的话。
- **记忆**：每一轮交上次以后新记的（块的类别 `fresh`）；同一个人格别的会话的「现在在干嘛」（`<life-doing>`）；人生循环开的回合进回合索引；结束时清掉这段人生记下的记忆和回合索引；「后天」（类 `change`、条目上的 `why`、常驻的那一块）。
- **工具、包、预设**：`end_life`、`note_change`；内置包 `base-life`、清单的 `needs = ["life"]`；功能编号 `life`、`change`；出厂预设 `life`；预设的 `hidden` 键（施工 LF-5 上只为它加的，没有别的预设用）。
- **人格**：`persona.toml` 的 `[life] context`；人格协议里的推演方法、上下文上限、「后天」那一列。
- **会话索引**：`life` 一列；人生循环交进来的那段话不算第一句的那条规矩。索引的版本从 17 升到 18，以前的索引删掉照日志重建。
- **资源和字**：`resources/software/life/` 整个目录、`resources/packages/base-life/`、`resources/presets/life.toml`、`software/loop/next/unstoppable.txt`，三种语言里给人看的那些句子；提示词登记簿里对应的 9 行。
- **文档**：手册的「推演人生」两页、协议参考的 `life` 两页、蓝图 `life.md`、`tools/end_life.md`、`tools/note_change.md`，别的页里说到它的行和半句。设计 `70-推演人生.md` 标「已作废」留着；施工 LF-1 到 LF-15 的施工单、这张表里它们的行是当时的记录，原样留着。

### 留着的（那几天顺带做的通用的东西）

- 会话的上下文上限和 `/context`（设计 68）。只去掉了「人格的 `[life] context` 管人生会话」那一格。
- 会话列表照人最后一次发话排（`last_spoken`，施工 SX-1）。
- 普通循环：会话正忙时到点的等这一轮结束、`/stop` 连循环一起停（施工 LO-2），她没定下一次循环就结束、`/loop clear`、`stop`（施工 LO-3），工具名 `schedule_next`、标签 `<loop-round>`、说「继续」「下一轮」（施工 WK-2）。
- 工具改名表（`miyu_tool::RENAMED`，施工 WK-2）：改名前建的会话在下一轮换到新工具。
- 深的几步在定义处装箱（施工 LF-12 修）：`/life` 那几步没了，新开会话（`Sessions::create`）、命令分发、模板命令、提示词命令照旧装箱，类型 `Heaped` 挪到 `sessions.rs`。守着它的测试原来走 `/life` 那一串，换成「连接、新开会话、敲两条命令」在 1.5 MiB 的栈上走一遍（`tests/command_stack.rs`）。
- `miyu-sandbox` 的 `lifeline`、`miyu-session` 的 `actor/life.rs`（actor 的起停）：名字像，和推演人生无关，没动。

### 怎么拆的

把人生的 21 个提交（LF-1 到 LF-15、LF-6 修、手册那一步 DOC-4）照从新到旧一个个还原，冲突手解；LF-12 修不还原（上面「留着的」）。还原解不了的地方（后来的步子改过同一处，例如 WK-2 改过人生的字、SX-1 在 `life` 旁边加了 `last_spoken`）照编译器和 grep 手清。

中途出过一次错：自动解冲突的脚本遇到「还原那一边整段删」时，把后来别的步子加在旁边的行也一起删了（`last_spoken` 的读写、LO-3 的 `end_unpaced`、LO-1 的自主循环那句、`mod foresee`、人格背景的三个错误码、文档里 `/loop` 的两条）。查法和补法：

1. 把每一次还原在另一棵工作树里回放一遍，列出「还原那边是空的、现在这边比当时多出来的行」，逐条看是不是人生的，不是的补回去。
2. 交叉核对：拆完以后还在的文件里，删掉的每一行应该是人生的提交加的，加回来的每一行应该是人生的提交删的；对不上的列出来逐条看。代码和资源 208 个改过的文件，对不上的只剩有意改的（注释、`Heaped` 挪位置、索引版本）。

### 部署前要清的（旧数据不做兼容）

在有人生数据的数据根上直接起新核心，不会起不来，但有三样会出问题、一样有警告（沙箱里试过，见「做下来的」）。下面的路径相对数据根里的 `root/`。

| 哪里 | 怎么清 | 不清会怎样 |
|---|---|---|
| 人生会话：`sessions/<编号>/`，日志里有 `"kind":"life.started"` 的（`grep -l '"kind":"life.started"' sessions/*/*.jsonl`）；连同 `workspace/<编号>/`；删过、还在回收处的 `trash/sessions/<编号>/` 也照这个认 | 整个目录删掉 | 会话列表里还列着，打开报「会话日志第 N 行坏了」、只读：日志里 `by` 的 `"loop":"life"` 新核心不认 |
| `schedules.db` | `DELETE FROM schedules WHERE loop='life';`<br>`DROP TABLE IF EXISTS doing;`<br>`DROP TABLE IF EXISTS rounds;`<br>`DROP TABLE IF EXISTS life_unseen;` | 人生那一行读不懂：到点跳过，`miyu check` 和运行日志各报一条警告（`bad loop`）。三张小表留着没人读 |
| 每个人格的 `personas/<编号>/persona.toml` | 删掉 `[life]` 这张表（里面只有 `context`） | **这个人格读不出来**（`persona_invalid`：不认识的表 `[life]`），用它的会话照没有人格跑 |
| 每个人格的 `personas/<编号>/prompts/life.md` | 删掉 | 没人读，不报错 |
| 自己写的预设 `presets/*.toml` | 删掉 `[preset]` 里的 `hidden`、`[software]` 里的 `base-life` | 这份预设读不出来（不认识的键 `preset.hidden`） |
| 记忆（`memory/<人格>/*.jsonl`）里人生记下的、「后天」那一类（`"class":"change"`） | 不用动 | 人生会话删了以后，只出自它的记忆不再列出、不再想得起；那几行还在文件里。要连文件里也没有：升级前用旧核心对每个人格调一次 `life.kill`（它照规矩清，记一条 `ext.memory.cleared`） |
| 会话索引 | 不用动 | 版本不对，新核心第一次起来删掉照日志重建，运行日志一条 `session index rebuilt` |

最干净的做法是升级前、旧核心还在跑的时候走它自己的入口：`life.list` 列出来，每个人格 `life.kill`，再把每个人生会话（`session.list` 里带 `life` 的）`session.delete`；旧核心会把定时行、三张小表里的行、记忆、回合索引一起清掉。之后只剩表格里人格文件、预设、回收处、三张空表这几样。两条路都走过。

### 验收

- 守护测试：源码、资源里不再出现这个功能的名字（`crates/miyu-store/tests/removed_life.rs`）；改前红。
- 全仓编译、测试、门禁。
- 实测：带人生旧数据的沙箱照上面的清单清完，新核心起来 `miyu check` 没有问题，旧的普通会话打得开、跑得了一轮；留着的几样（`/loop`、`/stop`、`/context`）照常。

### 做下来的

- 守护测试改前红：同一份名单拿脚本对拆之前的代码扫，979 处、171 个文件；拆完 0 处。
- 实测一（旧数据，2026-11-18，沙箱数据根和家目录，模型地址指到连不上的口、没花请求）：用带人生的旧核心（WK-2 那一版）造旧数据——两段结束了的人生、一段正在跑的（`life.start` 开的，定时表里一行 `loop = life`）、同一个人格的普通会话、人格文件里的 `[life] context`、`prompts/life.md`、一份带 `hidden` 和 `base-life` 的自己写的预设，另外手写了一条「后天」的记忆、`doing` 和 `life_unseen` 各一行。
  1. **不清直接起新核心**：起得来。`miyu check` 报 2 处错误、1 处警告（人格的 `[life]`、预设的 `hidden`、定时表里读不懂的一行）；普通会话打得开、发得出一句（走到请求模型那一步）；三个人生会话都报「会话日志第 4 行坏了」、只读；带 `[life]` 的人格读不出，它的会话照没有人格跑；那条「后天」的记忆照普通的一条列出来（类写着 `change`）。
  2. **只照表格裸删**（不经旧核心）：`miyu check`「没有问题」，会话列表剩 3 个普通会话，那个人格的记忆列出来是空的（出自的会话没了）。
  3. **先走旧核心的入口再清**：`life.kill`、三个人生会话 `session.delete` 以后，定时表 0 行、`rounds`、`doing`、`life_unseen` 都是 0 行，记忆日志多一条 `ext.memory.cleared`；再删人格的 `[life]`、`life.md`、预设里的两格、回收处里那三个会话、三张小表。新核心起来 `miyu check`「没有问题」，三个人格都读得出，旧的普通会话订阅得上、人格是原来的、发得出一句，运行日志里没有 `load failed`、`persona unreadable`。
- 实测二（留着的几样，新的空沙箱，真模型 `opencode-go/deepseek-v4.1-flash`，核心日志里 3 次请求都是它，没遇到 429）：
  1. `/life` → 「命令不存在。」；命令列表里没有它；`life.list` → 「方法不存在。」；`preset.list` 是 `dev`、`full`、`minimal`、`none` 四份，没有 `hidden`；`session.list` 的每一项没有 `life`。
  2. `/context` → 「未设上下文上限，压缩线约 850k。」；`/context 200000` → 「上下文上限设为约 200k，压缩线约 170k。」；再看是设过的数。
  3. `/loop <一句话>` → 「已开始循环：由 AI 定下一次 · …」。定时表里一行「循环，节奏自定」；那句话交进会话（`by` 是 `{"kind":"schedule","schedule":"t1","loop":"dynamic","person":true}`），她调了 `schedule_next`（回执 `Next iteration at 2026-11-18 17:35.`）、回了一个「好」，这一轮照常结束，表里有了下一次。
  4. `/stop` → 「已全部停止。」，定时表空了。
