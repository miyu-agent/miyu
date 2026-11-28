## 施工单 UP-5：安装脚本读发行版订阅，不再问 GitHub API

状态：做完，等验收（分支 `step/up-5-install-feed`）。UP-4 只改了 `miyu upgrade`，安装脚本照旧问 API；项目主人 2026-11-28 定安装脚本也不走 API（core1 派）。

### 目的

`packaging/install.sh`、`install.ps1` 挑版本时问的是 `https://api.github.com/repos/miyu-agent/miyu/releases?per_page=100`。这是 GitHub API，匿名每个 IP 每小时只给 60 次，同一个出口后面人多就会用完，脚本停在「GitHub 限流」。改成和 `miyu upgrade`（UP-4）同一套认法：读 `https://github.com/miyu-agent/miyu/releases.atom`，它是普通网页，不算 API 的额度。下载本来就走 `github.com/…/releases/download/…`，不变。

### 照哪几张图纸

- 设计 `59-发版.md` 第三节第 2 条（安装脚本）；施工单 RL-3、WD-8（只认新格式的版本号）、UP-4（订阅的认法）。
- 用户手册 `install.md`（中英）：「Linux、macOS」一节的参数表下面多一段讲挑版本读订阅；「脚本的校验步骤」里 `GITHUB_TOKEN` 和限流那一段删掉。

### 做法

1. **订阅里的标签**（`install.sh` 的 `feed_tags`、`install.ps1` 的 `Get-FeedTags`）：照 `crates/miyu-cli/src/upgrade/feed.rs` 一个判法一个判法抄。没有 `<feed`、没有 `</feed>` 的不是订阅；是订阅的，取每个 `<entry>`（`<entries` 这样的别的元素不算）里第一个 `href` 带 `/releases/tag/`、后面不再有 `/` 的 `<link>`。正文 `<content>` 是转义过的网页，里面的 `<`、`"` 写成了 `&lt;`、`&quot;`，不会被认成链接。sh 用 awk 的 `index`、`substr` 做，不用 jq、不用 XML 工具，mawk（Ubuntu 的 `awk`）、macOS 的 awk 都认；ps1 用 `IndexOf(…, Ordinal)`，不是 UTF-8 的（解码器抛错）也算不是订阅。
2. **能算的标签**（`candidates`、`Get-Candidates`）：照 `plan::candidates`，标签要是 `v<主>.<次>.<补丁>[-<预发布>]`，带 `-` 的是预发布、只在 `--prerelease`（`-Prerelease`）时算；照 semver 从新到旧排，同一个版本只算一次。另外照 WD-8 只认四位年份开头的版本号：`miyu upgrade` 有「不比现在新的就停」挡着旧版 Miyu 的 `v0.7.x`，安装脚本没有现在的版本可比，照旧跳过它们。排法是给每个标签拼一个按字节排就对的键（数字补齐 20 位；正式版后缀 `~` 排在预发布的 `-` 后面；预发布每段数字的前面加 `0`、字的前面加 `1`、段尾加比字母数字都小的 `!`），sh 交给 `LC_ALL=C sort -r`，ps1 交给 `[Array]::Sort` 加 `StringComparer.Ordinal`。
3. **怎么挑**（`find_release`、`Find-Release`）：照 `flow::usable`，从新到旧一个一个试，下载地址照标签拼 `releases/download/<标签>/SHA256SUMS`；回 200 的就是它，下好的清单直接拿去核签名，不再下第二遍；回 404 的（作废的 rc.3 那样：订阅里有、没传包）跳过、接着试更旧的；回别的状态码的停下，说「GitHub 返回 HTTP …」。都不成的：加了 `--prerelease` 的说「未找到可用的发行版」；没加的，订阅里有预发布就照旧提示「还没有正式版；如果要安装预发布版，加上 --prerelease」（同 `plan::has_pre`），没有就说「未找到可用的发行版」。`--version` 指定版本的不读订阅，照旧直接下载。
4. **读不了**：订阅回的不是 200 的说「GitHub 返回 HTTP …」；连不上的说「无法连接 GitHub」；回来的不是订阅（网页、空的、读到一半的）说「无法读取发行版列表：<地址> 返回的不是 Atom 订阅，检查网络或代理后重试」，和 `miyu upgrade` 的 `Said::BadFeed` 一个意思。
5. **限流那条路撤掉**：两份脚本里 `MIYU_INSTALL_API`、带 `GITHUB_TOKEN` 的请求头、「GitHub 限流：匿名访问每小时 60 次」那一句一起删。测试用的环境变量换成 `MIYU_INSTALL_FEED`（订阅的地址），`MIYU_INSTALL_BASE` 不变。`install-test.yml` 不再传 `GITHUB_TOKEN`。
6. **签名、校验和、跑新版 `--version` 一样不变**：安全靠写在脚本里的发版公钥，不靠列表从哪来。`install.ps1` 注释以外照旧只有 ASCII，新的中文提示写成 `\uXXXX`。

### 验收

1. `crates/miyu-source/tests/install_scripts.rs` 新加 `both_scripts_read_the_release_feed_not_the_api`：两份脚本里没有 `api.github.com`、有 `releases.atom` 的地址。改前（main 上的两份脚本）红，改后绿。
2. `packaging/test-install.sh`、`test-install.ps1` 在本机起的下载处里多一份照 GitHub 的样子写的订阅，每个 entry 的正文里都放一个转义过的、指向 `v2099.2.0` 的链接：
   - 订阅的先后打乱成「真的那一版、`v2099.1.0-rc.9`、`v2099.1.0-rc.10`、`v0.7.4`」，加 `--prerelease`：照下载处的请求记录，先试 rc.10、再试 rc.9（都回 404），最后装上真的那一版；`v2099.2.0`、`v0.7.4` 一次都没被请求；
   - 只有预发布、不加 `--prerelease`：提示加上，退出码 1；
   - 只有一个没传包的正式版：说没找到，退出码 1；
   - 订阅是网页：说读不了，退出码 1，什么都没装。
   新的测试脚本对着 main 上的旧脚本跑是红的（旧脚本不认 `MIYU_INSTALL_FEED`，照旧去问 API）。
3. `gh workflow run install-test.yml --ref step/up-5-install-feed`：Ubuntu、macOS、Windows 三台干净的机器照公开的 rc.5 装、核、卸一遍，逐台 success。
4. 门禁、三平台 CI。

### 定下的（core1 2026-11-28 照推荐定）

1. **只警告限流的那一步先留着，待撤**：`test-install.sh`、`test-install.ps1` 里「装上的预发布自己跑 `miyu upgrade --prerelease` 撞了限流就记一句没验成」那一条管的是装上的程序，不是安装脚本。`install-test.yml` 装的是公开的 rc.5，它的 `miyu upgrade` 还在问 API（UP-4 在它以后，还没发版），现在撤掉 CI 会随机红。**待撤**：带 UP-4 的预发布发出来以后，两份测试脚本里这一条一起撤掉，`miyu upgrade` 撞了别的错照失败算。
2. **只认四位年份开头的版本号，留着**：UP-4 的认法里没有这一条，`miyu upgrade` 靠「不比现在新的就停、只升不降」把旧版 Miyu 的 `v0.7.x`、演练的 `v0.0.0` 挡在外面。安装脚本没有现在的版本可比：不过滤的话，没有新格式的正式版时会去试 `v0.7.4`，它的包不是这个脚本装得了的（WD-8 撞过的那一回），所以照 WD-8 跳过它们。

### 不做什么

- 订阅只有最近 10 个发布：10 个都没传包的照「没找到」说，不翻页（同 UP-4）。
