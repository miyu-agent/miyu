# 贡献代码

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

这一页写给想给核心或某个头（头是连接核心的界面程序，例如终端界面、网页界面）提交改动的开发者，讲先读什么，怎么编译、运行、通过门禁，以及提交和 PR 怎么写。各个部件在哪里见 [架构](architecture.md)。

## 先读什么

核心仓库根目录的 `AGENTS.md` 是给在仓库里工作的 AI 和人定的规矩，下面只挑出外来贡献者需要知道的部分。仓库里的文档分在几个地方：

| 目录 | 写什么 | 什么时候看 |
|---|---|---|
| `internal/blueprint/` | 蓝图：Miyu 现在是什么样子，一个部件一页，包括格式、界面上的每一格、给模型看的文字，以及它们在哪些代码里、由哪些测试守着 | 改一个部件之前，先读它那一页 |
| `internal/designs/` | 设计：定了什么、为什么这样定、没有选什么，按编号读，从 `00-设计理念.md` 开始 | 想知道某处为什么这样做，或者想改一个已经定下的做法时 |
| `internal/construction/` | 施工单：一次改动怎么做、改蓝图的哪几节、怎么验收；`README.md` 是总表和施工单的格式 | 想知道某个功能是哪一步加进来的、当时是怎么验收的时 |
| `docs/zh/reference/`、`docs/en/reference/`、`docs/zh/dev/`、`docs/en/dev/` | 协议参考、开发者文档（包括写软件包、扩展的教程），有中文、英文两份，中文是正本 | 改了协议或者软件包的写法时 |

开发按照「蓝图 → 施工单 → 代码」的顺序进行：设计要变时，先改蓝图的那一页，再改代码；合并时代码和蓝图要对得上。

## 准备环境

编译核心之前，需要准备好下面的环境：

- Rust 工具链钉在 `rust-toolchain.toml` 里：`1.97.0`，带 `rustfmt`、`clippy`。在仓库目录里运行 `rustup toolchain install` 即可安装。
- 核心的三平台 CI 除了工具链什么都不装，所以在 Linux、macOS、Windows 上编译核心都不需要另外安装系统库。Windows 上发布用的是 MSVC 工具链（`x86_64-pc-windows-msvc`）。
- 可选：在 Linux 上检查只有 Windows 才会报的 clippy 问题（只在 Windows 上没用到的导入、用 `cfg` 分出来的写法）。先安装 mingw-w64，再运行：

  ```sh
  rustup target add x86_64-pc-windows-gnu
  cargo clippy --target x86_64-pc-windows-gnu --workspace --all-targets -- -D warnings
  ```

其他仓库需要的东西另外安装：托盘在 Linux 上需要 `libgtk-3-dev`，在 macOS、Windows 上托盘仓库的 CI 不另外安装系统库；网页的页面测试需要 Node。

## 编译和运行

下面四步从编译核心开始，到用开发数据根运行、停止核心为止：

1. 在核心仓库根目录编译：

   ```sh
   cargo build
   ```

   编译完成后，`target/debug/` 里有主程序 `miyu`、沙盒助手 `miyu-sandbox`、`miyu-extract` 和看门的小程序 `miyu-watchdog`（Windows 上都带 `.exe`；Windows 上的 `miyu-watchdog` 什么都不做，也不打进发行包）。如果只编译 `-p miyu`，沙盒助手会停留在旧版，只读模式下的命令会报错，说助手和核心不是同一份。

