# PK-2：依赖的版本下限

2026-11-05 core1 排给记忆线（PK 三步的第一步，蓝图 `packages.md`「还没有的」那一条）。新版的包要靠旧包的新本事时，原来只认装没装：旧的装着也照装、照起，坏在运行时。照 pacman 加「至少某一版」。

## 目的

- `[depends]`、`[recommends]` 的 `packages` 里一项可以写 `<编号>>=<版本>`（`"onebot>=1.2.0"`）。只认 `>=`；版本照 semver 比。别的写法照旧 `bad_dependency`。不写版本的照旧。
- 装：版本不够的拒 `depends_missing`，`data.missing` 带要的版本。从软件源装时照下限挑、装着的不够一起升。
- 读：装着的版本不够了同缺依赖那条（`package.list` 的 `status`、`missing`，`miyu check`，扩展不拉起）。
- 收录：索引照原样带上。

## 蓝图改哪几节

- `packages.md`：「在哪」加 `depend.rs`；格式示例；「依赖」第 1 条（写法、怎么比、给出去照原样）、第 2 条、第 4 条、第 6 条（索引），新第 7 条（从软件源装时的依赖），原第 7 条挪成第 8 条；「出错」`bad_dependency`、`depends_missing`；「守着它的」一行；「还没有的」去掉这一条。
- `protocol.md`：`package.info` 的 `depends`、`recommends`，`package.install {"source"}` 的依赖和拒绝，错误码表 `depends_missing`。
- 参考 `packages.md`、`errors.md`（中英）；作者文档 `manifest.md`（中英）`[depends]` 一节和 `version` 的注释；设计 31 第三节第 2、7 条。

## 怎么做

1. **一项依赖**（`miyu-config` 新 `package/depend.rs`）：`Dependency { id, at_least: Option<String> }`。`parse` 照第一个 `>=` 切开，编号照包编号的写法（`secret::valid_name`），版本照 semver 2.0.0 读得成才收；`Display`、`Serialize` 都是写的原样。`fits(装着的版本)`：没下限的都够；有下限的，装着的没写、读不成 semver 的不够，照 semver 第 11 条比不低于下限的够。
2. **清单**：`Manifest::depends`、`recommends` 从 `Vec<String>` 换成 `Vec<Dependency>`（换类型，编译器把每一处用到的都找出来）。`links.rs` 读：`packages` 里能带下限，`workers` 里带了报 `bad_dependency`；同一编号写两次（带不带下限都算）报；读不成的、含 `>=` 的信息里说要 `>=` 加 `1.2.0` 这样的版本。
3. **装着的够不够**（端点 `packages/depends.rs`）：`missing` 照 `fits` 判，交回照写的原样的字（`Vec<String>`）；装、看一眼、装回出厂的、`package.info {path}`（都经 `manage.rs` 的 `clear_of_conflicts`）、`package.list`（`item.rs`）、`miyu check`（`check.rs`）、扩展拉起（`extensions.rs`）都走它，不另改。`Refusal::depends_missing` 收 `&[String]`。`dependents` 照编号比。
4. **从软件源装**（`packages/source/fetch.rs`）：装着的换成「编号到清单写的版本」的表。排依赖的那几样收进 `Walk`（行数和参数个数）：装着、版本够的跳过；排过的照排进去的那一份比，不够的记缺；没装、不够的照先后找第一个索引里那一版够的源，深度优先接着排；哪个都不够的记缺（照原样、不重复）。一起升不另写：排进去的照装包文件那一条装，同编号装着的就是升级。
5. **看一眼的推荐**（`glance.rs`）：装着、版本够的不列；索引里那一版不够的不列。
6. **索引**（`miyu-source`）：`Entry` 的两格照旧是字的数组，收录写 `to_string()`。`well_formed` 改成照 `Dependency::parse`，编号另合原来拼文件名的 `plain_id`；加 `depends_on`、`recommended` 读成 `Dependency`。
7. 内置语义模型找推荐（`miyu-core` 的 `embed.rs`）照编号比。

## 我定的（技术题）

- **自己写 semver 的比较，不加 `semver` crate**：仓库里没有直接用 semver 库的地方，`[package] miyu` 的版本比较（`package/version.rs`）也是手写的三段数字；`semver` 在锁文件里只是 `rustc_version` 这些编译时依赖带进来的，加成直接依赖会进发出去的程序、动第三方许可证清单。要的只有读和比（semver 2.0.0 第 2、9、10、11 条），一百来行、测试照规范里那一串先后对。`version.rs` 不并过来：它管 Miyu 自己的版本，不认预发布、`0.0.0` 当最新，口径不同。
- **下限照严格的 semver 写**：两段的 `1.2`、带 `v` 的、前导 0 都不收（信息里给出 `1.2.0` 的样子）。现在各包的 `version` 都是三段数字。
- **装着的版本读不成 semver 的算不够**：清单的 `version` 原来只给人看、写法不查；比不了就当不满足，宁可装不上说清缺什么，不让它跑起来再坏。
- **协议、索引上照写的原样给**（`onebot>=1.2.0`），`data.missing`、`missing`、`miyu check` 的话也是：形状不变（还是字的数组），带下限的多一截；照 pacman（`-Qi` 的 Depends On、报错都是原样）。
- **索引 `version` 不升**：两格照旧是字的数组。读索引的写法检查放宽到清单的写法，旧核心照旧的检查会丢掉带下限的那几条（它们本来也读不了那份清单），新核心照收。
- **`workers` 不收下限**：派活只说 `packages`；`workers` 是以前的写法，照旧只写编号。
- **从软件源只挑索引里列的那一版**：索引每个源只列最新的一版，照下限挑就是照先后找第一个最新版够的源。
- **降级照装**：装、升级本身照旧不比大小（设计 31 第三节第 2 条）；降到不够别人下限的，别人照「装着的缺了依赖」报，不拒这一次装。

