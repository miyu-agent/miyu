# 贡献代码

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

给想给核心或某个头提一个改动的开发者：先读什么，怎么编、怎么跑、怎么过门禁，提交和 PR 怎么写。各部件在哪见 [架构](architecture.md)。

## 先读什么

核心仓库根目录的 `AGENTS.md` 是给在仓库里干活的 AI 和人的规矩，下面只拎外来贡献者要知道的。仓库里的文档分几处：

| 目录 | 写什么 | 什么时候看 |
|---|---|---|
| `docs/blueprint/` | 蓝图：Miyu 现在长什么样，一个部件一页，连格式、界面上的每一格、给模型看的字、在哪些代码里、由哪些测试守着 | 改一个部件之前，先读它那一页 |
| `docs/designs/` | 设计：定了什么、为什么、没选什么，按编号读，从 `00-设计理念.md` 开始 | 想知道一处为什么这样做、想改一个已定的做法 |
| `docs/construction/` | 施工单：一次改动怎么做、改蓝图哪几节、怎么验收；`README.md` 是总表和施工单的格式 | 想看某个功能是哪一步加的、当时怎么验的 |
| `docs/perf/` | 量尺跑出来的原始数据和对照预算的表 | 改动可能影响启动、内存、请求的快慢 |
| `docs/reviews/` | 审阅、调研、走查的记录 | 查一个问题以前有没有人看过 |
| `docs/reference/`、`docs/authors/` | 协议参考、作者教程，中英两份，中文是正本 | 改了协议或包的写法 |

开发照「蓝图 → 施工单 → 代码」走：设计要变，先改蓝图那一页，再改代码；合进去时代码和蓝图要对得上。

## 环境

- Rust 工具链钉在 `rust-toolchain.toml`：`1.97.0`，带 `rustfmt`、`clippy`。在仓库目录里跑 `rustup toolchain install` 装上。
- 核心的三平台 CI 除了工具链什么都不装，Linux、macOS、Windows 上编核心不用另装系统库。Windows 上发布用的是 MSVC 工具链（`x86_64-pc-windows-msvc`）。
- 可选：在 Linux 上查 Windows 才报的 clippy（只在 Windows 上没用到的导入、`cfg` 分出来的写法）。装 mingw-w64，再：

  ```sh
  rustup target add x86_64-pc-windows-gnu
  cargo clippy --target x86_64-pc-windows-gnu --workspace --all-targets -- -D warnings
  ```

别的仓库要的另外装：托盘在 Linux 上要 `libgtk-3-dev`，macOS、Windows 上托盘仓库的 CI 不另装系统库；网页的页面测试要 Node。

## 编和跑起来

1. 在核心仓库根目录编：

   ```sh
   cargo build
   ```

   `target/debug/` 里有主程序 `miyu`、沙盒助手 `miyu-sandbox`、`miyu-extract`（Windows 上都带 `.exe`）。只编 `-p miyu` 的，沙盒助手停在旧版，只读下的命令会报助手和核心不是同一份。

2. 造一个开发用的数据根，不碰自己的 `~/.miyu`：

   ```bash
   MIYU_DEV_BASE_URL=https://api.example.com/v1 MIYU_DEV_MODEL=example-chat cargo xtask dev-home $HOME/miyu-dev
   ```

   ```powershell
   $env:MIYU_DEV_BASE_URL = 'https://api.example.com/v1'
   $env:MIYU_DEV_MODEL = 'example-chat'
   cargo xtask dev-home "$HOME\miyu-dev"
   ```

   PowerShell 里 `$env:` 设的变量留在这个窗口里，后面几步在同一个窗口里接着做。

   它在 `$HOME/miyu-dev` 建好骨架，写 `etc/config.toml`：一家供应商 `dev`（驱动 `openai-chat`），默认模型 `dev/example-chat`。地址照环境变量 `MIYU_DEV_BASE_URL` 取，key 照 `MIYU_DEV_API_KEY` 取，两样都不写进文件。`MIYU_DEV_WINDOW` 可以另给上下文窗口。已经有配置的目录不盖。

3. 跑：

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

   设了 `MIYU_HOME` 的窗口里，之后跑的 `miyu` 都用开发数据根。

   `MIYU_RESOURCES` 指到源码树的 `resources/`：`target/debug/` 旁边没有资源目录。地址和 key 只有拉起核心的那个进程的环境里有才算，核心起来以后再改变量不生效。

4. 停掉这一份的核心：

   ```bash
   MIYU_HOME=$HOME/miyu-dev target/debug/miyu core stop
   ```

   ```powershell
   .\target\debug\miyu.exe core stop
   ```

### 让终端、网页连开发中的核心

终端：在 `miyu-tui` 仓库根目录，先把终端的软件包装进开发数据根，再跑：

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

