# 应急修复

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

当 Miyu 坏了（核心启动不了、配置改坏了、界面打不开）的时候，可以用 `miyu rescue` 启动 LiteMiyu，让 AI 帮你在终端里把 Miyu 修好。这一页介绍 LiteMiyu 是什么、怎么安装，以及怎么用它修复 Miyu。

## 什么是 LiteMiyu？

LiteMiyu 是一个单独的程序 `litemiyu`：只有一个进程、一个文件，在终端里全屏运行，自己就能和模型对话、读写文件、运行命令。它不使用 Miyu 的核心，也不读取 Miyu 的会话和记忆。目前只有 Linux x86_64（glibc 2.39 及以上）可以使用。

LiteMiyu 的文件放在它自己的目录里，和 Miyu 的数据根（默认是 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`）互不影响：

| 内容 | 位置 |
|---|---|
| 配置 `config.toml`、密钥 `secrets.toml` | `~/.config/litemiyu/` |
| 会话、记忆 | `~/.local/share/litemiyu/` |
| 缓存 | `~/.cache/litemiyu/` |
| 运行日志、输入历史 | `~/.local/state/litemiyu/` |

如果设置了 `XDG_CONFIG_HOME` 这类变量，就按照这些变量指定的位置存放。

## 安装 LiteMiyu

LiteMiyu 需要单独安装，不同的发行版安装方法不同：

- Arch Linux：从 AUR 安装 `litemiyu`，例如运行 `paru -S litemiyu`。程序安装在 `/usr/bin/litemiyu`。
- 其他发行版：在 [发布页](https://github.com/miyu-agent/litemiyu/releases) 下载 `litemiyu-<版本>-linux-x86_64.tar.gz` 和 `SHA256SUMS`，校验之后把压缩包里的 `litemiyu` 放进 `PATH` 里的某个目录：

  ```sh
  sha256sum -c SHA256SUMS
  tar xf litemiyu-<版本>-linux-x86_64.tar.gz
  install -m755 litemiyu-<版本>-linux-x86_64/litemiyu ~/.local/bin/
  ```

`miyu rescue` 会按照以下顺序查找 `litemiyu`：你运行的那个 `miyu` 所在的目录、`miyu` 实际所在的目录（解析链接之后）、`PATH`。`PATH` 里的相对目录不算在内。

## 使用 miyu rescue

运行：

```sh
miyu rescue [<参数>…]
```

这条命令不连接核心，也不启动核心，而是做下面这几件事：

1. 根据 Miyu 的系统配置 `~/.miyu/etc/config.toml` 和密钥文件，算出默认模型（`models.chat`）连接哪一家供应商、使用哪个模型、什么接口、什么密钥。
2. 把这四项放进四个环境变量，交给 LiteMiyu。如果其中某个变量你自己已经设置了，就保留你设置的值。

   | 环境变量 | 内容 |
   |---|---|
   | `LITEMIYU_RESCUE_BASE_URL` | 供应商的地址 |
   | `LITEMIYU_RESCUE_MODEL` | 模型名 |
   | `LITEMIYU_RESCUE_API` | 接口：`openai-chat`、`anthropic`、`openai-responses` |
   | `LITEMIYU_RESCUE_API_KEY` | 密钥；如果默认模型没有密钥，就不设置这个变量 |

3. 运行 `litemiyu --rescue`，把你写的参数原样接在后面传过去。退出码就是 LiteMiyu 的退出码。

Miyu 的数据根在哪里不会传给 LiteMiyu，所以修复时需要你告诉 AI。

LiteMiyu 能识别的参数有三个，写了的参数会覆盖环境变量：`--base-url <地址>`、`--model <名字>`、`--api <种类>`。密钥只能通过环境变量传入。

在下面几种无法填写的情况下，`miyu rescue` 会先输出一行警告，四个变量一个都不填，照样启动 LiteMiyu，由 LiteMiyu 使用它自己的配置：

| 看到的提示 | 原因 |
|---|---|
| `miyu: 警告：无法读取 Miyu 的配置：…，由 litemiyu 自己查找模型` | 配置文件或密钥文件整份都无法解析，或者默认模型无法使用。如果只是某一项写错了，会跳过那一项，其他项照常读取 |
| `miyu: 警告：对话模型使用扩展驱动 <包>/<驱动名>，litemiyu 不支持，由它自己查找模型` | 默认模型使用的是软件包自带的驱动 |

## 应急模式和平常模式的区别

`litemiyu --rescue` 是专门为修复问题准备的，和平常模式相比有这些不同：

- 不往磁盘写任何东西：不记录会话、不修改配置、不记录输入历史。所以即使磁盘满了、目录只读也能使用。关掉之后什么都不会留下。
- AI 只有五个工具：`read`、`write`、`edit`、`shell`、`jobs`。没有联网、子代理、记忆、技能和 MCP。
- AI 没有关于 Miyu 的专门说明，所以要修什么、文件在哪里，需要你在第一句话里说清楚。
- 没有可以切换的权限级别，文件和命令都会直接执行，Miyu 的数据根也可以修改。危险命令的识别方式和 Miyu 相同（见 [权限和沙盒](permissions.md#要确认的命令)）：会被直接拒绝的那几类照样拒绝；需要确认的那几类会先问你，只能选择允许这一次或者拒绝，没有开关可以调整。
- 输入框上边框的右侧显示 `应急模式`。命令列表里没有 `/sessions`、`/config`、`/loop`、`/model`、`/effort`；`Ctrl+G` 的外部编辑器无法使用。

## 修复示例

下面用一个例子演示完整的修复过程：你手动修改了 `~/.miyu/etc/config.toml` 之后，运行 `miyu` 时报错 `核心无法启动：…`。

1. 运行：

   ```sh
   miyu rescue
   ```

   屏幕会切换到 LiteMiyu 的全屏界面，输入框上显示 `应急模式`。

2. 把情况告诉 AI，例如：

   ```text
   Miyu 的核心无法启动。它的数据根是 ~/.miyu，系统配置是 ~/.miyu/etc/config.toml，核心日志是 ~/.miyu/var/log/core.log。
   看日志最后几十行找出原因，把配置改好，告诉我改了哪里。
   ```

3. AI 会读取日志、修改文件，最后告诉你改了什么。按照它的说明自己检查一遍。
4. 按 `Ctrl+D`（输入框为空时）或者输入 `/exit` 退出。
5. 运行 `miyu`，确认核心能否正常启动。

如果配置文件整份都无法解析，第 1 步会先输出 `无法读取 Miyu 的配置` 的警告。如果 LiteMiyu 自己也没有配置模型，它不会进入界面，而是输出缺少的内容和用法后退出（退出码 `2`），第一行是：

```text
litemiyu --rescue：没有能用的模型，缺 LITEMIYU_RESCUE_BASE_URL、LITEMIYU_RESCUE_MODEL。
```

此时需要你自己把模型告诉它（把地址和模型名换成你的供应商的，示例里的密钥是假的）：

```sh
LITEMIYU_RESCUE_API_KEY=sk-example miyu rescue --base-url https://api.deepseek.com/v1 --model deepseek-chat
```

没有写 `--api` 时按 `openai-chat` 处理。没有提供密钥时照样会进入界面，界面上会提示 `未提供密钥（LITEMIYU_RESCUE_API_KEY），请求不带认证`。

如果想以后不用手动输入这些：趁 Miyu 还正常的时候，运行 `litemiyu config` 打开配置界面，在添加供应商的地方选择 `从 Miyu 导入…`，把 Miyu 的供应商和密钥导入 LiteMiyu 自己的配置，再选好主对话的模型即可。环境变量和参数都没有提供模型时，应急模式会使用 LiteMiyu 自己配置里的模型。

如果 `miyu` 这个程序本身坏了、无法运行，可以直接运行 `litemiyu --rescue`，模型按照上面的方法通过环境变量和参数提供。

## 其他系统

macOS 和 Windows 上没有 LiteMiyu。`miyu rescue` 找不到它时会输出：

```text
miyu: 未找到 litemiyu
提示：目前只支持 Linux
```

退出码是 `1`。在 Linux 上找不到 LiteMiyu 时，提示会换成安装方法。

在 macOS 和 Windows 上，如果核心坏了，需要你自己按照下面的步骤排查：

1. 查看核心日志的最后几十行，找出出错的原因：

   ```bash
   tail -n 50 ~/.miyu/var/log/core.log
   ```

   ```powershell
   Get-Content -Tail 50 $HOME\.miyu\var\log\core.log
   ```

2. 如果是手动修改把配置改坏了，用编辑器打开 `~/.miyu/etc/config.toml`，把改坏的那几行改回去。核心启动时，整份都无法解析的配置会被当作空配置，核心照样能启动；此时运行 `miyu check`，按照它输出的行号修改即可。
3. 修改完成后，运行 `miyu core stop` 停止正在运行的核心，再运行 `miyu` 打开。

每种报错的含义和处理方法，可以看 [出了问题怎么查](troubleshooting.md#核心无法启动或无法连接)。

## LiteMiyu 的日常用法

不带 `--rescue` 运行就是平常模式，会话和配置都会保存到磁盘：

```sh
litemiyu            # 开对话
litemiyu -c         # 打开最近的会话
litemiyu config     # 进配置界面
litemiyu ask "…"    # 说一句话并输出回答，不进入全屏界面
```

- 第一次运行时会先进入引导：接入一家供应商（如果电脑上装有 Miyu，`从 Miyu 导入…` 会排在最前面）、选择主对话使用的模型、选择按键模式和明暗主题。
- 按键：`Enter` 发送，`Alt+Enter` 或 `Ctrl+J` 换行，`@` 补全文件路径，`Ctrl+C` 在 AI 回答时打断，`Ctrl+D` 在输入框为空时退出。输入 `/help` 可以查看全部按键和命令。
- 供应商、模型和会话都在全屏界面里管理（`/config`、`/sessions` 等命令），命令行里只有上面这几条命令。
- 每条命令都有 `--help`，例如 `litemiyu ask --help`。

## 修复失败时

如果还是修不好，带上 `~/.miyu/var/log/core.log` 的最后几十行，按照 [出了问题怎么查](troubleshooting.md#报告问题) 里的方法报告问题。
