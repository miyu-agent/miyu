## 知识库

### 是什么

你放给她查的文档（`19-知识库.md`）：一个知识库是一个目录，里面是文档，外加一份派生的索引（N1）。不是记忆（记忆是她自己从对话里记下的，`memory.md`）。三种来源：你自己的、挂进来的现成目录（K-4，只读，N7）、软件包带来的（K-8，只读，N1、N6）。

- 知识库是你的图书馆：全部她都能查，人格可以收窄（N10、N11，2026-10-12 项目主人定）；群里、别人的私聊里也查得到（N12，同一天定：和记忆不一样，不照「谁在听」挡）。
- 检索用记忆那一套底子（`recall.md`，N3、N8）：关键词一路（K-1 上）、照意思那一路（K-2），两路照名次合。
- 仿 Linux：文档就是文件（19 第二节）。真相是目录里的文件，索引随时能删掉照文件重建（`07-存储.md` S1、S3）。

状态：图纸（2026-10-12 起草，照 `19-知识库.md` N1 到 N14、`docs/reviews/2026-10-07-记忆知识库embedding调研.md` 第五节第 7 条）。每一节标着由哪一步做，步子见末尾「施工步子」。做完一步，这一页照做好的样子改写那几节。做好了的：K-1 上（底子、协议）、K-1 下（她的两件工具、人格收窄、知识库这个包）、K-1 补（system 里列出来的那一块）、K-3（她写知识库）、K-2（照意思找）、K-4（挂进来的目录）、K-5（PDF 和网页）、K-7 上（人经协议管理）、K-7 下（命令行 `miyu kb`，`cli/kb.md`）、K-6 上（自动想起的测评集，`knowledge/eval.md`）、K-6 下（自动想起）、K-8（软件包带来的知识库）、K-10（列出库里的文档）。

### 在哪

| 代码 | 管什么 | 哪一步 |
|---|---|---|
| `crates/miyu-recall/src/chunk.rs` | 切块（纯逻辑）：Markdown 照标题、纯文本照段落，每块封顶（「怎么走」第二条） | K-1 上 |
| `crates/miyu-store/src/recall/stamps.rs` | 检索库里每个来源的指纹、整份换掉一个来源（「怎么走」第三条） | K-1 上 |
| `crates/miyu-store/src/knowledge/skip_stamp.rs` | 跳过的文档记的指纹：写法、认回来、库的文档数（「怎么走」第三条第 4 款） | W-15 再补 |
| `crates/miyu-store/src/knowledge.rs` | 知识库：列出全部、读 `kb.toml`、开索引、照文件补、搜（K-2 起关键词、照意思两路合，`Meaning`、`index_of`）、读文档 | K-1 上 |
| `crates/miyu-endpoint/src/knowledge.rs`、`refusal/knowledge.rs` | 协议 `kb.*`：读参数、在阻塞线程里调、写回应；`unknown_kb`、`unknown_document` | K-1 上 |
| `crates/miyu-tool/src/knowledge.rs` | 两件工具的名字、最多给几块、知识库的端口 `KnowledgePort` 和它交回的几样；`Call` 多一格 `knowledge` | K-1 下 |
| `crates/miyu-knowledge/` | 知识库这个软件包的五件工具 `kb_search`、`kb_read`（K-1 下）、`kb_save`、`kb_edit`、`kb_delete`（K-3）（第 3 层，同 `miyu-memory`）：读参数、照端口交回的写给她看的字、报效果 | K-1 下 |
| `crates/miyu-store/src/knowledge/mount.rs` | 库的文档在哪：自己的目录，还是挂进来的（`mount` 换成读得了的目录，`~/` 照核心的家目录），还是包里的（K-8）；写的三件照它拒挂进来的、包里的（「对外的样子」、第三条第 6 款） | K-4 |
| `crates/miyu-store/src/knowledge/place.rs` | 一个库在哪（K-8 从 `knowledge.rs` 挪出来）：看得见哪几个（自己的加包里的、撞名谁赢）、索引放哪、包里的库人改过的开关、装卸以后跟上（第一条、第十条） | K-8 |
| `resources/software/knowledge/common/packaged.txt` | 她写到软件包带来的库时那一句（26 第十节登记） | K-8 |
| `crates/miyu-store/src/knowledge/extract.rs` | 文档是哪一种、上限、哪几种写得了；文字的照原样读，PDF、网页拉起 `miyu-extract` 抽、等它、卡住了杀掉；主程序旁边找它（`extractor_beside`）（第二条第 5、6 款） | K-5 |
| `crates/miyu-extract/`（`src/lib.rs`、`radicals.rs`、`main.rs`，`data/EquivalentUnifiedIdeograph.txt`） | 抽字的小程序：一次抽一份 PDF 或网页，照 JSON 交回去就退；PDF 一页一页抽、部首的码位换回平常的字（Unicode 的对照表原样带着），网页换成文字（第二条第 5、6 款） | K-5 |
| `crates/miyu-endpoint/src/knowledge.rs`（`shelf`、`extractor`、`Core::with_extractor`） | 核心一份的知识库带上家目录、主程序旁边的 `miyu-extract`（测试里换成 cargo 编出来的那个）、照资源目录读的搜的下限（K-2 修） | K-5 |
| `resources/core/knowledge/search.toml`、`Knowledge::with_floors`、`floor`，`miyu_recall::fuse_above` | 搜的时候只照意思找到的那一路的下限，照模型（第四条）；数据，不给模型看 | K-2 修 |
| `crates/miyu-session/src/knowledge/recall.rs`、`resources/core/knowledge/recall.toml`、`resources/software/knowledge/recall/`，`crates/miyu-session/src/actor/model.rs`（`hooks`、`recalling`） | 自动想起：挑、拼那一块、门槛表、外壳；回合开始和记忆的挂接点并着问（第九条）；`Knowledge::sections`、`auto` 列出一个库的每一段、开没开 | K-6 下 |
| `crates/miyu-recall/src/kb/keys.rs`、`resources/core/knowledge/keys.toml` | 小节的关键词那一行（K-9）：写法表（开头、冒号、分隔，数据）、认那一行、分开、交回那一行以后的正文（「对外的样子」、第九条第 3、4 款） | K-9 |
| `crates/miyu-store/src/knowledge/manage.rs` | 人管理：建库、改名字说明、挂目录、加文档、删库、重建索引（第八条）；`RecallIndex::discard`（关上、删掉索引） | K-7 上 |
| `crates/miyu-cli/src/kb.rs`（详见 `cli/kb.md`） | 命令行 `miyu kb`：照子命令发 `kb.*`，加文件照 `blob.put` 本机路径 | K-7 下 |
| `crates/miyu-endpoint/src/knowledge/manage.rs`、`refusal/knowledge.rs` | 协议管理的七个方法：读参数、在阻塞线程里调、写回应；`kb_exists`、`document_exists`、`kb_read_only`、`folder_unavailable`、`trash_failed` | K-7 上 |
| `crates/miyu-store/src/knowledge/listing.rs`、`crates/miyu-endpoint/src/knowledge.rs` 的 `kb.documents` | 列出一个库里的文档（K-10）：照补的时候列出来的、索引里的块数、这一回跳过的哪一种（`Reason`）和为什么、读不了的子目录 | K-10 |
| `crates/miyu-store/src/knowledge/write.rs` | 她写知识库那一层：建库、新建、追加、照原文换一段、挪进回收站，交回改前改后的内容（第七条） | K-3 |
| `crates/miyu-session/src/knowledge/write.rs` | 写哪几个、建得了建不了新库（第七条第 1 款） | K-3 |
| `crates/miyu-session/src/memory/vectors.rs`（`fill_knowledge`） | 在后台补一个知识库的索引缺的向量（第四条，同记忆的回合库） | K-2 |
| `crates/miyu-session/src/knowledge.rs` | 一个会话查哪几个库（人格收窄）、给不给两件（`Shelves::listed` 是不是空的）、端口的实现；system 里列出来的那一块（`listing`，K-1 补） | K-1 下 |
| `crates/miyu-policy/src/compose.rs`（`Snapshot::with_knowledge`）、`snapshot.rs`（`knowledge`） | 那一块拼进 system 第 4 块、记进快照（K-1 补） | K-1 补 |
| `crates/miyu-session/src/agents.rs`（`Holds`）、`actor/persona.rs` | 工具面照查不查得到给两件；回合开始照这一刻再判，变了换快照（第六条） | K-1 下 |
| `crates/miyu-policy/src/persona/tables.rs` | `persona.toml` 的 `[knowledge] bases`（`personas.md`） | K-1 下 |
| `resources/packages/miyu-base-knowledge/package.toml`、`resources/software/knowledge/` | 内置包的清单（没写功能，整个包算一个，编号 `knowledge`）；两件工具的说明、给她看的几句（进登记簿）、给人看的字 | K-1 下 |

