## 施工单 PK-5补：卸包也留配置里的设置项

状态：做完待合（2026-11-05）。core1 2026-11-05 派（派工单第 6 节，排在 PK-3 上之前），项目主人改了 PK-5 时的推荐：`etc/config.toml` 里这个包的设置项也照 pacman 留着，`purge` 才删。记号放哪、没装的能不能 `purge` 由 core1 定（抄 sub1）。

### 为什么

PK-5 只留下了配置目录 `etc/<编号>/`，系统配置里这个包的设置项（清单 `[settings.<名字>]` 声明、人写进 `etc/config.toml` 的）还是一卸就删（施工 F-8 中下补「卸就是清干净」）：卸了重装要从头再设。留着又有个麻烦：包卸了，这些键不在配置清单里，读配置、`miyu check` 都报「不认识的键」。

### 做了什么

1. **卸**（端点 `packages/remove.rs`，从 `manage.rs` 挪出来，行数）：设置项只在 `purge` 时删（`config/forget.rs` 的 `forget`）。不 `purge` 卸你装的包、配置里真写了它的设置、下面一层（系统加装的、出厂的，施工 SD-1 起被它盖住、在清单里记成 `Issue::Covered`，`package.list` 里是 SD-1补 改名的 `shadowed`）没有同编号的，记一份 `etc/leftover/<编号>.toml`，一行 `prefix = "<前缀>"`（`miyu_store::packages::settings_left::mark`），在删文件的同一个阻塞线程里、重读清单之前写。`purge` 的删掉这一份。出厂的、系统加装的不记。回应照 SD-1 带 `layer`。
2. **新路径**（core1 定，抄 sub1）：`DataRoot::leftover_settings_dir()` 是 `etc/leftover/`，`DataRoot::leftover_settings(编号)` 是 `etc/leftover/<编号>.toml`（`miyu-store` 的 `root/places.rs`）。读写在 `miyu_store::packages::settings_left`（`mark`、`unmark`、`has`、`read`）：文件名合编号写法、后缀 `.toml` 的普通文件才算；读不懂的、前缀写法不对的当前缀就是编号。`leftover` 进保留编号（`miyu_config::package::RESERVED`，17 → 18），设计 55 第二、三节和蓝图 `store.md` 跟着加。
3. **读配置时怎么认、怎么跳过**（端点 `config/left.rs`、`packages/settings_left.rs`）：
   - 认：配置服务手里多一份 `Left`（前缀 → 留下它的包）。来处两样（`packages::settings_left::left`，照核心的三层 `Packages`）：卸掉的出厂、系统加装的包照清单（`Packages::read_removed`，SD-1 起取盖得住的那份，声明了设置项的才算，前缀照 `Manifest::prefix_of`）；`etc/leftover/` 记的。这时装着的编号（清单里有的，读坏了的也算）不算。
   - 什么时候换：核心起来时 `miyu-core` 的 `start.rs` 照读到的清单、系统加装那几处（`packages::distro_dirs`）算好，经 `settings::read` 交给 `Config::load_leaving`；装卸以后 `Core::reload_packages` 照新的清单算，和配置清单一起交给 `refit`（两样都没变的不动；没设端口的测试核心配置清单照旧、只换 `Left`）。没装的 `purge` 删了记号以后照这时的清单再算一次（`settings_left::refresh`）。
   - 跳过：解析照旧（`miyu_config::parse`，不改签名）。`config/file.rs` 读、写回、重新认一份文件时，解析出的问题里 `code` 是 `unknown_key`、键的第一段（照 `key::split`，带引号的也认）是留下的前缀的，拿掉、记到这份文件的 `leftover`（键和包）。于是：不报错（不进 `config.get`、`config.changed` 的问题，不进起来时 `WARN config problems` 的计数，握手的 `config_errors` 本来就不算警告）；不生效（不认识的键本来就不进最终值）；不影响别的（别的不认识的键照报，拼错的照给最近的键名；前缀被这时装着的包占着的，照那个包的项认，根本不是不认识的）。`config.check` 同样不报。
