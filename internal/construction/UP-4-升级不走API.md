## 施工单 UP-4：`miyu upgrade` 读发行版订阅，不再撞 GitHub API 的匿名限流

状态：做完，等验收（分支 `step/UP-4-升级不走API`）。有用户报 `miyu upgrade` 撞上匿名限流（2026-11-28 core1 派）。做法是项目主人 2026-11-28 定的。

### 目的

原来的版本列表取自 `https://api.github.com/repos/miyu-agent/miyu/releases?per_page=30`。这是 GitHub API，匿名每个 IP 每小时只给 60 次，同一个出口 IP 后面人多就会用完。改成读 `https://github.com/miyu-agent/miyu/releases.atom`：它是普通网页，不算 API 的额度（项目主人实测 200、`application/atom+xml`）。下载本来就走 `github.com/…/releases/download/…`，不变。

### 蓝图改哪几节

- `cli/upgrade.md`「在哪」多 `feed.rs` 一行，`net.rs` 不再分限流；「怎么走」第 4 条整条换成读订阅、照标签认预发布、没带文件的跳过；第 6 条写文件地址按标签拼、`.sig` 和压缩包回 404 的报缺少；「测试」两行跟着改、多 `feed/tests.rs` 一行。
- 用户手册 `install.md`（中英）「升级」第 1 步、`--prerelease` 那一行；`cli.md`（中英）`--prerelease` 那一行。

### 做法

1. **订阅里的标签**（`upgrade/feed.rs`）：不加 XML 依赖。不是 UTF-8、没有 `<feed`、没有 `</feed>` 的不是订阅（交回没有）；是订阅的，取每个 `<entry>` 里第一个 `href` 带 `/releases/tag/` 的 `<link>`。正文 `<content>` 是转义过的网页，里面的 `<`、`"` 写成了 `&lt;`、`&quot;`，不会被当成元素：真的订阅里正文带着提交链接，测试里换成了别的标签的链接，确认不被认出来。读回来最多 8 MB（`LIST_LIMIT`，不变）。
2. **能算的标签**（`plan::candidates`、`plan::has_pre`）：标签要是 `v<版本>`，照现有的 `Version::parse` 认；带 `-` 的是预发布，只在 `--prerelease` 时算；新的在前，同一个版本只算一次。`has_pre` 同一个口径（原来还认 API 里的 `prerelease` 标记，订阅里没有）。草稿本来就不在订阅里。原来的 `Asset`、`newest`（读 JSON）删掉，`plan.rs` 不再用 `serde_json`。
3. **怎么走**（`flow.rs`）：从新到旧一个一个看，不比现在新的就停、说已是最新（只升不降）；比现在新的先下它的 `SHA256SUMS`，回 404 的（作废的 rc.3 那样：订阅里有、没有包）跳过、接着往旧的找；比现在新的都跳过了也说已是最新。`SHA256SUMS` 回别的错不跳，停下说。找到能用的、`SHA256SUMS.sig` 验过签、清单里有这个压缩包那一行以后才问「升不升」（技术题：原来问在下载清单以前，那时资产在不在是照 API 列表判的；现在要下了才知道，问在核对以后，问的就是一个真能升的版本，压缩包照旧答了升才下）。文件地址按标签拼 `https://github.com/miyu-agent/miyu/releases/download/<标签>/<文件名>`，资产名照 `plan::archive_name`；`SHA256SUMS.sig`、压缩包回 404 的报「发行版里缺少 …」。签名、校验和、对换、版本核对一样不变：安全靠编进程序里的发版公钥，不靠列表从哪来。
4. **限流那条路撤掉**（照「否掉的功能连下层一起撤」）：API 不用了，`NetError::Limited`、`net::classify` 里认 `x-ratelimit-remaining` 的判法、`language/upgrade.rs` 里「匿名访问每小时 60 次」那一句一起删。`github.com` 偶尔回的 429 照普通状态码说「GitHub 返回 HTTP 429」。`Net::get` 的第三个参数从 `api`（带 API 要的头、短超时）改成 `quick`（只管短超时），不再带任何头。
5. **订阅读不了**：新加 `Said::BadFeed(<地址>)`，说「无法读取发行版列表：<地址> 返回的不是 Atom 订阅」，提示检查网络或代理后重试，退出码 1。空的订阅（一个 entry 都没有）、标签都认不出的说「未找到可用的发行版」。

### 验收

1. 改前红：先只加 `Said::BadFeed` 和它的字，`flow` 照旧，换上 atom 替身的测试 15 条红（`tests.rs`、`tests/refused.rs`、新的 `tests/feed.rs`：挑最新正式版、加 `--prerelease` 挑 rc、只有 rc 时提示加 `--prerelease`、`SHA256SUMS` 404 跳到下一个、跳到不比现在新的就停、`SHA256SUMS` 500 不跳、订阅是网页/空的/不是 UTF-8 说读不了），改完全绿。另加 `feed/tests.rs` 三条（照真的订阅认、正文链接不算、空的和坏的）、`plan/tests.rs` 的 `candidates`、`has_pre`。
2. 实测（2026-11-28，真的 `releases.atom`：rc.1 到 rc.5、v0.7.4 等 10 个 entry，rc.3 的 `SHA256SUMS` 是 404）：本机 debug 编的 `miyu`（临时把 `miyu-cli` 的版本写成 `2026.11.0-rc.4`，编完改回），拷进沙箱安装目录、放 `channel = "script"` 的标记，数据根和 `HOME` 指沙箱。不碰 `/usr/lib/miyu`、不碰主人装着的程序。
   - `echo n | miyu upgrade --prerelease`：清单、签名下好验过，问「升级 Miyu 2026.11.0-rc.4 → 2026.11.0-rc.5？[Y/n]」，答不升，「已取消」，退出码 1。
   - `miyu upgrade --prerelease --yes`，沙箱安装目录里的 `resources/` 设成 0555：「正在下载 miyu-2026.11.0-rc.5-linux-x86_64.tar.gz」「正在核对新版」「正在替换」，对换挪不动 `resources/`，「升级失败（move-old）：Permission denied (os error 13)，已保留原来的版本」，退出码 1；下载、对哈希、解开、跑新版 `--version` 都走过了。
   - `miyu upgrade`（不带 `--prerelease`）：订阅里的正式版只有 v0.7.4，不比现在新，「已是最新版本 2026.11.0-rc.4」，退出码 0。
   - 三趟以后安装目录逐个文件的 SHA-256 和开始前一样。
   - 同一份改动在 .91（Linux x86_64）上照同样的三趟又跑了一遍（主人要用本机，后面的编译、实测都搬到 .91），说的、退出码、安装目录都一样。
3. 门禁、`clippy --workspace --all-targets`、Windows 目标的 clippy、`cargo doc -D warnings`、三平台 CI。

### 不做什么

- 安装脚本 `packaging/install.sh`、`install.ps1` 照旧问 API（有 `GITHUB_TOKEN` 时带上）、说限流的那几句不动：这一步只改 `miyu upgrade`。要不要跟着改交 core1 定。
- 订阅只有最近 10 个发布：比现在新的 10 个都没带文件的说已是最新，不翻页。
