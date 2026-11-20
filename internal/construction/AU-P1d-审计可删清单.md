## 施工单 AU-P1d：审计中危里可以删的几处

状态：做完了，等合并（分支 `step/AU-P1d`，从 main 开）。清单来自 `AU-P1-审计中危.md`，项目主人 2026-11-20 批准；AU-040 是主会话定的；AU-049 项目主人同一天改成保留。

### 为什么

审计核实时留下一张「可删清单」：这些代码不影响行为，但是多编了用不到的东西，或者让测试专用的函数出现在正式的公开接口里。

### 改成什么样

| 条目 | 改动 |
|---|---|
| AU-044 | `crates/miyu-core/Cargo.toml` 的正式依赖里删掉 `miyu-drivers`。`src/` 里只有 `models/tests.rs` 用它，开发依赖里已经有。程序里它仍然由 `miyu-session`、`miyu-http` 带进来 |
| AU-045 | 删掉 `DataRoot::imported_file`。导入和读标记的几处一直是把 `bundle::MARKER` 接在会话目录后面，没有人调用它。蓝图 `store.md` 的那一行、`FH-盘点-核心.md` 的两行跟着改对 |
| AU-047 | 只有测试用的五个公开函数不再编进正式程序。`miyu_core::builtin_tools`、`Packaged::as_released`、`Human::with_dir`、`Watcher::by_helper`（连同它读的那一格 `helper`）放在 `testkit` 特性后面；`Personas::at_home` 只有同一个模块的单元测试用，放在 `cfg(test)` 后面。`miyu-core` 新加 `testkit` 特性，`miyu-core`、`miyu-store`、`miyu-sandbox` 的开发依赖里带上自己的 `testkit` |
| AU-048 | 画图库 `third_party/mermaid-rs-renderer` 的 `layout_dump` 模块前面加 `#[cfg(feature = "cli")]`。这个模块只有 `cli.rs` 用，Miyu 不开 `cli` 特性，所以这 159 行不再编进来。记在 `third_party/README.md` 第 11 条 |
| AU-049 | 测试用的小程序 `miyu-test-embed` 保留，`[[bin]]` 加 `required-features = ["testkit"]`：不带测试的构建不编它，运行测试时仍然编 |
| AU-040 | `crates/miyu-heap/Cargo.toml` 加 `undocumented_unsafe_blocks = "deny"`。现有的 `unsafe` 块都写了 `SAFETY`，不需要修改代码 |

### 和清单不一样的两处

- **AU-048 没有删文件**。清单写的是删掉 `layout_dump`。删掉文件以后，画图库自己的 `cli` 特性无法编译（`cli.rs` 引用它），和上游的差别也更大，升级时多一处要处理。加一行条件编译可以达到同样的结果：不编进 Miyu。命令行 `cli.rs` 本来就不编，没有修改。
- **AU-049 没有删程序**。靠它的测试有 47 条，分布在 `miyu-session`、`miyu-endpoint`、`miyu` 三个 crate，其中约 20 条测试的就是「拉起小程序」这条路径（启动进程、等待就绪、闲了退出、启动失败三次不再拉起、换包时旧进程退出、核心被杀时子进程一起结束）。本机向量没有可以注入假实现的接口，进程内的假实现测不到这些。项目主人定：保留，改成只在测试时编。

### 不做什么

- 不修改 `miyu-endpoint` 的 `miyu-test-extension`、`miyu-test-mcp`：清单里没有它们。
- 不删画图库的命令行。

### 验收

- 这一步没有新的行为，所以没有新测试；验收看原有测试和构建。
- `cargo build --workspace` 以后 `target/debug/` 里没有 `miyu-test-embed`；`cargo test --workspace --no-run` 以后有。
- 门禁、Windows 版 clippy、三个平台的 CI 全部通过。`miyu-heap` 的新 lint 在 macOS 和 Windows 上各有 `unsafe` 块，由三个平台的 CI 检查。

### 做下来的

- 六条都按上面的表修改，没有修改程序的行为。
- `miyu-test-embed` 找不到时的两句提示（`crates/miyu-session/tests/support/package.rs`、`crates/miyu-endpoint/tests/memory_meaning.rs`）改成 `cargo build -p miyu-session --features testkit`，因为不带特性的 `cargo build -p miyu-session` 不再编它。
- `AU-P1-审计中危.md` 里这六条的「怎么处理」一格写上了结果。

### 怎么实测验收

这一步没有界面和模型可以实测，验收的是构建。2026-11-20 在构建机上实测（每一步之前先删掉 `target/debug/miyu-test-embed`）：

| 命令 | 之后 `miyu-test-embed` 在不在 |
|---|---|
| `cargo build --workspace` | 不在 |
| `cargo test -p miyu-session --no-run` | 在 |
| `cargo test --workspace --no-run` | 在 |
| `cargo test -p miyu-endpoint --no-run` | 不在 |
| `cargo test -p miyu --no-run` | 不在 |

最后两行和修改之前相同：单独测试其他 crate 时，cargo 本来就不编 `miyu-session` 的程序，需要先编它（提示里写了命令）。编出来以后，`miyu-session` 的 `embed` 相关测试 14 条、`miyu-endpoint` 的 `memory_meaning` 4 条和 `check_embedding` 3 条都通过。

其他几条：整个工作区的编译、clippy（包括核心不开可选包的那一遍）、`cargo doc` 都通过；`miyu-heap` 的新 lint 在本机对 Apple、Windows、Linux 三个目标各运行了一遍 clippy，都通过。

自己验收时可以运行：

```bash
rm -f target/debug/miyu-test-embed
cargo build --workspace
ls target/debug/miyu-test-embed   # 应该提示文件不存在
cargo test -p miyu-session --test all embed
```
