## 施工单 PK-4：卸包说清状态目录

状态：做完待合（2026-11-05）。core1 2026-11-05 排（蓝图 `packages.md`「还没有的」，派工单第 2 节）：迁移程序把人的数据放进了自己的状态目录，卸包时整个删掉，人事先看不出来。

### 为什么

卸包会把状态目录 `var/lib/<编号>/` 整个删掉（`packages.md`「装卸」第 7 条）。`package.remove` 看一眼原来只答 `state: true`/`false`，命令行印「一并删除：……、状态目录」：人看不到在哪、多大，几 KB 的登录态和几 GB 的资料看起来一样。作者文档也没说人的数据不该放那里。

### 做了什么

1. **协议**（端点 `packages/preview.rs` 的 `removing`）：`package.remove` 带 `preview` 的回应里 `state` 改成 `{"path", "size"}`。`path` 照 `DataRoot::package_state` 拿（`root/places.rs`，不手拼），`size` 是底下普通文件的字节数之和；不在的、是普通文件的不写这一格。量大小在阻塞线程里（`measured`）。
2. **不跟符号链接**：状态目录本身是链接的，卸的时候 `remove_dir_all` 只删链接、指到的不动，所以算 0；底下的链接（`DirEntry::file_type` 不跟）既不算大小、也不进去；读不了的子目录、文件跳过，不报错（只是给人看的数）。
3. **命令行**（`miyu-cli` 的 `pkg/confirm.rs`、`language/confirm.rs`）：卸之前那一屏在「一并删除：」那一行后面另起一行「会删掉 <路径>（<大小>）」，英文 `Will delete <path> (<size>)`；大小用 `pkg/query.rs` 已有的 `size`（照 pacman：B、一位小数的 KiB、MiB、GiB）。「一并删除」只剩设置的键。旧的核心答 `state: true` 的照旧在「一并删除」里列「状态目录」。顺手把 `plan` 里拼一行的闭包改成交回 `Option<String>`，好在两行之间插这一行。
4. **文档**：蓝图 `packages.md`「装卸」第 8 条、测试表，`cli/pkg.md`（怎么印、例子、字表），`protocol.md`，参考 `packages.md`（中英），作者文档 `README.md` 新第 4 条、`extensions.md`「被拉起」（中英）：人的数据别放状态目录，卸包会删；要留给人的放 `MIYU_DOCUMENTS_DIR`（`root/documents/`）这些人看得见的地方。「还没有的」里 PK-4 那条挪走。

### 定下的技术题

- 状态目录是链接的算 0、照样写这一格：卸的时候确实会删掉那个链接，写出来人知道有这么一处；指到的东西不删，不算进大小。
- `size` 只算普通文件的字节数（文件长度，不是占的块），同本地库 `desc` 的 `size`、`package.remove` 看一眼的 `size`。
- 不另加一格、直接改 `state` 的形状：派工单写的就是 `state: {path, size}`；旧的 `true` 变成对象，`== true` 判的头这一行会不印（不会崩），终端、网页要跟着改（见下）。

### 头要跟着改

- 终端 `miyu-tui` 的 `src/drawer/package.rs`（`deleted` 里 `preview["state"] == true`）、网页 `web-demo/packages/asking/package-plan.js`（`p.state === true`）：改成看有没有 `state` 这一格，写上路径和大小。合了以后 core1 通知。

### 验收

- `crates/miyu-endpoint/tests/packages_preview.rs` 的 `removing_shows_what_goes_with_it_and_changes_nothing`：状态目录里两个文件（10 + 2048 字节，一个在子目录里）、一个指到外面 4 KiB 文件的链接（Windows 上换成空目录），`state` 是 `{"path": <DataRoot::package_state>, "size": 2058}`；出厂的没有状态目录的不写 `state`，那个位置上是普通文件的也不写；什么都不动。
- 同文件 `a_linked_state_directory_counts_as_nothing`（Unix）：状态目录本身是指到外面目录的链接，`size` 是 0。
- `crates/miyu-cli/src/pkg/confirm/tests.rs` 的 `a_removal_says_what_goes_with_it`：「会删掉 <路径>（3.5 MiB）」单起一行、「一并删除」只剩设置；英文、0 B 的；旧核心的 `true` 照旧列「状态目录」。
- 变异（各改坏一处、对应测试变红再还原）：量的时候跟进链接（`removing_…` 红）、状态目录本身照链接指到的量（`a_linked_…` 红）、是普通文件的也写 `state`（`removing_…` 红，加了普通文件那一段以后）、普通文件不算大小（`removing_…` 红）、命令行不印「会删掉」那一行（`a_removal_says_what_goes_with_it` 红）。
- 门禁 11 项、CI 六项。
