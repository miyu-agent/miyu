## SD-1 系统加装层

状态：已合（6b184555a）；补见末尾「SD-1补」（2026-11-05，sub1）。设计 `60-系统源分包.md` 第三节、第六节第 1 条；蓝图 `packages.md`「三层怎么认」「系统加装」
「协议」「`miyu check`」「本地库」「装卸」、`protocol.md`、`cli/pkg.md`；参考 `packages`（中英）；作者文档 `README`（中英）。

### 目的

系统的包管理器（AUR、Homebrew）要能把 Miyu 的终端、网页和别人的扩展装成系统包。核心原来只认两层：出厂的（资源目录）和装上的（数据根
`opt/`），系统包没处放。这一步加第三层「系统加装」，同编号几层都有时上面的盖住下面的。

### 定了的（core1 11-05）

1. **三层统一盖**：装上的盖系统加装的，系统加装的盖出厂的，被盖住的不算错，原来出厂赢、装上的报 `duplicate` 的老规矩去掉。
   `miyu check` 和 `package.list` 说清被哪一层盖住、两边的版本；被盖住的那份更新时提示怎么换回来（防止核心升级了，`home` 里的旧终端还盖着新的）。
   蓝图「两层怎么认」整段重写，`command_taken` 这些的先后跟着改成照盖的顺序。
2. **`package.check` 查系统加装的**只看清单读不读得成、`bin/` 的程序在不在，不比哈希，说一句文件归系统包管理器管。
3. **Homebrew 只在 macOS 上找**；Linux 一律照 `XDG_DATA_DIRS`（Linuxbrew 的 `share` 用户写进去），这一句写进作者文档。

### 做什么

- **store**（`crates/miyu-store/src/packages.rs`、`packages/distro.rs`）：
  - `Layer::Distro`（协议上 `distro`），先后 `Shipped < Distro < Home`。
  - `distro::dirs(&Where, shipped)`：`MIYU_DISTRO_PACKAGES`（绝对路径，设了只看它）；Linux 照 `XDG_DATA_DIRS` 用 `:` 拆（没设、空的照
    `/usr/local/share:/usr/share`），相对的不算、重复的留第一次；macOS 照 `miyu` 被叫起来的路径（不解开链接）上一级的
    `share/miyu/packages/`；Windows 没有；和出厂那一层真实路径一样的去掉。
  - `Packages::new(resources, root, distro)` 多一个参数（测试传空的），`Packages::system` 照这个进程的环境算；`Packages::distro()`。
  - 读出来照 `(编号, 层从高到低)` 稳定排序：同一个编号排第一的就是认的那份，后面的记成 `Issue::Covered { by, version }`（不加
    `Found` 的字段：五十多处字面量）。下游原来「照编号找第一个」照旧拿到认的那份。
  - 先到先得（子命令名、功能、前缀）照盖的先后：装上的、系统加装的、出厂的，同一层照编号。
  - 卸掉的记号管出厂的、系统加装的；`read_removed` 一个编号一份，取盖得住的那份（系统加装的先于出厂的）。
  - 去掉 `Code::Duplicate` 和它三种语言的字。
- **endpoint**：
  - `Core` 多 `distro`（`with_distro`），默认没有；`packages::load(resources, root, distro)`；`packages(core)`、重读照它。
  - `miyu-core` 起来时照环境算一次（`distro_dirs`），读清单、交给核心都用它。
  - `package.list`：`layer` 多 `distro`、`from` 多 `distro`；被盖住的带 `covered: {by, version, over, newer}`、没有 `code`
    （`packages::cover` 照 semver 比，两边读得成才比）。
  - `miyu check`：被盖住的报警告 `covered`，说被哪一层盖住、两边版本；更新的接一句：盖它的是装上的说 `miyu pkg remove <编号>`，是系统
    加装的说用系统包管理器卸（卸掉的记号照编号记，`pkg remove` 会连下面那份一起藏起来）。
  - `package.check`：认的那份是系统加装的另有一项（`layer: distro`，`manifest` 写读不成的原因，`missing` 写 `bin/` 里没有的程序）。
  - `package.remove`：系统加装的同出厂的记一笔、文件不动；回应多 `layer`。本地库只记装上的。
- **命令行**：`pkg remove` 卸系统加装的接一行提示；`pkg check` 系统加装的说「正常（系统的包管理器装的，文件归它管）」、清单读不成的
  算坏了；`pkg owns` 系统加装的算装着的。转交子命令、找界面照 `Packages::system`。
- **字**：`package-problems/covered`、`covered_newer`、`covered_newer_distro`、`package-layers/*`，中英日。
- **文档**：蓝图 `packages.md` 那几节重写、`protocol.md`、`cli/pkg.md`，参考 `packages`（中英），作者文档 `README` 加系统源装在哪一段
  （「经系统源发」整节是 SD-3）。

### 怎么验

- store 单元：三层同编号谁盖谁（系统加装两处先列的优先、不看版本）、照编号找到的是认的那份；卸掉的记号连系统加装的一起藏、
  `read_removed` 取系统加装那份；子命令名、功能、前缀照新的先后；`distro::dirs` 的几种（XDG 先后、默认、相对的、重复的、macOS 前缀、
  Windows、变量、和出厂同一处）。
