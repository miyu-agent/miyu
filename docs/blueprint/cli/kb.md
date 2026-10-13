## `miyu kb`

### 是什么

在 shell 里管知识库（施工 K-7 下，`knowledge.md`）：列、搜、读、建、改名字说明、加文件、删文档、挂现成的目录、删整个库、重建索引。连上核心（没在跑就拉起来），走 `kb.*`（K-7 上），不另起一套。加文件照附件上传：每个文件先 `blob.put` 本机路径，再 `kb.add`（2026-10-13 项目主人定）。给人看的只放人要的：库的编号、名字、几篇、说明，搜到的哪一篇哪一节；索引、块数、来源这些不印（2026-10-08 项目主人定的设计原则）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `kb`；换上帮助页 |
| `crates/miyu-cli/src/kb.rs` | 参数、找数据根、连核心、握手、照子命令发请求、`add` 一个个传、退出码 |
| `crates/miyu-cli/src/kb/shown.rs` | 库一行、搜到的一段、读的一页印成什么样（纯的，测试里照它算） |
| `crates/miyu-cli/src/kb/listing.rs` | 列、搜的回应怎么印：`json` 原样，`text` 照 `shown.rs`；列表里有软件包带来的库的先问一次包的名字（施工 K-8 挪出来） |
| `crates/miyu-cli/src/language/kb.rs` | 给人看的几句，中英文 |
| `crates/miyu-cli/src/help/{zh,en}/kb.txt` | 帮助页 |

### 对外的样子

| 子命令 | 做什么 | 发什么 |
|---|---|---|
| `list`（不写子命令也是它） | 列出知识库 | `kb.list` |
| `search <词>` | 搜，最相关的在前 | `kb.search`，`--kb` 只搜一个 |
| `read <库> <文档>` | 读一篇的一页 | `kb.read`，`--page` |
| `new <库>` | 建一个库 | `kb.create`，`--name`、`--description` |
| `edit <库>` | 改名字、说明、自动想起（至少写一样） | `kb.update` |
| `add <库> <文件>...` | 把文件加进库，什么格式都行，一个最多 20 MiB | 每个文件 `blob.put` 本机路径，再 `kb.add`：路径是文件名，`--to <目录>` 放进库里的那个目录；`--replace` 换掉已经有的 |
| `remove <库> <文档>` | 删一篇，挪进回收站 | `kb.remove` |
| `mount <库> <目录>` | 把现成的目录挂成一个库，只读 | `kb.mount`：相对路径照当前目录接成绝对的，`~` 开头的照原样；`--name`、`--description` |
| `delete <库>` | 删整个库，挪进回收站；挂进来的只拿掉登记，不动你的目录 | `kb.delete` |
| `reindex <库>` | 重建索引 | `kb.reindex` |

| 选项 | 做什么 |
|---|---|
| `--kb <库>` | `search`：只搜这一个库 |
| `--page <页>` | `read`：第几页，不写是第一页 |
| `--name <名字>` | `new`、`edit`、`mount`：给人看的名字 |
| `--description <说明>` | `new`、`edit`、`mount`：一句话说这个库是什么；`edit` 写空字符串是拿掉 |
| `--auto` | `new`、`edit`、`mount`：开自动想起（`auto: true`，`knowledge.md` 第九条；施工 K-6 补，2026-10-13 项目主人定写法） |
| `--no-auto` | `edit`：关自动想起（`auto: false`，拿掉 `kb.toml` 那一格）；`new`、`mount` 本来就关着；和 `--auto` 只能写一个，参数不对 |
| `--to <目录>` | `add`：放进库里的这个目录（前后的 `/` 去掉） |
| `--replace` | `add`：已经有的换掉 |
| `--format text\|json` | `list`、`search`：`json` 原样印核心回的那一串，给脚本 |

- 几个词用一个空格连起来（照 `miyu memory`）。
- 删的都不问：进回收站，找得回来（同 `miyu memory` 清空不问，2026-10-07 项目主人定的那条）。
- 界面语言照 `cli/main.md`。用到的环境变量：`MIYU_HOME`。

### 怎么走