4. **`check`**（`check.rs`、`check/left.rs`（行数，分出来），`config/checking.rs` 的 `check_leaving`）：查系统配置、项目配置时照拿掉的按包数一数，一个包一条 `{"kind": "config", "file", "code": "settings_left", "level": "warning", "message"}`，不带行列。字在 `resources/core/human/{zh,en,ja}.json` 的 `config/settings-left`（英文多一句 `/one`，一项时用）：「有 N 项是已经卸掉的 <编号> 留下的设置：重装 <编号> 会照原样生效；不要了用 miyu pkg remove <编号> --purge 删掉」。
5. **重装同编号**：`from_path` 装成以后（`placed.keep()` 之后，同 P-11 划掉留下的改动那一步）删记号。配置清单换回来，`refit` 照手里的字重新认，设置照原样生效。新清单换了前缀的以新清单为准：旧前缀下的键没有记号了，照不认识报。
6. **看一眼**（`packages/preview.rs`）：`config` 那一格改成 `{path?, kept, settings?}`（`config_entry`）：`path` 照 PK-5（目录有东西才写），`settings` 是配置里写了的它的设置项有几项（`forget::present`，0 的不写），`kept` 是 `!purge`；`path`、`settings` 都没有的整格不写。顶上的 `settings`（键的列表）照旧是「一并删掉的配置项」：只有 `purge` 的列，不 `purge` 的是 `[]`。
7. **没装的也能 `purge`**（core1 定；`packages/asked.rs` 分，`packages/settings_left.rs` 的 `purge`）：`package.remove` 带 `purge: true`、编号这时没装的，只要还留着配置里的设置项（`config/forget.rs` 的 `left_by`：先照盘上重读，再照 `leftover` 找这个包的键）、`etc/<编号>/`（在就算，空的也删）或记号之一，就只删这三样（`etc/removed/` 不动：卸掉的出厂包照旧卸着），回 `{"package", "removed": true}`；看一眼回 `{"package", "settings": [<要删的键>], "config"?}`。什么都没留的、不带 `purge` 的照旧 `unknown_package`，编号写法不对的 `bad_params`。拿同一把装卸的锁。
8. **命令行**（`miyu-cli` 的 `pkg/confirm.rs`、`language/confirm.rs`）：`config_dir` 改成照 `path`、`settings`、`kept` 三样说：留下的「配置留在 <路径>」，另有设置的接「，另有 N 项设置」（`Config kept in <path>, plus N setting(s)`），没有配置目录的「N 项设置留在配置里」（`N setting(s) kept in the config`）；`--purge` 的「会删掉 <路径>」，设置项照旧在「一并删除」那一行列键、不再说几项。没装的 `--purge` 走同一屏（第一行「将卸载 <编号>」）。
9. **测试的端口挪进 support**：`packages_config.rs` 里照 `miyu-core` 拼配置清单的端口挪成 `tests/support/ported.rs`（`Port`、`served`），另加 `started`：照核心起来时那样算 `Left`、读配置、装端口，重启就是再造一个。

### 定下的技术题

- **`settings_left` 用警告**：`check` 现有只有 `error`、`warning` 两级，命令行（`config/check.rs`）、网页把不是 `warning` 的一律算错误、退出码 1。新加 `info` 旧的头会当错误，所以用警告。一个包一条，不逐个键报：派工单要的是「有 N 项」一句。
- **跳过在端点的 `File` 里做，不改 `miyu_config::parse`**：`parse` 的调用方多（测试四十多处），留下的前缀是运行时的状态、不是清单。`File::read`、`written`、`refit` 都改成收 `(配置项, Left)` 一对（`Config::reading`），`Config::load` 不变、另加 `load_leaving`。
- **照第一段认**：前缀写法同包编号、只有一段，包的设置项键都是 `<前缀>.…`。整张表写成 `[xcfg]` 的、写成点号连着的、带引号的都照第一段认；记下的是解析出的整个键，`purge` 照它一个个 `Unset`。
- **装着的编号不算留下的**：读坏了的、程序不在（`absent`，配置项不进清单）的也算装着，它的键照旧报不认识：这是 F-6 上的老口径，这一步不动。
- **前缀撞了别的装着的包**：键照那个包的项认（本来就认），`Left` 里的那一条没用但无害；不删记号。
- **记号只写前缀，不写键**：键从配置里照前缀找得到；人手删了这些键，记号就是个没用的小文件，不报、不清（core1 第 3 条：只在真写了设置时才留记号）。以后要的话加 `removed_at`。
- **下面一层有同编号的不记**（sub1 第 2 条）：判断在 `settings_left::lower_has`：核心手里的清单里同编号、不是你装那一层的还有一份（SD-1 起被盖住的记成 `Issue::Covered`、照样在清单里，`package.list` 里是 `shadowed`）。卸了你装的那一份它顶上来，配置清单里有它的项，键照它认、照原样生效。起草时 SD-1 还没合（两层同编号的家目录那一份报 `duplicate`），变基到 SD-1 以后照它的三层改成照清单判，补了真卸的测试。
- **卸掉的出厂包不记**：清单还在资源目录里，`read_removed` 读得出前缀；这样 `etc/removed/` 的意思一点不动（core1 第 1 条）。
- **测试核心没设端口时也换 `Left`**：`reload_packages` 原来没端口就不 `refit`；现在拿手里的配置清单照样 `refit`，两样都没变的照旧什么都不做。
- **`manage.rs` 拆出 `remove.rs`**：加了记号那几行就过 500 行；`removable`、`setting_keys`、`remove_package` 照旧经 `manage::` 能用（`pub(crate) use`）。

