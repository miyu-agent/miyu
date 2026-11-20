## 施工单 PK-5：卸包默认留配置

状态：做完待合（2026-11-05）。core1 2026-11-05 派（派工单第 4 节，排在 PK-3 前面），项目主人定：照 pacman，卸包默认留配置。

### 为什么

卸包原来照 `DataRoot::package_places` 把各层它的 `<编号>/` 整个删掉（设计 55 第三节第 4 条），`etc/<编号>/`（`MIYU_CONFIG_DIR`）里人手改过的配置也一起没了：卸了重装要从头再配。pacman 卸包默认留下改过的配置（`.pacsave`），`-n`（`--nosave`）才删干净。

### 做了什么

1. **协议**（端点 `packages/asked.rs`、`manage.rs`）：`package.remove` 加布尔参数 `purge`（默认 `false`）。不 `purge` 的 `etc/<编号>/` 原样留下，`purge` 的同别的几处一起 `remove_dir_all`（不跟符号链接：本身是链接的只删链接，同 PK-4 的口径）。状态目录、缓存、日志、暂存、运行时照旧删，状态目录不跟着留。系统配置 `etc/config.toml` 里这个包的配置项照旧删。
2. **空的不留**：拉起扩展前各处都建好（`PackageDirs::ready`），没写过东西的配置目录也在；不 `purge` 卸的时候它是空目录的删掉（`drop_if_empty`，`remove_dir` 只删得掉空的），是链接的、不空的不动。
3. **看一眼**（`packages/preview.rs`）：`package.remove` 带 `preview` 的回应多一格 `config: {path, kept}`，`path` 照 `DataRoot::package_config`（不手拼），`kept` 是 `!purge`。不在的、空的、是普通文件的不写这一格（`purge` 的也一样）；本身是链接的照写（`holds_something`）。和 PK-4 的状态目录在同一个阻塞线程里看。
4. **别的卸法**：写组名的 `package.remove` 也收 `purge`、传给组里每个包；装回组写了 `purge` 的 `bad_params`。`package.disable` 关出厂的、删人格连包卸（`personas/delete.rs`）、她的工具经端口卸的一律不 `purge`。
5. **命令行**（`miyu-cli`）：`miyu pkg remove` 加 `--purge`（`-n`，pacman `-Rn`）；写了的看一眼、真卸都带 `purge: true`，不写的不带（旧核心的 `deny_unknown_fields` 不认这一格）。确认屏在 PK-4 那一行「会删掉 <状态目录>（<大小>）」后面另起一行：留下的「配置留在 <路径>」（`Config kept in <path>`），`--purge` 的「会删掉 <路径>」（`Will delete <path>`）；核心没给这一格的不印。帮助页中英：`remove` 那一行写「保留配置」，加 `-n, --purge`。会话里的 `miyu pkg remove --purge` 说这里用不了（`pkg-unavailable.txt`，`remove --purge`），退出码 2。
6. **留下的目录不算装着**：`package.list` 只照包文件夹（`opt/`）和出厂资源、卸掉的记号列，`check` 只查清单、配置文件，都不看 `etc/<编号>/`；装包不碰 `etc/<编号>/`，重装同编号拉起时 `MIYU_CONFIG_DIR` 还是它。这一步没有改这几处，测试证明。
7. **文档**：蓝图 `packages.md`「装卸」第 7 条（新「配置目录默认留下」）、第 8 条（新「卸的说清配置目录」）、测试表；`cli/pkg.md`（`remove` 那一行、`--purge` 选项、会话里那一行、确认屏、例子、字表、测试表）；`protocol.md`；参考 `packages.md`（中英）；作者文档 `README.md` 第 3 条、新第 5 条，`extensions.md`「被拉起」（中英）：给人改的放配置目录，卸包会留（`--purge` 才删）；状态放状态目录，卸包会删。`root/places.rs` 的 `package_places` 只改注释。

### 定下的技术题

