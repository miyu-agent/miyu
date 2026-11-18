## 施工单 HP-1：每个子命令的 `-h` 都印自己写的帮助页

状态：做完，等合（分支 `step/HP-1-帮助页挂全`）。来源：2026-11-18 外部审计报告 #037，主会话交来，先证实再修。

### 目的

`miyu <命令> <子命令> -h` 印的都是自己写的那一页，跟着界面语言；没有哪个子命令印 clap 照源码注释生成的帮助（中文、带施工号，英文界面下也是）。

### 证实

当前 main（`123727a35`）编的 `miyu`，沙箱数据根，把源码里全部 87 个子命令在中文、英文界面下各敲一遍 `-h`：印源码注释的有 12 个，比报告说的 9 个多 3 个。

- `pkg pack`、`pkg page`、`pkg update`、`pkg clean`、`pkg upgrade`、`pkg search`、`pkg sync`（`-S` 那一组的本名）
- `memory dream`、`memory cards`
- `mcp logout`
- `core`、`core stop`

英文界面下 `miyu pkg pack -h` 的第一行是「把一个包文件夹打成 `.miyu`（施工 G-1）：不连核心。」。前十个的帮助页上早有它们那几行（`pkg`、`memory`、`mcp` 三页），只是主程序挂帮助页的名单没跟着加。`core` 没有页：蓝图原来写的是「`miyu core --help` 照样印得出，是 clap 照代码注释生成的」，可 `miyu core stop` 是手册里教人用的命令。

另一处：`miyu mcp logout`（少了服务名）只说到「miyu mcp logout」为止，没照帮助页说怎么写，因为 `page_named` 里没有 `mcp`。

### 修法

- `crates/miyu/src/main.rs`：挂帮助页那一段改成一张表 `PAGES`（顶层子命令 → 哪一页）和两个函数：`dressed` 照表挂，`everywhere` 把一页挂到这个命令和它下面每一层的子命令上。不再有子命令的名单，后加的子命令自动跟着它的命令印同一页。
- `crates/miyu-cli/src/help.rs`：加 `Page::Core` 和中英两页 `help/{zh,en}/core.txt`；`page_named` 加 `mcp`。

### 蓝图改哪几节

- `cli/main.md`「在哪」帮助页那一行的文件名单、「怎么走」第 1 条、「给人看的字」的「帮助页」那一段。

### 测试

- `miyu`（主程序）的 `tests::every_subcommand_prints_the_page_of_its_command`：两种语言下把命令树走一遍，每个子命令照真的那样解析 `miyu … -h`，印出来的不是它那个命令的帮助页就红，列出是哪几个；顶层子命令不在 `PAGES` 里的也红。改前红，列出的正是上面 12 个；改后绿。
- `miyu-cli` 的 `help::tests`：`each_page_is_its_own_file`、`pages_fit_in_eighty_columns_and_end_with_one_newline` 加上 `mcp`、`core` 两页；`misuse::tests`：`miyu mcp logout` 少了参数时说「用法：miyu mcp logout <服务>」，`mcp` 进「嵌着的子命令有必写的就得在帮助页上」那一条。

### 实测

改后编的 `miyu`，沙箱数据根，Linux：

- 全部子命令（83 个，含 `pkg sync`、`pkg query` 这两个不列出来的）在中文、英文界面下各敲 `-h`、`--help`，共 332 次：退出码都是 0，第一行都是「用法：」或「Usage: 」，英文界面里没有一个中文字，没有施工号。
- `miyu core -h`、`miyu core stop --help`、`miyu help core`：印 `core` 那一页，中文「用法：miyu core stop」，英文「Usage: miyu core stop」。
- `miyu pkg pack -h`：印 `pkg` 那一页。
- `miyu mcp logout`：「miyu: 缺少参数」「提示：用法：miyu mcp logout <服务>」，退出码 2。

没实测：Windows、macOS（这一步的代码不分平台，看三台 CI）。

### 不做什么

- `miyu pkg pack` 的报错在中文界面下是英文（`package.licence is not a known key`）：另记，不在这一步。
- 帮助页的字不改，只加 `core` 一页。