- **和 SD-1补「系统加装的卸了能照编号装回」不打架**：卸出厂的、系统加装的照旧只在 `etc/removed/` 记一笔，不写 `etc/leftover/`（前缀照 `read_removed` 那一份认，两层都有的是系统加装的）；照编号装回（`bring_back`）去掉 `etc/removed/` 那一笔，配置清单换回来，留下的设置照原样生效。没装的 `--purge` 只删设置项、`etc/<编号>/`、`etc/leftover/`，不碰 `etc/removed/`，删完照样装得回来。

### 头要跟着改

- 终端 `miyu-tui` 的卸包抽屉、网页 `web-demo/packages/asking/package-plan.js`：看一眼的 `config` 多 `settings`（几项），可能没有 `path`（目录空、但有设置）；`kept` 为真印「配置留在 <路径>，另有 N 项设置」或「N 项设置留在配置里」。顶上的 `settings` 不 `purge` 时是 `[]`：照它印「一并删除：设置 …」的不用改。要给人清已卸的包留下的，`package.remove {"package", "purge": true}` 对没装的编号也行。合了以后 core1 通知。

### 验收

- `crates/miyu-endpoint/tests/packages_settings_left.rs`（照 `miyu-core` 拼配置清单的真核心）：
  - `removed_settings_stay_quietly_and_come_back_on_reinstall`：看一眼 `settings: []`、`config: {kept: true, settings: 1}`；卸了 `xcfg.port = 9000` 原样留着，记号是 `prefix = "xcfg"`，`config.get` 不认，`check` 只有一条 `settings_left`（带 1 项、编号、`--purge`）；重启照记号认；重装照原样生效、记号删了、不再提示；`purge` 的删、注释不动、不留记号。
  - `a_custom_prefix_is_remembered_and_a_new_manifest_wins`：`prefix = "xc"` 的记 `xc`、照它认；重装换成 `xd` 的，旧前缀下的照不认识报。
  - `purging_what_a_removed_package_left`：看一眼带 `path` 和 `settings`；没装的不带 `purge` 两种都 `unknown_package`；带 `purge` 的看一眼 `{package, settings, config}`、什么都不动，真删只删三样、别的项不动，再来一次 `unknown_package`。
  - `a_removed_shipped_package_is_known_by_its_manifest`：出厂的卸了不记、照资源目录的清单认，重启一样，装回来不再提示。
  - `removing_a_copy_that_covers_a_lower_one_needs_no_mark`：系统加装的 `xcfg` 被你装的盖住，卸了你装的不写记号、回应 `layer: "home"`，系统加装的顶上来，`xcfg.port` 照原样生效，`check` 什么都不说。
- `crates/miyu-endpoint/src/config/left/tests.rs`：拿掉的照文件先后、按包数；别的不认识的、写错类型的照报；空的不拿；照前缀不照编号；第一段带引号的也认。
- `crates/miyu-store/src/packages/settings_left/tests.rs`：记、读、删，没有的删不算错；写坏的当编号，不合写法的、目录、没后缀的不算。`root/places/tests.rs` 照各层方法推保留名，`leftover` 在里面。
- `packages_preview.rs`：不 `purge` 的 `settings: []`、`config: {kept: true, settings: 1}`，`purge` 的列键、`kept: false`。`packages_purge.rs`：装上的、出厂的不 `purge` 卸，设置项留着。`packages_config.rs`：卸了推的 `config.changed` 不带问题。
- 命令行 `pkg/confirm/tests.rs` 的 `a_removal_says_how_many_settings_stay`：中英两种留下的说法、单数，`--purge` 的只列键。
- 变异（各改坏一处、对应测试变红再还原，14 个都红）：
  1. 不 `purge` 也删设置项：`packages_settings_left` 四条红。
  2. `purge` 也不删设置项：`removed_settings_stay_quietly_and_come_back_on_reinstall` 红。
  3. 不写记号：`removed_settings_stay…`、`a_custom_prefix…`、`purging_what_a_removed_package_left` 红。
  4. 记号写编号不写前缀：`a_custom_prefix_is_remembered_and_a_new_manifest_wins` 红。
  5. 不认卸掉的出厂包：`a_removed_shipped_package_is_known_by_its_manifest` 红。
  6. 读配置不拿掉留下的键：`packages_settings_left` 四条、`packages_config` 那一条、`config::left` 的单元测试红。
  7. 重装不删记号：`removed_settings_stay…`、`a_custom_prefix…` 红。
  8. `check` 不说 `settings_left`：三条红。
  9. 看一眼不带设置项数：`packages_preview` 的 `removing_shows_what_goes_with_it_and_changes_nothing`、`packages_settings_left` 两条红。
  10. 没装的 `purge` 不删配置目录：`purging_what_a_removed_package_left` 红。
  11. 没装的不带 `purge` 也收：`purging_what_a_removed_package_left` 红。
  12. 下面一层恒为没有：`removing_a_copy_that_covers_a_lower_one_needs_no_mark` 红。
  13. 命令行不印设置项数：`a_removal_says_how_many_settings_stay` 红。
  14. `refit` 不换 `Left`：`packages_settings_left` 四条红。
- 门禁 11 项、CI 六项。
