## 施工单 VS-1：包加的命令认 `--version`，转交带上数据根

状态：做完，等验收（分支 `step/OA-1`）。项目主人 2026-11-17 本机验收第 11 条；转交不带 `MIYU_HOME` 是语音会话同日查出、主人本机实锤，主会话派进这一步。

### 目的

1. `miyu im --version` 报「未知命令」，`miyu tui --version` 印一大段 `Error: Custom { … }`：包的程序认不认 `--version`、印成什么样各家不一样。只敲 `miyu <包的命令> --version`（`-V`）的，主程序自己印 `miyu <命令> <包的版本>`，不交给包的程序。版本照清单 `[package] version`，同 `miyu pkg list`；没写版本的只印 `miyu <命令>`。后面还跟着别的参数的（`miyu tui config --version`）照旧交过去。
2. `miyu <包的命令>` 转交时只交了包的几处和 `MIYU_RESOURCES`、`MIYU_PROGRAM`，没交 `MIYU_HOME`；包的命令照 `MIYU_HOME` 连核心，人没设它的机器上（数据根照默认的 `~/.miyu`）`miyu say --voices`、`miyu deepseek-tools` 报「MIYU_HOME is not set」，测试里都手设了所以没发现。转交交的和核心拉起扩展交的改成同一份：`miyu_store::program::PackageDirs::launch_env`（`MIYU_HOME` 是找到它的数据根、`MIYU_RESOURCES`、`MIYU_PROGRAM`、它的几处）。

核心自己的子命令（`miyu ask --version`、`miyu pkg list -V`）原来报「unrecognized option」、退出码 2，照「照现有 `miyu --version`」改成印同一行 `miyu <版本>`（clap 的 `propagate_version`，它原样会印成 `miyu-ask <版本>`，主程序换成主程序那一行）。

### 蓝图改哪几节

- `cli/main.md`「怎么走」第 0 条第 1 款；两份帮助页的「选项」一行。
- `packages.md`「转交」第 5 条。
- 写包的 `manifest.md`（中英）`[command]`：只敲 `--version` 的不交给你。`extensions.md` 本来就写着「人在终端敲时也设这一套」，现在才对上，不改。
- 帮助页只有中、英两种（命令行没有日文），派工说的三语照这两种写。

### 验收

1. 改前红：`crates/miyu/tests/packages.rs` 的 `a_package_command_answers_its_version_itself`（`miyu own --version`、`-V` 印 `miyu own 1.2.3`，没写版本的印 `miyu bare`；程序没装也照答）、`a_package_command_is_told_the_data_root_without_miyu_home`（不设 `MIYU_HOME`、`HOME` 指场地，包的程序拿到的 `MIYU_HOME` 是 `~/.miyu`），在 `origin/main` 上两条红。`miyu-cli` 的 `packages/tests.rs`：`the_program_gets_its_package_folder` 多查 `MIYU_HOME`，`only_a_bare_version_flag_is_answered_here`（新）；`crates/miyu/tests/commands.rs` 多查 `miyu ask --version`、`miyu pkg list -V` 印的和 `miyu --version` 一样（改前退出码 2）。
2. 门禁十一项、长跑随机、Windows 目标的 clippy。

### 不做什么

- 不改包的程序（终端、im）自己怎么认 `--version`。