### 对外的样子

**一个知识库**（K-1 上）：数据根里 `kb/<编号>/`（`07-存储.md` 第二节），编号就是目录名，写法同人格的编号（小写字母开头，只有小写字母、数字、`-`、`_`，最多 64 个字符），不合写法的目录不算。目录里：

- `kb.toml`（可以没有）：

  ```toml
  name = "日语笔记"          # 给人看的名字，没写的照编号
  description = "学日语的笔记：语法、单词、例句"   # 一句话说这个库是什么，没写的空着；随 K-1 下进她的系统提示词
  ```

  别的格不认（读进来照规矩报错，`kb.list` 里这个库带 `error`，照没有 `kb.toml` 的样子用）。
- `auto = true`（K-6 下，19 N5）：这个库自动想起（「怎么走」第九条）；不写、写 `false` 的是关（默认关）。
- 挂进来的（K-4，19 N1、N7）：`kb.toml` 多写一格 `mount`，库的文档就是那个现成目录里的，库目录里只认 `kb.toml`：

  ```toml
  name = "笔记"
  mount = "~/Documents/notes"   # 绝对路径，或者 ~/ 开头（照核心的家目录）
  ```

  只读：她的写的三件写不进（「她的工具」）。挂的目录用不了的见「怎么走」第三条第 6 款。编号、名字、说明、索引的位置、人格收窄、system 里列出来的样子都和自己的一样。人经协议的 `kb.mount` 随 K-7，在那以前手写 `kb.toml`。
- 文档：扩展名是 `.md`、`.markdown`、`.txt`、`.pdf`、`.html`、`.htm`（K-5，19 N2）的文件，子目录里的照样算；以点开头的文件、目录、链接不算（挂的那个目录本身是链接的照它指的地方）；`kb.toml` 不算。别的格式（Word、EPUB）不认。文字的超过 4 MiB、不是 UTF-8 的不收；PDF、网页的文件超过 64 MiB、抽出来的字超过 4 MiB、抽不出来的（坏的、加了密的、一个字都没有的扫描件）不收；都记一行运行日志（「出错」）。
- 关键词（K-9，19 N9）：Markdown、网页的一节，标题下面正文的头一行（空行不算）照 `关键词：` 开头的，后面用 `、`、`，`、`,`、`；`、`;` 分开，每个是这一节的一个别名，开了自动想起的库里提到哪一个都带这一节（「怎么走」第九条）。开头认 `关键词`、`keys`、`キーワード`，不分大小写，冒号中英文都行（`resources/core/knowledge/keys.toml`，数据）：

  ```markdown
  ## 艾琳
  关键词：守灯人、灯塔的姑娘、Eileen

  守灯人，最怕打雷。
  ```

**软件包带来的**（K-8，19 N1、N6；项目主人 2026-10-13 定只读、自动想起人能开关、卸包以后跟着没有、列出来标「随 <包名>」）：装了的软件包（清单读成了的）目录里的 `kb/<编号>/`，照约定带（同 `skills/`，清单里不写，`packages.md`），里面 `kb.toml`（`name`、`description`、`auto`，同自己的；不认 `mount`）和文档。

- 都看得见；人格收窄照编号。同编号的自己的（连挂进来的）赢，包里的那一个不出来；两个包带了同编号的，出厂的先于装上的、同一层照包的编号先到先得（同技能；施工 F-8 六补：装的时候撞了的装不上，撞得上的只剩核心升级后出厂的新带了同编号的，`packages.md`「冲突」）。要建、要挂的编号和看得见的包里的撞了算已经有了（`kb_exists`）：自己的盖住包里的，只在装包以前就有同编号的库时出现。
- 只读：名字、说明、文档照包。她的写的三件拒（「她的工具」）；人改名字、说明、加删文档、删库拒 `kb_packaged`；重建索引照做（索引是我们的）。
- 自动想起的开关先照包里 `kb.toml` 的 `auto`；人改了的（`kb.update` 只收 `auto`）记在数据根的 `kb.toml`：`[auto]` 下一行 `"<包>/<编号>" = true/false`，照 `miyu_config::edit` 改，注释和别的照原样。
- 索引在 `index/kb/packages/<包>/<编号>.db`，照包目录补（同挂进来的）。
- 装卸（「怎么走」第十条）：核心起来时照读成的清单找；装、卸以后当场跟上，卸掉的包它的索引、开关一起删。

**索引**（K-1 上）：`index/kb/<编号>.db`，检索库（`recall.md`「库的结构」），派生的。

- 一块一行：键是 `<相对路径>#<第几块>`（路径照 `/` 分，块从 0 起），字是这一块（带着它的标题路径，第二条），时刻是文档改的时刻。
- 每个文档一个指纹：大小加改的时刻（毫秒），记在检索库的 `stamps(source, stamp)` 里（`recall.md`，K-1 上加的一张），来源是相对路径。指纹没变的不重切。

**协议**（K-1 上；原文写法见 `protocol.md`「`kb.*`」）：人经协议查，给知识库页、「试着搜一下」用。不管知识库这个软件包装没装都能用：文件是人的，人随时看得到（她的工具、系统提示词里的那一段随 K-1 下照包开关）。

| 方法 | 类型 | 参数 | 回应 |
|---|---|---|---|
| `kb.list` | 查询 | 没有 | `{"bases": [<一个库>…]}`，照编号排；一个库多 `indexing`、建好过的多 `indexed_at`（施工 W-15，下面「建索引的进度」） |
| `kb.search` | 查询 | `query`（不能是空白）；`kb`（只搜这一个，可以不写）、`limit`（1 到 50，不写 10） | `{"hits": [<一块>…]}`，最相关的在前 |
| `kb.documents` | 查询 | `kb`（K-10） | `{"documents": [<一篇>…], "folders": [<读不了的子目录>…]}`，照路径排 |
| `kb.read` | 查询 | `kb`、`path`（文档的相对路径）；`page`（从 1 起，不写 1） | `{"text", "page", "pages"}`：文档原文的那一页；PDF 是它的第几页抽出来的字、网页是换出来的字（K-5） |
| `kb.create` | 命令 | `kb`；`name`、`description`、`auto`（可以不写）（K-7 上；`auto` K-6 下） | 一个库 |
| `kb.update` | 命令 | `kb`；`name`、`description`、`auto`（至少写一个，空字符串、`false` 是拿掉；软件包带来的只收 `auto`，K-8） | 一个库 |
| `kb.mount` | 命令 | `kb`、`folder`（绝对路径或 `~/` 开头）；`name`、`description`、`auto` | 一个库（照挂的目录补完的） |
| `kb.delete` | 命令 | `kb` | `{"trash"}`：整个库目录在回收站里的位置 |
| `kb.add` | 命令 | `kb`、`path`、`blob`（`blob.put`、分块上传交回的）；`replace`（已经有了的换掉，不写 `false`） | `{}` |
| `kb.remove` | 命令 | `kb`，`path` 一样或者 `paths` 几样（施工 W-14：一篇文档或者一个文件夹，文件夹整个删） | `path` 的回 `{"trash"}`：在回收站里的位置；`paths` 的回 `{"trash": [...]}`，照先后。先全查一遍，有一样不在就一样都不删；两样都写或都没写、重了、一样在另一样里面的 `bad_params` |
| `kb.reindex` | 命令 | `kb` | 一个库（删掉照文件重建完的） |

