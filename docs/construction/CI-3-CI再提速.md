## 施工单 CI-3：CI 再提速

状态：已完成（2026-11-05）。core1 11-05 派三样，照 CI-2「没做的和建议」：「clippy（核心不开可选包）」只在 ubuntu 跑；CI 上 dev 的调试信息只留行号；Windows 上把工作目录、`~/.cargo`、临时目录加进 Defender 的排除（不关实时扫描）。做的时候查到编译缓存从 10-10 起就没换过，core1 当天定了改法（「编译缓存的键」一节）。

### 为什么

CI-2 以后 Windows 的 check 一趟 38～42.5 分钟，离 40 分钟没多少余量；分流挤不出多少了，剩下的大头是编测试（约 13 分钟）、clippy 两遍（约 4 分钟）、跑测试里每个测试都起核心、建文件（CI-2「没做的和建议」）。

### 改了什么

1. **「clippy（核心不开可选包）」只在 ubuntu 跑**：`cargo xtask check` 认一个新的环境变量 `MIYU_CHECK_SKIP_BARE_CORE`，设成非空就跳过这一项，结果表里照样有一行「clippy（核心不开可选包，这台跳过）」，项数不变。CI 的 check 在 job 上照 `matrix.os` 设：ubuntu 是空的（等于没设，照跑），Windows、macOS 是 `1`。本机门禁不设，照旧全跑。它查的是特性组合（`net` 下才有的代码别处还在用），和平台关系不大。
2. **CI 上 dev 只留行号**：check 这一项设 `CARGO_PROFILE_DEV_DEBUG=line-tables-only`。panic 的回溯照样有文件和行号，只是调试器里看不了变量。只在 check：randomized 照 release 编；perf 里照 dev 编的只有 xtask 自己，省不了什么。
3. **Windows 上 Defender 加排除**：check 头一步（Windows 才有）用 `Add-MpPreference -ExclusionPath` 把 `$env:GITHUB_WORKSPACE`、`$env:USERPROFILE\.cargo`、`$env:CARGO_HOME`、`$env:TEMP`、`$env:RUNNER_TEMP` 加进排除，再打一行实时扫描开着没有、一行现在的排除。不关实时扫描。整步 `continue-on-error`，里面 `try`/`catch` 打原因，加不上也不让 CI 红。门禁测试的临时目录（`temp::fresh`）在 `%TEMP%` 下面，一起排除了。
4. **编译缓存的键**：见下一节。

### 编译缓存的键

查到的：`Swatinem/rust-cache` 原来设了 `add-rust-environment-hash-key: false`（0-3 再补，为的是别把机器自带的 stable 算进键里）。这个开关把键的后一半（Cargo.lock 和各个 `Cargo.toml` 的哈希）也一起关了，键里只剩项名和系统，例如 `v0-rust-check-Windows_NT-x64`。GitHub 的缓存一个键只能存一次，键不变就再也不存新的：`gh cache list` 看到 check、randomized、perf 五份缓存都是 2026-10-10 存的，这以后 Cargo.lock 改过 71 次，main 一直命中那一份（`cache-hit` 为真，也不存新的），分支每趟照它取、重编 10-10 以后变了的依赖。原来注释里写的「钉住的工具链已经算在键的后一半」不对，一起改了。

只留行号要换一份缓存（旧的依赖照全量调试信息编的，换了开关都要重编），键不变的话 main 会一直命中旧的那一份。两种改法，core1 11-05 定了第二种：

- 只在 check 加一个词（`key: dev-lines`），键照旧不带锁文件：代价是以后依赖再变，缓存又一直停在合并那一刻。
- **照 rust-cache 默认的那一套**，带上锁文件的哈希：拿掉 `add-rust-environment-hash-key: false`。改了 Cargo.lock 的，rust-cache 按前一半（项名、系统、工具链版本、`CARGO_` 等开头的环境变量）退回到最近的一份，只重编变了的依赖，跑完照新键存一份；没改的照旧整份命中。代价：main 上每改一次 Cargo.lock 照新键要整套跑一遍、存一份（原来分支上测过的 main 直接跳过），每份 0.5 GB 上下，旧的照 GitHub 的规矩七天没用就清掉、总量过 10 GB 先清最久没用的。

照第二种做：三项都拿掉那个开关；check 另加 `key: dev-lines`（环境变量本来就算在前一半里，写明了好认）。退回前缀实测见「改后实测」。

### 卸掉机器自带的别的工具链

帮手照 rust-cache 的源码加的，不在派的三样里；core1 11-05 看过，定了保留。照默认的键，这一步必须有。

