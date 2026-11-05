## 施工单 PK-3 上：装包时说清来源和能力（核心、协议、文档）

状态：做完待合（2026-11-05）。core1 2026-11-05 派（派工单第 3、5 节），设计 `docs/designs/61-装包说清来源和能力.md` 跟这一步一起提交，第八节八条 core1 当天定了（第 3、6 条项目主人在旁边定）、都照推荐。施工分两步：这一步是核心、协议、蓝图、参考、作者文档；命令行两个确认屏、问的那一句、推荐和升级的那一段、答「是」带 `approve` 是 PK-3 下。

### 为什么

装包之前要写明包从哪来、有没有经过官方审核、要哪些扩展能力，能力在装的时候就批（原来装好以后打开时才批）。顺带修设计 61 第一节列的毛病 1–4：

1. 看一眼的 `source` 是源的名字，官方源里的社区包写成 `official`，命令行印成「official，签名已验证，没人审过」。
2. 一起装的 `with`、推荐的 `recommends` 不带能力，推荐的答「是」以后能力没给人看过。
3. 升级不重写 `approved`：v2 去掉了 `network` 还留着，v3 加回来不问就算批过。
4. 批准只认编号：本机文件夹盖同编号的照样接着用前一个批过的（AS-1 只挡了软件源条目和包文件编号对不上的那一种）。

毛病 5（网页软件市场详情页 `web-demo/packages/settings/market.js` 第 394 行把 `capabilities` 的对象 `join('、')`，印成 `[object Object]`）不归本仓库，写进合并说明请 core1 转网页。

### 做了什么

1. **看一眼的来源**（`packages/source/glance.rs`、`preview.rs`）：
   - `source` 改说这个包从哪来：从软件源装的 `official`（条目 `official` 是真、又是官方那个源，`glance::source`）或 `community`（官方源里社区收录的；配置里自己加的源里的一律算社区）；`package.install {"path"}` 的 `file`（包文件夹、包文件都是）；装回的 `shipped`、`distro`（照装回来那一份的层）。
   - 原来那一格源的名字改叫 `feed`，从软件源装的都写（官方的写 `official`）。顶上的 `official`（布尔）照旧留着。
   - `with`、`recommends` 每一项多 `source`、`feed`，声明了能力的多 `capabilities`；顶上那一个照旧总有 `capabilities`（没声明的是空的：没跟上的网页照它 `join`）。
   - 能力每一项这一刻批过、装完照旧留着的标 `held: true`（`approve::kept` + `mark_held`）：只在来路没变时有（下面第 3 条），本机文件装的永远没有。
   - `package.upgrade` 带 `preview` 的（`source/upgrade.rs` 的 `looked`）：每个要升的照从软件源装那一条看一眼（`fetch::install` 带 `preview`），`upgrades` 每一项补 `source`、`feed`、`capabilities`（声明了的才有）；它们一起装的新依赖（不是本来就升的）去重进 `with`；看不成的（缺依赖之类）那一项照旧只有编号、版本，真升时照旧拒。
2. **注意**（`packages/notice.rs`，`resources/core/human` 三份的 `packages/notice/*`）：社区的、本机文件的给 `notice`，一句一个字符串、照连接的语言挑好：
   - `packages/notice/unreviewed`「这个包没经过官方审核。」、`…/named`「{packages} 没经过官方审核。」；
   - 那几个里带会跑的东西的再一句：`packages/notice/unsandboxed`「它带的程序会以你的身份在这台机器上跑，不在沙盒里。」、`…/named`「{packages} 带的程序会……」。这两个单独的 key，**设计 62（扩展沙盒）落地后改**（`notice.rs` 的常量注释写着）；
   - `packages/notice/separator` 是几个编号之间的连接符（`、`、`, `）。
   - 只装一个包（顶上的、没有 `with`）的说「这个包」，几个的写出是哪几个；推荐的另放 `recommends_notice`（照写出编号的那一种）；升级的照 `upgrades` + `with` 算。
   - 「会跑的东西」（`notice::runs`）：种类是 `process`、`ui`、`worker`（`builtin` 编进核心不算），或者带子命令、MCP 服务、脚本。从软件源装的照索引条目的 `kind`、`provides` 认（以前生成的索引没有 `kind` 的照 `provides`）；从包文件夹装的照看一眼的 `program`、`command`、`mcp`、`scripts` 几格认。