- 一个库：`{"id", "name", "description", "documents", "chunks", "auto"}`（`auto` 是自动想起开着没有，K-6 下；软件包带来的人改过的照改的，K-8），软件包带来的多一格 `package`（包的编号，K-8），挂进来的多一格 `mount`（`kb.toml` 里写的目录，照原样、`~/` 不展开，K-10），读 `kb.toml` 出错的、挂的目录用不了的（K-4）多一格 `error`（英文的一句为什么）。`documents`、`chunks` 是照文件补完以后的数（挂的目录用不了的是索引里现有的）。
- 一篇（K-10，网页的知识库页列文档用；2026-10-19 项目主人提出缺知识库的后台页面）：`{"path", "kind", "size", "at", "chunks"}`：库里的相对路径；`markdown`、`text`、`pdf`、`html`；文件的字节数；文件改的时刻；索引里几块。这一回补的时候读不了的多 `reason`、`why`（英文的一句，同运行日志），读不了的以前进过索引的 `chunks` 是留着的旧块。`reason`：`too_large`（文件或抽出来的字超过上限）、`not_utf8`、`no_text`（PDF 里没有字）、`no_extractor`（PDF、网页，没有抽字的小程序）、`extract_failed`（抽字的出错、超时）、`unreadable`（读不了的文件）。读不了的子目录（挂进来的常有）：`{"path", "reason": "unreadable", "why"}`。列的和补的时候看的一样：不是文档的、点开头的、链接、`kb.toml` 不列。不分页（3000 篇的回应几百 KiB，补那一下本来就全扫）。挂的目录用不了的拒 `folder_unavailable`。
- 一块：`{"kb", "path", "chunk", "heading", "text", "at"}`：哪个库、哪个文档、第几块、标题路径（没有标题的是空的）、这一块的字（不带标题那一行）、文档改的时刻。PDF 的那一块多一格 `page`（第几页，从 1 起，K-5），别的不写这一格。
- 一页 32 KiB，截在字的边界上；`pages` 是一共几页，空文档是一页空的。PDF 照它自己的页：`pages` 是 PDF 的页数（K-5）。
- 出错：`unknown_kb`（没有这个库）、`unknown_document`（库里没有这个文档：不是文档的、不在库里的、`..` 跑出去的都算）、`bad_params`（`query` 空白、`limit` 出范围、编号不合写法、`page` 是 0 或超过 `pages`）。
- 管理的出错（K-7 上，「怎么走」第八条）：`kb_exists`（要建、要挂的库已经有了）、`document_exists`（要加的已经有了，没写 `replace`）、`kb_read_only`（往挂进来的库里加、删）、`kb_packaged`（改软件包带来的库的名字、说明，加、删文档，删它，K-8）、`folder_unavailable`（要挂的目录不在、读不了、不是绝对路径也不是 `~/` 开头，`data.message` 一句英文）、`trash_failed`（挪不进回收站，`data.message`）、`unknown_attachment`（`blob` 不在，同 `session.send`）；`bad_params`：编号、路径不合写法（`kb.add`、`kb.remove` 的路径照「文档」的写法，什么格式都行）、`kb.update` 两格都没写、`kb.mount` 没写 `folder` 或别的写了 `folder`、`kb.add` 没写 `blob` 或别的写了 `blob`、`replace`、多写了不认识的格。

**她的工具**（K-1 下、K-3，19 N4、N14）：知识库这个内置包（`miyu-base-knowledge`，功能编号 `knowledge`，出厂装着；预设照功能开关：全部功能开、常规开发关）带五件；查的两件访问类别 `read`，写的三件 `write`（只读级别下不能用）。

| 工具 | 做什么 | 参数 |
|---|---|---|
| `kb_search` | 照关键词搜她查得到的库 | `query`；`kb`（只搜这一个，可以不写） |
| `kb_read` | 读一个文档的一页 | `kb`、`path`；`page`（从 1 起，不写 1） |
| `kb_save` | 新建一篇，或者接在一篇后面；写到还没有的库就是建它（K-3） | `kb`、`path`、`text`；`append`（接在后面，没有就新建）、`about`（建库时这个库是什么，一句） |
| `kb_edit` | 照原文换一段（K-3，同 `edit`） | `kb`、`path`、`old`、`new` |
| `kb_delete` | 整篇挪进系统回收站（K-3，同 `trash`） | `kb`、`path` |

- `kb_search` 最多 5 块，几个库照名次轮流合（同 `kb.search`）。一块一段：头一行 `[<库>] <路径>: <标题路径>`（没有标题的 `[<库>] <路径>`，PDF 的 `[<库>] <路径>, page <页>`（K-5），照模板，换进去的转义），下面是这一块的字原样（文档是数据，同 `read` 交回文件的内容，末尾的换行去掉），段和段之间空一行。什么都没搜到的说一句，不算出错。
- `kb_read` 一页 32 KiB（同 `kb.read`；PDF 是它的第几页，K-5），交回那一页的字原样；不止一页的末尾另起一行 `(Page 2 of 3.)`。
- 给头看的那一份（`tool.result` 的 `detail`，施工 W-11）：`kb_search` 交 `{"hits": [{"kb", "path", "heading"?, "page"?, "snippet"}]}`（没有标题的、不是 PDF 的不写那一格，`snippet` 是那一块的字、末尾的换行去掉；没搜到的空列表）；`kb_read` 交 `{"kb", "path", "page", "pages", "text"}`，`text` 是这一页的正文，不带 `(Page 2 of 3.)` 那一行。出错的不交，头照 `human` 画。写的三件不交，头照参数画。
- 写的三件做成了说一句（新建了、接上了、建了库写了、改好了、挪进回收站了），报效果：`kb_save`、`kb_edit` 报「改了文件」（改前改后的内容），`kb_delete` 报「挪进回收站」，撤销那一轮时改回去、移回来（同 `write`、`trash`）。
- 出错如实说，算出错：没有这个库（连同人格没列的，不让她知道有）、库里没有这个文档、没有这一页、读不了、参数不对；新建的已经有了（说用 `append` 或 `kb_edit`）、写完太大、要换的原文找不到或不止一处、挪不进回收站（K-3）、要建的库编号不合写法（说清编号怎么写，K-3 补：R-9 真跑时她把编号写成中文，听到「没有这个库」又试了两次）、写到挂进来的库（说它只读、写到别的库里，K-4）、写 PDF、网页（说只有 `.md`、`.markdown`、`.txt` 写得了，K-5）；这个会话没有知识库的端口（核心没交）的也说一句。
- 原文见 `prompts.md`「`software/knowledge/`」，登记在 `26-提示词.md` 第十节。
- 系统提示词里列出她查得到哪几个库：见下一段（K-1 补）。

**列出来的那一块**（K-1 补，19 第三节）：工具面上有知识库那两件的会话，system 第 4 块（核心的几行后面、预设那一行前面，`26-提示词.md` 第四节）列出她查得到的库，先后照「怎么走」第六条第 1 款：

```
Knowledge bases you can search:
- notes: 日语笔记 — 学日语的笔记：语法、单词、例句
- world: 世界观
- plain
```

- 一个库一行：写了说明的 `- <编号>: <名字> — <说明>`，只写了名字的 `- <编号>: <名字>`，都没写（名字就是编号）的 `- <编号>`。名字、说明照模板转义（`<` 写成 `\u003c`）。`kb.toml` 写错的照没写的列。
- 最多 20 个，多的最后一行 `- and <几> more`，`kb_search` 照样搜得到。
- 记进快照（`knowledge`，这一块拼好的原文；没有的不写，以前的快照、没有库的会话 system 一字不变）。回合开始照这一刻再拼，和快照里的不一样就换快照（建了库、改了名字说明、收窄变了），缓存断一次。
- 为什么加：K-1 下量了，不列她只有 7/20 想起去搜，列了 19/20，无关的两种都不乱搜；项目主人 2026-10-12 看了数定加。原文见 `prompts.md`「`software/knowledge/`」。

