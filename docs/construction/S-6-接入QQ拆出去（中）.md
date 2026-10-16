## 施工单 S-6 中：开 `miyu-agent/miyu-onebot`

状态：施工中（2026-10-16）。设计 `32-仓库拆分.md` 第九节；S-6 上（ff02d938、修 68457b03）已经让桥只依赖门面、从包文件夹读自己的文件。接入QQ 的会话 10-16 对过结构：两个 crate 一个 workspace，包文件夹保留 `data/`，图纸和登记簿、门禁一起搬。

### 这一步做什么

1. **带着历史开新仓库** `miyu-agent/miyu-onebot`（公开，同组织里别的仓库）：照路径从核心 main 滤出来（`git filter-repo`，提交和作者都在），只留下面这几样，路径改成新仓库里的：

   | 核心里 | 新仓库里 |
   |---|---|
   | `crates/miyu-onebot/`、`crates/miyu-chat/` | 同名 |
   | `resources/packages/onebot/`（清单、`page/`、S-6 上以后的 `data/`） | `package/` |
   | `resources/software/onebot/`（S-6 上以前的历史） | `package/data/` |
   | `docs/blueprint/onebot.md`、`docs/blueprint/chat.md` | 同名 |

2. **新仓库自己的几样**：根上的 `Cargo.toml`（workspace，两个 crate）、`Cargo.lock`、`rust-toolchain.toml`、`clippy.toml`、`LICENSE`（同核心，GPL-3.0-or-later）、`README.md`（中文，怎么装、怎么从源码编）、`AGENTS.md`（照核心的写法，只留这个仓库用得上的）。
3. **git 依赖钉核心**：`miyu-client`、群聊内核要的 `miyu-kernel`、`miyu-config`，测试要的核心几个 crate（开发依赖）都写 `{ git = "https://github.com/miyu-agent/miyu", rev = "<同一个提交>" }`，起头钉 68457b03。升级就是改这一处、过 CI。
4. **测试夹具拼资源目录**：核心的资源目录里以后没有 `packages/onebot/`（S-6 下删）。要「出厂的 onebot 包」的测试，照钉住那份核心检出的 `resources/`（门面 `testkit` 的 `WORKSPACE_ROOT`）拼一个临时资源目录，再把新仓库的 `package/` 放成它的 `packages/onebot/`；直接拉桥的照旧设 `MIYU_PACKAGE_DIR` 指新仓库的 `package/`。
5. **门禁**：格式、clippy（`-D warnings`）、文档、测试、行数（每个 `.rs` 不超 500 行）、许可证（照核心 `xtask` 的那一份，四个平台、能和 GPL-3.0-or-later 合在一起发）、**登记簿对账**：给模型看的字（`package/data/` 下判官、平台工具、事实、工具结果的那几份）登记在 `docs/ledger.md`（设计 26 第十节接入QQ 那几段搬过来），门禁照文件算指纹、对账，不对的、没登记的拦（照核心 `xtask prompts`）。
6. **CI 三平台**（`.github/workflows/ci.yml`，照终端仓库那一份）：push 到 `main`、`step/**` 和 PR 跑上面几项。
7. **Release 打包**（`.github/workflows/release.yml`）：推 `v*` 标签时三个平台各编 release 的 `miyu-onebot`，拼成 `onebot/` 文件夹（`package.toml`、`page/`、`data/`、`bin/miyu-onebot`，Windows 带 `.exe`），打成 `miyu-onebot-<版本>-<平台>.tar.gz`（Windows 用 `.zip`），挂到 GitHub Release；用的人解开以后 `miyu pkg install ./onebot`。

### 主会话定的技术细节

- 包文件夹放 `package/`，同 `miyu-memes`：这个仓库只出一个包。
- 群聊内核照旧是单独一个 crate：别的平台要用照 git 依赖它（设计 18 Q1）。
- 第一个 Release 等项目主人说发 0.8.0 时一起打标签；这一步只把流程写好、在 `step/**` 上用 `workflow_dispatch` 试打一次（不建 Release）。

### 不做什么

- 核心仓库这一步不删东西（S-6 下删桥、群聊内核、包文件夹，测试改夹具，图纸换成一行指路）。
- 不改桥的行为：拆完以后接入QQ 的会话照本机回放测具过一遍，输出和 68457b03 逐字节一样。

### 验收

1. 新仓库本机：门禁全过；`git log` 里有桥、群聊内核从 O 线开始的提交。
2. 三平台 CI 绿；`workflow_dispatch` 试打的三个包解开以后结构对，`miyu pkg install` 装得上（Linux 本机试装）。
3. 接入QQ 的会话照新仓库跑回放，和 68457b03 逐字节一样。
