# 联网搜索和抓网页

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

AI 怎么搜网页、读网页，用哪一家联网服务、怎么填 key，怎么关掉，和它上网时碰不到哪些地址，都在这一页。

## 两件工具

| 工具 | 界面上叫 | 做什么 |
|---|---|---|
| `web_search` | 网络搜索 | 搜网页，交回最多 8 条标题、网址、日期和摘录。AI 能限定只要、或者不要哪几个网站 |
| `web_fetch` | 抓取网页 | 照网址取一个网页或文本文件，换成文字一页一页读 |

两件都在出厂软件包「联网搜索」（`base-net`）里，出厂装着。出厂的预设里，「全部功能」「常规开发」开着它们，「极简开发」「无功能」没开。

不配任何 key 也能搜：出厂会依次试几家不要 key 的搜索服务，见下面。

## 联网服务

设置页里叫「联网服务」，配置键 `net.search.source`，管 AI 搜网页、抓网页用哪一家。能选的：

| 选项 | 要什么 | 配置键 | 管什么 |
|---|---|---|---|
| `auto`（「自动（免费额度）」） | 不要 | | 搜索照下面免费的几家依次试；自己抓不下来的网页请 Exa、Firecrawl、Parallel 代抓 |
| `deepseek`（DeepSeek） | key。没单独配的，照用模型那边 DeepSeek 供应商的 key。搜索、代抓都按次收费 | `net.search.deepseek.key` | 搜索、抓网页都先交它 |
| `tavily`（Tavily） | key | `net.search.tavily.key` | 搜索先交它，抓网页照自动 |
| `brave`（Brave） | key | `net.search.brave.key` | 搜索先交它，抓网页照自动 |
| `searxng`（SearXNG） | 你自己搭的 SearXNG 的地址 | `net.search.searxng.url` | 搜索先交它，抓网页照自动 |

key 去各家的网站申请。装了的扩展也能提供搜索服务，选项写成 `<包>/<编号>`，只在你选了它时用，只管搜索。

自动用的是不要钱的几家，不能单选，依次是：

| 先后 | 哪一家 | 要什么 |
|---|---|---|
| 1 | Exa | key 可配可不配：不配用免费额度，配了用你自己的额度（`net.search.exa.key`），搜索、代抓都是 |
| 2 | Parallel | 不要 |
| 3 | Firecrawl | 不要 |
| 4 | AnySearch | 不要 |
| 5 | You.com | 不要 |
| 6 | DuckDuckGo | 不要 |
| 7 | Bing | 不要 |

自动不用 DeepSeek，模型那边配了 DeepSeek 也不用，要用就选它。

搜索的先后和回退：

- `auto`：照上表的先后，一家一家试，第一家搜成了就用它的结果。
- 选了一家：先用它；它没配 key 或地址、出错、被限额了，再照自动的先后试。
- 一家超时（20 秒）、出错的，这一次换下一家。
- 一家说被限额了，歇一阵再用：连着被限额依次歇 30 分钟、1 小时、2 小时、6 小时，之后每次 6 小时；成功一次就从头算。歇着的记在核心的内存里，核心重启就清了。DeepSeek 搜索、代抓一起歇。设置页的下拉里，歇着的那一家后面写「冷却到 15:30」这样的时刻，没配 key 的写「要 key」，没配地址的写「要地址」。
- 几家都在歇着，这一次搜不成，时间线上写「几家搜索都被限额了，过会儿再试」。