**人格收窄**（K-1 下，19 N11）：`persona.toml` 的 `[knowledge] bases = ["notes", …]`（`personas.md`）：用这个人格的会话只查得到列着的、在的那几个；空的列表一个都查不到。不写的查全部。

### 怎么走

**一、哪些库**（K-1 上）：`kb/` 下每一个合写法的目录是一个库，加上装了的软件包带来的（K-8，「对外的样子」那一节；同编号的自己的赢），照编号排。目录不在的当一个都没有。库的文档在它自己的目录里；`kb.toml` 写了 `mount` 的在挂的那个目录里（K-4）；包里的在包目录里（K-8）。做法在 `knowledge/place.rs`。

**二、切块**（K-1 上，`miyu_recall::chunk`，调研第五节第 7 条：200～400 token 一块）：

1. Markdown 照标题切：`#` 到 `######` 开头的一行起一节，一节一块；每块的标题路径是它上面各级标题照层级连起来，中间是 ` › `（`语法 › て形`；K-1 下从 ` > ` 改过来：给她看的字段一律转义，`>` 会写成 `\u003e`），写在这一块的字的头一行（`# 语法 › て形`，没有标题的不写）。代码块（三个反引号起止）里的 `#` 不算标题。
2. 纯文本照段落切：空行分段，连着的几段合成一块。
3. 每块最多 1200 字节（约 300 token）：一节、一段太长的照段落再切，一段自己就超过的照字切（截在字的边界上）；切开的每一块都带着同样的标题路径。只有空白的块不要。
4. 同样的文档切出同样的块（照编号对得上），纯逻辑，有单元测试。
5. PDF（K-5）：交给抽字的小程序 `miyu-extract`（主程序真实位置的旁边，`miyu-extract pdf <文件>`，交回 JSON 的一页一份）：核心不链抽字的两个库（编进核心空闲时多 1.6 MB 上下的常驻，过了设计 23 的预算，K-5 头一回合进去量出来又撤回了）；一份文档拉起一次，等 60 秒还没抽完的杀掉，当抽不出来；核心旁边没有它的（测试里自己造的知识库没交的）PDF、网页都跳过、读不了。小程序里一页一页抽字（`pdf-extract`，抽的库碰到坏文件可能 panic，接住了当抽不出来），每页去掉头尾的空白，部首的码位换回平常的字（中文 PDF 常把「子」映射成 U+2F26 `⼦`、「页」成 U+2EDA `⻚`；照 Unicode 的 `EquivalentUnifiedIdeograph.txt` 换，别的字不动；不用 NFKC：它换不了部首补充那一段，还会把全角标点换成半角）。每页照纯文本切（第 2、3 款），每块的头一行记着第几页（搜到的时候拆出来，不给她看）；一个字都抽不出来的不收。
6. 网页（K-5）：同样交给 `miyu-extract`（`miyu-extract html <文件>`），换成文字（`html2text` 的 `plain_no_decorate`，不折行）：标题写成 Markdown 的 `#`，脚本、样式不要，粗体这些不加记号，链接、图只留字（`[链接]`）；换出来的照 Markdown 切（第 1、3 款）。只去掉标签，不挑正文：导航、页脚的字照留。

**建索引的进度**（施工 W-15，网页的知识库页要的）：照文件补（下面第三条）的时候，要重建的文档（新的、指纹变了的）先数出来，一篇篇报给核心登记的那一头（`miyu_store::knowledge::IndexWatch`）：开头 0、最后一篇照报，中间同一个库至多 0.1 秒一次；补完报建好了，有重建过的在索引的 `meta` 表记 `indexed_at`。没有要重建的什么都不报。人重建、列出来、搜、挂目录、她用工具，都走这一条。核心写成 `kb.indexing`、`kb.indexed` 推给订阅着 `kb` 流的连接（`knowledge/progress.rs`）。`kb.list` 碰上别处正在补的库不等它，照索引现有的样子列、带 `indexing: true`。向量（照意思找的那一路）在搜过以后后台补，不算在进度里。

**三、照文件补**（K-1 上）：`kb.list`、`kb.search` 之前先补这个库（在阻塞线程里）：

1. 列出库里的文档，每个算指纹（大小、改的时刻），和 `stamps` 里记着的比。
2. 新的、变了的：读出来、切块，在一个事务里拿掉它原来的块（连同它们的向量，K-2：下一次搜完照新的补）、放进新的、记下新的指纹（`RecallIndex::replace`）。
3. 没了的（`stamps` 里有、目录里没有）：拿掉它的块（连同向量）和指纹。
4. 一个文档读不了、太大、不是 UTF-8：跳过它（原来的块照留），记一行 `WARN knowledge document skipped kb=… reason=…`（文档路径、抽字程序说的带着人的东西，不写；施工 LG-1 上），别的照补。跳过的也记指纹（W-15 再补，`knowledge/skip_stamp.rs`）：文件指纹后面接原因和为什么；下一次文件没变的照记下的报跳过，不重读、不算进建索引的进度（「没有抽字的小程序」那一种装上了就重读）。库的文档数只数正常的指纹和跳过了还留着旧块的。
5. 一个库同一时刻只有一个在补（库一把锁，核心一份登记）：两个请求同时来，后来的等前面的补完。
6. 挂进来的照挂的目录补（K-4）。挂的目录用不了（不在、不是目录、读不了，写的不是绝对路径也不是 `~/` 开头、家目录不知道）：不补，索引不动（U 盘拔了再插回来不用重建），`kb.list` 里这个库带 `error`、数照索引里现有的，搜的时候跳过它（第四条），读里面的文档是没有这个文档；她的 system 里照旧列着（只有这一刻才知道用不了，跟着变就每回断一次缓存）。
7. 不另监视文件（K-4 量过）：每次列、搜之前照指纹补就跟得上原处的改动。3000 篇（24000 块）的库不变的重扫 8 到 10 毫秒、改了一篇 8 到 13 毫秒、头一次补 0.6 秒上下（Linux、release，`measure_three_thousand_documents`）；监视省下的只有这 10 毫秒和「改过的块下一次搜完才补上向量」。量出重扫成了瓶颈再加。

**四、搜**（K-1 上、K-2）：每个要搜的库（挂的目录用不了的跳过，K-4）里照关键词（`recall.md` 第一条）找 `limit` 的四倍；有问句的向量的（K-2，`recall.md` 第三条：问句照调的一方手里的配置算，`models.embedding` 是 `off` 的、本机的没装的、算不出的、1 秒等不到的没有）再照意思找同样多，两路照名次合（加权 RRF，只有照意思那一路找到的过下限：照模型，表在 `resources/core/knowledge/search.toml`，`local:bge-small-zh-v1.5` 是 0.50，出厂换成的 granite r2 是 0.82（R-5 七补），表里没有的照 0.40，`miyu_recall::FLOOR`；核心起来时读一次交给知识库，读不出来的记一行 `WARN knowledge search floors unreadable`，都照 0.40；K-2 修：原来都照 0.40，装了内置语义模型以后平常的话也搜得出东西，验收手册查出来，bge 的 0.50 照测评集量、2026-10-13 项目主人定，`knowledge/eval.md`「搜的下限」），取 `limit` 块；几个库的再照名次轮流合（每个库的第一名在前，同名次照库的编号，收窄的照列的先后），取 `limit` 块。

搜完在后台补搜过的几个库缺的向量（K-2，同记忆的「搜的时候补」，`Vectors::fill_knowledge`）：一个库一个名字（`kb <编号>`），她的工具、人经协议补同一个库同一时刻只有一个在补；远程的花费照用量记进日志。开了 `auto` 的库，自动想起的时候也照这样补（第九条第 2 款，同一个名字）。没补上向量的块只有关键词那一路找得到；新写进来的、改过的块下一次搜完（开了 `auto` 的，下一次自动想起）再补。

