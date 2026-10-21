## 施工单 E-1 上：认识的人的类和 who

状态：合了（2026-10-21 写、施工，2026-10-22 自己验收；本机门禁八项，三台机器的 CI 全绿）。设计 42（`docs/designs/42-认识的人.md`，项目主人 2026-10-21 提、「推进吧」）第五节第 1 步，PN1、PN2、PN5。编号是 E 线（认识的人）：原来草稿写的是 P-1 上，和人格、预设那条线的 P-1 撞了，2026-10-21 主会话改成 E 线；主会话做的说话人那一格叫 E-0。

### 做什么

她记得自己认识的人，先从记得下开始：记忆多一类 `person`（界面上叫「认识的人」），一条记一件关于某个人的事，带上是谁。人卡（整理时一人合一张，E-1 下）、他一开口就认出来（E-1 补，要主会话的 E-0）不在这一步。

### 改了什么

1. **类和 `who`**：`miyu_recall::CLASSES` 多 `person`（`PERSON`）；`ext.memory.saved` 多可选的 `who`（平台编号 `qq:10086`，没有平台号的是名字；别的类、以前记的没有，读旧的照读）；底账的 `Entry` 跟着有。主人的事照旧 `user`。改一条没给 `who` 的带上旧的，整理改写的照旧的（同类、出处、听众）。
2. **给她看的一行**：`memory_search`、常驻那一块、联想、整理、抽取的「这以后忘掉的」都照 `memory_search/memory.txt` 写一行，`person` 的类后面接是谁（`miyu_recall::shown_class`：`m12 person qq:10086 2026-10-21: …`），模板不改。
3. **她的 `remember`**：说明多「someone else you know」、`class` 的 `enum` 多 `person`、多一格可选的 `who`（「For person: who it is about, their name or an id like qq:10086.」）；新记的 `person` 没写 `who` 的不记，说一句（新加 `remember/no-who.txt`）；别的类写了 `who` 不收；`unknown-class.txt` 列五类。她写的 `who` 照她写的记（群里多半是群名片）：换成平台编号要出处那一轮的说话人，E-1 下整理合卡时照出处认。
4. **抽取**：指令（`extract/instruction.txt`）的类多一样「anyone else you know: who they are, what they like, have or plan, how you met, their birthday or contacts (person)」，`user` 改成「who your owner is」；交回的 `person` 多 `who`（那个人在这一段里的名字）。拼这一段时照日志认出说话的人怎么叫（`Voices::names`：群名片、平台编号对着他的平台编号；主人的 `Owner`、称呼、在群里的名字对着没有），随 `Plan::Ask` 交给抽取任务；读回来以后 `place_people` 把名字换成平台编号，说的是主人的改记成 `user`、不写是谁，对不上的照名字留着。
5. **协议**：`memory.remember` 收 `who`（`person` 必写，没写 `bad_params`；别的类不收）；`memory.update` 多 `who`（`text`、`keywords`、`who` 至少一个；只写 `who` 的照旧文新记一条）；一条记忆多 `who`（没有的是 `null`）；`memory.list` 照 `class: "person"` 列得出。
6. **命令行**：`--class` 多 `person`，多 `--who`（`add`、`edit`）；`add --class person` 没写 `--who` 的命令行这边先拦下，说「认识的人要写 --who（平台编号或名字）。」、退出码 2，不发（不让核心回一句泛泛的「参数错误。」）；`list`、`search` 认识的人那一行正文前面写 `[<是谁>] `；帮助页中英文。

### 不做什么

- 她当场用 `remember` 记的 `who` 照她写的存，群里多半是群名片，不是平台编号：换成平台编号要出处那一轮是谁说的，留给 E-1 下整理合卡时照出处那一轮的说话人换。抽取那一路这一步已经换成平台编号。
- 人卡（一人合一张，E-1 下）、和她说话的那个人一开口就带上他的卡（E-1 补，要主会话的 E-0）。
- 头上的「认识的人」一栏：等 E-1 下的人卡；现在照类（`class: "person"`）筛得出来。
- 命令行只有中文、英文两份帮助页，日文环境照英文印（命令行本来就没有日文）。

### 验收

1. 测试：`miyu-recall` 的 `a_person_says_who_and_other_classes_do_not`（只有 `person` 收 `who`、去空白、空的当没写）；`miyu-session` 的 `names_are_read_from_the_log`、`people_are_placed_by_who_spoke`；`miyu-memory/tests/person.rs` 两条（没写 `who` 不记、说那一句，写了的去空白，别的类不收，改一条没写的照收；搜到的一行接是谁）；`miyu-endpoint/tests/memory_person.rs`（没写 `who` 的 `bad_params`、照类列得出带 `who`、别的类不收、只换 `who` 新记一条、只改正文带过去）；`miyu-cli` 的 `a_person_shows_who`、`tests/memory.rs` 的 `a_person_needs_who`（没写、只写空白的退出码 2、说那一句、什么都不记，写了的照记、列出来带是谁）。原有的测试照过（`Saved`、`Remember`、`FoundMemory` 多一格 `who: None`）。
2. 手写变异十一个，全被逮住：别的类也收 `who`、名字不换平台编号、说主人的不改成 `user`、说话人的名字不记、改一条不带旧的是谁、`remember` 没写是谁照记、别的类的 `who` 照收、协议没写是谁照记、给她看的一行不接是谁、整理改写不带是谁（`memory_merge.rs` 里改正文的那一条换成认识的人，查类、是谁照旧）、命令行不写是谁。`cargo xtask check` 八项；三台 CI。
3. 给模型看的字（`26-提示词.md` 第十节，2026-10-21 照 magpie 网关的 `group/auto-deepseek-v4-1-flash` 量）：`tools/remember.json` 三件一起时的边际 184 → 225（多 41）；`extract/instruction.txt` 337 → 386（多 49，只在抽取那一次辅助请求里）；`remember/unknown-class.txt` 15 → 17；新加 `remember/no-who.txt` 28。记忆三件工具的说明合计 1298 字节，预算照实测加一成改成 1430（`miyu-memory/tests/budget.rs`、设计 10 第九节）。
4. 真模型抽取（仓库外，照出厂的指令和这一段的拼法，同一个网关的模型，三段各跑三次）：群里两个人（一个让她记显卡和搬家、一个说生日，主人说要体检）、私聊里外人自我介绍（认识的经过、爱好、手机号）、主人私聊说起大学室友。九次里认识的人一共出现 12 回，11 回记成 `person`、`who` 照这一段的名字一字不差（对得上平台编号）；1 回记成了 `reference`（这一类不带是谁）；主人的事全记成 `user`、不带是谁。
5. 顺带：`miyu-endpoint` 的 `memory.rs` 过了 500 行，记一条、改一条挪进 `memory/write.rs`；`miyu-session/tests/memory_merge.rs` 过了 500 行，读日志的两个帮手挪进 `tests/support/extracting.rs`。