2. 创建一个开发用的数据根，不碰你自己的 `~/.miyu`：

   ```bash
   MIYU_DEV_BASE_URL=https://api.example.com/v1 MIYU_DEV_MODEL=example-chat cargo xtask dev-home $HOME/miyu-dev
   ```

   ```powershell
   $env:MIYU_DEV_BASE_URL = 'https://api.example.com/v1'
   $env:MIYU_DEV_MODEL = 'example-chat'
   cargo xtask dev-home "$HOME\miyu-dev"
   ```

   在 PowerShell 里用 `$env:` 设置的变量只留在这个窗口里，所以后面几步要在同一个窗口里接着做。

   这条命令会在 `$HOME/miyu-dev` 建好骨架，并写入 `etc/config.toml`：一家供应商 `dev`（驱动是 `openai-chat`），默认模型是 `dev/example-chat`。地址从环境变量 `MIYU_DEV_BASE_URL` 读取，密钥从 `MIYU_DEV_API_KEY` 读取，两样都不写进文件。可以用 `MIYU_DEV_WINDOW` 另外指定上下文窗口。如果目录里已经有配置，不会覆盖它。

3. 运行：

   ```bash
   MIYU_HOME=$HOME/miyu-dev MIYU_RESOURCES=$PWD/resources \
     MIYU_DEV_BASE_URL=https://api.example.com/v1 MIYU_DEV_API_KEY=sk-example \
     target/debug/miyu ask "hello"
   ```

   ```powershell
   $env:MIYU_HOME = "$HOME\miyu-dev"
   $env:MIYU_RESOURCES = "$PWD\resources"
   $env:MIYU_DEV_API_KEY = 'sk-example'
   .\target\debug\miyu.exe ask hello
   ```

   在设置了 `MIYU_HOME` 的窗口里，之后运行的 `miyu` 都会使用开发数据根。

   `MIYU_RESOURCES` 指向源码树里的 `resources/`，因为 `target/debug/` 旁边没有资源目录。地址和密钥只有出现在启动核心的那个进程的环境里才有效，核心启动以后再修改变量不会生效。

4. 停掉这个数据根的核心：

   ```bash
   MIYU_HOME=$HOME/miyu-dev target/debug/miyu core stop
   ```

   ```powershell
   .\target\debug\miyu.exe core stop
   ```

### 让终端界面、网页界面连接开发中的核心

终端界面：在 `miyu-tui` 仓库根目录，先把终端界面的软件包安装进开发数据根，再运行：

```bash
MIYU_HOME=$HOME/miyu-dev MIYU_RESOURCES=<核心检出>/resources \
  MIYU_DEV_BASE_URL=https://api.example.com/v1 MIYU_DEV_API_KEY=sk-example \
  <核心检出>/target/debug/miyu pkg install ./tui --yes
MIYU_HOME=$HOME/miyu-dev MIYU_RESOURCES=<核心检出>/resources MIYU_CORE_BIN=<核心检出>/target/debug/miyu cargo run
```

```powershell
$env:MIYU_HOME = "$HOME\miyu-dev"
$env:MIYU_RESOURCES = '<核心检出>\resources'
$env:MIYU_DEV_BASE_URL = 'https://api.example.com/v1'
$env:MIYU_DEV_API_KEY = 'sk-example'
<核心检出>\target\debug\miyu.exe pkg install ./tui --yes
$env:MIYU_CORE_BIN = '<核心检出>\target\debug\miyu.exe'
cargo run
```

`miyu pkg install` 会启动核心并让它留着运行，所以地址和密钥的变量要写在这一条命令上。终端界面启动核心时只认 `MIYU_CORE_BIN`，不会去 `PATH` 里找 `miyu`；如果核心已经在运行，不设置这个变量也能连上。

网页界面：在 `miyu-web` 仓库根目录编译演示用的桥，它负责提供页面文件，并把浏览器的 `/ws` 转给核心：

```bash
cargo build --manifest-path web-demo/bridge/Cargo.toml
MIYU_HOME=$HOME/miyu-dev MIYU_RESOURCES=<核心检出>/resources MIYU_CORE_BIN=<核心检出>/target/debug/miyu \
  MIYU_DEV_BASE_URL=https://api.example.com/v1 MIYU_DEV_API_KEY=sk-example \
  web-demo/bridge/target/debug/miyu-web-bridge 8766
```