抓网页的先后见下面 [抓网页的先后](#抓网页的先后)。

## 换一家、填 key

以 Tavily 为例：

1. 存 key（贴的时候不显示）：

   ```sh
   miyu provider login tavily
   ```

2. 让配置引用它：

   ```bash
   miyu config set net.search.tavily.key '{ secret = "tavily" }'
   ```

   PowerShell 里外面用双引号，里面用 TOML 的单引号字符串，Windows PowerShell 5.1 和 PowerShell 7 都能照抄：

   ```powershell
   miyu config set net.search.tavily.key "{ secret = 'tavily' }"
   ```

3. 选它：

   ```sh
   miyu config set net.search.source tavily
   ```

从下一轮起生效。key 也能写成 `{ env = "变量名" }`，照核心起来时的环境变量取。密钥怎么存、怎么删见 [配置](config.md#密钥)。

在设置页里做：设置页（终端、网页里敲 `/settings`）「软件包」，打开「联网搜索」这个包的设置。「联网搜索」一组里是「联网服务」下拉；「API Key 和地址」一组里有 Exa、DeepSeek、Tavily、Brave Search 的 key 和「SearXNG 的地址」。key 粘贴进框里，存下的是密钥的引用，key 本身进密钥文件。

这几个键只认 `~/.miyu/etc/config.toml`（数据根默认 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`），写进项目配置不算。

### 用 DeepSeek

模型那边已经配了 DeepSeek 供应商的，选它就行，key 照用那一家的：

```sh
miyu config set net.search.source deepseek
```

想给联网单用一个 key，照上面 Tavily 的做法存好，再写 `net.search.deepseek.key`。

「模型那边的 DeepSeek 供应商」这样认：`[providers.<编号>]` 写了 `catalog` 的，`catalog` 是 `deepseek` 才算；没写 `catalog` 的，`base_url` 里有 `api.deepseek.com` 才算（写成 `{ env = "…" }` 的不算）；两样都没写的，编号是 `deepseek` 才算。几家都算的，用编号排在最前、写了 key 的那一家。

### 自建的 SearXNG

SearXNG 要开着 JSON 格式的结果。填地址，再选它：

```sh
miyu config set net.search.searxng.url http://127.0.0.1:8888
miyu config set net.search.source searxng
```

地址也能写 `{ env = "变量名" }`。搜索服务的地址不受下面「碰不到的地址」限制，SearXNG 搭在本机、内网都行。

## web_fetch 能抓什么

| | |
|---|---|
| 网址 | 只认 `http`、`https` |
| 网页 | 换成 Markdown。脚本、样式、导航、页头页脚、表单扔掉；有 `<main>` 的只要第一个 `<main>`，没有的、只有一个 `<article>` 的只要它 |
| 文本 | 纯文本、Markdown、JSON、XML、YAML、JavaScript 照原样。服务器没说类型的，像网页的当网页，是 UTF-8 文字的当文字 |
| 别的 | 图片、PDF、压缩包这些不收，时间线上写「… 是 <类型>，读不成字」 |
| 页面里的脚本 | 不跑 |
| 字符集 | 照服务器说的，没说的看网页开头的 `<meta charset>`，再没有照 UTF-8。GBK 的老网站照样读得对 |
| 大小 | 最多读 5 MB，多的截掉，最后一页末尾说一声 |
| 超时 | 每一跳 20 秒，解析、连接、读完都算在里面 |
| 重定向 | 自己一跳一跳地跟，每一跳都重新查地址，最多 5 次，多了算失败 |
| 分页 | 每页最多 30000 个字，AI 带页码接着读。同一个网址 10 分钟内翻页不重抓 |

网页结构太大（超过 20 万个节点）或嵌套太深（超过 160 层）的不换，时间线上写「… 太大或嵌套太深，换不成字」。

### 抓网页的先后

1. 联网服务选了 DeepSeek、配了 key 的，先交 DeepSeek 抓，不先自己抓。下面「碰不到的地址」里那些、通讯平台上别人让抓的内部域名，还有本机解析不出来、名字又像内网的（只有一个词、或者以 `local`、`lan`、`internal`、`intranet`、`corp`、`home`、`home.arpa`、`localdomain` 结尾），不交给它。网址超过 8192 个字、带用户名密码的也不交。
2. 自己抓。没选 DeepSeek 的从这一步开始。
3. 自己抓不下来的，请 Exa、Firecrawl、Parallel 依次代抓。抓不下来指：碰上人机验证页、回 `401`、`403`、`429`、`451`、`503`、连不上、一个字都没有、或者短得像登录页（少于 200 个字）。第 1 步问过 DeepSeek 的不再问它。

代抓的最多 20 万个字，45 秒超时，抓回来少于 200 个字的当没抓到。代抓成了的，时间线上写是哪一家抓的。都不成的，照自己抓的那个原因报。

### 碰不到的地址

网址是 AI 写的，也可能是网页里骗它的。下面这些不去，时间线上写「… 是本机或内网的地址，抓不了」：

- 网址里带用户名、密码的。
- `localhost`、`*.localhost`、`*.local`。
- 解析出来的地址只要有一个不是公网的：本机（`127.0.0.0/8`、`::1`）、内网（`10.0.0.0/8`、`172.16.0.0/12`、`192.168.0.0/16`、`fc00::/7`）、链路本地（`169.254.0.0/16`、`fe80::/10`）、运营商 NAT（`100.64.0.0/10`）、组播、广播、文档用的、保留的。IPv6 里嵌着 IPv4 的照那个 IPv4 判。
- 网址直接写 IP 的照同样的规矩判，`127.1`、`0x7f000001` 这类写法也认得出。

查过的地址就是连上的地址，中间不再解析第二次。每一跳重定向都重新查。

### 通讯平台上别人让 AI 抓的

群里、不是你本人的私聊里开口让 AI 抓网页的，多两条：

- `net.internal_domains` 里的域名和它们的子域名一律不去，不管解析成什么地址、走不走代理：

  ```bash
  miyu config set net.internal_domains '["example-corp.com", "intra.example.net"]'
  ```

  ```powershell
  miyu config set net.internal_domains "['example-corp.com', 'intra.example.net']"
  ```

- 走代理、本机又解析不出来的主机名，只有一个词的（没有点）或者以 `local`、`lan`、`internal`、`intranet`、`corp`、`home`、`home.arpa`、`localdomain` 结尾的，不交给代理。

你自己让 AI 抓的不受这两条限制。你抓到的网页不会给别人看。

## 代理

没有自己的配置键，照核心的环境变量：`HTTPS_PROXY`、`HTTP_PROXY`、`ALL_PROXY`、`NO_PROXY`，小写的也认。搜索和抓网页都走它。核心头一次用到时读一次，改了要重启核心：

```sh
miyu core stop
```

用服务起的核心看不到你 shell 里的变量，见 [常驻和服务器](service.md)。

走代理时，本机解析不出来的主机名照样交给代理去连，被墙的网站也抓得到。

## 配置键

| 键 | 做什么 | 默认 |
|---|---|---|
| `net.search.source` | 联网服务 | `"auto"` |
| `net.search.exa.key` | Exa 的 key | 不配，用免费额度 |
| `net.search.deepseek.key` | DeepSeek 搜索、代抓的 key | 不配，照用模型那边 DeepSeek 供应商的 key |
| `net.search.tavily.key` | Tavily 的 key | 不配 |
| `net.search.brave.key` | Brave Search 的 key | 不配 |
| `net.search.searxng.url` | 自建 SearXNG 的地址 | 不配 |
| `net.internal_domains` | 别人让 AI 上网时一律不去的域名，每项最多 253 个字 | 不配 |

都从下一轮起生效，只认 `~/.miyu/etc/config.toml`。全部键见 [配置键全表](config-keys.md)。

## 关掉

- 只关一件：在预设里关「网络搜索」（`web-search`）或「抓取网页」（`web-fetch`）。预设文件里写：

  ```toml
  [features]
  web-search = false
  web-fetch = false
  ```

- 整个包关掉，你所有会话里都没有这两件：设置页「软件包」里「联网搜索」的开关，或者：

  ```sh
  miyu config set packages.base-net.off true
  ```

预设怎么改见 [人格和预设](personas-presets.md)。

## 权限

- 两件都只读不改，切到只读照常用，也不问你。见 [权限和沙盒](permissions.md)。
- 通讯平台上的会话也有这两件，别人开口的照上面「通讯平台上别人让 AI 抓的」多两条限制。

## 隐私

- 搜的词发给你选的那一家联网服务，它用不了时发给自动试的几家。自动试的几家都在国外。
- 选了 DeepSeek 的，AI 要抓的网址先发给 DeepSeek，自己抓得下来的网页也一样。
- 自己抓不下来的网页，网址会发给 Exa、Firecrawl 或 Parallel 代抓。只有你的内网认得的主机名，连不上时网址也一样发出去。
- 搜到的结果、抓到的网页内容进这个会话的上下文，和对话一起发给你的模型供应商。

## 出了问题

| 时间线上写 | 怎么办 |
|---|---|
| 「几家搜索都被限额了，过会儿再试」 | 等设置页下拉里写的冷却时刻过去；或者配一个要 key 的，选它 |
| 「搜不了：…」 | 几家都失败了，后面是最后一家的原因。看代理、网络；选了要 key 的，看 key 对不对 |
| 「… 是本机或内网的地址，抓不了」 | 这类地址抓不了。要 AI 读本机的服务，让它用命令（例如 `curl`）去取 |
| 「… 回的是人机验证页」 | 代抓的几家也没抓到。换一个网址，或者自己打开复制给 AI |
| 「… 是 <类型>，读不成字」 | 图片、PDF 这类，下载下来交给 AI 读文件 |
| 「连不上 …」 | 看网络、代理。核心起来以后才设的代理，要重启核心 |