## 不做什么

- 不做上限、`=`、`<`、`~` 这些（派活只要 `>=`）。
- 软件源仓库（packages）不改：合进 main 以后 core1 去换 `miyu-rev`。
- 看一眼的 `with` 不另标「升级」：照 pacman 的「软件包 (N)」只列编号和要装的版本。

## 验收

1. `miyu-config` `depend/tests.rs` 5 条：只写编号的装着就够；`>=` 后面的原样留着、序列化成原字；拒的 23 种写法；装着的版本照 semver 比（`1.10.0` 够 `1.2.0`，`1.2.0-rc.1`、`1.2`、`r53`、没写的不够）；semver 第 11 条那一串两两比（正反两向、下限低的够、高的不够），构建信息不算。
2. `links/tests.rs` 加 `packages_may_carry_a_version_floor`：读成 `Dependency`、照原样；`1.2`、`>`、`=`、带空格、同编号两次、`workers` 里带下限的都是 `bad_dependency`、报在那一格。原来几条改成照编号比。
3. `miyu-source`：索引的写法检查多四条（带下限的收、两段的、别的符号、编号带 `..` 的丢掉），`depends_on`、`recommended` 读得出；收录照原样带 `alpha>=1.2.0`、`memes>=0.3.0-rc.1`。
4. 端点 `fetch/tests.rs` 加 `version_floors_pick_new_enough_and_upgrade_along`：自己源里的 1.1.0 不够照先后挑官方的 1.3.0；装着的 1.0.0、`1.2.0-rc.1`、没写的、`r5` 一起升，够的不动；哪个都不够的、没有的照原样报缺（照写的先后）；排过的那一份不够另一条下限的报缺。
5. 端点 `tests/package_depends_floor.rs`（真核心）两条：没写版本、比下限低、预发布的装不上（真装、看一眼都拒，`data.missing` 是 `["base>=1.2.0"]`）；`1.10.0` 照装、列表的 `depends` 照原样；降到 1.0.0 以后列表 `depends_missing`、`missing` 照原样、`miyu check` 说「缺少依赖的软件包：base>=1.2.0」。假软件源上装 `app`（要 `lib>=1.2.0`），装着 1.0.0 的 `lib` 一起升到 1.3.0、先升依赖；看一眼的推荐：装着不够的照列、装着够的不列、索引里不够的不列。
6. 原来的依赖测试（`package_depends.rs`、`packages_fetch.rs`、`embed_package.rs`）照过。
7. 手写变异 16 个都逮住：

   | 改坏的地方 | 逮住它的 |
   |---|---|
   | `fits` 用「大于」不用「不小于」 | `depend` 两条 |
   | 正式版比预发布的低 | `precedence_follows_the_semver_spec`（第一次没逮住，补了反向比较） |
   | 三段少了的补 0（认 `1.2`） | `depend` 两条 |
   | `workers` 带下限放过 | `packages_may_carry_a_version_floor` |
   | 不认下限（都当够） | `depend` 两条 |
   | 预发布的数字段照字母比 | `precedence_follows_the_semver_spec` |
   | 预发布段少的不算低 | `precedence_follows_the_semver_spec` |
   | 装着就算够（`missing` 不比版本） | `package_depends_floor` 第一条 |
   | `missing` 只给编号 | `package_depends_floor` 第一条 |
   | 软件源装着就跳过 | `version_floors_pick_new_enough_and_upgrade_along` |
   | 软件源不照下限挑 | 同上 |
   | 排过的不比 | 同上 |
   | 推荐的装着就不列 | `package_depends_floor` 第二条 |
   | 推荐的不照下限挑（第一次没逮住，补了索引里不够的那一条） | 同上 |
   | 索引照旧只认编号 | `entries_whose_id_or_version_is_not_a_plain_name_are_dropped` |
   | 收录丢掉下限 | `recommends_author_upstream_and_installed_size_come_from_the_package` |

8. clippy、远程门禁 11 项、三平台 CI。

## 风险

- 旧的核心读到带下限的清单报 `bad_dependency`，读新索引看不到带下限的那几条。作者文档写明用了这种写法的在 `[package] miyu` 写上最低版本。
- 软件源（packages 仓库）的收录照 `miyu-rev` 编的 `miyu-index` 读清单，没换之前带下限的包收录不进去：合进 main 以后 core1 换 `miyu-rev`。
- 头（终端、网页）把 `depends`、`missing` 里的字当编号用的，带下限的那几项对不上编号：要照 `>=` 切开（协议的形状没变，多了写法）。
