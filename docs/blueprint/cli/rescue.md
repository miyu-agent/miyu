## `miyu rescue`

### 是什么

核心坏了时进 LiteMiyu 的应急模式修（施工 RC-1，项目主人 2026-11-12、11-13 定）：一个入口，拉起 `litemiyu --rescue`，照 Miyu 的
对话模型 `models.chat` 把模型交给它。核心可能正坏着，所以整条路不经过核心：不连、不拉起、什么都不发给它。LiteMiyu 由系统的包管理器
分发（AUR、Homebrew 这类），装好就是 `PATH` 里的 `litemiyu`；这里不替人装。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu/src/main.rs` | 子命令 `rescue`；换上帮助页；算模型的函数（`miyu_core::rescue::model`）交给命令行的头 |
| `crates/miyu-cli/src/rescue.rs` | 参数（其余参数原样收下）、入口、填哪几个环境变量（`filled`）、驱动换成 LiteMiyu 的接口种类（`api`）、拉起 |
| `crates/miyu-cli/src/rescue/find.rs` | 找 `litemiyu` |
| `crates/miyu-cli/src/language/rescue.rs` | 给人看的字，中英文 |
| `crates/miyu-cli/src/help/{zh,en}/rescue.txt` | 帮助页 |
| `crates/miyu-core/src/rescue.rs` | 照磁盘读：系统配置（只认模型这一块的几项）、密钥文件、资源目录的档案和认原厂的表、models.dev 的目录；什么都不写 |
| `crates/miyu-models/src/direct.rs` | 照配置算直接连的那一个模型（纯逻辑）：模型照真发时的驱动和地址，池取第一个用得了的成员 |

### 怎么走

1. **找程序**，照这个先后，取第一个是文件的（Windows 上找 `litemiyu.exe`）：
   1. `miyu` 被敲的那个路径（`argv[0]`）的同一个目录，不解链接：AUR 的 `/usr/bin/miyu` 链到 `/usr/lib/miyu/miyu`，找的是
      `/usr/bin/litemiyu`。只敲了名字的照 `PATH` 找到它被敲的那一个；相对路径照当前目录接。
   2. `miyu` 真实位置（`current_exe` 解了链接）的同一个目录。
   3. `PATH`。`PATH` 里相对的目录不认（同转交软件包的子命令找程序）。
2. **找不到**：照下面「给人看的字」报错，退出码 1，不做别的、不自动装。
3. **填环境变量**：四个都已经在环境里的（设了就算，空的也算），不读配置。不然照 Miyu 的配置算出一个模型，只填本来没有的那几个
   （人自己给的优先）：

   | 变量 | 填什么 |
   |---|---|
   | `LITEMIYU_RESCUE_BASE_URL` | 那一家的地址（`base_url`，路径由驱动接在后面；写的是 `{ env = … }` 的照这个进程的环境取） |
   | `LITEMIYU_RESCUE_MODEL` | 模型名，照供应商那边的叫法 |
   | `LITEMIYU_RESCUE_API` | 驱动：`openai-chat`、`anthropic`、`openai-responses` 照同名填。扩展提供的（`<包>/<驱动名>`）LiteMiyu 认不了：四个变量一律不填（第 6 条） |
   | `LITEMIYU_RESCUE_API_KEY` | key：`{ secret = … }` 照密钥文件，`{ env = … }` 照这个进程的环境；那一家没写 key 的不填。只走环境变量，不进命令行参数 |

   算法：
   1. 读系统配置 `etc/config.toml`，只认用途、池（连三个固定池）、供应商、模型手写的资料这几组；写错的一项照核心的规矩丢掉，别的照用。
      读密钥文件 `etc/private/secrets.toml`，写错的一行不收。
   2. 资料照核心一样的几份：资源目录的 `models/profiles.toml`、`models/vendors.toml`，models.dev 的目录（快照和缓存目录里的挑新的）。
      用出来的、供应商的列表、池的指针、冷却都不读：记在核心的状态里，应急时不靠它们。
   3. 模型 `p/m`：那一家的驱动、地址照手写的、档案、目录推（同 `models.md`「怎么走」第一条），再照真发的那个模型换驱动（同一家里的
      模型可以各走各的，施工 8-14）。地址解不出、是空的，写了 key 又取不到的，算用不了。
   4. 池 `@名字`：照写的先后取第一个用得了的、走内置驱动的成员（同上一条的「用得了」），走扩展驱动的成员跳过，不看分法、指针、
      钉着的。内置的一个都用不了、有走扩展驱动的成员的，照第 6 条；都没有的算用不了，原话带上最后一个成员为什么用不了。
   5. 读配置、读密钥出错的，`models.chat` 没配、解析不出、用不了的：一个变量都不填，在标准错误上警告一行（下面「给人看的字」），
      照常拉起，由 LiteMiyu 照它自己的配置找模型。
   6. 对话模型走扩展驱动的（模型直接写的，或者池里只剩它）：LiteMiyu 认不了，四个变量一律不填，另警告一行，照常拉起（core1
      2026-11-13 定：给了地址、模型又不给种类，LiteMiyu 会照 `openai-chat` 发，多半是错的）。扩展驱动的地址、key 不取。
4. **拉起**：`litemiyu --rescue <miyu rescue 后面的其余参数原样>`，环境照这个进程的加上填的。Unix 上换成它（终端、信号直接交给它）；
   Windows 上起它、等它，这期间自己不理 Ctrl+C，照它的退出码退（同转交软件包的子命令，`packages.md`「转交」）。起不来的说系统的
   原话，退出码 1。
5. 不核 `litemiyu` 的版本：它还没有 `--version`。

`rescue` 后面的参数一律原样交过去，`-` 开头的也是；只有第一个是 `-h`、`--help` 时印这一页的帮助。`miyu help rescue` 同。

### 给人看的字

| 什么时候 | 中文 | 英文 |
|---|---|---|
| 找不到，Linux | `miyu: 未找到 litemiyu`，`提示：安装：Arch Linux 用 AUR（paru -S litemiyu 或 yay -S litemiyu）；其他发行版从 https://github.com/miyu-agent/litemiyu/releases 下载` | `miyu: litemiyu not found`，`hint: to install it, use the AUR on Arch Linux (paru -S litemiyu or yay -S litemiyu); on other distributions, download it from https://github.com/miyu-agent/litemiyu/releases` |
| 找不到，macOS、Windows | `miyu: 未找到 litemiyu`，`提示：暂只支持 Linux` | `miyu: litemiyu not found`，`hint: only Linux is supported for now` |
| 算不出模型（照常拉起） | `miyu: 警告：读不了 Miyu 的配置：<原因>，交给 litemiyu 自己找模型` | `miyu: warning: cannot read Miyu's configuration: <reason>; litemiyu will find a model itself` |
| 对话模型走扩展驱动（照常拉起） | `miyu: 警告：对话模型用的是扩展驱动 <驱动>，litemiyu 认不了，交给它自己找模型` | `miyu: warning: the chat model uses the extension driver <driver>, which litemiyu cannot use; litemiyu will find a model itself` |