```powershell
cargo build --manifest-path web-demo/bridge/Cargo.toml
$env:MIYU_HOME = "$HOME\miyu-dev"
$env:MIYU_RESOURCES = '<核心检出>\resources'
$env:MIYU_CORE_BIN = '<核心检出>\target\debug\miyu.exe'
$env:MIYU_DEV_BASE_URL = 'https://api.example.com/v1'
$env:MIYU_DEV_API_KEY = 'sk-example'
.\web-demo\bridge\target\debug\miyu-web-bridge.exe 8766
```

运行后打开它输出的链接（`http://127.0.0.1:8766/#k=…`）即可。命令最后的那个数字是端口，不写时是 `8765`。演示用的桥在网页仓库的 CI 里只在 Linux 上编译和测试。

### 让头使用核心里修改过的代码

头通过 Cargo git 依赖钉住核心的一个提交。如果要让头使用你本机检出里修改过的核心来编译，就在头的 `Cargo.toml` 里临时加一段 `[patch]`，头的 `Cargo.toml` 里直接写到的每个核心 crate 各占一行：

```toml
[patch."https://github.com/miyu-agent/miyu"]
miyu-client = { path = "../miyu/crates/miyu-client" }
miyu-ipc = { path = "../miyu/crates/miyu-ipc" }
```

不要提交这一段。核心那边的改动要先合进核心的 main，然后再把头的 `rev` 改成那个提交。

## 门禁

门禁是提交之前必须全部通过的一组检查。运行：

```sh
cargo xtask check
```

本机验收和三平台 CI 运行的都是这同一条命令。它会逐项运行，最后输出一张结果表；只要有一项没有通过，退出码就不是零。各项检查的内容如下：

| 项 | 查什么 | 没过怎么办 |
|---|---|---|
| 格式 | `cargo fmt --all --check` | 运行 `cargo fmt --all` |
| clippy | `cargo clippy --workspace --all-targets -- -D warnings`；工作区禁止 `unwrap`（测试里可以用），禁止用 `let _ =` 和 `.ok()` 吞掉错误，禁止 `unsafe` | 按照报的问题修改；如果确实要丢掉一个结果，就写 `#[expect(..., reason = "...")]` |
| clippy（核心不开可选包） | `cargo clippy -p miyu-core --no-default-features -- -D warnings`：关掉 `mermaid`、`net` 也要能编译通过；CI 上只在 Linux 上运行这一项，本机照样要运行 | 把只在可选包里用到的代码放到对应的 `cfg(feature)` 后面 |
| 文档 | `cargo doc --workspace --no-deps --document-private-items`，警告当作错误；另外还检查下面七项 | 修好断掉的文档链接和写坏的写法 |
| 文档：登记簿 | `resources/` 里给模型看的每一份文件都登记在 `internal/designs/26-提示词.md` 的登记簿里，并且指纹（文件字节 SHA-256 的前 8 位）对得上 | 新加或修改的文件，在登记簿里加一行或者改那一行：文件、进到哪里、什么时候加进来、token、为什么加、指纹 |
| 文档：提示词页 | `internal/blueprint/prompts.md` 和登记簿、资源一致 | 运行 `cargo xtask prompts` 重新生成 |
| 文档：样本块 | 蓝图里标着 `样本` 的代码块和它指向的文件一字不差 | 两边一起修改 |
| 文档：`heads.toml` | 正好有 `tui`、`webui` 两张表，每张表只有 `repo`、`rev`，仓库在 `miyu-agent` 下，`rev` 是完整的提交号或者以 `v` 开头的标签 | 按照这个要求写 |
| 文档：协议参考 | 代码里分发的每个方法，在中文、英文两份参考里都有条目；参考里作为标题写出的方法在代码里都存在；`docs/zh/reference/`、`docs/en/reference/` 里的 `#锚点` 都能找到对应的标题 | 补上参考的条目，两份一起补 |
| 文档：扩展能调什么 | 代码里分发的每个方法，都在 `crates/miyu-endpoint/src/connection/rights.rs` 的表里写明扩展能不能调用 | 在表里加一行 |
| 文档：冻结的方法表 | `internal/blueprint/methods-v1.txt` 和代码对得上：快照里有、代码里没有了的方法（删掉了或改了名）会报错，代码里新加的方法也会报错 | 新方法运行 `cargo xtask methods` 登记进去；已有的方法不能删除，也不能改名 |
| 分层 | 每个 crate 都登记在 `internal/designs/01-架构.md`「代码的分层」那张表里，并且只依赖同一层或更低的层；纯逻辑两层的外部依赖只能从白名单里选 | 把新 crate 登记进表里；把依赖挪到正确的层 |
| 纯逻辑 | 内核、模块两层的 `src/` 里没有文件、网络、子进程、环境变量、线程、时钟、标准输入输出，也不使用 `HashMap`、`HashSet` | 把 I/O 挪到执行器那一层；改用 `BTreeMap`、`BTreeSet` |
| 行数 | 每个 `.rs` 文件不超过 500 行，测试文件也算在内 | 拆分文件 |
| 日志 | 运行日志不写对话正文、工具的参数和结果、路径：字段名不许是路径、正文一类，值里不许出现 `.display()`、`format!(`，消息里不许插值 | 只写编号、长度、类别；单独运行 `cargo xtask logs` 更快 |
| 许可证 | 发布的四个平台上用到的第三方依赖，都能和 AGPL-3.0-or-later 一起发布；`THIRD-PARTY-LICENSES.txt` 是最新的 | 更换依赖；加了或升级了依赖之后运行 `cargo xtask notices` |
| 测试 | `cargo test --workspace --no-fail-fast`，设置 `CI=1`，临时目录指向一个新的空目录；每个测试文件都已经加进测试程序 | 修好测试；新的测试文件要在 `tests/all.rs` 里加一行 `mod <名>;` |
| 临时目录 | 测试运行完以后临时目录里不留下任何东西，只在 Linux 上检查 | 让测试的夹具自己清理干净 |