1. **找数据根、连核心、握手**：照 `miyu memory`（`cli/memory.md` 第 1 步）。请求的编号是 `kb-<16 位十六进制>-<序号>`。
2. **发**：照上面的表；没给的选项不写那一格。`edit` 名字、说明、开关一样都没写的不发，说一句，退出码 1。
3. **加文件**：一个个来：`blob.put`（`path` 是照当前目录接好的绝对路径），交回的 `name` 是文件名；路径是 `--to` 加文件名；再 `kb.add`。碰到没成的那一个就停（前面成了的不退），照原话说，退出码 1。传以前先看一遍文件名：扩展名不在认的几种里的（`miyu_store::knowledge::EXTENSIONS`：`.md`、`.markdown`、`.txt`、`.pdf`、`.html`、`.htm`）一个都不传，说「只收 …：<文件名>」，退出码 1（施工 K-7 修：核心只回「参数错误。」，还白传一份）。
4. **印**（标准输出）：
   - `list`：一个库一行，编号照最长的对齐，后面是名字（和编号一样的不写）、几篇、软件包带来的接「随 <包名>」（施工 K-8，2026-10-13 项目主人定；包名照 `package.list`、照界面语言，列表里有包里的才问它，拿不到的照包的编号）、开了自动想起的接「自动想起」（施工 K-6 补，2026-10-13 项目主人定）、说明（没写的不写），两个空格隔开；挂的目录用不了的接上「（用不了：<为什么>）」。一个都没有的：「还没有知识库。」
   - `search`：一块一段，段和段之间空一行：头一行是库、文档、标题路径（PDF 的是「第 <页> 页」，没有标题的不写），两个空格隔开；下面是这一块的字，每行空两格。一块都没有的：「没找到。」
   - `read`：那一页的字；不止一页的另起一行「（第 <页>/<页数> 页）」。
   - 别的一句：「建好了：<库>」「改好了：<库>」「加进去了：<库>/<路径>」（一个文件一行）「挪进回收站了：<位置>」「挂上了：<库>，<几> 篇」「删掉了 <库>，在回收站里：<位置>」「重建好了：<库>，<几> 篇」。
5. **被拒绝**：发以前先看库的编号（子命令的、`--kb` 的），不合写法的（`miyu_store::layers::valid`）说清写法，退出码 2（施工 K-7 修）。`read` 写了 `--page`、核心回 `bad_params` 的是页码出界：再读一次第一页看一共几页，说「没有第 <页> 页，一共 <页数> 页。」，退出码 1。别的核心的原话照原样印在标准错误上，退出码 1；`folder_unavailable`、`trash_failed` 的原话（去掉句号）后面接「：」和核心给的为什么（`data.message`）。核心断开、请求写不出去照 `miyu memory`。

### 样子

```text
notes  日语笔记  12 篇  学日语的
world  雾港  4 篇  随 雾港世界观  自动想起  世界观
usb    U 盘  3 篇（用不了：mounted folder /mnt/usb: No such file or directory (os error 2)）
```

```text
notes  grammar.md  语法 › て形
  食べる → 食べて

manuals  cat.pdf  第 3 页
  每年十月打疫苗。
```

### 退出码

| 码 | 什么时候 |
|---|---|
| 0 | 做成了，列、搜一个都没有的也是 |
| 1 | 找不到数据根、连不上核心；被拒绝；核心断开；`edit` 什么都没写 |
| 2 | 参数不对：子命令写错、少了库、文件、目录、词，`--page` 不是数，`--format` 写错（`cli/main.md`）；库的编号不合写法（施工 K-7 修） |

### 给人看的字

| 键 | 中文 | 英文 |
|---|---|---|
| 一个库都没有 | 还没有知识库。 | No knowledge bases yet. |
| 没找到（`search`） | 没找到。 | Nothing found. |
| 几篇 | {n} 篇 | 1 document、{n} documents |
| 用不了 | （用不了：{why}） | (unavailable: {why}) |
| PDF 第几页 | 第 {page} 页 | page {page} |
| 读的第几页 | （第 {page}/{pages} 页） | (page {page} of {pages}) |
| 建好了 | 建好了：{kb} | Created {kb} |
| 改好了 | 改好了：{kb} | Updated {kb} |
| 改什么 | 改什么：写 --name、--description、--auto 或者 --no-auto。 | Nothing to change: give --name, --description, --auto or --no-auto. |
| 开了自动想起 | 自动想起 | auto recall |
| 软件包带来的 | 随 {package} | from {package} |
| 不认的格式 | 只收 .md、.markdown、.txt、.pdf、.html、.htm：{name} | Only .md, .markdown, .txt, .pdf, .html, .htm can be added: {name} |
| 编号写错 | 库的编号要小写字母开头，只能有小写字母、数字、-、_，最多 64 个：{id} | A knowledge base id starts with a lowercase letter and has only lowercase letters, digits, - and _, at most 64: {id} |
| 页码出界 | 没有第 {page} 页，一共 {pages} 页。 | There is no page {page}; it has 1 page.、…; it has {pages} pages. |
| 加进去了 | 加进去了：{kb}/{path} | Added {kb}/{path} |
| 删了一篇 | 挪进回收站了：{trash} | Moved to the trash: {trash} |
| 挂上了 | 挂上了：{kb}，{n} 篇 | Mounted {kb}: {n} documents |
| 删了库 | 删掉了 {kb}，在回收站里：{trash} | Deleted {kb}; it is in the trash: {trash} |
| 重建好了 | 重建好了：{kb}，{n} 篇 | Rebuilt {kb}: {n} documents |

核心拒绝时说的话照核心写的原样印（`kb_exists`「知识库已经有了。」、`document_exists`「这一篇已经有了。」、`kb_read_only`「挂进来的知识库是只读的。」、`folder_unavailable`「这个目录挂不上。」、`trash_failed`「挪不进回收站。」，K-7 下补的）。

