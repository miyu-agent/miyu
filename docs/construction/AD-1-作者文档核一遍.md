## AD-1：写包的文档对着现在的核心核一遍

状态：做了（2026-11-12）。起因：已经有人往社区上传软件包了，项目主人要查一遍写包的文档完不完善、规不规范（todolist 第 56 条）。这一步只改文档，不改代码。

### 一、查了什么

- `docs/authors/` 下中英十份：README、manifest、scripts-skills、extensions、mcp。
- 对照的真相源：蓝图 `packages.md`（清单格式、出错、冲突、内容包、依赖）、`crates/miyu-config/src/package/`（`fields.rs` 认的表和格、`mcp.rs`、`capability.rs`）、`crates/miyu-endpoint/src/provide.rs`（扩展登记工具认的格）、`crates/miyu-policy/src/script.rs`（脚本开头认的键）、蓝图 `knowledge.md`「软件包带来的」（K-8）、`personas.md`、`presets.md`、`session/guard.md` 判法表、设计 `11` 的 A16、A17（权限只剩只读、完全放开）、`FL-1-工具一律全量.md`、设计 `48-包的命名.md`；软件源仓库 `miyu-agent/packages` 的 README、`pack.yml`、`collect.yml`、`sources.toml`，收录代码 `crates/miyu-source/src/collect.rs`、`collect/topic.rs`。
- 实际跑：用验收包里的核心（2026.11.0，比 main 旧）和一份临时数据根，照文档从零做包、`miyu pkg pack`、`miyu pkg info <目录>`、`miyu pkg install`、`miyu pkg list`、`miyu kb list`；文档里的每一份清单例子都拿出来读一遍。旧核心读不过的先对照 main 的代码确认是不是文档错。
- 改完跑了 `miyu-config` 里对作者文档的三条测试（`package::fields::tests`：每张表、每一格、每项能力、MCP 每一格都在中英两份里），全过；脚本环境变量那条（`miyu-basesystem` 的 `every_variable_a_script_gets_is_in_the_authors_docs`）照它的名单逐个查过，一个没少。

### 二、问题清单

按严重排。「改了」的中英两份同步改。

**照着做会失败、会被误导的**

| # | 哪一份、哪一处 | 问题 | 怎么办的 |
|---|---|---|---|
| 1 | manifest `[package] version`、README、scripts-skills 的例子清单 | 写的是「可以不写，只给人看」；`miyu pkg pack` 没有版本直接报「清单没有写版本」，软件源收录也拒（`no version`），scripts-skills 的例子清单照抄就打不了包 | 改了：装可以不写，打包、发到软件源必写；例子补上 `version`；README 发版第一步写版本，说清同一个版本重发不算更新 |
| 2 | manifest「一份完整的扩展清单」 | 带了 `[[slash]]`，`capabilities` 没写 `commands`，照抄读不成（`bad_slash`） | 改了：补上 `commands`，注释写明 `[[widget]]`、`[[slash]]` 各要哪项 |
| 3 | mcp、manifest `[[mcp]]` 的 `need_load`，scripts-skills 的 `Need load`，extensions 工具规格的 `need_load` | 都还写成「不写是先加载，省钱」。FL-1 起工具一律全量，这几格照认、不起作用 | 改了：写成「以前的写法，照认、不起作用」；行留着（冻结的格式只加不删，测试也要求 MCP 那张表有这一行），mcp 的例子里删掉这一行 |
| 4 | mcp「问不问人」、extensions「改文件的工具」第 1 条、scripts-skills「怎么跑」倒数第二条 | 还按「工作区」那一级写（「工作区那一级要人确认」「工作区一级下写工作区外的先问人」「在沙盒里跑，沙盒用不了要人确认」）。A16 起只有只读、完全放开 | 改了：照 `session/guard.md` 判法表重写：完全放开（默认）下联网不问、只读时问；只读时写一律拒，数据根哪一级都拒；脚本进不进沙盒指到下面那一节 |
| 5 | 全部 | 知识库包（`kb/<编号>/kb.toml`）、人格包、预设包一个字没写，作者不知道能做 | 加了：scripts-skills 新增「知识库」「人格和预设」两节（目录、`kb.toml` 三格、文档格式和上限、关键词、只读、冲突、怎么验）；README「包能带什么」、目录图、manifest 种类那一段跟着提 |
| 6 | README「发到软件源」 | 只有三句：没给 `release.yml` 怎么写，没说 tag 要等于版本、只看最新的正式 Release、文件名不能改、Release 里一个包读不成整个仓库不收、多久收一次、怎么查没收进来 | 重写成六步：写版本、打包、发（带 `release.yml`、`folder`、`build`、矩阵）、进社区收录（时间、只看最新正式 Release、分叉和存档不收）、编号先到先得（同一趟抢的建得早的得）、怎么查（`pkg update` + `pkg search`，没收的看 collect 的摘要） |