**五、读**（K-1 上）：照路径直接读目录里的文件（真相；挂进来的读挂的目录，K-4），不读索引；一页 32 KiB。PDF 读它的第几页抽出来的字（同第二条第 5 款），网页读换出来的字照 32 KiB 分页（K-5）；每次读现抽，不存。PDF、网页的文件超过 64 MiB 的不抽，算读不了。

**六、给不给、查哪几个**（K-1 下，19 N11、N13）：

1. 查哪几个：全部库；人格收窄的只留列着的、在的那几个，照列的先后（搜的时候同名次照这个先后）。每次调用照那一刻的目录和人格文件算：建了库、改了人格，下一次调用就照新的。人格的文件没了的（会话照快照接着用）当没收窄。
2. 给不给：知识库这个包装着、预设开着、照第 1 款查得到至少一个库，工具面上就有两件；本机的、群里的、别人私聊里的、子代理都一样（N12）。一个都查不到的没有（2026-10-12 项目主人定）。目录、人格文件读不了的当查不到，记一行 `WARN knowledge unreadable error=…`。
3. 什么时候判：造会话时判一次（列出来的那一块一起拼，K-1 补）；载入的照快照；每个回合开始照这一刻再判（那一块也照这一刻重拼，变了一样换）（`actor/persona.rs`，同人格、预设改了下一个回合换上）：该有的两件和快照里的不一样才换快照（建了第一个库下一个回合有，删光了、收窄成空的下一个回合没有），换的时候工具面变了，缓存断一次。预设找不回来的照旧。
4. 端口：每个会话一份（`Shelves`），每次调用造一个端口，碰磁盘的在阻塞线程里做；跳过的文档、读不了的照第三条第 4 款、「出错」记一行（目标 `miyu::session`）。

**七、她写**（K-3，19 N14）：

1. 写哪几个：她这一刻查得到的库（第六条第 1 款），挂进来的除外（K-4，N7：只读，拒，什么都不动），软件包带来的除外（K-8：拒，那一句 `common/packaged.txt` 说清是随包来的、写到别的库里；`kb_save` 写到包里的库不另建一个盖住它）。`kb_save` 写到还没有的库就是建它（目录、写了 `about` 的 `kb.toml` 的 `description`）；人格收窄了的不建（建了她也查不到），有、人格没列的当没有；编号不合写法的不建、说清编号怎么写（K-3 补）；路径不合写法的不建。
2. 给不给：`kb_save` 照知识库这个包装着、预设开着就给，一个库都没有也给（好建第一个，2026-10-12 项目主人定）；人格收窄了、一个都查不到的（收窄成空的、列着的一个都还没有）不给：收窄了的建不了库（第 1 款），给了也写不进（K-3 修，验收手册待定 13，2026-10-15 项目主人定写也照收窄）；`kb_edit`、`kb_delete` 同 `kb_search`（第六条第 2 款）。本机的、群里的、别人私聊里的都给（同一天项目主人定）。回合开始照这一刻再判的照第六条第 3 款，五件一起比。
3. 路径同读（第五条）：库里的相对路径、文档的扩展名、不许 `..` 和以点开头的一段；上级目录没有的建；已经有了却不是普通文件的当不是文档。`kb_save`、`kb_edit` 只写 `.md`、`.markdown`、`.txt`（K-5：PDF、网页照原样存着，写进字会弄坏），写别的拒、什么都不动；`kb_delete` 照样删得了 PDF、网页。
4. 新建的：这一篇已经有了不写。接在后面的：原来末尾没换行的先补一个；没有就新建。写进去的末尾没换行的补一个。改的：`old` 得正好出现一次；不是 UTF-8 的当读不了。写完超过 4 MiB 的不写（同第三条的上限）。
5. 写照 `miyu_fs::replace`（先写临时文件再改名），删照 `miyu_fs::trash`（系统回收站；Linux 照这台机器的家目录找）。写之前库的目录换成真实的位置：`replace` 一层层不跟链接地打开上级目录，数据根的路上有链接的（macOS 的临时目录 `/var` 是 `/private/var` 的链接）照原样写不进（同 `write` 先换真实位置，CI 上 macOS 查出来的）。效果里的路径是真实的位置。索引不在这里动：下一次搜时照文件补（指纹变了）。
6. 每次交回改前改后的内容、回收站里的位置，工具照它报效果（撤销那一轮时改回去、移回来）。

**八、人管理**（K-7 上，19 第六节；2026-10-13 项目主人定删库、加文档、建库这三条）：人经协议的七个方法：建、改、挂、删、重建索引、加文档、删文档。做法在 `knowledge/manage.rs`。

1. 建库（`kb.create`）、挂目录（`kb.mount`）：编号合写法、还没有；`name`、`description`、`mount` 写进 `kb.toml`。挂的目录先照「对外的样子」挂进来的那一段查用不用得了，用不了的什么都不建；挂上的照它补一遍，回应带补完的数。
2. 改名字、说明（`kb.update`）：照配置的那一套只改那一格（`miyu_config::edit`）：注释、`mount`、别的格照原样；空字符串是拿掉那一格。
3. 加文档（`kb.add`）：人照附件上传（`blob.put`、分块上传，一份最多 20 MiB；更大的请挂目录），什么格式都行（她写的照旧只有文字的，第七条第 3 款）。路径照「文档」的写法、上级目录没有的建；已经有了的只在 `replace` 时换掉；挂进来的拒（只读，N7）。写照 `miyu_fs::replace`。索引不在这里动：下一次搜、列的时候照文件补。
4. 删文档（`kb.remove`）：同她的 `kb_delete`，挪进系统回收站；挂进来的拒。施工 W-14 起也收文件夹（整个挪）、一次收几样（`paths`，先全查一遍，有一样不在就一样都不删；`Knowledge::trash_all`）。
5. 删库（`kb.delete`）：先拿这个库的补的锁，关上索引的连接、连同 `-wal`、`-shm` 删掉（Windows 上开着的 SQLite 删不掉；会话在后台补向量拿着的是同一份，关上以后照「库用不了的不写」），再把整个库目录挪进系统回收站（2026-10-13 项目主人定）。挂进来的那个库目录里只有登记，挂的目录在外面，一个字都不动。挪不进回收站的，库目录照旧在，索引下一次补的时候照文件重建。她照旧删不了整个库（N14）。
6. 重建索引（`kb.reindex`）：拿补的锁、删掉重建一份空的（`RecallIndex::reset`），放开锁照文件补一遍；向量下一次搜完再补。挂的目录用不了的照列出来的样子带 `error`。
7. 软件包带来的库（K-8）：改名字、说明，加、删文档，删库拒 `kb_packaged`；改 `auto` 照做，记在数据根的 `kb.toml`；建、挂同编号的回 `kb_exists`。
8. 不认请求编号去重：建、挂的再发一次回 `kb_exists`，删的再发一次回 `unknown_kb`，加的再发一次回 `document_exists`（写了 `replace` 的照样换，结果一样）。改完不推通知：头自己再 `kb.list`。

**九、自动想起**（K-6 下，19 N5；项目主人 2026-10-12 定开关放 `kb.toml`、关键词也算，2026-10-13 看了测评集定照标题名算、照意思门槛 0.60，`knowledge/eval.md`）：做法在 `crates/miyu-session/src/knowledge/recall.rs`，回合开始的挂接点在 `actor/model.rs` 的 `hooks`。

