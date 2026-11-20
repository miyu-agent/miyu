## 施工单 N-6：链接卡片认抖音、TikTok 的视频

状态：待核心主会话审。经过：主人定的（核心主会话 2026-10-23 转来）：`link.preview` 给抖音、TikTok 出 `kind=video` 的卡片，终端已经会画视频卡片。硬约束：不用 yt-dlp、不调外部程序，只用 `miyu-net` 自己的 HTTP。终端 2026-10-24 实测、录了页面（`~/.cache/miyu-remake/link-cards-douyin-tiktok/`），事实照它。图纸 `net.md`「怎么走」第 12 条（认得的站）。

### 目的

抖音、TikTok 的链接现在照普通网页抓：抖音的页面是空壳，拿不到标题；TikTok 的页面没有 `og:`。贴进来只剩一个主机名。照 B 站、YouTube 的样子，认出视频页，出视频卡片：标题、作者、时长、封面。

### 认哪些地址

- 抖音：`www.douyin.com/video/<id>`、`www.iesdouyin.com/share/video/<id>/`、图文 `…/note/<id>`（同结构，没实测到，照同一套读）；短链 `v.douyin.com/…` 先跟完跳转再认，落到首页（不是视频）的照普通网页。
- TikTok：`www.tiktok.com/@<用户>/video/<id>`（用户名可以是空的，短链跳过去就是 `@/video/<id>`）；短链 `vm.tiktok.com`、`vt.tiktok.com` 先跟完跳转再认。
- `<id>` 是一串数字。

### 抖音怎么取

1. **主路**：
   - 拿匿名的 `ttwid`：`POST ttwid.bytedance.com/ttwid/union/register/`，桌面 Chrome 的 UA、`Content-Type: application/json`，请求体照规矩里写的；要的是回应头 `Set-Cookie` 里 `ttwid=` 的值（原样）。它的 `Domain` 是 `bytedance.com`，不会自己带给别的站，所以手拼 `Cookie` 头。拿到的记一天，一个核心共用一份；拿不到的这一次走兜底，下一次再拿。
   - 带手机 Safari 的 UA 和 `Cookie: ttwid=<值>`（只带这一个），`GET www.iesdouyin.com/share/video/<id>/`（图文是 `share/note`），整页读完。
   - 页面脚本里 `window._ROUTER_DATA = ` 后面那个 JSON（后面紧跟 `</script>`，读第一个完整的 JSON 值，有没有分号都行）：遍历 `loaderData` 的值，找带 `videoInfoRes.item_list` 的那个，不写死键名。
   - `item_list[0]`：`desc` 当标题（可能带 `#话题`，照原样），`author.nickname` 作者，`video.duration` 毫秒、四舍五入成秒，`video.cover.url_list[0]` 封面。
   - `item_list` 空、`filter_list` 不空（删了、只自己可见）：当不可达（`unreachable`）。
   - 都空、没有 `videoInfoRes`（没带上 cookie 是这样）、页面拿不到：走兜底。
2. **兜底**：Bingbot 的 UA，`GET www.douyin.com/video/<id>`；404 当不可达。页面里 `<script … type="application/ld+json" …>` 有几段，挑 `@type` 是 `VideoObject` 的：`name` 去掉末尾的「 - 抖音」当标题，`thumbnailUrl[0]` 封面（是字的也认），`duration`（ISO 8601，`PT0H0M15S`）换成秒，`creator.name` 作者。`description` 是套话，不用；`uploadDate`、`width`、`height` 不用。
3. 兜底也没有的（改版、拦了）：退回普通网页卡片，不报错。
4. 卡片的地址写 `www.douyin.com/video/<id>`（短链、分享页的都换成它），站名「抖音」，图标照 `www.douyin.com/favicon.ico`。

### TikTok 怎么取

1. 桌面 Chrome 的 UA、`Accept-Language: en-US,en;q=0.9`，`GET` 视频页，整页读完（约 430 KB，在页面上限 2 MiB 以内）。
2. `<script id="__UNIVERSAL_DATA_FOR_REHYDRATION__" type="application/json">` 到它的 `</script>`：`__DEFAULT_SCOPE__["webapp.video-detail"]`，`statusCode` 不是 0 的（`10204` 不存在）当不可达；`itemInfo.itemStruct`：`desc` 当标题，`author.nickname` 作者，`video.duration` 秒，`video.cover` 封面。
3. 拿不到这段脚本、没有 `itemStruct` 的（多半是被 WAF 拦了）：当不可达。连不上照旧不可达。
4. 站名「TikTok」，图标照页面的 `/favicon.ico`。

### 两家都

- 标题（`desc`）是空的：照作者名。两样都没有的照没有标题（不出卡片）。
- 主机、路径、UA、ttwid 的请求体、站名都是数据，写在 `link_preview.json` 的 `sites` 里；代码里只有怎么取。
- 所有请求都经 `fetch.rs`：同一道闸、同一套代理、复用的客户端。`fetch.rs` 补两样：照给的 UA、`Cookie`、`Accept-Language` 抓一页；`POST` 一次、交回回应头里某个 cookie 的值（`fetch/asked.rs`）。
- 接口、兜底页的协议、端口照认出来的那一页（同 B 站的接口）：测试里的假服务器是 `http://` 加端口。

### 测试

全在本机的假服务器上，测试不连外网。页面照录下来的片段：抖音的 `item`、兜底的 `VideoObject`、TikTok 的 `itemStruct` 原样存进 `crates/miyu-net/tests/fixtures/`，外面套一层照真页面原样抄下来的标签。

| 测试 | 管什么 |
|---|---|
| `crates/miyu-net/src/sites/douyin/tests.rs`、`tiktok/tests.rs` | 认地址（各种写法、不认的）；从片段里读出标题、作者、时长、封面；`filter_list` 不空算删了；兜底挑 `VideoObject`、去尾巴、ISO 时长；TikTok 的 `statusCode`、没有 `itemStruct` |
| `crates/miyu-net/tests/sites_video.rs` | 假服务器：抖音主路（注册拿到 ttwid、分享页带着它、出视频卡片、地址换成 `www.douyin.com/video/<id>`）、ttwid 记着不再注册、主路拿不到走兜底、删了的不可达、都没有的退回普通网页；短链跟完再认、落到首页的照普通网页；TikTok 出视频卡片、不存在的、没有脚本的不可达、短链 |

变异 11 个全红：抖音认地址的数字、毫秒换秒、`filter_list` 的判断、兜底 404 当删了、去「 - 抖音」、`ttwid` 记着、分享页带 `Cookie`；TikTok 的 `statusCode`、英文的 `Accept-Language`、认地址的数字；`Set-Cookie` 取到 `;` 为止。头一遍活下来两个（兜底 404 当删了、`statusCode`），结果一样、只是测试没盯着：补了「那个地址只请求一次」「`statusCode` 不是 0 带着 `itemStruct` 的也不算」两句，原样测试先过，再跑都红。

### 验收

本机门禁、Windows 版 clippy；碰了 `fetch.rs`，推 `step/` 过三台 CI。合了告诉终端 sha，终端拿真视频实测。
