## 施工单 AUD-5：LiteMiyu 审计出的七条和量尺的家目录

状态：已完成（2026-11-18）。

### 为什么

LiteMiyu 那边 2026-11-18 做审计时，顺着读了 Miyu 的代码，判了七条 Miyu 上也有的毛病，另有一条量尺的（它自己的量尺踩过）。这些是读代码判的，没在 Miyu 上跑过。核心主会话交来：逐条先核实，成立的改前红再修，不成立的写清为什么。

八条里七条成立，一条（#057②）不成立。

### 逐条

| 审计编号 | 毛病 | 在哪 | 怎么修的 |
|---|---|---|---|
| #064① | `miyu ask` 把模型给的思考、回答原样写到终端：里面的终端控制序列（它转述的网页、文件里的也算）能改标题、清屏、挪光标、伪造一行输出 | `miyu-cli` 的 `ask/follow.rs` | 口径照项目主人给 LiteMiyu 定的：**接终端的那一路洗，接管道的原样**。`Screen` 多一格「标准输出是不是终端」；回答照它、思考照标准错误是不是终端，控制字符换成 `�`，换行、制表符照留，回车去掉。逐个字洗，一个序列被切在两段增量里也洗得掉 |
| #056 | 远程 MCP 服务回排了版的（多行）JSON、一个 SSE 事件拆成几行 `data:` 时握手失败 | `miyu-mcp` 的 `http.rs`、`http/answer.rs` | JSON 回应整段是一个 JSON 值的当一条、紧凑写成一行，解不出的照旧按行拆；SSE 一个事件接出来带换行的解成 JSON 再写成一行。原来「一行一条」能过的照旧 |
| #057② | （审计说的）配置里自己写的 `Authorization` 头被硬冠上 `Bearer` | `miyu-endpoint` 的 `mcp/launch.rs` | **不成立，没改**，见下面「不成立的一条」 |
| #053 | 会话日志一批写到一半失败（盘满这类），段文件里留下半行，下一批接在半行后面 | `miyu-store` 的 `log.rs` | 写失败就把这一段截回写之前的长短，再交回写的那个错 |
| #061② | `journal.jsonl` 新建时是 0644 | `miyu-store` 的 `journal.rs` | Unix 上新建是 0600，已经在的不动 |
| #062② | 读日志的一截时，终点那一段文件没了，少读一截还交回读成了 | `miyu-store` 的 `log/open/lines.rs` | 终点落在一段中间或末尾而那一段没了的，照起点那一段没了一样报「读的时候日志变了」；终点正好是一段的开头的合法，照旧 |
| #066① | 原子写的临时文件名比原名多二十来个字节：原名 235 到 255 个字节的合法文件写不了 | `miyu-fs` 的 `replace.rs` | 临时名总长限在 255 个字节里，超了截短原名那一截、截在字的边界上 |
| 量尺 | 对比基准起被测程序时带着宿主的真家目录：Miyu 只指了数据根和缓存、codex 只指了 `CODEX_HOME`，`HOME` 是继承的 | `bench/rivals/rivals.toml`、`toolstep.py` | 每一家的 `HOME`、`XDG_*` 都指进它自己的沙箱家目录 |

核实时多看到、一起改的一处（核心主会话点头）：会话日志的**段文件**新建时也是 0644，和 #061② 是同一类。Unix 上新建的改成 0600，已经在的段不动。

### 不成立的一条：#057②

- **审计说的**：`launch.rs` 把 `bearer` 放在请求头最前面、取完值以后给「排第一的 `Authorization`」冠上 `Bearer `；人没写 `bearer`、在 `headers` 里自己写了 `Authorization: Basic …` 的，也会被冠上。
- **为什么不成立**：`headers` 里写不进 `Authorization`。读配置时就拒了：`crates/miyu-config/src/package/mcp.rs` 的 `valid_header` 不收 `Authorization`（它归 `bearer`）和 `Mcp-Session-Id` 那几个，写了的整张表报 `bad_mcp`；`Reach::Url` 全仓只有这一处造（同一个文件），手写的配置、`mcp.set`、包的清单都经它。测试 `crates/miyu-config/src/package/mcp/tests.rs` 里 `headers = { Authorization = "b" }` 那一条钉着。所以到 `launch.rs` 的 `headers` 里不会有别人写的 `Authorization`，排第一的 `Authorization` 只可能是 `bearer` 换出来的那一条。
- **没改代码**。读代码时我和帮手都先判了成立（只看了 `launch.rs` 那一段），帮手还写了测试、改成「只冠在 `bearer` 那一条上」；对蓝图时看到「`Authorization` 不收」才回头查到上面这一层，改动撤掉了。那段写法确实靠着上游的检查才对，要不要改成不靠它，另说。

