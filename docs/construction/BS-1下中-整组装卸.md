## BS-1 下中：装卸整个组

状态：主会话做。设计 44（基础系统拆包）第三节第 2 条：`miyu pkg install basesystem`、`package.install {"package": "basesystem"}` 把组里卸掉了的出厂包一起装回来，`remove` 一次卸掉全组（照 pacman 写组名）。

### 一、做了什么

1. **认组**（`packages/group.rs`）：组表里有、又不是哪个出厂包的编号的（组和包撞名先认包，BS-1 下上的测试守着出厂资源不撞）。组里的包是出厂清单写了这个组的。组表的读法挪进这里，`package.list` 顶上的 `groups` 照用。`package.install`、`package.remove` 写组名的在分派那一处（`packages/methods.rs`）就分给它（`group::call`），软件源、装卸一个的那几条路不动（`manage.rs` 守 500 行）。
2. **装回**：`package.install {"package": <组>}`（软件源那一路照组名不走）把组里卸掉了的出厂包一个个装回（同装回一个），回 `{"group", "packages": [<列表那一项>…]}`。
3. **卸**：`package.remove {"package": <组>}` 先看组外装着的包依赖不依赖组里的，有的整个拒 `package_depended`（`data.by` 是它们）、什么都不卸；组里互相依赖的先卸依赖别人的（`shell` 在 `packages`、`clicall` 后面），一个个卸（同卸一个），回 `{"group", "removed": [<编号>…]}`。
4. **看一眼**：两样带 `preview` 的回 `{"group", "packages": [<编号>…]}`：这一次会装回、会卸掉的那几个。
5. **命令行**（`pkg/group.rs`）：`miyu pkg install|remove <组>` 看一眼回组的照组走：印「将装回/将卸载 <组>」「包含：…」、问一句（`--yes` 不问），一个一行说装好了、卸掉了谁；组里没有要动的说一句、退出码 0。`remove` 那一支从 `pkg.rs` 挪进来（守 500 行）。
6. **文档**：协议参考 `packages.md` 中英、蓝图 `protocol.md`、`cli/pkg.md`。

### 二、不做什么

- 她那一头（会话里的 `miyu pkg install|remove <组>`）：她的 `package` 工具装编号只走软件源（`PackagesPort::fetch`），连装回一个卸掉的出厂包也不认，组更不认；要的话另排（连带装回单个出厂包）。
- 斜杠命令的拥有者是 BS-1 下下。

### 三、怎么验

1. 端点 `package_group_manage.rs`：看一眼卸 `basesystem` 列出组里的、组外的不动、什么都没卸；卸掉以后组里的都没了、组外的留着；看一眼装回的就是卸掉的那几个；装回来都在。组外的包依赖 `shell` 的整个拒、组里一个都没卸。组里只卸了一个的，装回组只装回它。
2. 命令行 `miyu/tests/pkg_group.rs`：组里没有要动的说一句；卸印「将卸载 basesystem」「包含：…」、答了一个一行「卸掉了：…」；`--yes` 装回不问、一个一行「装好了：…」。
3. 手写变异 7 个抓到 6 个：不认组、装回不挑卸掉的、组外依赖不拒、软件源那一路照组名走、命令行不认组、组里没有要动的不说。没抓到的一个是「照依赖先后卸」：出厂组里被依赖的只有 `shell`，依赖它的 `packages`、`clicall` 照编号本来就排在它前面，眼下的资源造不出反过来的情形；这一道守的是以后组里加包。
4. 本机门禁。