**帮助页**：`-h`、`--help`、`miyu help kb`、各个子命令的 `-h` 印的都是这一页，规矩见 `cli/main.md`「帮助页」。

样本 `crates/miyu-cli/src/help/zh/kb.txt`（中文）：

```text
用法：miyu kb [命令] [选项]

管知识库：放给 AI 查的文档。不写命令就是 list。
加的文件、删的文档、删的库都进系统回收站，找得回来。

命令：
  list                    列出知识库
  search <词>             搜
  read <库> <文档>        读一篇的一页
  new <库>                建一个库
  edit <库>               改名字、说明、自动想起
  add <库> <文件>...      把文件加进库，PDF、网页、文本都行，一个最多 20 MiB
  remove <库> <文档>      删一篇
  mount <库> <目录>       把现成的目录挂成一个库，只读，不改你的文件
  delete <库>             删整个库；挂进来的只拿掉登记，不动你的目录
  reindex <库>            重建索引

选项：
      --kb <库>           search：只搜这一个库
      --page <页>         read：第几页，不写是第一页
      --name <名字>       new、edit、mount：给人看的名字
      --description <说明>
                          new、edit、mount：一句话说这个库是什么
      --auto              new、edit、mount：开自动想起：人提到这个库里某一段的
                          名字、或者意思够像时，自动带给 AI 那一段的开头
      --no-auto           edit：关自动想起
      --to <目录>         add：放进库里的这个目录
      --replace           add：已经有的换掉
      --format text|json  list、search：json 给脚本
  -h, --help              印帮助
```

样本 `crates/miyu-cli/src/help/en/kb.txt`（英文）：

```text
Usage: miyu kb [command] [options]

Manage knowledge bases: documents the AI can look things up in.
Without a command it is list. Removed documents and deleted bases go to
the system trash, so you can get them back.

Commands:
  list                    List knowledge bases
  search <words>          Search
  read <kb> <document>    Read one page of a document
  new <kb>                Create a knowledge base
  edit <kb>               Change the name, description or auto recall
  add <kb> <file>...      Add files: PDF, web pages, text; 20 MiB each at most
  remove <kb> <document>  Remove a document
  mount <kb> <folder>     Mount an existing folder, read-only; your files stay
                          as they are
  delete <kb>             Delete a whole knowledge base; a mounted one only
                          loses its registration, the folder stays
  reindex <kb>            Rebuild the index

Options:
      --kb <kb>           search: only this knowledge base
      --page <page>       read: which page, the first one by default
      --name <name>       new, edit, mount: the name people see
      --description <description>
                          new, edit, mount: one sentence about the base
      --auto              new, edit, mount: turn on auto recall: when someone
                          names a section of this base or says something close
                          to it, the AI gets its opening lines
      --no-auto           edit: turn off auto recall
      --to <dir>          add: put the files in this folder of the base
      --replace           add: replace ones that are already there
      --format text|json  list, search: json for scripts
  -h, --help              Print help
```

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-cli/src/kb/tests.rs` | 印的样子：库一行一个、编号对齐、名字和编号一样的不写、用不了的接上为什么、英文的单复数、开了自动想起的篇数后面接一格（K-6 补）；搜到的一块一段、PDF 说第几页；读的不止一页的末尾说第几页；不认的格式、编号写错、页码出界的话中英文（K-7 修） |
| `crates/miyu-cli/tests/kb.rs` | 真的核心：每个子命令发什么、印什么、退出码；加文件照本机路径传、`--to`、已经有的拒、`--replace` 换；挂目录照当前目录接相对路径、挂不上的接上为什么、挂进来的删拒、删库挂的目录没动；`edit` 什么都没写的说一句、退出码 1；没有的库照原话；`--auto` 开、`--no-auto` 关（`kb.toml` 那一格有没有），`new`、`edit`、`mount` 都收，只写开关的 `edit` 也算改了，两个一起写读不懂（K-6 补）；不认的格式一个都不传、说只收哪几种，编号写错说写法、退出码 2，页码出界说一共几页，别的错照原话（K-7 修） |
| `crates/miyu/tests/kb.rs` | 真跑主程序：`kb -h`、`--help`、`help kb`、几个子命令的 `-h` 印的都是这一页，跟着界面语言；参数写错的九种退出码 2 |
| `crates/miyu-cli/src/help/tests.rs` | 这一页列的选项和程序真有的对得上 |
| `crates/miyu-endpoint/src/knowledge/tests.rs` | 知识库的每一种拒绝都有给人看的话（K-7 下补的五种） |

### 出处

- `19-知识库.md` 第六节（命令行：`miyu kb list`、`add`、`remove`、`mount`、`search`、`read`、`reindex`）。
- 施工单 `K-7-命令行（下）.md`；建库叫 `new`、加文档照附件上传、删库进回收站（2026-10-13 项目主人定）；只放人要的（2026-10-08 项目主人定）。

### 还没有的

- 终端界面、网页的知识库页：两个头的会话做。
- 加整个文件夹：要整个目录的请挂。