`miyu pkg install` 拉起核心并让它留着跑，所以地址和 key 的变量写在这一条上。终端拉起核心只认 `MIYU_CORE_BIN`，不去 `PATH` 里找 `miyu`；核心已经在跑的不设也连得上。

网页：在 `miyu-web` 仓库根目录编演示的桥，它给页面文件、把浏览器的 `/ws` 转给核心：

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

打开它印出来的链接（`http://127.0.0.1:8766/#k=…`）。最后那个数是端口，不写是 `8765`。演示的桥在网页仓库的 CI 里只在 Linux 上编、测。

### 头要用核心里改了的代码

头经 Cargo git 依赖钉住核心的一个提交。要让头编进你本机检出里改过的核心，在头的 `Cargo.toml` 里临时加一段 `[patch]`，头的 `Cargo.toml` 里直接写了的核心 crate 每个一行：

```toml
[patch."https://github.com/miyu-agent/miyu"]
miyu-client = { path = "../miyu/crates/miyu-client" }
miyu-ipc = { path = "../miyu/crates/miyu-ipc" }
```

这一段不提交。核心那边的改动先合进核心的 main，头再把 `rev` 改成那个提交。

## 门禁

```sh
cargo xtask check
```

本机验收和三平台 CI 跑的是同一条。它一项一项跑，最后打一张结果表，有一项没过就退出码非零：

| 项 | 查什么 | 没过怎么办 |
|---|---|---|
| 格式 | `cargo fmt --all --check` | `cargo fmt --all` |
| clippy | `cargo clippy --workspace --all-targets -- -D warnings`；工作区禁 `unwrap`（测试里可以）、禁 `let _ =` 和 `.ok()` 吞错误、禁 `unsafe` | 照报的改；真要扔掉一个结果，写 `#[expect(..., reason = "...")]` |
| clippy（核心不开可选包） | `cargo clippy -p miyu-core --no-default-features -- -D warnings`：关掉 `mermaid`、`net` 也编得过；CI 上只在 Linux 跑这一项，本机照跑 | 只在可选包里用到的代码放到对应的 `cfg(feature)` 后面 |
| 文档 | `cargo doc --workspace --no-deps --document-private-items`，警告当错；另查下面七样 | 修断了的文档链接、写坏了的写法 |
| 文档：登记簿 | `resources/` 里给模型看的每份文件都登记在 `docs/designs/26-提示词.md` 的登记簿里，指纹（文件字节 SHA-256 的前 8 位）对得上 | 新加、改了的文件在登记簿里加一行或改那一行：文件、进到哪、什么时候加进来、token、为什么加、指纹 |
| 文档：提示词页 | `docs/blueprint/prompts.md` 和登记簿、资源一致 | `cargo xtask prompts` 重新生成 |
| 文档：样本块 | 蓝图里标着「样本」的代码块和它指到的文件一字不差 | 两边一起改 |
| 文档：`heads.toml` | 正好 `tui`、`webui` 两张表，各只有 `repo`、`rev`，仓库在 `miyu-agent` 下，`rev` 是完整提交号或 `v` 开头的标签 | 照这个写 |
| 文档：协议参考 | 代码里分发的每个方法，中文、英文两份参考里都有条目；参考里当标题写的方法代码里都有；`docs/reference/` 里的 `#锚点` 都找得到标题 | 补参考的条目，两份一起 |
| 文档：扩展能调什么 | 代码里分发的每个方法都在 `crates/miyu-endpoint/src/connection/rights.rs` 的表里写明扩展能不能调 | 在表里加一行 |
| 文档：冻结的方法表 | `docs/reference/methods-v1.txt` 和代码对得上：快照里有、代码里没了的报（删了、改了名），代码里新加的也报 | 新方法跑 `cargo xtask methods` 登记进去；已有的方法不能删、不能改名 |
| 分层 | 每个 crate 登记在 `docs/designs/01-架构.md`「代码的分层」那张表里，只依赖同层或更低层；纯逻辑两层的外部依赖只能从白名单里挑 | 新 crate 登记进表；依赖挪到对的层 |
| 纯逻辑 | 内核、模块两层的 `src/` 里没有文件、网络、子进程、环境变量、线程、时钟、标准输入输出，不用 `HashMap`、`HashSet` | I/O 挪到执行器那一层；改用 `BTreeMap`、`BTreeSet` |
| 行数 | 每个 `.rs` 不超过 500 行，测试也算 | 拆文件 |
| 日志 | 运行日志不写对话正文、工具的参数和结果、路径：格名不许是路径、正文一类，值里不许 `.display()`、`format!(`，消息不许插值 | 只写编号、长度、类别；单独跑 `cargo xtask logs` 更快 |
| 许可证 | 发布的四个平台上用得到的第三方依赖都能和 AGPL-3.0-or-later 合在一起发；`THIRD-PARTY-LICENSES.txt` 是最新的 | 换依赖；加了、升了依赖跑 `cargo xtask notices` |
| 测试 | `cargo test --workspace --no-fail-fast`，设 `CI=1`，临时目录指到一个新的空目录；每个测试文件都进了测试程序 | 修测试；新测试文件在 `tests/all.rs` 加一行 `mod <名>;` |
| 临时目录 | 测试跑完临时目录里不剩东西，只在 Linux 上拦 | 测试的夹具自己清干净 |