- **为什么加**：默认的键前一半要算机器上装着的**每一个**工具链的版本（`rustc -vV`，再对 `rustup toolchain list` 列出的每一个跑 `rustup run <它> rustc -vV`）。0-3 再补当初关掉那个开关就是为这个：GitHub 的机器自带的 stable 一批一批地换，同一天有 1.98.1 的、有 1.99.0 的，前一半对不上的不光整份命中不了，连按前缀退回都退不到，分支从零编。只拿掉开关不管它，这个老问题就回来了。
- **怎么做**：三项「装上 rust-toolchain.toml 钉住的工具链」那一步改成 `shell: bash`：`rustup toolchain install` 装好钉住的 1.97.0，`rustup show active-toolchain` 认出它，`rustup toolchain list` 里别的逐个 `rustup toolchain uninstall`（卸不掉的 `|| true`，不拦），最后打一行 `rustup toolchain list`，日志里看得到只剩哪个。剩下的只有钉住的那一个，前一半只随 rust-toolchain.toml 和 `CARGO_` 开头的环境变量变。
- **卸了不碍事**：托管机器每趟都是一台新的，用完就扔，卸掉的工具链不影响别的项、别的趟；仓库里用的只有 rust-toolchain.toml 钉住的那一个。三台上这一步各花多久见「改后实测」。
- **以后要是去掉它**：三处 `run:` 改回一行 `rustup toolchain install`、去掉 `shell: bash` 和注释即可，键不用动；代价是前一半随托管机器自带的 stable 换，赶上换批的那几天分支从零编。

### 量（改前）

main 上 CI-2 合进去以后还没有整套跑过的一趟（合的那一趟分支上测过、直接跳过），改前照 CI-2 的三趟分支 run：41929612529、41932485427、41939943755（含 Windows 重跑那一次，Windows 四个数、别的三个数），代码和 main 一样。check 里每一步的秒数照日志里门禁打的每一节标题的时刻算（脚本、日志在 `scratchpad/ci3/`）。注意改前的「热」不是真热：缓存是 10-10 那一份，这以后变了的依赖每趟照样重编。

### 改后实测

分支上不存缓存，`step/CI-3-CI再提速` 上跑几趟都是冷的。量热的另推了一个分支 `step/CI-3-测热缓存-r2`：这一步的代码加一个只把 check 的 `save-if` 改成 `true` 的提交（不合），跑一趟（冷，存下三台 check 的缓存，分支上存的只有这个分支自己取得到），整个 run 重跑一次（热）：run 41953103021 第一次、第二次。量完删了分支和它存的三份缓存，main 的没碰。randomized、perf 在这个分支上照旧不存，两次都是冷的（键也换了）。

#### 每项一趟（分钟）

| 项 | 改前（中位，范围） | 冷 | 热 |
|---|---|---|---|
| check（windows-latest） | 38.6（38.0～42.5） | 34.0 | 33.4 |
| check（ubuntu-24.04） | 17.4（14.0～18.9） | 16.8 | 15.2 |
| check（macos-latest） | 20.6（19.8～22.6） | 17.3 | 16.6 |
| randomized | 28.7（22.8～28.8） | 23.4 | 28.5（没改，两次都冷，托管机器之间差） |
| perf | 10.5（10.5～10.9） | 10.9 | 11.9（同上） |
| tested | 0.1 | 0.0 | 0.1 |

#### check 里每一步（秒；改前是中位）

| 步 | Windows 改前 | 冷 | 热 | ubuntu 改前 | 冷 | 热 | macOS 改前 | 冷 | 热 |
|---|---|---|---|---|---|---|---|---|---|
| 开头（腾盘、Defender、取代码、装工具链、取缓存） | 70 | 30 | 61 | 130 | 117 | 135 | 30 | 18 | 29 |
| 编 xtask | 52 | 74 | 31 | 24 | 35 | 16 | 33 | 50 | 26 |
| 格式 | 9 | 10 | 8 | 6 | 6 | 6 | 7 | 5 | 6 |
| clippy（整个工作区） | 195 | 219 | 112 | 102 | 128 | 86 | 132 | 170 | 103 |
| clippy（核心不开可选包） | 61 | 跳过 | 跳过 | 32 | 35 | 34 | 50 | 跳过 | 跳过 |
| 文档 | 62 | 58 | 51 | 34 | 37 | 37 | 57 | 58 | 52 |
| 许可证 | 5 | 6 | 5 | 3 | 3 | 3 | 5 | 9 | 5 |
| 编测试（`cargo test --no-run`） | 767 | 522 | 323 | 320 | 238 | 190 | 441 | 293 | 352 |
| 跑测试 | 1092 | 1030 | 1404 | 410 | 389 | 399 | 482 | 404 | 418 |

