## 施工单 MI-1：记忆导入

状态：做完，等合（2026-11-04）。设计 `17-记忆.md` 第九节、L19；蓝图 `memory.md`「协议」、记忆日志、第二条，`protocol.md` 的 `memory.*` 第 11 条；参考 `reference/memory.md`、`.en.md`。项目主人 2026-11-03 定旧版到新版的迁移「知识点转、其余存档」，迁移程序是单独的仓库、官方源里的一个包，不进核心；记忆线起草 `memory.import`，core1 审过（11-03）。

### 目的

旧版 Miyu 的知识点（default 人格 122 条、dev 人格 39 条）要转成新版的记忆，保住原来记下的时刻。`memory.remember` 不收时间、没有去重，迁移包只能经协议进来，不能离线写记忆日志（日志格式、序号、分段、锁、记忆库都是核心的事）。

### 做了什么

- 协议 `memory.import`（`miyu-endpoint` `memory/import.rs`）：`items` 1 到 500 条，每条 `at`（RFC 3339 带时区，不晚于现在）、`class`、`text`、`origin` 必写，`who`（`person` 必写，别的类不收）、`people`（只有 `episode`）、`keywords` 照类写。先全查后写：一条不对整批不写，`bad_params` 的 `data` 写 `{index, field}`（整批的毛病只有 `field`）。回 `{ids, imported, skipped}`，`ids` 和 `items` 一一对上，跳过的是 `null`。权利同 `memory.remember`（`memory.write`）。`methods-v1.txt` 加一行（只加不改）。
- `miyu-recall` `Saved` 加 `noted`（原来记下的时刻）、`origin`（在原来那边是哪一条）两格，没有的不写，以前的日志照读（代码里是平铺的一个 `Option<Box<Origin>>`：两格放在外面，`MemoryEvent` 里这一种比别的大出一截，clippy 的 `large_enum_variant`）；底账一条的 `at` 照 `noted`，没有的照事件的时刻；底账另收导进来过的 `origin`（作废、改掉、清掉了的也在），`MemoryBook::imported`。`miyu-store` 记忆库那一行的时刻也照 `noted`（同 `put_of`）。
- `miyu-session` `Keeper::import`（`memory/keeper/import.rs`）：照先后一条一个 `ext.memory.saved`，导进来过的 `origin` 跳过；`by` 照人、出处空、听众是主人（同人记的）；`person` 说的是她自己的同 `save` 记成 `episode`。`Keeper::settle_imported`：回应以后在后台写关键词、补向量，再照 E-9 起一次后台整理的判断（从没整理过的马上整理，整理过的满 `memory.merge_every` 才整理）；核心没交整理要的几样的只补向量。
- 抽取记下以后写关键词、补向量那一段挪进 `memory/landed.rs`（`Landed::settle`），抽取和导入共用；写关键词一次最多 20 条（`tag::BATCH`），多的接着分几次（抽取一次很少超过 20 条，原来是一次全交）。
- 文档：设计 17 第九节加一行、L19；蓝图 `memory.md`（文件表、事件表、`noted`/`origin`、协议表、「人记的」后面一段、测试表）、`protocol.md`（方法索引、表、第 11 条，十一个改十二个）、参考两份；设计 `04-核心协议.md` 的方法表、扩展的权利表（蓝图 `extensions.md`、参考 `extensions.md`、`.en.md`）也列上 `memory.import`。

### 不做什么

- 不认命令编号（同 `memory.dream`）：同一个 `id` 再发照 `origin` 全跳过，回 `imported` 0。多条事件共用一个编号，底账的「一个编号一个结果」装不下；`origin` 去重已经够重跑。
- 不收 `sources`、`replaces`、`venue`、`place`；不给她的工具和斜杠命令。
- 不加 `merge` 开关：要马上去重的，迁移包接着调 `memory.dream`（core1 11-03）。
- 不放宽 120 字：超长的不导，迁移包列进报告（主人 11-03 定的迁移总原则）。
- 整理改写导进来的一条，新版本的时刻是改写的那一刻（同别的记忆）；正文按规矩把日期写全。

### 验收

1. `crates/miyu-endpoint/tests/memory_import.rs`（真核心）：
   - `imported_memories_keep_their_time_in_order`：三条照先后编号，`at` 是原来的（带时区的换成 UTC），`by` 是人、出处空，正文、是谁去掉前后空白，关键词、涉及谁带上，照关键词搜得到。
   - `one_wrong_item_writes_none_and_says_which`：十六种不对的（空的、501 条、不是对象、缺 `at`、写法不对、没时区、将来的、类不认识、空白正文、121 字、`person` 没 `who`、别的类写 `who`、别的类写 `people`、关键词 201 字、空的 `origin`、一批里重复的 `origin`）都回 `bad_params`、`data` 对，一条都没记；120 字记得下。
   - `the_same_origin_is_never_imported_twice`：同一个命令编号再发全跳过；新旧混着的只记新的；作废、整间清空以后再导照样跳过。
   - `after_an_import_keywords_are_written_and_a_first_merge_runs`（请求模型是真路由）：没带关键词的另发一次写上，从没整理过的接着整理一次；带了关键词的不发、刚整理过的不整理；再导一条没带的只发写关键词那一次。
2. `crates/miyu-recall/src/memory/tests/imported.rs`：`noted`、`origin` 读写得回、没有的不写、以前的日志照读；底账的时刻照 `noted`；作废、改掉、清空以后 `origin` 还记着。`crates/miyu-store/tests/memory_imported.rs` 的 `an_imported_memory_is_indexed_at_when_it_was_noted`：记忆库那一行的时刻照 `noted`，删了重开照底账补回来的也是。
3. `crates/miyu-session/tests/memory_tagging.rs` 的 `an_import_is_tagged_twenty_at_a_time_then_merged`：25 条分两次写关键词（20、5），接着整理一次，整理落完补一次。
4. 原有的写关键词、抽取、整理测试照旧全过（抽取走 `Landed`）；`robust_methods.rs` 的方法表加上 `memory.import`，乱给参数照样回应、不断连接。
5. 手写变异（只跑相关测试）都红，见提交说明。
6. `cargo xtask check`（门禁 `gate.sh`），三平台 CI。
