## `web_fetch`

### 是什么

联网包（`miyu-base-net`）的工具（施工 N-1，设计 `36-联网搜索.md` 第一节第 3、4 条）：抓一个网址，直接把正文交给她。网页换成 Markdown，纯文字、JSON、XML 照原样；太长的分页，同一个地址十分钟内翻页不重抓。图、PDF、压缩包这些说一句读不了。地址闸、代理、跳转和链接卡片是同一套（`net.md`）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-net/src/web_fetch.rs` | 参数、分页、交回的字、给人看的那一句、出错的几句 |
| `crates/miyu-net/src/document.rs` | `Pages`：懒读规矩、抓、认种类、换字、记着翻页用的几份 |
| `crates/miyu-net/src/document/convert.rs` | 认种类、照字符集解成字、HTML 换成 Markdown（`htmd`）、标题、收拾空行 |
| `crates/miyu-net/src/fetch.rs` | `Fetcher::document`：一跳一跳抓整份（有上限），过闸、跟跳转、认人机验证页 |
| `crates/miyu-net/src/web_fetch/rescue.rs` | 抓不下来的值不值得请别家、请到的记进 `Pages`（施工 N-4） |
| `crates/miyu-net/src/search/pages.rs` | `Search::fetch`：照 `search.json` 的 `fetch` 一家家代抓，各家怎么问、怎么读（施工 N-4） |
| `crates/miyu-core/src/lib.rs` | `groups()`：联网包装着就登记（cargo 开关 `net`） |
| `resources/software/net/tools/web_fetch.json` | 说明和参数格式 |
| `resources/software/net/web_fetch/*.txt` | 输出里给她看的几句 |
| `resources/software/net/human/{zh,en,ja}.json` | 显示名、结果那一句 |
| `resources/software/net/link_preview.json` 的 `document` | 时限、上限、`Accept`、一页几个字、记多久、记几份 |
| `resources/packages/miyu-base-net/package.toml` | 功能 `web-fetch`（「抓取网页」） |

### 对外的样子

访问类别 `read`：只读、不改东西，本机的工作区级别不弹确认，只读模式下也能用；场所的会话（群聊、私聊）、子会话的工具面上都有（设计 36 第一节第 4 条，项目主人 2026-10-18 定）。

样本 `resources/software/net/tools/web_fetch.json`：

```json
{
  "description": "Fetch a web page or text file by URL and read it as Markdown, a page at a time.",
  "parameters": {"type":"object","properties":{"url":{"type":"string"},"page":{"type":"integer","description":"From 1. Default 1."}},"required":["url"]},
  "clicall": true
}
```

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `url` | 是 | 去掉前后空白读成网址，只收 `http`、`https` |
| `page` | 否 | 第几页，从 1 起，默认 1 |

交回的字（成了的）：

```text
Redirected to https://example.com/new.        ← 只有跟过跳转的，空一行接正文

# 正文的标题
……正文……
(Page 1 of 3.)                                ← 只有不止一页的
(The rest of the page was too large to fetch.) ← 只有超了上限、最后一页
```

给人看的那一句：有标题的写标题（超过 60 个字的截断加 `…`），不止一页的接上第几页；没有标题的一页写「抓到了」。不带 `detail`。

### 怎么走

1. 读参数，读不懂的：参数不对（`bad-args`）。
2. 读成网址：读不成、不是 `http`/`https` 的，`not-a-url`，不碰网络。
3. 规矩第一次用时读（`link_preview.json`），读不懂的记一条 `WARN web fetch not ready`，这个核心的生命周期里不再试，每次都回 `not-ready`。
4. 同一个网址（读成网址以后的写法）在 `keep_seconds`（600）之内抓过的，照记着的那一份分页，不重抓。记满 `keep_entries`（32）份整个清空。一个核心一份，各会话共用；替机器主人抓的、替别人抓的分开记（施工 MB-2，见下一条）。
   - 替谁抓（施工 MB-2，2026-10-22 项目主人定）：这一轮开口的是主人（调用的事实 `Caller::owner`：本机的会话是，场所会话照这一轮开口的人）的照这一页；别的（群里的人，拿不准的也算）另有两条：系统配置 `net.internal_domains` 里的域名和它的子域名一律不去，先查它再看记着的，每一跳都查（跳过去的也拒）；本机解析不出来、要交给代理的，主机名是一个词的（`nas`）、带内网后缀的（`link_preview.json` 的 `internal_suffixes`：`.local`、`.lan`、`.internal`、`.intranet`、`.corp`、`.home`、`.home.arpa`、`.localdomain`）不交，别的照旧交（被墙的境外站照样抓得到）。这两条拦下的都是 `blocked`，不请别家代抓（内部的网址不发给第三方）。
5. 抓（`Fetcher::document`）：每一跳过地址闸（`net.md`「地址闸」），不走代理的钉住解析好的地址，自己跟跳转，最多 5 跳；每一跳时限 `timeout_seconds`（20），读到 `max_bytes`（5 MiB）为止，超了截断、记下 `cut`。请求头照 `user_agent`、`accept_language`，`Accept` 照 `document.accept`。
   - 闸拦下的：`blocked`；连不上、超时：`unreachable`；跳太多：`redirects`。
   - 人机验证页（`net.md` 第 13 条那一套认法，不看状态码）：`challenge`。
   - 2xx 以外：`status`，带状态码。
6. 认种类（`convert::kind`）：`text/html`、`application/xhtml+xml` 是网页；`text/*`、JSON、XML、YAML、`+json`、`+xml` 是文字；没写、写 `application/octet-stream` 的照内容猜（开头像 HTML 的是网页，整份是 UTF-8、没有 NUL 的是文字）；别的 `unsupported`，带媒体类型。
7. 照字符集解成字：回应头的 `charset`，网页再看开头 2048 字节里的 `<meta charset>`、`content="…; charset=…"`，都没有照 UTF-8；认不出的字符集照 UTF-8，读不了的字换成替换符。
8. 网页换成 Markdown（`htmd`）：`head`、`script`、`style`、`noscript`、`template`、`svg`、`canvas`、`iframe`、`object`、`embed`、`nav`、`footer`、`form`、`button`、`select`、`dialog` 整段扔掉。先建树，节点超过 20 万个或者深过 160 层的不换（`too-complex`）：换的时候是递归的，故意嵌套几万层的页面能把栈用光。换之前剪掉读的人看不到、用不上的：带 `hidden`、`aria-hidden="true"` 的，`role` 是 `navigation`、`banner`、`contentinfo`、`search`、`complementary` 的，链接带 `hreflang` 的列表项（换语言的那一串）；只照属性认，不照 class 猜。有 `<main>` 的只换第一个 `<main>`，没有的、只有一个 `<article>` 的只换它，都没有的换整份（维基、文档站的页头、侧栏、语言列表在外面，2026-10-18 照维基百科、ArchWiki 实测）。换完去掉没有字的链接 `[](…)`。换在阻塞线程里做，同时至多两份。标题照 `<title>`。
9. 收拾：每行去掉行尾空白，连着的空行并成一个，去掉前后空白。一个字都没有的：`empty`。
10. 分页：每页至多 `page_chars`（30000）个字，尽量断在这一页后五分之一里最后一个换行上，断不了的照字数断；拼回去一个字不差。要的那一页没有的（0、超过页数）：`no-page`，带一共几页。
11. 叫停：不看叫停的旗，抓最多 5 跳 × 20 秒。
12. **请别家代抓**（施工 N-4，设计 36 的 N-4，2026-10-19 项目主人点头）：自己抓的是 `challenge`、`empty`、`unreachable`、`status` 401、403、429、451、503，或者抓到了但正文少于 `fetch.min_chars`（200）个字（登录墙、只有「载入中」的壳）的，交给搜索的家底（`Search::fetch`）照 `search.json` 的 `fetch.order`（Exa → Firecrawl → Parallel，照实测排：Exa 快、命中多，Parallel 能抓知乎、豆瓣但慢、免费额度小）一家家试；设置里挑的搜索服务供应商是这三家之一的排在最前；冷却着的跳过；和搜索共用冷却，报限额的搜和抓一起歇。一家回来的正文少于 `min_chars` 的当没抓到、换下一家（不冷却）；超过 `max_chars`（20 万）的截掉、记 `cut`。时限 `fetch.timeout_seconds`（45）：代抓常要现抓，Parallel 实测 12～28 秒。抓到的记进 `Pages`（翻页不再请），给她的字和自己抓的一样，给人的那一句多 `via`（是哪一家，同 `web_search`）；都不成的照自己抓的那个原因说。不是网址、内网、跳转太多、读不成字的种类、404 这些不请别家。
   - Exa 调 `web_fetch_exa`（`urls`、`maxCharacters`），回一段字：去掉开头的 `# 标题` 和紧跟着的 `URL:`、`Published:` 这类行；Firecrawl 调 `firecrawl_scrape`（`formats: [markdown]`、`onlyMainContent`），读 `markdown`、`metadata.title`；Parallel 调 `web_fetch`（`full_content`、`session_id` 同搜索的），读 `results[0]` 的 `full_content`（没有的接 `excerpts`），没结果的照 `errors[0].error_type`。

### 出错

都是 `Done::error`，给她一句英文（`web_fetch/*.txt`），给人一句（`human/` 里同名的键）：`bad-args`、`not-a-url`、`blocked`、`unreachable`、`redirects`、`challenge`、`status`、`unsupported`、`too-complex`、`empty`、`not-ready`、`no-page`。

运行日志（目标 `miyu::net`）：`WARN web fetch not ready error=…`，只记第一次。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-net/src/document/convert/tests.rs` | 认种类、猜种类；字符集照头、照 `<meta>`、退回 UTF-8，GBK；标题收拢、实体；扔掉脚本、导航、表单、页脚；太深的不换（去掉这道检查的变异把测试的栈用光）；只换 `<main>`、一个 `<article>`，两个的换整份；剪掉藏起来的、导航类的 `role`、换语言的列表项；去掉没有字的链接；收拾空行 |
| `crates/miyu-net/src/web_fetch/tests.rs` | 分页：不超上限、拼回去一字不差、断在换行上、太靠前的换行不用、空的是一页 |
| `crates/miyu-net/tests/fetch_members.rs`（施工 MB-2） | 别人要抓：本机、内网、`*.local`、`localhost` 都 `blocked`；配了代理、本机解析不出来的，一个词的、带内网后缀的不交给代理（`blocked`），别的经代理照常；机器主人照旧交；机器主人抓到的不给别人；拿不到调用的事实的照最严；内部域名（大小写、带点都认）和子域名、解析成公网的、跳过去的都 `blocked`，只是长得像的不算；机器主人不受清单限制 |
| `crates/miyu-net/tests/fetch.rs` | 本机的假服务器上整条走通：访问类别 `read`、只有 `url` 必填；网页换成 Markdown、标题给人看；JSON 照原样、GBK 照 `<meta>`、照回应头的 `charset` 解；超了 5 MiB 截断、最后一页说一声、页数照截下来的算；人机验证页说一句；十万字分四页、拼回去一字不差、翻页只抓一次、没有的页；跳转写落到了哪；404、图、换完是空的、`ftp`、解析到内网的各说一句；没开测试的口子时回环上的服务器被闸拦下、一次都没连 |
| `crates/miyu-net/src/search/pages/tests.rs`（施工 N-4） | 三家的参数；Exa 去掉开头几行、正文里像那样的行留着；Firecrawl、Parallel 读 JSON，Parallel 没结果照 `errors`；读不懂的照原话认限额 |
| `crates/miyu-net/tests/fetch_rescue.rs`（施工 N-4） | 假服务器上：403 的请 Exa、给人写 `via`、翻页不再抓也不再请；自己的、Exa、Firecrawl 都短得像登录墙的走到 Parallel；挑了 Firecrawl 的先问它；都不成的照自己的 `status`；404 不请别家；空页也请 |
| `crates/miyu-net/tests/budget.rs` | 工具说明不超过 283 字节 |
| `crates/miyu-net/tests/live.rs` 的 `real_pages_are_read_by_web_fetch`（`#[ignore]`，还要设 `MIYU_NET_LIVE=1`） | 真网络：英文、中文维基，ArchWiki，GitHub 的接口（JSON），raw 的 README（纯文字），rust-lang.org（跳转）各读得出、开头和末尾印出来给人看 |
| 同一份的 `blocked_pages_are_fetched_by_other_sources`（施工 N-4） | 真网络：Stack Overflow、纽约时报、B 站专栏、豆瓣自己抓不下来，请别家抓到了（2026-10-19：前三个 Exa、豆瓣 Parallel） |

### 出处

- 设计 `36-联网搜索.md` 第一节（项目主人 2026-10-18 定：直接交正文、分页，哪里都不问人）。
- Claude Code 的 `WebFetch`（名字、只读）；旧版 `html_conversion.rs`（嵌套太深把栈用光的教训）。
- `htmd` 0.5（Apache-2.0）：照 turndown.js 的规矩，底下是 html5ever。

### 还没有的

- PDF 换字。