- endpoint 集成（`tests/package_distro.rs`，真核心）：系统加装的照列、来源 `distro`；装上的 0.2.0 盖住系统加装的 0.3.0，`covered`
  四格对、没有 `code`；`miyu check` 一条 `covered` 警告、带两个版本和 `miyu pkg remove dterm`；`package.check` 系统加装的一项、删掉
  程序后 `missing` 是 `bin/miyu-dview`；卸系统加装的回 `layer: distro`、文件还在、列表里只当装回的候选；清单写坏的 `manifest`。
- 原来守着「出厂的赢、`duplicate`」的两条测试照新规矩改；`package.remove` 回应整个比的那一条加 `layer`。
- 受影响的几个 crate 测试 2861 个全过。

### 变异

手写 12 个，都抓住：排序不照层从高到低、不记被盖住、记成被自己那一层盖住、卸掉的记号只管出厂的、先到先得不照盖的先后、系统加装几处
不去重、不去掉和出厂同一处的、不认 `MIYU_DISTRO_PACKAGES`、从不算「更新」、两种提示用反、`package.check` 连被盖住的系统加装那份也查、
核心重读不带系统加装那几处。「被盖住的也查」第一次写法让类型推不出来编不过，换个写法补跑抓住了。

### 坑

- **门禁第一趟卡行数**：store 的 `packages/tests.rs` 601 行、`miyu-core` 的 `lib.rs` 508、`miyu-endpoint` 的 `lib.rs` 512。拆法：盖、先到先得的几条
  测试挪进 `packages/tests/layers.rs`；core 读清单的 `distro_dirs`、`load_packages` 挪进它的 `packages.rs`；endpoint 的 `with_built_in`、
  `with_distro` 挪进 `packages.rs` 里另起的 `impl Core`（挪到 PK-5 之后，`manage.rs` 那边加了 `purge` 已经 497 行）。

- **不往 `Env`、`Found` 上加字段**：两边各有几十处测试里的字面量。系统加装那几处另用 `distro::Where` 读，层的先后靠排序。
- **测试里造的核心默认没有系统加装那一层**：开发机以后真装了系统包（`/usr/share/miyu/packages`），测试照样不受影响；要的测试用
  `Core::with_distro` 指一处。
- **`pkg remove` 的记号照编号记**：盖它的是系统加装的、被盖的出厂那份更新时，提示不能叫人 `pkg remove`（会两份一起藏），只能用系统的
  包管理器卸。
- **Windows 上 `/usr/share` 不算绝对路径**：`distro::dirs` 那几条用 Unix 路径的测试只在 Unix 上跑；XDG 照规定用 `:` 拆，不用随平台变的
  `split_paths`。

### SD-1补：系统加装的装回来，被盖住的那一格改名 `shadowed`

core1 拿 6b184555a 和网页实测，找到两处问题。

1. **系统加装的包卸掉以后装不回来**。`MIYU_DISTRO_PACKAGES` 下放一个 `kit/`，先 `miyu pkg remove --yes kit`，再 `miyu pkg install --yes kit`，
   报「软件包不存在」；网页「装回」发的 `package.install {package: "kit"}` 也一样。原因是 `package.install` 只写编号时，只有出厂包的
   编号才走装回，别的编号都去了软件源。改法：
   - 只写编号时，出厂、系统加装哪一层有这个编号（卸没卸都算，`packages::underneath`），就走装回；
   - 看一眼（`restores: true`、版本、带了什么）和装回前查冲突，都照 `read_removed` 里装回来的那一份，两层都有的取系统加装的；
   - 记号原来就是照编号去掉的，不用改；
   - 她的 `package` 工具那一路（`port.rs` 的 `Back`）本来就照记号认，只把名字从 `Shipped` 改成 `Removed`。
2. **包级的 `covered` 撞名**。F-9 补早就用 `covered: true` 表示「包里的 MCP 服务全被同名自制服务替代」，带它的是认的那一份；SD-1 又用
   `covered: {by, …}` 表示「被上面一层盖住」。只看 `covered` 真不真的头，会把 F-9 那种认的那份当成被盖住、直接丢掉。改法：
   - `package.list` 的这一格改叫 `shadowed: {by, version, over, newer}`；
   - `miyu check` 的代码改成 `shadowed`；
   - 字的键改成 `package-problems/shadowed`、`shadowed_newer`、`shadowed_newer_distro`；
   - F-9 的 `covered` 不动；
   - Rust 里 `Issue::Covered` 不改名，人格、预设的 `layers.rs` 也用这个词，改了牵连太大；
   - 终端、网页由 core1 通知。

**怎么验**：`tests/package_distro.rs` 加一条真核心测试，依次查这几步：
- 卸系统加装的 `dkit`，记号写上；
- 只写编号看一眼，得 `restores: true`、版本 `1.2.0`；
- 装回后是 `layer: distro`、没有 `removed`，记号去掉，列表里只剩一项；
- 再装一次，得 `unknown_package`，不去软件源。

另一条：卸掉以后家目录装了一个子命令也叫 `dkit` 的包，看一眼、真装回都报 `package_conflict`，记号留着（查的是系统加装那份的清单）。

原来那几条照新名字改。文档改了蓝图 `packages.md`、`protocol.md`，参考 `packages`（中英）。

**变异**：手写 7 个，都抓住：
- 只写编号从不走装回；
- 只有出厂的走装回；
- 看一眼照出厂那层找；
- 查冲突照出厂那层找；
- 格子写回 `covered`；
- `miyu check` 的代码写回 `covered`；
- 字用旧键。