1. **哪些回合**：人开的一轮（`said` 有字的，同记忆的联想，`memory.md` 第四条第 1 款），本机的、场所的都算；会话这一刻的工具面上有 `kb_search`（知识库这个包装着、预设开着、查得到至少一个库）；只看她查得到的库（第六条第 1 款，人格收窄照旧）里 `kb.toml` 写了 `auto = true` 的。
2. **补**：那几个库先照文件补（第三条，同搜之前；挂的目录用不了的跳过），再照索引里的每一段挑（`Knowledge::sections`）。这一轮的模型在门槛表里的（第 3 款），那几个库在后台补缺的向量（同搜完补，第四条，同一个名字）：她不搜，照意思那一路也补得上；头一回开的库这一轮照意思找不到，补完以后的回合找得到。不在表里的不补。
3. **挑**：
   - **标题名和关键词**：人说的话（前 1000 个字）里出现了某一段标题路径的最后一段、或者这一节写的关键词（两个字以上，不分大小写）的，带（关键词 K-9，项目主人 2026-10-14 定：写在标题下一行、当别名用，只看这一轮人说的；标题名同一天跟着改成不分大小写）。关键词那一行在这一节头一块的正文开头（「对外的样子」）：一节切成几块的，每块都照这一节的关键词算。认那一行、分开、去掉在 `miyu_recall::kb::Keys`，写法表照 `core/knowledge/keys.toml`（会话开的时候和门槛表一起读）。PDF 的段、没有标题的段没有标题也没有关键词，只走照意思。
   - **照意思**：这一轮的模型（同她的工具、联想，照这一轮的配置）在门槛表里的，算话的向量，每个库找最像的 32 块，过门槛的带（`miyu_recall::associate::pick`）。门槛照模型，数据在 `resources/core/knowledge/recall.toml`：出厂的 granite r2 是 0.86（R-5 七补，`knowledge/eval.md`；原来 bge-small-zh-v1.5 的 0.60 随 R-18 删掉 bge 拿掉了）。表里没有的模型不算向量（远程的不白花钱），只走标题名；没有照意思那一路的（`off`、没装、算不出、等不到）也只走标题名。
   - 先标题名和关键词、再照意思（最像的在前），去掉重的；这一段上下文里自动想起交过的不带（`present` 里模块 `knowledge`、类别 `recall` 的 `refs`）；一轮最多 3 段；联想和自动想起合起来一段上下文累计 40 条（压缩以后从零数），到了不再带（19 N5 共用一份预算：两边各自数的时候都把对方的算上）。
4. **交什么**：一块 `context.injected`，模块 `knowledge`、类别 `recall`，`refs` 是带的那几段（`<库>/<路径>#<第几块>`）。外壳 `<knowledge>`、`</knowledge>`（`software/knowledge/recall/`，登记簿），中间一段两行：头一行照 `kb_search` 的那一行（`[<库>] <路径>: <标题路径>`，PDF 的 `[<库>] <路径>, page <页>`，没标题的 `[<库>] <路径>`），下面是这一段的字（关键词那一行不算，K-9；`kb_search`、`kb_read` 照原样带着它），连着的空白（换行）换成一个空格，超过 120 个字的截到 120 个字、照 `recall/cut.txt` 接省略号。要全文她自己 `kb_read`。一块不超过 600 字节，放不下的那一段和以后的不加。追加在这一轮的话前面、原样回放，不改以前的前缀。
5. **时限、排**：和记忆的挂接点并着问（`tokio::join!`），各自限时 300 毫秒，过了的这一轮不交，记一行 `DEBUG turn start hook timed out module=knowledge part=recall`；照模块编号排，知识库那一块在记忆的几块前面。照意思那一路从自动想起开始数最多等 200 毫秒（`recall.rs` 的 `MEANING`，留出拼块的工夫；读段慢了照意思那一路就少等），过了的这一轮只走标题名，记一行 `DEBUG knowledge recall by meaning timed out`：本机模型冷启动的那一轮（真 bge 试过，头一轮算话的向量连拉起小程序要三百毫秒上下）、那一家回得慢的，标题名不跟着整块超时丢掉；算向量的那一件照样算完。
6. **出错**：库、索引读不了的这一轮不带，记一行 `WARN knowledge recall not read error=…`；会话开的时候外壳的字、门槛表、关键词的写法表读不出来的这个会话不自动想起，记一行 `WARN knowledge recall texts unreadable error=…`（目标 `miyu::session`）。

**十、装卸**（K-8）：做法在 `knowledge/place.rs`，核心那一头在端点的 `knowledge.rs`（`of_core`、`carried`、`follow`）、`packages.rs`（`adopt_packages`）、`packages/manage.rs`（`switch_packages` 里一行）。

1. 核心起来时把读成了的清单交给知识库（包的编号、包目录，`Knowledge::with_packages`）；主程序交进核对过的清单时只换不清（`use_packages`：没收的包不是卸了）。
2. 装、卸以后（`switch_packages`）照新的清单跟上（`follow_packages`）：新装的下一次列、搜就有；卸掉的包，开着的索引关上、`index/kb/packages/<包>/` 删掉、`kb.toml` 里 `"<包>/…"` 那几行拿掉（项目主人 2026-10-13 定：卸包以后跟着没有，同 F-8 五补「卸包清开关」）。清不掉的记一行 `WARN knowledge package not forgotten`，照样换上新的清单。

### 出错