**缺了、不准的**

| # | 哪一份、哪一处 | 问题 | 怎么办的 |
|---|---|---|---|
| 7 | extensions 工具规格表 | `access`、`venues` 是必写（少了整份 `provide` 不收），表里没标；`plugin`、`result` 两格没写 | 改了：标必写，补两行（照参考 `extensions.md`） |
| 8 | mcp、manifest `[[mcp]]` 的 `name`、`venues`、`timeout_ms` | 服务名要小写字母开头没写；`venues` 不写是只给本机、`timeout_ms` 的范围和默认只在例子注释里 | 改了 |
| 9 | mcp「跑起来的样子」 | 说「资源、`sampling` 这些不接」，和上面「资源」一节自相矛盾（资源接了） | 改了：资源接，`sampling` 这些不接 |
| 10 | mcp「说明」一条 | 「工具多半先只给名字和一句说明」，FL-1 以后不对 | 改了 |
| 11 | extensions「被拉起」 | 「她在沙盒里跑的脚本读不到状态目录」，A16 以后本人的会话平时不进沙盒，这句说法不对 | 改了：这几个变量她跑的命令、脚本工具拿不到，文件工具碰不到数据根 |
| 12 | manifest `[command]` | 一个包加几个子命令（`[[command]]`）没写 | 加了 |
| 13 | README 编号那一条 | 例子用的 `dice` 官方源里已经有了，照抄发出去不收；没教起名前怎么查 | 加了：起名前 `miyu pkg search <词>`，说明例子只是例子 |
| 14 | README「装、试、查」 | 没有「装之前看一眼」（`miyu pkg info ./目录`）、看知识库的 `miyu kb list`；试完不停沙箱核心会留一个核心在跑；没有常见报错 | 加了：看一眼一步、`kb list`、`core stop`；新增「常见报错」一节（照这次实际撞到的几条写） |

**规范**

| # | 哪一份、哪一处 | 问题 | 怎么办的 |
|---|---|---|---|
| 15 | README「让核心一直开着」、scripts-skills 脚本包的配置项 | 写着施工号（Z-1、PS-1）、设计文档路径，作者看不懂（仓库规矩：作者文档不写施工号） | 改了：去掉 |
| 16 | README 同一处 | 让作者照 `packaging/README.md` 手装服务；有现成的 `miyu service enable` | 改了 |
| 17 | manifest 能力表 `fs.write` | 「扩展进沙盒以后照它放行」读着像扩展现在在沙盒里；扩展进沙盒（设计 62）暂缓 | 改了：现在扩展不在沙盒里，程序自己写文件不经这一项 |
| 18 | manifest `upstream` | 至多 300 个字、不带空白没写（`bad_upstream`） | 加了 |
| 19 | scripts-skills 开头的注释 | `//` 开头的注释行也认，没写 | 加了 |
| 20 | README「装、试、查」的「卸」 | 写「设置里它的项一起删」，和同一页上面第 5 条（设置项、配置目录默认留着，`--purge` 才删，PK-5补）前后矛盾 | 改了：照第 5 条写，带上 `--purge` 的写法 |

