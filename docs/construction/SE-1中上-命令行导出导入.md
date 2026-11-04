# SE-1 中上：命令行导出导入

设计 `58-会话导出和导入.md` 第六节第 2 条。`miyu session export [<会话>] [-o <文件>]`、`miyu session import <文件> [--archive]`，照 `session.export`、`session.import` 走；Markdown 随 SE-1 中下。

## 我定的（技术题，照推荐）

1. **不写会话照上一次 `miyu ask` 开的**：同 `miyu rename`、`recap`。调研里「在终端里列出来挑」等终端界面的 `/export` 做。
2. **默认文件名** `miyu-session-<编号前 8 位>.zip`，写在当前目录；核心只认绝对路径，命令行把相对的照当前目录接上。
3. **标准输出只印要接着用的**：导出印写到哪（绝对路径），导入印新导进来的编号；重导什么都不印、退出码 0。
4. **总表里写「导入导出会话」**：中文帮助的总表不列「、」串（`text_rules`）。

## 测试

`crates/miyu-cli/tests/session.rs`：两个核心，一个 `miyu ask` 说一句、`export` 不写会话写相对路径，包写到当前目录下、印绝对路径；另一个 `import --archive` 印出编号，再导一次什么都不印；写到已经有的文件照核心的话说、退出码 1。帮助页照 `help/tests.rs`、`text_rules` 的规矩查。
