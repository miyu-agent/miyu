## 施工单 RC-1：`miyu rescue` 拉起 LiteMiyu 的应急模式

状态：做完了（2026-11-13）。项目主人 2026-11-12、11-13 定。

### 为什么

核心坏了的时候，Miyu 自己的头都要连核心，修不了它。LiteMiyu 有一个不写盘、只带文件和命令工具的应急模式（`litemiyu --rescue`，
LiteMiyu 仓库 `docs/线-界面.md`「第八趟：应急」）。项目主人定：`miyu rescue` 就是进它的一个入口；核心可能正坏着，所以整条路不经过
核心（不连、不拉起）。LiteMiyu 用系统的包管理器分发（AUR、Homebrew 这类），装好就是 `PATH` 里的 `litemiyu`，还没发版。

### 蓝图改哪几节

- 新加 `cli/rescue.md`：是什么、在哪、怎么走（找程序、填变量、拉起）、给人看的字、帮助页样本、守着它的。
- `cli/main.md`：子命令表加 `rescue`，帮助页文件列表加 `rescue`，总表样本（中、英）「系统」组加一行，「怎么走」第 4 条写明
  `rescue` 交的是算模型的函数、不是拉起核心的命令。
- `README.md`（蓝图）：命令行那一行登记 `cli/rescue.md`。

### 做了什么

1. **找程序**，取第一个是文件的（Windows 上 `litemiyu.exe`）：`miyu` 被敲的那个路径（`argv[0]`，不解链接；只敲了名字的照 `PATH` 找到
   被敲的那一个，相对路径照当前目录）的同一个目录 → `miyu` 真实位置（`current_exe` 解了链接）的同一个目录 → `PATH`。`PATH` 里相对的
   目录不认（同 `miyu_store::packages::locate`）。AUR 的 `/usr/bin/miyu` 链到 `/usr/lib/miyu/miyu`，第一条找的是 `/usr/bin/litemiyu`。
2. **找不到**：报错、退出码 1，不做别的、不自动装。

   | 平台 | 中文 | 英文 |
   |---|---|---|
   | Linux | `miyu: 未找到 litemiyu`，`提示：安装：Arch Linux 用 AUR（paru -S litemiyu 或 yay -S litemiyu）；其他发行版从 https://github.com/miyu-agent/litemiyu/releases 下载` | `miyu: litemiyu not found`，`hint: to install it, use the AUR on Arch Linux (paru -S litemiyu or yay -S litemiyu); on other distributions, download it from https://github.com/miyu-agent/litemiyu/releases` |
   | macOS、Windows | `miyu: 未找到 litemiyu`，`提示：暂只支持 Linux` | `miyu: litemiyu not found`，`hint: only Linux is supported for now` |

   照设计 47 的写法：程序名打头、不带句号，第二行写成提示。
3. **变量映射**：照 Miyu 的 `models.chat` 算出一个模型，只填环境里本来没有的（人自己给的优先；四个都有的不读配置）。

   | 变量 | 来源 |
   |---|---|
   | `LITEMIYU_RESCUE_BASE_URL` | 那一家的 `base_url`（手写的、档案的、目录的，同核心；`{ env = … }` 照这个进程的环境） |
   | `LITEMIYU_RESCUE_MODEL` | 模型名 |
   | `LITEMIYU_RESCUE_API` | 驱动 `openai-chat`、`anthropic`、`openai-responses` 照同名 |
   | `LITEMIYU_RESCUE_API_KEY` | key：`{ secret }` 照密钥文件，`{ env }` 照环境；没写 key 的不填。只走环境变量 |

   - 驱动照真发的那个模型（同一家里的模型可以各走各的，施工 8-14），同核心发请求时一样。
   - 池 `@名字`：照写的先后取第一个用得了的、走内置驱动的成员。用得了：那一家合得出、这个模型有驱动、地址解得出也不是空的、写了
     key 的取得到。走扩展驱动的成员跳过。不看分法、指针、钉着的、冷却：那些在核心的状态里，应急时不读。
   - 对话模型走扩展驱动（`<包>/<驱动名>`，模型直接写的，或者池里内置的都用不了、只剩它）：四个一律不填，标准错误上警告一行
     `miyu: 警告：对话模型用的是扩展驱动 <驱动>，litemiyu 认不了，交给它自己找模型`（英文 `miyu: warning: the chat model uses the extension driver <driver>, which litemiyu cannot use; litemiyu will find a model itself`），照常拉起。
     core1 2026-11-13 照推荐定：原来定的「只不填种类」会让 LiteMiyu 拿着地址、模型照 `openai-chat` 发，多半是错的。
   - 读配置、读密钥出错，`models.chat` 没配、解析不出、用不了：一个都不填，标准错误上警告一行
     `miyu: 警告：读不了 Miyu 的配置：<原因>，交给 litemiyu 自己找模型`（英文 `miyu: warning: cannot read Miyu's configuration: <reason>; litemiyu will find a model itself`），照常拉起。
