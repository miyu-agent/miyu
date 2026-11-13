## AD-2：蓝图跟上只剩两级的权限，设计 48 的前缀长度照代码

状态：已完成（2026-11-13）。起因：AD-1 第四节 C、D 两条。这一步只改文档，不改代码。

### 目的

A16（施工 LV-1、LV-2、LV-5）以后权限只剩只读、完全放开两级，蓝图还有不少地方按「工作区」那一级写。把写现状的蓝图改成两级的写法；设计 48 写错的前缀写法照代码改。项目主人同一天同意顺带让作者文档指向模板仓库。

### 蓝图改哪几节

- `session/guard.md`：实际生效的那一级、判法表、不报路径的调用两张表去掉工作区那一列；问人时的规则只剩只读时读私密目录（例子换成读 `~/.ssh/config`）；说明里 `sandbox` 那一格；守着它的几行照现在的测试写，删掉 LV-1 删了的 `sandbox_workspace.rs`。
- `session/tools.md` 第 1a 条：去掉工作区那一级的规格、沙盒自己的临时目录、工具链的缓存（LV-1 删了），测试两行。
- `core.md`、`store.md`：沙盒的缓存随它去掉。
- `mcp.md`、`providers.md`、`sandbox.md`、`tools/shell.md`、`tools/package.md`、`tools/trash.md`、`tools/web_fetch.md`、`tools/interface.md`、`cli/pkg.md`、`cli/ask.md`、`kernel/asking.md`、`kernel/blocks.md`、`kernel/events-bodies.md`、`kernel/session.md`、`kernel/request.md`、`protocol.md`、`models.md`、`config.md`：照两级改说法、例子、测试行。
- `prompts.md`：登记簿 `shell/hazard.txt` 的「什么时候加进来」改成完全放开下拒，`cargo xtask prompts` 重新生成。
- `packages.md` 的 `bad_prefix`；设计 `48-包的命名.md` 第二节第 1 条；作者文档 `manifest` 中英的 `prefix`：前缀照 `miyu_config::key::ID`，最多 32 个、不是 Windows 保留名，比包编号（64）短。AD-1 记成「代码按包编号允许 64」，核对下来清单读 `prefix` 查的是 `key::ID`，32 对，错的是「和包编号一样」。
- 作者文档 `README` 中英：写包可以从模板仓库 `miyu-agent/miyu-package-template` 开始。

### 不做什么

- 写当时的施工单、设计记录（设计 11、施工 5-x、X-5 的实测记录）不改；登记簿里 2026-10-01 量 token 的条件（`previous` 按 `workspace`）是量的记录，不改。
- 代码不改：`miyu-perf` 量工具那一步还按「工作区（关进沙盒）和完全放开」各跑一遍，A16 以后两边都是完全放开，量不出沙盒的开销，`perf.md` 第 5 条照代码留着，另排一步改量尺；`untargeted.rs` 的注释、`bad_prefix` 的报错还写「工作区」「package id」，随下次碰代码改；没写 `prefix` 的包前缀照编号最长 64，和写了的 32 不一致，要不要统一另定。

### 验收

1. `cargo xtask prompts` 跑两遍，第二遍没有差异；`prompts.md` 只差 `hazard.txt` 那一行。
2. `cargo test -p miyu-config --lib package::`（作者文档的表）、`cargo test -p xtask` 全过。
3. `grep -rnE '工作区那一级|工作区一级|工作区这一级|工作区、只读|三个级别|"level":"workspace"' docs/blueprint` 只剩 `perf.md` 第 5 条和写「随它去掉了」的地方。
