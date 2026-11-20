## 施工单 WD-3：「这个包可能要更新的 Miyu」改成「这个包可能需要更新版本的 Miyu」

状态：做完了，等合并（分支 `step/WD-3`，从 main 开）。文档会话查到，主会话 2026-11-20 定的新说法。

### 为什么

软件包的清单里有正在运行的核心不认识的表或者键，清单又写了 `[package] miyu` 时，核心在报错后面补一句提示（施工 VER-1，`package-problems/maybe_newer_miyu`）。中文原来写的是「这个包可能要更新的 Miyu」。这句话读不通：「要更新的 Miyu」既可以读成「需要更新版本的 Miyu」，也可以读成「将要被更新的 Miyu」。

### 改成什么样

- 中文字表 `resources/core/human/zh.json`：`{problem}。这个包可能要更新的 Miyu` 改成 `{problem}。这个包可能需要更新版本的 Miyu`。
- 英文 `{problem}. This package may need a newer Miyu`、日文「このパッケージには新しい Miyu が必要かもしれません」没有歧义，不修改。
- 手册 `docs/zh/dev/manifest.md` 里照抄的那一句、`crates/miyu-config/src/package/version.rs` 和 `crates/miyu-endpoint/src/packages/folder.rs` 两处注释跟着修改。
- 以前的施工单（VER-1、DR-2）和施工图里引用的旧说法是当时的记录，不修改。

### 同一个字表里查过的

在中文字表里查了「要更新的」「更新的」「新版」「升级」这几种说法，同样有歧义的只有这一句。`package-problems/needs_newer_miyu`「这个包要 Miyu {detail} 以上，先更新 Miyu」读得通，没有修改；它用的是口语的「要」，要不要改成「需要」交主会话决定。

### 验收

- `crates/miyu-core/tests/human_wording.rs` 加一条测试：中文字表里不出现「要更新的」。修改之前这条测试是红的。
- 门禁、Windows 版 clippy、三个平台的 CI 全部通过。

### 做下来的

- 中文字表改了一句；手册一句、两处代码注释跟着修改；`human_wording.rs` 加一条测试。
- 新测试在修改之前是红的：`left: ["said.package-problems/maybe_newer_miyu：{problem}。这个包可能要更新的 Miyu"]`，修改之后三条都通过。

### 怎么实测验收

2026-11-20 在沙箱里用编出来的 `miyu` 实测（`MIYU_HOME` 和 `HOME` 都指向临时目录，不需要模型）：造一个包文件夹，清单写 `miyu = "<这一版的版本号>"`，再加一张不认识的表 `[future]`，运行 `miyu pkg info <目录>`。

- 中文界面：报错的结尾是 `…[recommends]、[page]。这个包可能需要更新版本的 Miyu`。
- 英文界面：结尾是 `…[recommends] and [page]. This package may need a newer Miyu`，没有变化。