4. **拉起**：`litemiyu --rescue <其余参数原样>`。Unix 上 exec 换掉当前进程，Windows 上起子进程、等它、这期间不理 Ctrl+C，照它的
   退出码退（共用转交软件包子命令的 `packages::run`）。
5. **分层**：读盘的一段在 `miyu-core`（`rescue.rs`：系统配置只认模型这一块、密钥文件、档案和认原厂的表、目录；不整理目录、不写盘、
   不建数据根），纯逻辑在 `miyu-models`（`direct.rs`），找程序、填变量、拉起在 `miyu-cli`（`rescue.rs`、`rescue/find.rs`）。命令行的头
   不依赖 `miyu-core`：主程序把 `miyu_core::rescue::model` 交给 `miyu_cli::rescue`，同交拉起核心的命令那样。`miyu-cli` 的 `miyu-models`
   从开发依赖挪成依赖（要它的 `Direct`、`Driver`）。
6. 帮助：总表「系统」组加 `rescue [<参数>...]  用 LiteMiyu 的应急模式修复`，`miyu rescue -h` 一页（中、英）。`rescue` 后面的参数一律
   原样交过去，只有第一个是 `-h`、`--help` 时印帮助。

### 没做的

- 不核 `litemiyu` 的版本：它还没有 `--version`。
- 不替人装 LiteMiyu，不提示 Homebrew：LiteMiyu 现在只发 Linux 的包。
- 命令行自己的字只有中文、英文（`ja` 照英文），这一步不加日文。

### 验收

- 新命令：改之前 `rescue` 是不认识的命令（退出码 2），下面的测试本身就是红的证明。
- `crates/miyu-cli/src/rescue/tests.rs`：找程序的先后（敲的路径旁边、真实位置旁边、`PATH`，是目录的不算）；只敲名字的照 `PATH` 找到
  被敲的那一个；相对的 `argv[0]`；`PATH` 里相对的目录不认；Windows 找 `.exe`；链接先找链接旁边、再找目标旁边。填变量：四个都填、
  驱动映射、扩展驱动的一个都不填、没有 key 的只不填 key、本来有的不覆盖、四个都有的不读配置、算不出的一个都不填。找不到时两种语言、两种平台，扩展驱动的警告两种语言。
- `crates/miyu-models/src/direct/tests.rs`、`crates/miyu-core/src/rescue/tests.rs`、`crates/miyu/tests/rescue.rs`：见蓝图
  `cli/rescue.md`「守着它的」。
- 命令：
  - `cargo test -p miyu-models -p miyu-core -p miyu-cli -p miyu --no-fail-fast`
  - `cargo clippy --workspace --all-targets -- -D warnings`，同上加 `--target x86_64-pc-windows-gnu`
  - `cargo fmt --check`
  - `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`
  - `cargo xtask prompts` 无差异
- 手测：装一个假的 `litemiyu`（把参数和 `LITEMIYU_RESCUE_*` 印出来的脚本）放进 `PATH`，`miyu rescue --foo` 看印出来的；
  `PATH` 里没有时看两行提示、退出码 1。

### 风险

- 配置读法和核心分了两处：这里只认模型这一块的几项，丢掉写错的一项、整份读不懂就不填，和核心读同一份文件的规矩一样
  （`miyu_config::parse`、`merge`）；模型怎么算全在 `miyu-models`，和核心共用。
- 目录约 5 MB，每次 `miyu rescue` 现读一遍（debug 下不到一秒）：应急时用一次，不整理、不写盘。