- **编测试**：只留行号以后 Windows 767 → 冷 522、热 323 秒（少 58%），ubuntu 320 → 190，macOS 441 → 293～352。冷的那一趟依赖也从零编，照样比改前快：PDB 和调试信息是大头。
- **clippy（整个工作区）**：热的 Windows 195 → 112、ubuntu 102 → 86、macOS 132 → 103。clippy 不受调试信息影响，省下的是缓存：改前那一份停在 10-10，以后变了的依赖每趟重编；冷的那一趟比改前还慢（全部从零编）。
- **跳过核心那一遍**：Windows 省 61 秒、macOS 省 50 秒。
- **跑测试**：没动。Windows 热的那一趟 1404 秒，比冷的 1030 慢，是拿到的机器慢：同一份测试，`miyu-endpoint` all 311 → 481 秒、`miyu-session` all 257 → 374 秒，还多报了两条「跑了 60 秒以上」（`memory::lane::tests::laid::laying_piece_by_piece_writes_what_the_whole_log_did`、`fork_streamed::forking_reads_the_parent_in_blocks_and_writes_what_the_whole_read_wrote`）。托管机器之间差三四成，CI-2 也见过。
- **缓存大小**：check Linux 560 MB（改前 597）、macOS 448 MB（451）、Windows 615 MB（600）。

#### Defender：加上了，但本来就没在扫

两趟日志一样：`加了：C:\Users\RUNNER~1\AppData\Local\Temp; C:\Users\runneradmin\.cargo; D:\a\_temp; D:\a\miyu\miyu`，没报错；`实时扫描：False`；加之前排除里就已经有 `C:\`、`D:\` 整盘（打出来的是 `C:\; …; D:\; …`）。GitHub 托管的 Windows 机器实时扫描本来就关着、两块盘整盘排除，这一步是空操作，一趟花 5 秒上下。所以 CI-2 猜的「Windows 上起停测试慢 9～12 倍可能是 Defender」排除了，剩下的是建文件、刷盘（见 CI-2「没做的和建议」第 1 条）。留着这一步：以后换了镜像、真开了扫描，日志里看得出来，排除也照样加上。

#### 退回前缀（键带上锁文件以后）

在量热的分支上再推一个只改 Cargo.lock 的提交（`itoa` 1.0.18 → 1.0.17，不合；run 41957700841，三台取完缓存就取消了，没存）。三台 rust-cache 都照前缀退回到了上一份：

- ubuntu：键 `v0-rust-dev-lines-check-Linux-x64-944d29f9-e65aca45`，`Restored from cache key "v0-rust-dev-lines-check-Linux-x64-944d29f9-507609a0" full match: false.`
- macOS：`…-Darwin-arm64-96a900ce-e65aca45` 退回 `…-96a900ce-507609a0`，`full match: false`。
- Windows：`…-Windows_NT-x64-d635ab92-e65aca45` 退回 `…-d635ab92-507609a0`，`full match: false`。

热的那一趟三台都是 `full match: true`。冷的第一趟三台都是 `No cache found.`：新键的前缀 `v0-rust-dev-lines-…` 以前没存过，没得退。键里「考虑的环境」只有 `1.97.0` 一个工具链和 `CARGO_INCREMENTAL`、`CARGO_PROFILE_DEV_DEBUG` 两个变量（rust-cache 自己设 `CARGO_INCREMENTAL=0`）。

合进 main 以后头一趟：五份缓存的键都换了，`tested` 判「分支上测过」也照样整套跑（`cache-hit` 不为真），跑完照新键各存一份；10-10 那五份七天没人取，GitHub 自己清掉。

#### 卸掉别的工具链花多久

每台机器自带一个 `stable`（`stable-x86_64-unknown-linux-gnu`、`stable-aarch64-apple-darwin`、`stable-x86_64-pc-windows-msvc`），卸掉它连同再列一遍（从 `rustup toolchain install` 打完「it's active because」到 `rustup toolchain list` 打出来）：ubuntu 0.05、0.11 秒，macOS 0.20、0.16 秒，Windows 0.7、3.4 秒（冷、热两趟）。rustup 会打一行 `warn: removing the default toolchain; proc-macros and build scripts might no longer build`：仓库里 rust-toolchain.toml 盖过默认的，编 proc-macro、build.rs 照用 1.97.0，两趟都编过了。

### 验收

- 远程门禁（.91）11 项全过（本机门禁不设开关，「clippy（核心不开可选包）」照跑）。
- 本机 xtask 的 clippy、Windows 版 clippy 过。
- CI：量热的分支两趟六项全绿（41953103021 第一次、第二次）；Windows、macOS 的门禁结果表里是「✓ clippy（核心不开可选包，这台跳过）」，ubuntu 照跑。这一步的正式提交推 `step/CI-3-CI再提速` 另跑一趟。

### 风险

- CI 上调试器看不了变量：CI 上本来也不开调试器，回溯的文件、行号照有。本机不受影响。
- 「clippy（核心不开可选包）」Windows、macOS 不跑：只在这两个平台上才有的 `cfg` 分支、又只在关掉可选包时才编得出错的，CI 上拦不住，要等本机门禁（Linux）或者 Windows 版 clippy（本机 `cargo clippy --target x86_64-pc-windows-gnu` 照 `--workspace` 跑，不是这一项）。目前没有这样的代码。
- 缓存照锁文件换：main 上改依赖的那一趟要多跑一遍整套。
