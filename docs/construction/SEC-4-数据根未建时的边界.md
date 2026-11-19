## 施工单 SEC-4：数据根还没有建好、上级目录是链接时，边界表仍然认得数据根

状态：做完了，等合并（分支 `step/SEC-4`，从 main 开）。来源是 LiteMiyu 的实测反哺（litemiyu `ba84f4b`，LM-7）。

### 为什么

LiteMiyu 的权限策略记自己的数据目录时只调用 `canonicalize`，目录还没有建好时记下原样的路径；判断目标时又把目标换成真实位置（还不存在的路径按最近的上级目录算）。上级目录里有链接时（macOS 的 `/var` 指向 `/private/var`；Linux 上 `~/.config`、`~/.cache` 被链到其他位置），两边对不上，对数据目录的写入没有被拦住。这段代码是从 Miyu 搬过去的，所以需要在 Miyu 上核实。

### 核实的结果

2026-11-19 核实，结论分三条：

1. **产品里的路径不受影响（读代码判断）。** 核心启动时就建好数据根（`DataRoot::prepare`），之后才有会话，所以记数据根时 `canonicalize` 总是成功。关起来的会话（群聊、陌生人的私聊）的文件守卫是白名单式的：目标的真实位置必须在工作区或者加进来的目录里（`guard/confined.rs`），记错工作区只会多拒绝，不会多放行。本人的会话在完全放开时本来就可以写数据根（施工 RO-3），只读时哪里都不能写。macOS 的 CI 上临时目录就在 `/var` 下，现有的守卫测试一直在那里通过。
2. **边界表有同一类问题（红测试证明）。** `Boundary::new` 对不存在的目录一律不算。数据根不存在时，边界表里没有数据根这一片，它底下的路径落到其他片里。新加的测试在修改之前是红的：数据根在链接下面、已经建好时结果正确；还没有建好时，`link/later/etc/secrets.toml` 的结果是 `Outside`，应该是 `DataRoot`。
3. **其他同样写法的地方没有修改（读代码判断）。** 下面十处是「`canonicalize` 失败就用原样路径」，记的都是数据根或者家目录，核心运行时它们一定存在；默认工作区那一处记错时只会多拒绝。

| 位置 | 记的是什么 | 为什么不改 |
|---|---|---|
| `crates/miyu-session/src/guard.rs`（`workspace`） | 数据根，用来算默认工作区的位置 | 数据根存在 |
| `crates/miyu-session/src/sandbox.rs`（`real`） | 数据根、家目录、技能文件夹 | 数据根和家目录存在；技能文件夹是放开读的，换不成时沙盒助手不运行 |
| `crates/miyu-session/src/default_workspace.rs`（`real`） | 会话自己的工作区 | 还没有建好时记成原样的路径，结果是判成「宽」并退回默认工作区，方向是收紧 |
| `crates/miyu-session/src/attached.rs` | 数据根 | 数据根存在 |
| `crates/miyu-endpoint/src/sessions/places.rs` | 数据根 | 数据根存在 |
| `crates/miyu-endpoint/src/config.rs` | 数据根、家目录 | 都存在 |
| `crates/miyu-endpoint/src/provide/remote.rs` | 数据根 | 数据根存在 |
| `crates/miyu-basesystem/src/trash.rs`（两处） | 数据根、家目录 | 都存在 |
| `crates/miyu-session/src/skills.rs` | 技能文件夹 | 是扫描出来的，存在；放开读的一片，换不成时边界表不算它 |

另外核对了打包：Miyu 的门禁和 CI 里没有检查动态库的白名单（`xtask`、`.github` 里没有 `otool`、`readelf`、`dylib` 相关的检查），所以不需要把 `libiconv` 加进任何名单。

### 改成什么样

蓝图 `fs.md` 的 `Boundary::new` 那一行：数据根和「这个人自己的几处」一样，还没有建好时按最近的、已经存在的上级目录换成真实的位置（`resolve`）；放开的几片不存在时仍然不算。

- 改之前：`data_root: real(&places.data_root)`，不存在就没有这一片。
- 改之后：`data_root: resolved(&places.data_root)`，`resolved` 调用 `crate::resolve`；换不成（空的路径、上级里有指向不存在位置的链接）时没有这一片。

### 不做什么

- 不修改上表里的十处。以后数据根拆成几处、有的目录用到时才建时，再把它们换成同一个办法。
- 不改变文件守卫和沙盒的行为。

### 验收

- 新测试 `the_data_root_under_a_linked_parent_is_known_whether_it_is_made_or_not` 在修改之前是红的（`left: Outside, right: DataRoot`），修改之后通过。测试自己建链接，所以在 Linux 上就能看到；标了 `cfg(unix)`，macOS 的 CI 也运行。
- 门禁、Windows 版 clippy、三个平台的 CI 全部通过。

### 怎么实测验收

这一步没有界面和模型可以实测：数据根不存在的状态在运行着的核心里造不出来。验收看测试：

```bash
cargo test -p miyu-fs --test all boundary
```

把 `crates/miyu-fs/src/boundary.rs` 里的 `resolved(&places.data_root)` 改回 `real(&places.data_root)` 再运行，新测试应该失败。
