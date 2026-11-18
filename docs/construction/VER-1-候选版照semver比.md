## 施工 VER-1：装包时候选版也照 semver 比版本

状态：做完，等验收（分支 `step/CB-3`）。核心主会话派（2026-11-18），agy-cli 会话实测报来。

### 查到的

- agy-cli 实测：清单写 `[package] miyu = "2026.11.0-rc.2.4"` 和一张新表 `[providers]`，本机 rc.2.3 的核心 `miyu pkg info <目录>`、`install <目录>` 回「package.toml:39: 不认识的表 [providers]」，没先报 `needs_newer_miyu`（X-31 的设计是先比版本）。
- 两处的版本串没问题：`miyu-endpoint` 的 `packages/folder.rs`（从包文件夹装、看一眼，`pkg info <目录>` 走它）用 `env!("CARGO_PKG_VERSION")`，`miyu-store` 的 `packages.rs`（读装了的包）用 `RUNNING`，也是它。所有 crate 都是 `version.workspace = true`，打候选版只改根目录 `Cargo.toml` 的 `[workspace.package] version`（`deploy-1117/build.sh` 就是这么改的），两处和 `miyu --version` 都是 `2026.11.0-rc.2.3`。
- 根因在比法：`version.rs` 的 `Version::parse` 只认三段数字。正在跑的 `2026.11.0-rc.2.3`、清单要的 `2026.11.0-rc.2.4` 两头都读不成，`required` 当「不比」放过去，往下读到 `[providers]` 报不认识的表。VR-1 那时写的「候选版不比，同开发版」就是这样来的，不是有意的规则：同一个 crate 的依赖下限（`depend.rs`）早就照 semver 比，候选版不够 `>=2026.11.0`。

### 改了什么

- `miyu-config`：`depend.rs` 里的 `Semver` 挪进 `package/semver.rs`，依赖的下限、`[package] miyu` 共用一份。`version.rs` 照它读、照它比：候选版比同号的正式版低，预发布一段一段比（数字照大小）；`0.0.0` 照旧当最新。写法查得过候选版（`"2026.11.0-rc.2.4"`）。
- 正在跑的版本：`miyu_store::packages::RUNNING` 改成公开的，`folder.rs` 照它，两处是同一个常量。
- 不认识的表、键（`unknown_table`、`unknown_key`），清单又写了 `[package] miyu` 的，`package_invalid` 那一句后面补「这个包可能要更新的 Miyu」（新的 `miyu_config::package::maybe_newer`；`core/human` 的 `package-problems/maybe_newer_miyu`，三语，字段 `problem`）。开发版、比得过、读不成要求的照旧往下读，碰上新表只能这样提醒。

### 定下的（技术细节，按推荐定）

- 正在跑的候选版也比，`2026.11.0-rc.2.3` 装不了写 `miyu = "2026.11.0"` 的包。照 semver，和依赖下限一个口径；现在只有 agy-cli 一个包写了 `miyu`（写的是 `2026.11.0-rc.2.4`）。改了 VR-1 那条测试的一格（`2026.11.0-rc.1` 跑要 `2026.11.0` 的包：原来过，现在要更新）。

### 蓝图改哪几节

- `packages.md`「格式」`[package] miyu` 那一行（候选版带 `-rc.N`），「出错」`needs_newer_miyu` 那一行（照 semver、两处同一个常量、补的那一句）。
- 包作者文档 `manifest.md`（中英）`miyu` 那一行。

### 验收

1. 改前红：`miyu-config` 的 `package::version::tests::a_candidate_build_compares_with_a_candidate_requirement`，在 main 的 `version.rs` 上跑红在第一句：`line 6: unknown table [providers]`，同 agy-cli 报的。`an_unknown_table_hints_at_a_newer_miyu_only_when_the_manifest_names_one` 查补那一句的条件。
2. 改版本号的构建实测（照 `deploy-1117/build.sh` 的改法，拷贝里把工作区版本改成 `2026.11.0-rc.2.3`）：见文末「实测」。
3. 门禁十一项、长跑随机、Windows 目标的 clippy。

### 不做什么

- 打包（`miyu pkg pack`）、软件源收录照旧不比，只查写法。
- 读装了的包那一路（`package.list` 读不成的那一项）不补「可能要更新」那一句：装得上的包本来比过了。

### 实测

照 `deploy-1117/build.sh` 的改法：拷一份工作树，只把根目录 `Cargo.toml` 的 `[workspace.package] version` 改成 `2026.11.0-rc.2.3`，编 `miyu`（`miyu --version` 印 `miyu 2026.11.0-rc.2.3`）；沙箱数据根，包文件夹拷 agy-cli 那一份清单（`miyu = "2026.11.0-rc.2.4"`）。

- `miyu pkg info <目录>`、`miyu pkg install <目录>` 都回 `无法安装 …：package.toml:7: 这个包要 Miyu 2026.11.0-rc.2.4 以上，先更新 Miyu`（原来是「package.toml:39: 不认识的表 [providers]」）。
- 同一个核心看一份写 `miyu = "2026.11.0"` 的：`This package needs Miyu 2026.11.0 or newer: update Miyu first`（候选版比同号的正式版旧）。
- 工作树编的（`2026.11.0`）看一份写 `miyu = "2026.11.0"`、带一张不认识的表 `[future]` 的：`package.toml:6: 不认识的表 [future]：清单里只能有 …。这个包可能要更新的 Miyu`；看 agy-cli 那一份照常列出来（`2026.11.0` 比 `-rc.2.4` 新）。