3. **装的时候批**（`packages/approve.rs`）：
   - `package.install`（`path`、`source`）、`package.upgrade` 多参数 `approve: {"<编号>": ["<能力>", …]}`。`approve::valid`：编号写法不对、能力名不是十七个之一的 `bad_params`，先于一切检查（看一眼也查）；清单没声明的、编号不是这一次装的不理；不写的照旧装上不批。装回出厂的（`{"package"}`）收这一格、不理。
   - `manage::from_path` 拷文件之前在阻塞线程里读旧的 `desc`（`miyu_store::packages::local::desc`，只读 `desc` 不读文件清单），照 `approve::same_way` 判断来路：旧的和这一次都是从软件源装的、`origin`、`feed` 都一样才算没变。
   - 装成以后（重读通过、本地库记好），在 `switch_packages` 拉起扩展之前 `approve::record`：拿 `core.extensions.one_at_a_time()` 读开关，`settled(接着用, approve[编号], 清单声明的)` = (接着用 ∪ 批的) ∩ 声明的，照名字排、不重；和旧的一样不写，空的去掉这一项。写不进的记 `WARN extension approval not recorded`，装照算成。装不成、撤回的走不到这里。
   - 来路随装一起记：`local::record` 多收 `came`（`Came {origin, feed}`），从软件源装的 `source/fetch.rs` 把每一步的来路经 `archive::install`（`Then::Install(Arrival)`）交给 `from_path`；原来装完另补的 `set_origin` 去掉。
   - `extension.enable`、`package.enable` 批的时候（`extensions/approval.rs`）也照 `settled` 记，结果照旧是声明的全部。
4. **她的工具**（`miyu_tool::PackagesPort`、`miyu-basesystem` 的 `package`、`miyu-session`）：
   - 端口的 `install`、`fetch`、`upgrade` 多一个 `looked: Value`（看一眼的那一份），核心那一头照 `approve::of_looked` 拼表：顶上的、`with`、`upgrades` 各自列出来的能力，推荐的不算。
   - 卡片上那一份怎么到执行：`Tools::previewed` 另交回看成了的那一份，会话照调用编号记下（直接调用记在 `tools/checked.rs` 的 `Judged`，回合结束清；嵌套调用记在等着的那一份上），跑的时候放进新的 `Call::previewed`。`package` 工具执行时有它就用它，没有的（完全放开不问人）当场看一眼再传，看不成的照拒绝交回。
   - 她开扩展（`enable`）照旧把声明的全批：装的时候批过了，这一步多半什么都不用批。
5. **命令行**（只为不坏）：`pkg/summary.rs` 读源名的地方改读 `feed`，印的照旧。两个确认屏、问的那一句、带 `approve` 是 PK-3 下。
6. **文档**：蓝图 `protocol.md`（`package.install` 看一眼、「装的时候批」「从哪来、注意」两条，`package.upgrade`），`packages.md`（「依赖」第 7 条、「装卸」第 8 条加「说清来源和能力」、新第 9 条「装的时候批能力」、本地库第 2 条、测试表、「还没有的」去掉能力的审批和 PK-3 那条、加 PK-3 下和 PK-6），`extensions.md`（「能力」第 2 条、「还没有的」去掉第一条）；参考 `packages`、`extensions`（中英）；作者文档 `README`、`manifest`（中英）；设计 31 第一节、第六节，37 第四节第 4、6、9 条各加一句指到设计 61。

### 定下的技术题

