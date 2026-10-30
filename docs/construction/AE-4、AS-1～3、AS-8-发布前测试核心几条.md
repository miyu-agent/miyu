# AE-4、AS-1～3、AS-8：发布前测试核心几条

2026-10-30 发布前测试（材料在 `~/.cache/miyu-audit/2026-10-30/`：`edge.md`、`security.md`，红测试补丁 `core-edge-tests.patch`、`core-sec-tests.patch`，基于 2e0c7576a）查出来、交给 sub1 的五条：前四条挡发布，AS-3 中危。编号照审计的类：边缘 AE-，安全 AS-（E-、S- 早被记忆线、拆仓库线占了）。一条一个提交，每条先在没修的代码上红、再修绿。

## 目的

- **AE-4**：唤醒表里一行读不懂，只跳过那一行，别的定时照常到点；核心起来时记一行，`check` 报出来，人照编号删得掉。原来一行坏了 `due`、`list` 整份出错，全机的定时悄悄停掉。
- **AS-1**：从软件源装的包文件，里面的编号（最上层文件夹）和索引那一条对不上的不装（`package_file_bad`）。原来第三方源写着 `foo` 的条目能装进来另一个已装的 `bar`，盖掉它、接着用它批过的能力。
- **AS-2**：读索引时编号、版本、依赖、推荐写法不对的条目丢掉；下载只落在缓存目录正下面。原来这几格直接拼成缓存里的文件名，能落到缓存外面。
- **AS-3**：`schedule.add` 的 `target.session` 也要这个连接碰得到，和连接的第 4 道检查一套判法（`sessions::reachable`）。原来平台桥能往本机的会话、人的头能往关着的群会话里定时说话。
- **AS-8**：解包、打包时每一级名字都要哪个系统上都只是一个普通名字：带 `:`、`\`、控制字符的，Windows 保留名，结尾是 `.`、空格的都拒；拼好以后再看一眼还在暂存处下面。原来 Windows 上中间一级写成 `C:x` 的会写到 C 盘当前目录下。

## 蓝图改哪几节

- `schedules.md` 第一节第 3 条（新加「一行读不懂」，后面几条顺延）、第四节 `list()`、第五节 `schedule.add` 的 `session`、测试表两行。
- `protocol.md`：`check` 的 `kind` 多 `schedule`；`package.install {"source"}` 编号对不上的 `package_file_bad`；「关着的场所会话只给桥」补一句别处写着会话的照同一套判。
- `cli/check.md` 文件表多 `check/schedules.rs`。
- 设计 `37-软件源.md`：第二节第 5 条（解包的名字）、第三节第 3 条下（条目的写法）、第四节第 4 条（编号对不上不装）。

## 不做什么

- 补丁里的别的红测试：AE-1、AE-2、AE-3、AS-4、AS-6 等归别的步（core1 做的 AE-2、AS-4）。`audit_sec_1030.rs` 里只放 AS-3 的两条。
- 读不懂的定时行不挪表、不推通知：跳过、记日志、`check` 报，人自己删。
- 从本机 `.miyu` 文件装的不比编号：文件是人自己挑的。

## 验收

1. AE-4：`miyu-store` 的 `one_unreadable_row_does_not_stop_the_others`（补丁原样）修前红（`Err(Bad("schedule t2: bad when"))`）、修后绿；`a_cell_of_the_wrong_type_only_spoils_its_row`；端点 `schedules::an_unreadable_row_is_skipped_reported_and_removable`（坏的先到点，好的照跑，`check` 报、删了不再报）。
2. AS-1：端点 `packages_fetch::a_source_file_holding_another_package_id_is_refused` 修前红（bar 被换成 2.0）、修后绿。
3. AS-2：`miyu-source` 的 `index::tests::entries_whose_id_or_version_is_not_a_plain_name_are_dropped`、端点的 `download::tests::a_name_that_leaves_the_cache_is_refused` 修前红（读进 11 条坏的；交回 `cache/../escape.miyu`）、修后绿。
4. AS-3：`audit_sec_1030` 的两条修前红（都建成了 `t1`）、修后绿。
5. AS-8：补丁的 `audit_a_drive_letter_inside_the_path_is_refused_on_windows` 修前在 Windows CI 上红（run 41012432900：「解包写到了暂存目录外面：C:audit-s8-4516」），修后看这个分支的 Windows CI；不分平台的 `names_windows_cannot_hold_are_refused_everywhere`、`packing_a_name_windows_cannot_hold_is_refused` 修前红、修后绿。
6. 手写变异：AE-4 5 个、AS-1 2 个、AS-2 8 个、AS-3 4 个、AS-8 8 个。没抓住的只有 AS-2「下载不查上一级」：只有一级的普通文件名拼出来上一级一定是缓存目录，等价，留着当兜底。AS-8 的「整串查 `\`」「拼好再看在不在暂存处下面」在 Linux 上测不到，前一个由 Windows 上那条测试管，后一个是兜底。
7. 本地门禁、Windows clippy、三平台 CI。

## 风险

- AS-8 拒的名字收紧了：扫过本机 `miyu-agent` 下各仓库，没有会被拒的；第三方包里真有 `aux.md` 这类名字的，打包时就会被拒，报 `bad path in the package`。
- AS-2 丢掉的条目不报给人：签名有效的源里写坏了的条目搜不到、装不了，和没收录一样。
- AE-4 跳过的行一直留着、不到点，靠 `check` 和起来时的日志让人知道。