### 不做

- #053 截短本身也失败时（比如文件描述符用完）：交回的照旧是写的错，截的错不另记，也不留「记着坏了、下次先重截」的标记。那以后的写仍可能接在半行后面，打开时的自检查得出。核心主会话定这一步不加。
- #057 的环境变量继承：照旧全继承（项目主人定的）。

### 验收

- 每一条改前红：先写测试，在没改的代码上看到它红、红的就是这个毛病，再修。
- 门禁、Windows 目标的 clippy、三台 CI。
- 实测：`miyu ask` 接终端和接管道各看一遍；起一个回排了版的 JSON 的本机 MCP 服务，配上自己写的 `Authorization` 头，看握手成不成、服务收到的头是什么；量尺的环境照新的配一遍看数据根认不认。

### 做下来的

改前红（各条测试在没改的代码上的样子）：

- #064①：`ask::follow::tests::escapes` 三条里红两条——接终端时回答 `"答\x1b]0;x\x07\x1b[2J\r伪造\n"` 原样出来了，该是 `"答�]0;x��[2J伪造\n"`；接管道原样的那条本来就绿。
- #056：`http::tests::pretty` 三条都红——握手等到超时；排了版的回应被拆成 `["{", "  \"id\": 1,", …]` 五条。原来有一条测试钉着带换行的 `"{\"b\":\n2}"`，钉的正是这个毛病，改成 `"{\"b\":2}"`。
- #053：`log::tests::torn` 两条——段文件长 568、该是 426；空的段留下 142 个字节。
- #061②、段文件：`a_new_journal_is_private`、`new_segments_are_private`——权限是 0644（420），该是 0600（384）。
- #062②：`a_stretch_whose_last_segment_is_gone_is_an_error`——交回的是读成了。
- #066①：`a_name_at_the_limit_is_written`——`File name too long`（253 个 `a` 加一个 `é`，共 255 个字节）。
- 量尺：`test_every_rival_gets_a_home_inside_its_sandbox`、`test_both_rivals_get_a_home_inside_the_out_dir`——Miyu、codex 的 `HOME` 是宿主的。

实测（2026-11-18，沙箱数据根和家目录，程序是这一步编出来的）：

- **#056**：本机起一个小的 MCP 服务，回应一律写成排了版的多行 JSON（不认识的方法回的错也是），配成 `[mcp.pretty]`、带 `bearer` 和一条 `X-Team` 头。这一步的程序：`mcp.list` 里它是 `running`、工具 `mcp__pretty__hello` 列出来了，运行日志 `mcp ready service=pretty`；服务每个请求收到的是 `Authorization: Bearer <配置里的值>`。没改的程序（main 上一版）连同一个服务：一直停在 `starting`，运行日志连着几行 `WARN MCP line not understood problem="not a JSON object"`。
- **#057②（不成立的那条）的证据**：把配置里的头改成 `headers = { Authorization = "Basic abc" }`，`miyu check` 报「MCP 服务的写法不对：mcp.pretty.headers」，到不了连服务那一步。
- **#064①**（真模型 `opencode-go/deepseek-v4.1-flash`，2 次请求，没遇到 429）：让她把一段带着 ESC、BEL 的字（`A<ESC>]0;PWNED<BEL>B<ESC>[2JC`）原样重复。接管道的：标准输出逐字节是 `A 033 ] 0 ; P W N E D \a B 033 [ 2 J C`，原样。在伪终端里跑的（`script` 转存下来的原始字节）：回答那一行是 `A�]0;PWNED�B�[2JC`，ESC、BEL 都没有了；思考外面我们自己加的灰色序列照旧在。
- **#061② 和段文件**：这一步的程序新建的 `journal.jsonl`、会话日志的第一段都是 `600`。
- **量尺**：照 `rivals.toml` 新配的环境（清空环境变量，`HOME` 和数据根是同一个空目录，`XDG_*` 指进去）跑 `miyu config set`、`miyu check`、`miyu core status`：数据根认得出、建得好、「没有问题」；往那个目录里再放 `.bashrc`、`.config/` 以后照旧。量尺自己的 31 条测试过。
- 没实测的：#053（要造盘满）、#062②（要在读的当口删掉一段）、#066①（要一个 255 字节的文件名走一遍写文件的工具），这三条只有测试。量尺没有真跑一趟对比。