- **「注意」给现成的句子，不只给标记**：core1 定了这两句是核心的字、放 `resources/core/human`。头拿不到核心的字，所以核心照连接的语言挑好、一句一个字符串交出去（`notice`、`recommends_notice`），头只管排版（「注意：」这样的前缀是头自己的）。不另加 `reviewed`、`unsandboxed` 这类布尔：`source` 已经说了审没审过，带不带程序头照 `notice` 有没有第二句就知道；以后扩展进了沙盒只改第二句的字。
- **句子分「这个包」和写出编号的两种**：设计 61 第三节的两个草样一个说「这个包」、一个写「weather」。只装一个包的用前一种，几个的（`with` 不空、升级几个、推荐的）用后一种，写出的只是社区的、本机文件的那几个；第二句只写带会跑的东西的那几个。
- **推荐的另放一格**：推荐的另问、答「是」才装，「注意」得另印（设计 61 第三节「推荐的」），所以不并进 `notice`。
- **`held` 的来路照每一项自己的源算**：依赖可能来自另一个源（先在同一个源找，找不到再找官方的），各算各的。
- **顶上的 `capabilities` 照旧总有**，`with`、`recommends`、`upgrades` 里的只在声明了时有：网页现在照顶上那一格 `join`，拿掉会报错；新加的格子照设计 2.3「写了的才有」。
- **升级的看一眼照从软件源装的看一眼一个一个看**：复用同一套（来源、能力、`held`、新依赖），不另写一份；索引照缓存拿，不多下。看不成的那一项不让整个看一眼失败（原来不失败）。
- **卡片上那一份照调用记在会话里**，不在工具里缓存：工具实例全会话共用，参数相同的两次会撞；问人以后多要的能力人没看过，照设计 4.1「照表批」只能用卡片那一份。直接调用记在原来记判过的位置的那张表旁边（合成 `Judged`，`tools.rs` 守 500 行），回合结束一起清。
- **`approve` 先于一切检查**：写法不对的一样都不动，看一眼也查。
- **装回出厂的收 `approve`、不理**：头可能照同一套拼了带上；拒掉反倒让一条好请求失败。
- **写开关在 `from_path` 的 async 里、拿着开关那把锁**：和 `extension.enable` 同一把锁、同一套读写（`read_switches` 读不成的当空的、带着原来字节的版本写回）。锁的先后照旧是装卸那把在外、开关那把在里。
- **`desc` 只读那一格**：`miyu_store::packages::local` 加 `desc()`，`origin()`、`installed()` 改成照它读；不读文件清单（大包的 `files` 好几千行）。

### 要拍板的

- 无。设计 61 第八节八条 core1 已定，「注意」的字面照 core1 给的两句（英文、日文是照着翻的，见 `resources/core/human`），命令行怎么排在 PK-3 下定。

### 头要跟着改（合了以后 core1 通知）

- 终端 `src/drawer/package.rs`、网页 `web-demo/packages/asking/package-plan.js`（她装包问人的卡片）：多「来源」一行（照 `source`、`feed`、`origin`）；`notice` 有的另起一行「注意」，照原样接；「需要」照一项一行带说明（`name` + `summary`）；依赖的照 `with` 分开写（每一项也有 `source`、`capabilities`）。`held` 的可以不印或标成已批。
- 网页 `web-demo/packages/settings/market.js` 详情：能力照 `name` 印、说明放提示（修第 394 行的 `[object Object]`）；来源照 `source`；装的时候带 `approve`：从看一眼的 `capabilities`、`with` 拼（推荐的另装另批）；「全部升级」弹窗每个包接新要的能力（`held` 的不印）、社区的 `notice`，升的时候带 `approve`（`upgrades` + `with`）。
- 网页 `extensions.js` 的「批准并开启」不动：旧装的、没带 `approve` 装的、升级没批的还走它。
- 看一眼的 `source` 改了意思：原来读它当源名的改读 `feed`（查过终端、网页都不读，本仓库命令行已经改了）。

### 验收

- `crates/miyu-endpoint/tests/packages_approve.rs`：
  - `a_folder_says_it_is_a_local_file_and_only_what_was_approved_is_kept`：看一眼 `source: "file"`、三项能力都没有 `held`、`notice` 两句；`approve` 写成不是能力的、编号写法不对的 `bad_params`、什么都不装；带 `network`、`tools`、没声明的 `fs.read`、别的编号 `other` 装：`approved` 只有 `["network", "tools"]`、没有 `other`，`unapproved` 是 `["search"]`；再从文件夹装只批 `search` 的，记的只有 `["search"]`；不带 `approve` 再装，整项去掉、三项都没批。
  - `an_approved_extension_runs_at_once_or_opens_without_asking`：`always` 的带批准装上就 `running`；`manual` 的带批准装上，`extension.enable` 不带 `approve` 就开了；不带批准装的打开 `needs_approval`。
  - `a_shipped_package_brought_back_records_no_approval`：卸了出厂的 `courier`，看一眼 `source: "shipped"`、没有 `notice`；带 `approve` 装回，`approved` 没有它。
  - `an_approval_that_cannot_be_written_still_installs_and_warns`：开关文件的位置是目录，装照算成，记了 `WARN … extension approval not recorded`，`unapproved` 照旧。
