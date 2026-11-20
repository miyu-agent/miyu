## 施工单 CM-1：分清官方包和社区包

状态：做完了（2026-11-12）。项目主人 2026-11-12 提的两条（命令行看包把社区包写成 official、装前「注意」的字），「注意」的中文是项目主人给的；`package.info` 的「包含」改名 `contains` 是 core1 和 UI 会话同天定的。别的细节主会话照推荐定。

### 为什么

1. `miyu pkg info <编号>` 看没装的包，第一行是「软件源 : official」：`official` 是软件源（索引）的名字，官方源里社区收录的包（话题 `miyu-package` 自动收录，设计 37 第三节第 5 条）也在这份索引里，第一行一样写 `official`。分得出官方、社区的那一格本来就有：索引每一条的 `official`（照来源仓库在不在 `official.toml` 写），`package.info`、`package.search` 的回应都带着（别的源里的一律是假的）；`pkg info` 也印了「来源 : <来源仓库>（社区）」，只是排在第十行，一眼看不到。
2. 装之前的「注意」：带程序、MCP 服务、脚本的包另有一句「它带的程序会以你的身份在这台机器上跑，不在沙盒里」。扩展沙盒（设计 62）暂缓，现在没有那层沙盒，这句不该再说。社区的、本机文件的原来同一句「没经过官方审核」，项目主人要说清风险；本机文件装的不是社区收录的，不能叫社区包。
3. `package.info` 装着的、带 `path` 看的那一份里「带了什么」叫 `provides`（看一眼「包含」的形状：`program`、`page`、`skills`……），和软件源索引、`package.search`、`package.list` 的 `provides`（种类到名字）同名不同形，头拿错过。

### 改了什么

1. 命令行 `pkg info`（`crates/miyu-cli/src/pkg/info.rs`、`language/pkg/info.rs`）：没装的那一份「来源」挪到紧跟「软件源」的第二行；社区的写「<来源仓库>（社区，没有经过官方审核）」（英文 `(community, not officially reviewed)`），装着的社区包同样写。「软件源」那一行照旧写源的名字。装着的「来源」位置不动。`pkg search` 的社区包本来就接「（社区）」，不改。
2. 「注意」（`crates/miyu-endpoint/src/packages/notice.rs`，字在 `resources/core/human/{zh,en,ja}.json`）：
   - 社区的「这是个社区包，没有经过官方审核，可能存在风险。」，几个的「{packages} 是社区包，没有经过官方审核，可能存在风险。」（`packages/notice/unreviewed`、`…/named`）。
   - 本机文件的「这个包来自本机文件，没有经过官方审核，可能存在风险。」，几个的「{packages} 来自本机文件，没有经过官方审核，可能存在风险。」（新加 `packages/notice/file`、`…/named`）。两样都有的先社区后本机文件各一句。
   - 去掉 `packages/notice/unsandboxed`、`…/named` 和认「带没带会跑的东西」的代码（`runs`、`entry_runs`、`Item.runs`），升级的看一眼不再为它翻索引。拼句子的那一段拆成纯函数，单元测试照它算。
3. `package.info` 装着的、带 `path` 的「带了什么」改名 `contains`（`packages/info.rs`、`file_info.rs`），命令行照它印「提供」。软件源那一份照旧是索引原样的 `provides`。没发版，不留旧名。
4. 文档：蓝图 `packages.md`（「本地库」第 4 条、「说清来源和能力」、测试表）、`protocol.md`（`package.info`、「注意」）、`cli/pkg.md`（`info` 的先后、样子、文案表、测试表、「注意」的样子）；协议参考 `packages.md` 中英；作者文档 `manifest.md` 中英的「注意」；设计 61 定了的第 3 条记下改动。

### 不做什么

- 不改软件源的收录格式、索引的写法：`official` 那一格够用。
- 网页、终端只是在核心的句子前面加「注意：」，不用改头；`package.info` 的 `contains` 由 UI 会话跟着改。

### 怎么验

1. 先红：
   - `crates/miyu-cli/src/pkg/info/tests.rs` 新加「官方源里社区收录的包第二行说清是社区的」，改之前报红（「来源」在第十行、没有「没有经过官方审核」）。
   - 端点 `packages_approve.rs`（本机文件一句）、`packages_approve_source.rs`（几个社区包写出是哪几个）、`package_feeds.rs`（一个社区包）、`packages_info.rs`、`packages_info_path.rs`、`extension_driver.rs`（`contains`）改之前报红；命令行 `tests/pkg_approve.rs` 逐字对的确认屏改之前报红。
2. 新单元测试 `crates/miyu-endpoint/src/packages/notice/tests.rs`：社区、本机文件各一句，几个包的先社区后本机文件，官方的、出厂的不说，取不到句子的不硬凑。
3. `cargo test -p miyu-endpoint -p miyu-cli -p miyu --no-fail-fast`、clippy（含 Windows 目标）、`cargo fmt --check`、`cargo doc --workspace --no-deps`（`-D warnings`）、`cargo xtask prompts`。
4. 手看：`miyu pkg info <社区包编号>` 第二行是「来源 : …（社区，没有经过官方审核）」；从本机包文件夹装一个扩展，确认屏的「注意：」是本机文件那一句、没有沙盒那句。