`cargo xtask` 还有其他子命令：

| 命令 | 做什么 |
|---|---|
| `cargo xtask dev-home <目录>` | 创建一个带配置的开发数据根（见上面「编译和运行」的第 2 步） |
| `cargo xtask logs` | 只运行门禁的「日志」那一项 |
| `cargo xtask methods` | 把代码里新加的协议方法登记进 `internal/blueprint/methods-v1.txt`，只增加不删除 |
| `cargo xtask prompts` | 根据登记簿和资源重新生成 `internal/blueprint/prompts.md` |
| `cargo xtask notices` | 重新生成 `THIRD-PARTY-LICENSES.txt` |
| `cargo xtask perf [量尺的参数]` | 编译 release 版的 `miyu`、`miyu-sandbox` 和量尺 `miyu-perf`，测量启动、内存、长会话上的请求、二进制大小，结果写进 `target/perf-out/`；后面的参数原样交给量尺，例如 `--runs 3`。内存只能在 Linux 上测量，macOS、Windows 上表里那几格写的是 `不量` |
| `cargo xtask dist [--target <系统>] [--out <目录>]` | 打出本机这一种系统的命令行版压缩包：按照 `heads.toml` 取终端、网页，编译 release 版的 `miyu`、`miyu-sandbox`、`miyu-extract`、`miyu-watchdog`（只在 Linux、macOS 上）、`miyu-tui`、`miyu-web`，打包成 `miyu-<版本>-<系统>.tar.gz`（Windows 上是 `.zip`），默认放在 target 目录的 `dist/` 下；`--target` 只能写本机的系统 |
| `cargo xtask dist-check <压缩包>` | 发布前检查压缩包：名字、程序、许可证、资源逐字节比较，并运行包里的 `miyu --version` 和 `miyu check`；Windows 的压缩包还会读每个 `.exe` 的导入表，如果程序依赖 VC++ 运行库（`VCRUNTIME*.dll`、`MSVCP*.dll`），检查不通过 |
| `cargo xtask packaging <标签> --sums <SHA256SUMS> [--applet <托盘的标签> --applet-sums <托盘的 SHA256SUMS>] [--out <目录>]` | 根据一次发布的 `SHA256SUMS` 填写 AUR 的 `PKGBUILD` 和 Homebrew 的 formula |
| `cargo xtask version <年.月.序号>` | 修改工作区的版本号（例如 `2026.12.0`），`Cargo.lock` 跟着修改 |