- `crates/miyu-endpoint/tests/packages_approve_source.rs`：
  - `the_preview_says_where_each_package_comes_from_and_holds_only_on_the_same_way`：官方的 `source`/`feed` 都是 `official`；官方源里社区收录的依赖 `community`、带能力；推荐的带 `source`、能力；`notice`、`recommends_notice` 写出编号的两句；照 `approve` 装上 app 记 `network`、依赖 lib 记 `search`；再看一眼 `network` 带 `held`、`tools` 不带；本机文件夹盖过去以后 `approved` 去掉、再看软件源的那一份也没有 `held`。
  - `an_upgrade_keeps_what_is_still_wanted_and_asks_again_for_what_comes_back`：1.0 批了 `network`、`tools`；2.0 只要 `tools`、多依赖社区的 lib：看一眼 `upgrades` 带 `source`、`feed`、`tools` 带 `held`，`with` 是 lib 带能力，`notice` 写 lib；带 `{"lib": ["search"]}` 升，app 只剩 `tools`、lib 记 `search`；3.0 又要 `network`：看一眼 `network` 没有 `held`、官方的没有 `notice`，不带 `approve` 升，app 照旧只有 `tools`。
- `package_feeds.rs`：配置里加的源看一眼 `source: "community"`、`feed: "friends"`、`official: false`，不带程序的只有一句「这个包没经过官方审核。」。
- `package_tool.rs` 的 `she_approves_what_the_card_showed`：工作区问人，卡片 `source: "file"`、只列 `network`；卡片出来以后包文件夹多要 `tools`，答应以后只批了 `network`；完全放开的照当场看的批 `network`、`tools`。
- `packages/approve/tests.rs`：写法；来路没变的几种（换源、换来源仓库、本机文件、没记、以前是本机文件）；交集；`held` 只标批过的；照看一眼拼表、推荐的不算、升级的照 `upgrades`。
- 跟着改的：`packages_fetch`、`packages_stages`、`package_depends_floor` 的 `with`、`recommends` 多 `source`、`feed`；`packages_extensions` 的 `an_installed_extension_starts_and_a_removed_one_stops` 装的时候带 `approve`（本机文件夹装的不再接着用装之前写进开关文件的批准）；`miyu-basesystem` 的 `package` 测试照新端口签名，问过人的照卡片那一份、不再另看一眼；命令行 `summary/tests.rs` 照 `feed`。
- 变异（各改坏一处、对应测试变红再还原，12 个都红）：
  1. 换了源也算来路没变（`same_way` 不比 `feed`）：`approve` 单元测试 `the_way_in_must_match_to_keep_what_was_approved` 红。
  2. 本机文件装的也算没变：`a_folder_says…`、`the_preview_says…` 红。
  3. 不和声明的取交集：`an_upgrade_keeps…` 红。
  4. 批准写在拉起之后：`an_approved_extension_runs_at_once…` 红（`always` 的停在要批准）。
  5. 嵌套调用不把卡片那一份交给工具：`she_approves_what_the_card_showed` 红。
  6. 不标 `held`：`packages_approve_source` 两条红。
  7. `approve` 写法不查：`a_folder_says…` 红。
  8. 官方的也说没审核：`packages_approve_source` 两条红。
  9. 写不进时日志换了字：`an_approval_that_cannot_be_written…` 红。
  10. 完全放开不先看一眼（传空的）：`she_approves_what_the_card_showed` 红。
  11. 别的源也算官方：`package_feeds`、`packages_approve_source` 两条红。
  12. 升级不把 `approve` 传下去：`an_upgrade_keeps…` 红。
- 门禁 11 项、CI 六项。
