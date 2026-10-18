## 施工单 N-3 上：要 key、要地址的几家

状态：2026-10-19 合了（fd20723a）。设计 `36-联网搜索.md` 第二节第 1 到 3 条、第三节第 1 条；图纸 `docs/blueprint/tools/web_search.md`、`config.md`「M8 的配置项」。N-3 施工时拆成上、下：下是免 key 的网页抓取兜底。

### 做什么

DeepSeek、Tavily、Brave、Exa 自己的 key、自己搭的 SearXNG：设置里各一格（`net.search.<来源>.key` 是密钥引用，SearXNG 是 `net.search.searxng.url`）；DeepSeek 没单独配的照用模型那边 DeepSeek 供应商的 key（项目主人 2026-10-18 定）。没配置时的顺序最后是有 key 的 DeepSeek；Tavily、Brave、SearXNG 在下拉里挑了才用；下拉里没配的那几家后面写「要 key」「要地址」。

### 做了什么

1. `search.json`：多四家，每家写 `needs`（`nothing`、`key_optional`、`key`、`address`）；没配置时的顺序加 DeepSeek。`rules.rs` 认它，要地址的那一家地址是空的、别的必须有。
2. `sources.rs`：四家的请求（`Rest`：方法、地址、头、正文；DeepSeek 查询截 128 个字、至多 50 条，Tavily、Brave 至多 20 条）和读法（DeepSeek 用长的 `summary`）；日期只收 `YYYY-MM-DD` 开头的。`rest.rs` 发请求，429、402（余额不足）当限额。
3. `using.rs`：`Using`（挑的、key、地址）；`Search::search` 照它挑先后，没配 key、没配地址的跳过，配了 key 的 Exa 把 key 接在地址后面。
4. `settings.rs`：五项设置（核心照 cargo 开关登记、记成联网包的）；`using` 照这一轮的配置拼 `Using`：key 照引用取（取不到、空的当没配），SearXNG 的地址可以写成环境变量，DeepSeek 照用供应商的 key（地址里有 `api.deepseek.com`，或者 `catalog` 是 `deepseek`）；`missing` 只看写没写，下拉的暗字照它。
5. 端点：`Builtins::option_state` 多收这个账号的最终值，状态多两种（要 key、要地址），字在 `core/config/option-needs-key`、`option-needs-address`；`config.schema` 拿着配置的锁，最终值由它交进去，不再拿第二次。
6. 给人看的字：五项的名字、说明，下拉多四个选项，中英日三份。

### 测试

见图纸 `tools/web_search.md`「守着它的」N-3 上那几行：`tests/search_keys.rs` 整条走通，`settings/tests.rs`、`kit/tests.rs`、`sources/tests.rs` 的单元测试。

- 变异 11 处：10 处杀了。活下来又补的：没配地址的 SearXNG 照样跳过（补「挑了它、别的都冷却着时照样是都在冷却」）；402 当限额（假服务器原来回空正文，照原话也认得出；改成官方那种 JSON 错误正文，只看正文会当成「没搜到」，这一处才守得住）。key 判空那一处查出来是多余的（密钥本身就去掉前后空白、不会是空的），删了。
- DeepSeek、Tavily、Brave 手上没有 key，没走真网络；格式照各家文档（2026-10-19 查的），假服务器照文档的样子回。