## 测试

写测试时要遵守下面这些规矩：

- 修 bug 时先写能复现它的测试，修之前这个测试必须是红的。新功能先写证明它正确的测试。
- 测试一律把 `MIYU_HOME` 指向临时目录，不碰真实的数据。
- 每个 crate 的集成测试合并成一个程序 `tests/all.rs`，一个文件一个模块，共用的夹具写在 `crate::support::` 里。安装日志订阅者的测试、测量整个进程内存的测试，另外列成单独的 `[[test]]`。
- 量尺类的测试标上 `#[ignore]`，断言结果，不断言耗时。
- 往坏的方向测：边界情况、奇怪的组合、故意把它弄坏。

核心的 CI（`.github/workflows/ci.yml`）在 `ubuntu-24.04`、`macos-latest`、`windows-latest` 上各运行一遍 `cargo xtask check`；另外在 Linux 上运行长跑（名字以 `for_longer` 结尾的 `#[ignore]` 随机测试）和量尺，量尺超出预算也算失败。推送到 `main` 和以 `step/` 开头的分支时会运行 CI，向 `main` 提交的 PR 也会运行，从分叉提交的 PR 一样会运行；如果只改了 `internal/construction/`、`docs/zh/dev/`、`internal/designs/`（`01-架构.md`、`23-性能预算.md`、`26-提示词.md` 除外），就不运行。涉及路径、文件系统、子进程、终端的改动，要等三个平台都通过以后再合并。

## 代码的规矩

写代码时要遵守下面这些规矩：

- 注释用中文，名字用英文。
- 公开的类型、函数、字段、枚举的每一种都要写 `///`，模块开头写 `//!`：说明它管什么、为什么这样做、容易踩的坑。会出错的写 `# Errors` 一节，会 panic 的写 `# Panics` 一节。公开项的文档注释不要链接到私有项。
- 给模型看的文字一律用英文短句，放在 `resources/` 里，不写进代码。只有非加不可的才加，加了就要登记进登记簿。
- 数值、文字、名单是数据，不写死在代码里。
- 路径、换行、编码都不写死；平台的差别关在边界上，代码问「这里能不能做」，不问「这是不是 macOS」。
- 不吞掉错误：要么处理，要么如实上报。
- 加第三方依赖之前先看许可证，必须能和 AGPL-3.0-or-later 一起发布。
- 按照周围代码的写法来写；文件要小，一个模块只有一种职责；这一次用不到的类型、字段、配置项不写。

## 改动需要同步的地方

改了下面左边的内容，还要同步修改右边的地方：

| 改了 | 还要改 |
|---|---|
| 协议的方法、参数、回应、出错、推送 | `docs/zh/reference/`、`docs/en/reference/` 两份；新方法要运行 `cargo xtask methods`，并在 `rights.rs` 的表里写明扩展能不能调用 |
| 软件包、扩展、脚本、技能、MCP 的写法 | `docs/zh/dev/`、`docs/en/dev/` 两份（`manifest.md`、`mcp.md`、`scripts-skills.md` 有测试核对每一格都写了） |
| `resources/` 里给模型看的文字 | 在登记簿里加一行或者改那一行，然后运行 `cargo xtask prompts` |
| 依赖 | 运行 `cargo xtask notices` |
| 一个部件的行为、格式、界面 | 蓝图里它的那一页 |

门禁会检查协议参考的两份是否对得上，但不检查作者教程，作者教程的两份要你自己核对。

## 提交和 PR

提交和 PR 要按照下面的格式写：