- **`config` 的形状 `{path, kept}`**：照派工单。`kept` 虽然看一眼的人自己知道带没带 `purge`，写出来头不用记参数，照这一格印就行。不加 `size`：留下的东西不丢，大小对人没用；`purge` 的人自己要删，也不用量（量要走一遍目录）。
- **不在的、空的不写，`purge` 的也一样**：照派工单「目录不在或者是空的就不写」。`purge` 删空目录人也丢不了什么，不必说。是普通文件的同 PK-4 的状态目录，不算。
- **本身是链接的照写**：卸的时候确实留下（或 `purge` 删掉）那个链接，写出来人知道有这么一处；指到的东西不动。不跟进去看空不空。
- **空的不留**：不然每个拉起过的扩展卸掉都剩一个空的 `etc/<编号>/`。只看本身是不是空目录，底下有空的子目录的算不空、照留（`remove_dir` 删不掉），和看一眼「底下至少一项」的口径一致。
- **组也收 `purge`**：`miyu pkg remove basesystem --purge` 不该被拒成参数不对。组的看一眼照旧只列编号，不逐个说配置目录（要不要说，报给 core1 拍板）。
- **她的工具不收 `purge`**：加参数就是加给模型看的字，规矩要先问；连配置一起删让人在终端里自己敲。会话里的 `miyu pkg remove --purge` 照「这里用不了」退出码 2，不悄悄丢掉 `--purge` 当普通卸。
- **`-n` 当短写法**：照「pacman 的写法当别名」（设计 31 定了的 A），pacman 的 `-Rn` 就是不留 `.pacsave`。没加 `--nosave` 长写法：一个长名够了。
- **命令行不写 `--purge` 时不带 `purge: false`**：旧核心的 `RemoveParams` 是 `deny_unknown_fields`，带了就 `bad_params`；新核心不写就是 `false`。

### 头要跟着改

- 终端 `miyu-tui` 的卸包抽屉（`src/drawer/package.rs`）、网页 `web-demo/packages/asking/package-plan.js`：看一眼回应里有 `config` 的，`kept` 为真印「配置留在 <路径>」，假印「会删掉 <路径>」；要给人「连配置一起删」的，真卸、看一眼都带 `purge: true`。合了以后 core1 通知。

### 验收

- `crates/miyu-endpoint/tests/packages_purge.rs` 新 `removing_keeps_the_config_until_purged`：真扩展（测试扩展程序记 `MIYU_CONFIG_DIR` 和 `etc/echo/settings.toml` 在不在）装上、拉起读得到；卸了配置原样留下、状态目录删了、`package.list` 不列、`check` 的问题里没有它；从包文件夹重装、再拉起，`MIYU_CONFIG_DIR` 还是那个目录的真实位置、读得到；`purge: true` 的卸掉以后 `etc/echo/` 没了。
- 同文件 `removing_a_package_forgets_its_settings_and_state`：配置目录留下（底下的文件也在），别的五处删了；`removing_a_shipped_package_forgets_its_settings_too`：空的配置目录删掉。
- `crates/miyu-endpoint/tests/packages_preview.rs` 新 `removing_says_whether_the_config_stays`：不在、空的不写（`purge` 两种都试）；有东西的 `{"path", "kept": !purge}`，不写 `purge` 的 `kept: true`；什么都不动。新 `a_linked_config_directory_is_still_shown`（Unix）：配置目录本身是链接的照写。
- `crates/miyu-endpoint/tests/package_group_manage.rs`：卸组留配置；装回组写 `purge` 的 `bad_params`；`purge` 卸组删配置。
- `crates/miyu-cli/src/pkg/confirm/tests.rs` 的 `a_removal_says_what_goes_with_it`：「配置留在」中英、`--purge` 的「会删掉」中英。`pkg/tests.rs`：`remove x --purge`、`-Rn x` 认成 `purge: true`，`-R x` 是 `false`。`pkg/session/tests.rs`：会话里两种写法都说用不了。
- 变异（各改坏一处、对应测试变红再还原）：
  1. 不 `purge` 也删配置目录（`filter` 恒真）：`removing_keeps_the_config_until_purged`、`removing_a_package_forgets_its_settings_and_state` 红。
  2. `purge` 也不删（`filter` 不看 `purge`）：`removing_keeps_the_config_until_purged`、`the_base_system_goes_and_comes_back_as_a_group` 红。
  3. 空的配置目录不删：`removing_a_shipped_package_forgets_its_settings_too` 红。
  4. 空的也写 `config`：`removing_says_whether_the_config_stays` 红。
  5. `kept` 恒为 `true`：`removing_says_whether_the_config_stays` 红。
  6. 本身是链接的不写：`a_linked_config_directory_is_still_shown` 红。
  7. 卸组不把 `purge` 传下去：`the_base_system_goes_and_comes_back_as_a_group` 红。
  8. 命令行不印配置那一行：`a_removal_says_what_goes_with_it` 红。
  9. 会话里放过 `--purge`：`what_works_only_on_this_machine_is_refused` 红。
- 门禁 11 项、CI 六项。