见「对外的样子」协议那一节、她的工具那一节的出错；补的时候的见「怎么走」第三条第 4 款。她的工具碰到读不了的，记一行 `WARN knowledge failed error=…`（目标 `miyu::session`）。索引坏了照检索库的规矩删掉重建（`recall.md`）。

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-store/tests/knowledge_mount.rs`（K-4） | 挂进来的：照挂的目录搜、读，库目录里别的文件、以点开头的目录不算，原处改了下一次搜就照新的；`~/` 照家目录，家目录不知道的带 `error`；挂的目录挪走了列出来带 `error`、数照索引里的、搜的时候跳过（只搜它的也不算出错）、读不到，挪回来照搜；写的不是绝对路径的带 `error`；写的三件拒、什么都没动；里面读不了的子目录跳过、照跳过的交回（Unix） |
| `crates/miyu-store/tests/knowledge_formats.rs`（K-5，PDF 在 `crates/miyu-extract/tests/fixtures/`，怎么做的写在那里的 README；照 cargo 编出来的 `miyu-extract` 抽） | 第二条第 5、6 款、第三条、第五条、第七条第 3 款：没交抽字的小程序的跳过 PDF、网页、读不了，主程序旁边找得到它；中英文的 PDF 一页一块、搜到的说第几页、没有标题路径、部首换回平常的字（康熙部首、部首补充两段）、照页读、页数、没有的页；一个字都没有的、坏的 PDF 跳过、读坏的照实说、比文字的上限大比 PDF 的上限小的照 PDF 的上限去抽；网页照标题切、脚本样式里的字不算、`.htm` 也认、读的是换出来的字；PDF、网页写不了（新建、追加、改），删照删 |
| `crates/miyu-store/tests/knowledge_documents.rs`（K-10）、`crates/miyu-endpoint/tests/knowledge_documents.rs` | `kb.documents`：照路径排、几种都认、大小、改的时刻、块数；读不了的哪一种、为什么、留着旧块；没有抽字小程序的 PDF；读不了的子目录另列；不是文档的、点开头的不列；挂进来的带 `mount`、拔掉的拒 `folder_unavailable`；参数写错的、没有的库 |
| `crates/miyu-extract/src/radicals.rs` 的单元测试、`crates/miyu-extract/tests/extract.rs`（K-5） | 部首的码位换成平常的字、全角的字不动、对照表每一行都读得懂；中英文的 PDF 一页一份、头尾空白去掉、只有画的一页是空的、不是 PDF 的说为什么；网页照标题写 `#`、脚本样式不要；小程序照参数抽、照 JSON 交回，参数少了、多了、种类不对、读不了的退出码 1、说为什么 |
| `crates/miyu-session/tests/knowledge_formats.rs`（K-5） | 「她的工具」，真会话：搜到 PDF 的那一块头一行说第几页、照那一页读、末尾说第几页；写 PDF 拒、说哪几种能写、文件没动 |
| `crates/miyu-endpoint/tests/knowledge.rs` 的 `a_pdf_hit_says_its_page_and_reads_by_page`（K-5） | 「协议」，真核心：PDF 的那一块多 `page`、别的不写；`kb.read` 读 PDF 的第几页、`pages` 是 PDF 的页数 |
| `crates/miyu-store/tests/knowledge.rs` 的 `measure_three_thousand_documents`（K-4，`#[ignore]`） | 第三条第 7 款的量尺：3000 篇头一次补、不变的重扫、改了一篇、搜一次各多久；断言块数 |
| `crates/miyu-recall/src/chunk/tests.rs`（K-1 上） | 第二条：Markdown 照标题切、标题路径照层级、代码块里的 `#` 不算；纯文本照段落；太长的照段落、照字再切，不超过 1200 字节、截在字的边界上；空白的不要；同样的文档同样的块 |
| `crates/miyu-store/tests/recall_stamps.rs`（K-1 上） | `recall.md` 的 `stamps`：整份换掉一个来源（原来的块拿掉、新的放进去、指纹记下），别的来源不动；列出指纹；拿掉一个来源连指纹一起；换掉、拿掉的连向量一起（K-2） |
| `crates/miyu-store/tests/knowledge.rs`（K-1 上） | 第一、三、四、五条：列出合写法的库、`kb.toml` 读得出读不出；照文件补：新的放进去、变了的重切、没了的拿掉、没变的不重读；太大的、不是 UTF-8 的跳过；搜得到、几个库轮流合、只搜一个；读一页、分页、截在字的边界上；路径跑出去的、不是文档的读不到 |
| `crates/miyu-endpoint/tests/knowledge.rs`（K-1 上） | 「协议」，真核心：`kb.list`、`kb.search`、`kb.read` 的回应一格一格对；写错的几种 `bad_params`，没有的库 `unknown_kb`，没有的文档 `unknown_document` |
| `crates/miyu-knowledge/tests/tools.rs`、`writes.rs`、`budget.rs`（K-1 下、K-3；写的在 `writes.rs`，假端口在 `support/`） | 「她的工具」，假端口：查的两件是 `read`、写的三件是 `write`；写的三件说的话、报的效果（改前改后、回收站里的位置）、`append` 不写是新建，写的拒绝八种如实说（K-4 加只读、K-5 加写不了 PDF 和网页）、PDF 的那一块头一行说第几页（K-5）、没写成的不报效果；搜到的一块一段、头一行逐字节对、字原样不转义；读一页、不止一页的末尾说第几页、不写是第一页；拒绝的四种两件都如实说、没有端口的、参数不对的；两份说明合计不超过 600 字节 |
| `crates/miyu-session/tests/knowledge_tools.rs`（K-1 下；K-1 补加了三条） | 第六条，真会话、真目录：system 里那一块逐字节对（照编号排、名字说明转义、只有编号的），改了名字下一个回合换上，没有库的、预设关着的没有，收窄的只列列着的，超过 20 个的末尾那一句（K-1 补）；一个库都没有的没有两件，建了第一个库下一个回合有、删光了下一个回合没有；人格收窄的只搜得到列着的、没列的库搜不到读不到、读一页、没有的文档，收窄成空的下一个回合没有；群会话里有，预设关着的没有 |
| `crates/miyu-store/tests/knowledge.rs` 的 `several_bases_take_turns_and_one_can_be_searched_alone`（K-1 下补） | 照收窄的几个搜：同名次照给的先后，有一个不在的不搜；`ids` 列出全部库 |
| `crates/miyu-policy/src/persona/tests.rs`（K-1 下） | `[knowledge] bases` 读得出、空的列表、不写是没有、上一层整格换掉，写错的四种（`personas.md`） |
| `crates/miyu-core/tests/tools.rs`（K-1 下、K-3） | 出厂的工具目录有五件；没装知识库的没有，只装它的只有它的五件 |
| `crates/miyu-store/tests/knowledge.rs` 的 `with_a_query_vector_a_chunk_is_found_by_meaning`、`the_meaning_side_lifts_a_lower_keyword_hit`（K-2） | 第四条：关键词对不上、向量像的照意思找得到；没有问句向量的只走关键词；只有向量那一路、不够像的不要；两路各找四倍，关键词排第二、照意思排第一的排得过只有关键词排第一的 |
| `crates/miyu-session/tests/knowledge_meaning.rs`（K-2） | 第四条，真会话、远程的假 embedding：头一次只走关键词、搜完补上向量以后照意思找得到，问句、块都照远程的算；关键词对得上的头一次就找得到；`off` 的不问那一家、不补，那一家出错的照关键词找得到；补向量的花费记进日志的用量 |
| `crates/miyu-endpoint/tests/knowledge.rs` 的 `kb_search_finds_by_meaning_once_the_vectors_are_filled`、`her_kb_search_finds_by_meaning_and_still_does_after_a_restart`（K-2） | 第四条，真核心、远程的假 embedding：人经协议的 `kb.search` 补上向量以后照意思找得到；核心把算向量的交给造的会话（她搜完补上）、载入的会话（照意思找得到） |
| `crates/miyu-endpoint/tests/knowledge.rs` 的 `a_mounted_folder_follows_the_home_and_an_unplugged_one_is_skipped`（K-4） | 第三条第 6 款，真核心：`~/` 照核心的家目录；挂的目录挪走了 `kb.list` 带 `error`、数照索引里的，`kb.search` 跳过它、不算出错 |
| `crates/miyu-session/tests/knowledge_recall.rs`（K-6 下） | 第九条，真会话、真目录：开了 `auto` 的库，话里提到标题名的那一轮那一块逐字节对、`refs` 对；带过的这一段上下文里不再带、只带新提到的；平常的话、没开 `auto` 的、预设关了知识库的不带；照意思（远程的假 embedding）：模型不在门槛表里的不带、不问那一家（也不补段的向量），在表里的（测试改的那一份门槛表）过门槛的带、不过的不带；她不搜，段的向量也在后台补上，补完的下一轮换了说法带得上；那一家停住不回的，标题名照带；原处加了一段的下一轮照文件补 |
| `crates/miyu-session/src/knowledge/recall/tests.rs`（K-6 下） | 第九条第 3、4 款：只有知识库交过的算交过，带过几条连同记忆的联想一起数；一块逐字节对，PDF 说第几页、没标题的只写路径、换行写成空格、长的截到 120 个字接省略号，超过 600 字节的那一段和以后的不加 |
| `crates/miyu-recall/src/kb/keys.rs` 的测试（K-9） | 关键词那一行：几种开头、中英文冒号、开头不分大小写、头一行是空行的、只写了开头的照样去掉、几种分隔、空的不要；不是头一行的、没冒号的、开头只是前缀的（`keystone:`）不算；写法表少了格、空的、有空字符串、多了格的读不进来 |
| `crates/miyu-session/src/knowledge/recall/tests.rs` 的 `keywords_under_the_heading_bring_their_section_like_its_name`、`the_keyword_line_is_left_out_of_the_block`，`tests/knowledge_recall.rs` 的 `a_keyword_under_the_heading_brings_its_section_without_that_line`（K-9） | 第九条第 3、4 款：关键词当别名，一节切成几块的每块都算；英文的关键词、标题名不分大小写；一个字的不算，不是头一行的不算，没有标题的段写了也不算；带上的字里没有那一行，没有标题的段原样；真会话里人说别名的那一轮那一块逐字节对，带过的不再带 |
| `crates/miyu-store/tests/knowledge_manage.rs`（K-7 上） | 第八条：建库带名字说明（转义）、有了的不建、编号不对的；改名字说明照留注释和别的格、空字符串拿掉（文件里没这一格）、没有的库；挂目录、有了的不挂、目录不在的和相对路径的什么都不建、改名字时 `mount` 照留；加文档（PDF 也行）、有了的不换、说换的换、路径不对的三种、挂进来的不加且挂的目录没动；删库整个目录进回收站、索引删掉、列不出来了、同一个编号再建照样搜得到，挂进来的拿掉登记、挂的目录没动，没有的库；重建数对得上、向量没了；`auto` 写进 `kb.toml`、列出来带上、`false` 拿掉（K-6 下）；一个库的每一段（键、标题路径、字）列得出（K-6 下） |
| `crates/miyu-endpoint/src/knowledge/tests.rs`（K-7 上） | 知识库那一层的每一种错写成哪一种拒绝，`folder_unavailable`、`trash_failed` 带 `data.message`；核心照资源目录交搜的下限：出厂的表 granite r2 0.82（R-5 七补；bge 那一行 R-18 拿掉了，照没有的算）、表里没有的 0.40、表读不出来的都 0.40（K-2 修） |
| `crates/miyu-store/tests/knowledge_floor.rs`、`crates/miyu-recall/src/fuse/tests.rs` 的 `a_floor_of_its_own`（K-2 修） | 第四条：表里的模型照表（0.55 找得到、0.45 不要），没有的照 0.40；出厂的表 granite r2 是 0.82（R-5 七补），没有 bge（R-18）；`fuse` 就是照 0.40 的 `fuse_above`，关键词也找到的不看下限 |
| `crates/miyu-endpoint/tests/knowledge_manage.rs`（K-7 上） | 「协议」管理的七个方法，真核心：建、改、加（照上传好的 blob）、换、搜得到、重建的回应；挂 `~/`、挂不上的、往挂进来的加删拒、删文档、删库（挪不进回收站的照实报）、挂的目录没动；写错的十三种 |
| `crates/miyu-store/tests/knowledge_package.rs`、`crates/miyu-endpoint/tests/knowledge_package.rs`、`crates/miyu-session/tests/knowledge_package.rs`、`crates/miyu-cli/tests/kb.rs` 的一条（K-8） | 「对外的样子」包带来的那一节、第一条、第十条：装了的包里的库列得出、带 `package`、搜得到、读得到，索引在包那一格；写的几样拒（她那一句、`kb_packaged`），改名字拒、开关照改、记在数据根的 `kb.toml`、包里的不动，建同编号的 `kb_exists`；自己的赢、两个包先到先得；没有 `kb/`、写错的目录、包目录不在的不碍事；数据根的路上有链接的照真实的位置记开关、拿掉开关；真核心装上当场有、卸掉当场没、索引和开关清掉，起来时照核对过的清单（没收的包不带库）；她搜得到、开着 `auto` 的自动想起、写进去拒那一句、不另建；命令行列表接「随 <包名>」、`edit --no-auto` 改得了、改名字照原话拒 |
| `crates/miyu-store/tests/knowledge_write.rs`（K-3） | 第七条：建库带说明（转义）、已经有的不动、没说明的不写 `kb.toml`、编号不对的；新建、已经有的不写；接在后面补换行、没有就新建；照原文换一段、找不到、不止一处、没有的文档；路径跑出去的七种、没有的库、太大的什么都不写；挪进回收站、没有的文档；数据根的路上有链接的照真实的位置写（Unix） |
| `crates/miyu-session/tests/knowledge_write.rs`（K-3） | 第七条，真会话、真目录：一个库都没有只有 `kb_save`、编号写成中文的说清怎么写、什么都不建（K-3 补）、写到没有的库就建它、下一个回合五件都有；接在后面、改、新建撞上已经有的，撤销改的那一轮文件改回去；人格收窄的写不了没列的、建不了新库、改不了没列的；收窄成空的、列着的都还没有的一件都不给，列着的有了下一个回合五件都有（K-3 修）；挪进回收站、撤销移回来（只在 Linux 上跑）；挂进来的照搜、照读，写的三件拒、挂的目录和库目录里什么都没动（K-4） |