别的子命令：

| 命令 | 做什么 |
|---|---|
| `cargo xtask dev-home <目录>` | 造一个带配置的开发数据根（上面「编和跑起来」第 2 步） |
| `cargo xtask logs` | 只跑门禁的「日志」那一项 |
| `cargo xtask methods` | 把代码里新加的协议方法登记进 `docs/reference/methods-v1.txt`，只加不删 |
| `cargo xtask prompts` | 照登记簿和资源重新生成 `docs/blueprint/prompts.md` |
| `cargo xtask notices` | 重新生成 `THIRD-PARTY-LICENSES.txt` |
| `cargo xtask perf [量尺的参数]` | 编 release 的 `miyu`、`miyu-sandbox` 和量尺 `miyu-perf`，量启动、内存、长会话上的请求、二进制大小，结果写进 `docs/perf/`；后面的参数原样交给量尺，例如 `--runs 3`。内存只在 Linux 上量得到，macOS、Windows 上表里那几格写「不量」 |
| `cargo xtask dist [--target <系统>] [--out <目录>]` | 打本机那一种系统的命令行版压缩包：照 `heads.toml` 取终端、网页，编 release 的 `miyu`、`miyu-sandbox`、`miyu-extract`、`miyu-tui`、`miyu-web`，打成 `miyu-<版本>-<系统>.tar.gz`（Windows 是 `.zip`），默认放在 target 目录的 `dist/` 下；`--target` 只能写本机 |
| `cargo xtask dist-check <压缩包>` | 发布前查包：名字、程序、许可证、资源逐字节比，跑包里的 `miyu --version` 和 `miyu check` |
| `cargo xtask packaging <标签> --sums <SHA256SUMS> [--applet <托盘的标签> --applet-sums <托盘的 SHA256SUMS>] [--out <目录>]` | 照一次发布的 `SHA256SUMS` 填出 AUR 的 `PKGBUILD` 和 Homebrew 的 formula |
| `cargo xtask version <年.月.序号>` | 改工作区的版本号（例如 `2026.12.0`），`Cargo.lock` 跟着改 |

## 测试

- 修 bug 先写能复现它的测试，修之前它是红的。新功能先写证明它对的测试。
- 测试一律把 `MIYU_HOME` 指到临时目录，不碰真的数据。
- 每个 crate 的集成测试并成一个程序 `tests/all.rs`，一个文件一个模块，共用的夹具写 `crate::support::`。装日志订阅者的、量整个进程内存的另列 `[[test]]`。
- 量尺类的测试标 `#[ignore]`，断言结果，不断言耗时。
- 往坏里测：边界情况、奇怪的组合、故意弄坏它。

核心的 CI（`.github/workflows/ci.yml`）在 `ubuntu-24.04`、`macos-latest`、`windows-latest` 上各跑一遍 `cargo xtask check`；另在 Linux 上跑长跑（名字以 `for_longer` 结尾的 `#[ignore]` 随机测试）和量尺，量尺超过预算也算红。CI 在推到 `main` 和 `step/` 开头的分支时跑，往 `main` 提的 PR 也跑，从分叉提的一样；只改了 `docs/construction/`、`docs/authors/`、`docs/reviews/`、`docs/designs/`（`01-架构.md`、`23-性能预算.md`、`26-提示词.md` 除外）的不跑。碰路径、文件系统、子进程、终端的改动，三个平台都绿了再合。

## 代码的规矩

- 注释用中文，名字用英文。
- 公开的类型、函数、字段、枚举的每一种写 `///`，模块开头写 `//!`：管什么、为什么这样、容易踩的坑。会出错的写 `# Errors`，会 panic 的写 `# Panics`。公开项的文档注释别链到私有项。
- 给模型看的字一律英文短句，放在 `resources/` 里，不写进代码。非加不可的才加，加了进登记簿。
- 数值、文字、名单是数据，不写死在代码里。
- 路径、换行、编码都不写死；平台的差别关在边界上，代码问「这里能不能做」，不问「这是不是 macOS」。
- 不吞错误：要么处理，要么如实上报。
- 加第三方依赖之前先看许可证，要能和 AGPL-3.0-or-later 合在一起发。
- 照周围的写法；文件小，一个模块一种职责；这一次用不到的类型、字段、配置项不写。