**中英一致**：十份逐节对过，节、表格行、代码块数两边一样；这次改的每一处两边同时改。

### 三、照文档从零做三个包

数据根是临时目录，核心用验收包里的那一份。

1. **脚本包 `dice`**：照 scripts-skills 的目录图和 `dice.py` 例子建好，`miyu pkg pack ./dice` 报「无法打包：清单没有写版本」——**卡在这里**（问题 1）。补 `version` 以后打包、`miyu pkg info ./dice`（提供：脚本 dice）、`miyu pkg install ./dice --yes`、`miyu pkg list` 都对，`miyu check` 没报错。另外 `miyu pkg info dice` 在没装时查的是官方源，那里已经有一个 `dice`（问题 13）。
2. **技能包 `notes`**：照例子的 `SKILL.md`，同样先卡在版本；补上以后从 `.miyu` 文件装成，`miyu pkg files notes` 列出 `skills/take-notes/SKILL.md`。
3. **知识库包 `lighthouse`**：原文档没有写法，**无从下手**（问题 5）；照蓝图 K-8 建 `kb/lighthouse/kb.toml`（`name`、`description`、`auto`）和一篇带「关键词：」的 Markdown，打包、装都成，`miyu kb list` 显示「来自 灯塔设定集、自动想起」。只是 `miyu pkg info ./lighthouse` 的「提供」一行没有知识库（见第四节 A）。

顺带：人格包（`personas/keeper/`）装上，「提供：人格 keeper」；extensions 里手写的 `hello` 扩展照抄装上，状态 `running`；完整扩展清单修了第 2 条以后读得成；MCP 包、外观包、内容包，manifest 里配置项（一组字段的列表、分节、`choices_from`）、`[features]`、`[guard]`、`[after]`、`[[side]]`、`[[driver]]`、`[depends]` 的例子拼在一起都读得成。

验收用的旧核心不认 `applies = "restart"`（CPU-1 在它之后），main 的代码认，文档不用改；2026.11.0 还没正式发版，清单的格暂时不用标「从哪一版起」。

### 四、没改的、留给主人拍板的

不是作者文档的事，记下来：

- **A. 「提供」不列知识库**：`package.list`、`package.info` 的 `provides`（PV-2）种类里没有知识库，装之前看一眼、「已安装」都看不出包带了库。推荐：加上 `knowledge` 一种（技术细节，可直接排给核心）。
- **B. 旧的环境变量名**：核心照旧给 `MIYU_SCRIPT_CACHE_DIR`、`MIYU_PACKAGE_DIR` 这些，文档写「下一版去掉」。照「发版前不做旧兼容」，发版前清兼容代码时一起删，文档跟着去掉这一句。
- **C. 蓝图没跟上 A16**：`blueprint/mcp.md`「她看到的工具」第 3 条、`session/guard.md` 的判法表还写着「工作区」那一列。不在这一步改，交给管权限的那一步。
- **D. 前缀长度**：设计 48 第二节写前缀不超过 32 个字符，代码照包编号的写法（64）。作者文档照代码写「写法同包的编号」；以哪个为准要定（推荐照代码，改设计那一句）。

产品取舍，请主人定：

1. **社区包要不要写许可证**：清单没有许可证这一格，收录不查。推荐：先不强制，作者文档建议仓库里放 LICENSE；以后界面要显示了再加 `[package] license`（格式只加不改，加得进）。
2. **要不要给作者一个模板仓库**（GitHub template：`package.toml`、`release.yml`、LICENSE、一个脚本例子）。推荐：做，成本低，作者照着点一下就有能发的仓库。
3. **打包时要不要查给模型看的字**（技能 `description`、知识库名字说明是不是英文、长不长）：现在只在文档里说。推荐：先不查，等真有包写得太长再说。