- 一个改动一个提交，每个提交都要能编译、门禁全部通过；顺手做的无关改动单独作为一个提交。
- 标题写成 `<类型>(<范围>): <这次改动的效果，说人话>`，类型是 `feat`、`fix`、`perf`、`refactor`、`docs`、`test`、`chore` 之一，例如 `fix(终端): 窄窗口里输入框不再错位`。标题里不写实现细节。
- 正文分三段：为什么改、改了什么、怎么验证的。
- PR 开到 `main`。三个平台的 CI 必须全部通过。
- 贡献的代码使用所在仓库的许可证。

## 其他仓库

终端、网页、托盘、im 这些仓库各有自己的 `AGENTS.md`、`README.md` 和门禁，规矩和核心的一样。它们通过 Cargo git 依赖钉住核心的一个提交（见上面的「让头使用核心里修改过的代码」）。各个仓库在本机要运行的检查和 CI 的系统如下：

| 仓库 | 本机跑 | CI 的系统 |
|---|---|---|
| `miyu-tui` | `cargo fmt --check`；`cargo clippy --all-targets --locked -- -D warnings`；`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items --locked`；`cargo test --locked --no-fail-fast`；`cargo xtask licenses` | Linux、macOS、Windows（Windows 上只运行 `--bins` 的单元测试；`cargo xtask licenses` 只在 Linux 上运行）；Linux 上另外运行量尺 `cargo test --release --locked --test meters -- --ignored --test-threads=1` |
| `miyu-web` | 和终端的前四条相同，再加 `cargo xtask licenses`；`MIYU_RESOURCES=<核心检出>/resources node --test 'web-demo/tests/**/*.test.js'`；`cargo clippy --manifest-path web-demo/bridge/Cargo.toml --all-targets --locked -- -D warnings`；`cargo test --manifest-path web-demo/bridge/Cargo.toml --locked` | Linux x86_64、Linux aarch64、macOS、Windows；`cargo xtask licenses`、页面测试、演示用的桥只在 Linux 上运行 |
| `miyu-desktop`（托盘） | `cargo fmt --check`；`cargo clippy --workspace --all-targets --locked -- -D warnings`；`cargo test --workspace --locked --no-fail-fast`；`cargo xtask licenses`；`cargo build --release --locked` | Linux、macOS、Windows；`cargo xtask licenses` 只在 Linux 上运行 |
| `miyu-im` | `cargo xtask check` | Linux、macOS、Windows |
| `miyu-migrate` | `cargo fmt --all --check`；`cargo clippy --workspace --all-targets --locked -- -D warnings`；`cargo test --workspace --locked` | Linux、macOS、Windows |
| `miyu-embed` | `cargo fmt --check`；`cargo clippy --all-targets --locked -- -D warnings`；`cargo test --locked` | Linux、macOS、Windows |
| `miyu-render`、`miyu-memes` | `cargo fmt --check`；`cargo clippy --all-targets --locked -- -D warnings`；`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items --locked`；`cargo test --locked --no-fail-fast` | Linux、macOS、Windows |
| `miyu-ocr` | 和上一行的四条相同，都带 `--workspace`；再加 `cargo xtask licenses` | Linux、macOS、Windows；`cargo xtask licenses` 只在 Linux 上运行 |
| `miyu-sdk` | `python -m unittest discover -s python/tests`；在 `rust/` 里运行 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` | Linux、macOS、Windows；Python 另外在 Linux 上用 3.9 运行一遍 |

表里的 `名字=值 命令` 是 bash 的写法；在 PowerShell 里要先运行 `$env:名字 = '值'`，再运行命令。

这些仓库的 CI 在 PR 上也会运行。除了 `miyu-migrate`、`miyu-sdk`，其他仓库都限制每个 `.rs` 文件不超过 500 行（`miyu-im` 在它的 `cargo xtask check` 里检查）。`miyu-web` 的页面测试会读取核心的出厂资源，所以 `<核心检出>` 必须是它的 `Cargo.toml` 里钉住的那个提交。