- 写法照设计 47（`cli/main.md`「给人看的字」）：程序名打头、不带句号、建议另起一行。命令行自己的字只有中文、英文，`ja` 的照英文。
- `<原因>` 是原话（英文，同 `models.md`「出错」那张表），控制字符换成 `�`。读不了的文件带路径和第几行，不带原文（密钥文件的原文就是密钥）。

样本 `crates/miyu-cli/src/help/zh/rescue.txt`（帮助页，中文）：

```text
用法：miyu rescue [<参数>...]

核心出问题时，用 LiteMiyu 的应急模式修复。不连接核心，按 Miyu 的对话模型
（models.chat）设好 LITEMIYU_RESCUE_* 环境变量后运行 'litemiyu --rescue'，
参数原样交给它。已经设置的变量保持不变。

依次在 miyu 所在的目录、miyu 实际所在的目录和 PATH 中查找 litemiyu。

选项：
  -h, --help  显示帮助
```

样本 `crates/miyu-cli/src/help/en/rescue.txt`（帮助页，英文）：

```text
Usage: miyu rescue [<args>...]

Fix Miyu with LiteMiyu's rescue mode when the core is broken. Without
contacting the core, set the LITEMIYU_RESCUE_* variables from Miyu's chat
model (models.chat) and run 'litemiyu --rescue' with the arguments as given.
Variables that are already set are kept.

litemiyu is looked for next to miyu as typed, next to the real miyu, and in
PATH.

Options:
  -h, --help  Print help
```

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-cli/src/rescue/tests.rs` | 找程序的先后：敲的路径旁边、真实位置旁边、`PATH`，是目录的不算；只敲名字的照 `PATH` 找到被敲的那一个；相对的 `argv[0]` 照当前目录；`PATH` 里相对的目录不认；Windows 找 `.exe`；链接（AUR 的样子）先找链接旁边、再找目标旁边。填变量：四个都填、驱动的映射、扩展驱动的一个都不填、没有 key 的只不填 key、本来有的不覆盖、四个都有的不读配置、算不出的一个都不填。找不到时、扩展驱动时两种语言说的 |
| `crates/miyu-cli/src/help/tests.rs` | 帮助页列的选项和程序对得上，最宽 80 列 |
| `crates/miyu-models/src/direct/tests.rs` | 档案推的驱动和地址、手写的和 `{ env = … }`、没写 key 的不带、模型自己的包换驱动（目录给地址）；key、地址取不到的原话；没配、供应商没有；池取第一个用得了的、一个都没有的带上最后的原因、空的固定池；扩展驱动照实交出、地址和 key 不取；池里先扩展后内置的取内置的，内置的都用不了的交扩展的 |
| `crates/miyu-core/src/rescue/tests.rs` | 真资源目录：档案加密钥文件、地址照目录的快照；配置、密钥文件写坏说哪一份第几行、不带原文；数据根不在也不建 |
| `crates/miyu/tests/rescue.rs` | 真程序：`rescue --help`、`-h`、`help rescue` 两种语言，总表有它；找不到的说的、退出码 1、核心没起来过。Unix 上照假的 `litemiyu`：参数原样（`-h` 不在第一个照样交过去）、四个变量、退出码照它的、核心没起来过；本来设了的不覆盖；配置写坏、没配的警告一行、一个都不填、照常拉起；对话模型走扩展驱动的警告一行、一个都不填、照常拉起；敲的链接旁边的先于 `PATH` |

### 出处

- 项目主人 2026-11-12、11-13：`miyu rescue` 就是一个入口，进 LiteMiyu 的 `--rescue`，整条路不经过核心；LiteMiyu 用系统的包管理器分发，
  还没发版。
- LiteMiyu 那一头的变量和规矩：LiteMiyu 仓库 `docs/线-界面.md`「第八趟：应急」的「环境变量和命令行」（给了地址、模型中的一样，两样
  都得给；没给接口种类的照 `openai-chat`；key 只认环境变量）。