## 改了这些还要改哪

| 改了 | 还要改 |
|---|---|
| 协议的方法、参数、回应、出错、推送 | `docs/reference/` 里中文、英文两份；新方法跑 `cargo xtask methods`，在 `rights.rs` 的表里写明扩展能不能调 |
| 软件包、扩展、脚本、技能、MCP 的写法 | `docs/authors/` 里中文、英文两份 |
| `resources/` 里给模型看的字 | 登记簿加一行或改那一行，跑 `cargo xtask prompts` |
| 依赖 | 跑 `cargo xtask notices` |
| 一个部件的行为、格式、界面 | 蓝图里它那一页 |

门禁查协议参考两份对不对得上，不查作者教程，作者教程两份要自己对。

## 提交和 PR

- 一个改动一个提交，每个提交都能编译、门禁全过；顺手做的无关改动单独一个提交。
- 标题写 `<类型>(<范围>): <这次改动的效果，说人话>`，类型是 `feat`、`fix`、`perf`、`refactor`、`docs`、`test`、`chore` 之一，例如 `fix(终端): 窄窗口里输入框不再错位`。标题不写实现细节。
- 正文三段：为什么、改了什么、怎么验。
- PR 开到 `main`。三个平台的 CI 要全绿。
- 贡献照所在仓库的许可证。

## 别的仓库

终端、网页、托盘、im 这些仓库各有自己的 `AGENTS.md`、`README.md` 和门禁，规矩照核心的。它们经 Cargo git 依赖钉住核心的一个提交（见上面「头要用核心里改了的代码」）。

| 仓库 | 本机跑 | CI 的系统 |
|---|---|---|
| `miyu-tui` | `cargo fmt --check`；`cargo clippy --all-targets --locked -- -D warnings`；`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items --locked`；`cargo test --locked --no-fail-fast`；`cargo xtask licenses` | Linux、macOS、Windows（Windows 只跑 `--bins` 的单元测试；`cargo xtask licenses` 只在 Linux 上跑）；Linux 另跑量尺 `cargo test --release --locked --test meters -- --ignored --test-threads=1` |
| `miyu-web` | 同终端的前四条加 `cargo xtask licenses`；`MIYU_RESOURCES=<核心检出>/resources node --test 'web-demo/tests/**/*.test.js'`；`cargo clippy --manifest-path web-demo/bridge/Cargo.toml --all-targets --locked -- -D warnings`；`cargo test --manifest-path web-demo/bridge/Cargo.toml --locked` | Linux x86_64、Linux aarch64、macOS、Windows；`cargo xtask licenses`、页面测试、演示的桥只在 Linux 上跑 |
| `miyu-desktop`（托盘） | `cargo fmt --check`；`cargo clippy --workspace --all-targets --locked -- -D warnings`；`cargo test --workspace --locked --no-fail-fast`；`cargo xtask licenses`；`cargo build --release --locked` | Linux、macOS、Windows；`cargo xtask licenses` 只在 Linux 上跑 |
| `miyu-im` | `cargo xtask check` | Linux、macOS、Windows |
| `miyu-migrate` | `cargo fmt --all --check`；`cargo clippy --workspace --all-targets --locked -- -D warnings`；`cargo test --workspace --locked` | Linux、macOS、Windows |
| `miyu-embed` | `cargo fmt --check`；`cargo clippy --all-targets --locked -- -D warnings`；`cargo test --locked` | Linux、macOS、Windows |
| `miyu-render`、`miyu-memes` | `cargo fmt --check`；`cargo clippy --all-targets --locked -- -D warnings`；`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --document-private-items --locked`；`cargo test --locked --no-fail-fast` | Linux、macOS、Windows |
| `miyu-ocr` | 同上四条，都带 `--workspace`；`cargo xtask licenses` | Linux、macOS、Windows；`cargo xtask licenses` 只在 Linux 上跑 |
| `miyu-sdk` | `python -m unittest discover -s python/tests`；在 `rust/` 里 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test` | Linux、macOS、Windows；Python 另在 Linux 上跑 3.9 |

表里 `名字=值 命令` 是 bash 的写法；PowerShell 里先 `$env:名字 = '值'`，再跑命令。

这些仓库的 CI 在 PR 上也跑。除了 `miyu-migrate`、`miyu-sdk`，都限每个 `.rs` 不超过 500 行（`miyu-im` 在它的 `cargo xtask check` 里查）。`miyu-web` 的页面测试读核心的出厂资源，`<核心检出>` 要是它 `Cargo.toml` 里钉的那个提交。