### 出处

- `19-知识库.md`（N1 到 N12）。
- `docs/reviews/2026-10-07-记忆知识库embedding调研.md`：旧版的实测和坑（第一节）、第五节第 7 条。

### 还没有的

见「施工步子」后面几步：命令行的 `--auto`、软件包带来的库、关键词条目。

- Markdown 第一个标题以前的那一节里，代码块被空行切开、某一块恰好以 `# ` 开头的，搜到时那一行会被当成标题路径（只影响显示，搜得到）。碰到了再改成每块都写标题那一行。
- 跳过的文档每次补都记一行：人经协议、她的工具补的都记，量出来吵了再只记头一次。
- 网页不挑正文、PDF 里的图和扫描件不认字（OCR）、抽出来的字不缓存（K-5 定了不做，量出来是问题再加）。恶意构造的 PDF 让抽字的库卡住接不住（panic 接得住），量出来是问题再挪到子进程里抽。

### 施工步子

| 步 | 做什么 |
|---|---|
| K-1 上 | 底子：自己的知识库目录、`kb.toml`、切块（Markdown、纯文本）、索引照文件补（关键词）、协议 `kb.list`、`kb.search`、`kb.read` |
| K-1 下 | 她的两件工具 `kb_search`、`kb_read`；一个库都查不到的不给（N13）；人格收窄（N11）；知识库这个软件包（N10）。系统提示词里先不列能查的库，量了再定 |
| K-1 补 | 系统提示词里列出她查得到的库：编号、名字、一句说明，放在 system 第 4 块；变了下一个回合换上（2026-10-12 项目主人看了 K-1 下的实测定） |
| K-3 | 她操作知识库（N4、N14，2026-10-12 项目主人定提前到 K-1 补以后）：`kb_save`（新建、追加，写到没有的库就建它）、`kb_edit`、`kb_delete`（系统回收站），撤销得回来；不能删整个库 |
| K-3 修 | 人格收窄了、一个库都查不到的不给 `kb_save`（第七条第 2 款）：写本来就照收窄（只写列着的、不建库），只差这一件还挂在工具面上（验收手册待定 13，2026-10-15 项目主人定写也照收窄） |
| K-2 | 照意思找：向量一路、两路合并，和记忆同一套（N3）；搜完在后台补搜过的库缺的向量 |
| K-2 修 | 只照意思找到的那一路的下限照模型（第四条）：bge 0.50，表里没有的照 0.40（验收手册查出来，2026-10-13 项目主人定） |
| K-4 | 挂进来的目录（只读，N7）；不另监视文件，照每次列、搜之前补（量过，第三条第 7 款）；头一次补照键的范围删（3000 篇从 5 秒上下到 0.6 秒上下） |
| K-5 | PDF（照页抽字、照页引用、照页读，部首的码位换回平常的字）、网页 HTML（换成文字、照标题切，不挑正文）（N2）；只有文字的文档写得了 |
| K-6 上 | 自动想起的测评集和量尺（`knowledge/eval.md`）：几种挑法量过，项目主人 2026-10-13 定标题名加照意思 0.60 |
| K-6 下 | 自动想起（第九条）：`kb.toml` 的 `auto`，默认关，协议 `auto`；标题名加照意思 0.60，一轮最多三段，每段头一行加开头 120 个字；和联想共用 40 条的累计 |
| K-6 补 | 命令行 `miyu kb` 的 `--auto` 开、`--no-auto` 关（`new`、`edit`、`mount`），列表在篇数后面接「自动想起」（`cli/kb.md`，2026-10-13 项目主人定写法） |
| K-7 上 | 管理的协议：`kb.create`、`kb.update`、`kb.delete`、`kb.mount`、`kb.add`、`kb.remove`、`kb.reindex`（第八条，2026-10-13 项目主人定删库进回收站、加文档照附件上传、建库改名也在这一步） |
| K-7 下 | 命令行 `miyu kb`（`cli/kb.md`）：`list`、`search`、`read`、`new`、`edit`、`add`、`remove`、`mount`、`delete`、`reindex`；补上 K-7 上那五种拒绝给人看的话 |
| K-8 | 软件包带来的知识库（N6，「对外的样子」那一节、第十条）：包目录里的 `kb/<编号>/`，只读，自动想起人能开关、记在数据根的 `kb.toml`，列出来带 `package`；卸包时索引、开关一起删（2026-10-13 项目主人定） |
| K-9 | 关键词触发的条目（N9，照 SillyTavern 的 World Info）：标题下一行 `关键词：…`，当别名，和标题名一样触发、同一份预算，那一行不进带上的字；只看这一轮人说的；标题名一起改成不分大小写（2026-10-14 项目主人定） |
| K-10 | 列出库里的文档（2026-10-19 项目主人提出缺知识库的后台页面，定在网页设置里做一页、顶上直接搜、终端用命令）：`kb.documents` 一篇一项带大小、时刻、块数，读不了的带 `reason`、`why`，读不了的子目录另列；`kb.list` 挂进来的带 `mount`；命令行 `miyu kb list <库>` |
