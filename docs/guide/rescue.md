# 应急修复

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

Miyu 坏了（核心起不来、配置改坏了、头打不开）时，用 `miyu rescue` 拉起 LiteMiyu，让 AI 帮你在终端里把它修好。这一页讲 LiteMiyu 是什么、怎么装、怎么用它修 Miyu。

## LiteMiyu 是什么

LiteMiyu 是一个单独的程序 `litemiyu`：一个进程、一个文件，在终端里全屏跑，自己就能和模型对话、读写文件、跑命令。它不用 Miyu 的核心，也不读 Miyu 的会话和记忆。只有 Linux x86_64（glibc 2.39 以上）能用。

它的东西放在自己的目录里，和 Miyu 的数据根（默认 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`）互不相干：

| 放什么 | 在哪 |
|---|---|
| 配置 `config.toml`、key `secrets.toml` | `~/.config/litemiyu/` |
| 会话、记忆 | `~/.local/share/litemiyu/` |
| 缓存 | `~/.cache/litemiyu/` |
| 运行日志、输入历史 | `~/.local/state/litemiyu/` |

设了 `XDG_CONFIG_HOME` 这几个变量的照它们。

## 装

- Arch Linux：从 AUR 装 `litemiyu`，例如 `paru -S litemiyu`。程序在 `/usr/bin/litemiyu`。
- 别的发行版：在 [发布页](https://github.com/miyu-agent/litemiyu/releases) 下载 `litemiyu-<版本>-linux-x86_64.tar.gz` 和 `SHA256SUMS`，核对后把里面的 `litemiyu` 放进 `PATH` 里的一个目录：

  ```sh
  sha256sum -c SHA256SUMS
  tar xf litemiyu-<版本>-linux-x86_64.tar.gz
  install -m755 litemiyu-<版本>-linux-x86_64/litemiyu ~/.local/bin/
  ```

`miyu rescue` 照这个先后找 `litemiyu`：你敲的那个 `miyu` 所在的目录、`miyu` 实际所在的目录（解开链接以后）、`PATH`。`PATH` 里的相对目录不算。

## 用 miyu rescue

```sh
miyu rescue [<参数>…]
```

它不连核心，也不拉起核心：

1. 照 Miyu 的系统配置 `~/.miyu/etc/config.toml` 和密钥文件，算出默认模型（`models.chat`）连哪家、用哪个模型、什么接口、什么 key。
2. 把这四样放进四个环境变量，交给 LiteMiyu。你自己已经设了的那个不动。

   | 环境变量 | 是什么 |
   |---|---|
   | `LITEMIYU_RESCUE_BASE_URL` | 供应商的地址 |
   | `LITEMIYU_RESCUE_MODEL` | 模型名 |
   | `LITEMIYU_RESCUE_API` | 接口：`openai-chat`、`anthropic`、`openai-responses` |
   | `LITEMIYU_RESCUE_API_KEY` | key；默认模型没有 key 的不设 |

3. 跑 `litemiyu --rescue`，后面接你写的参数，原样交过去。退出码照 LiteMiyu 的。

Miyu 的数据根在哪不会交给 LiteMiyu。修的时候要告诉 AI。

LiteMiyu 认得的参数有三个，写了的盖过环境变量：`--base-url <地址>`、`--model <名字>`、`--api <种类>`。key 只认环境变量。

几种填不上的情况，`miyu rescue` 先印一行警告，四个变量一个都不填，照样拉起 LiteMiyu，由它用自己的配置：

| 看到的 | 为什么 |
|---|---|
| `miyu: 警告：读不了 Miyu 的配置：…，交给 litemiyu 自己找模型` | 配置或密钥文件整份读不懂，或者默认模型用不了。只是某一项写错的，那一项跳过，别的照读 |
| `miyu: 警告：对话模型用的是扩展驱动 <包>/<驱动名>，litemiyu 认不了，交给它自己找模型` | 默认模型走软件包带的驱动 |

## 应急模式和平常有什么不同

`litemiyu --rescue` 是为修东西准备的：

- 什么都不写盘：不记会话、不改配置、不记输入历史。磁盘满了、目录只读也能用。关掉就没了。
- AI 只有五件工具：`read`、`write`、`edit`、`shell`、`jobs`。没有联网、子代理、记忆、技能、MCP。
- AI 没有关于 Miyu 的专门说明，要修什么、文件在哪，你在第一句话里说清楚。
- 没有权限级别可切，文件和命令都直接做，Miyu 的数据根也改得了。危险命令的认法同 Miyu（见 [权限和沙盒](permissions.md#要确认的命令)）：直接拒的那几类照样拒；要确认的那几类先问你，只能允许这一次或者拒绝，没有开关。
- 输入框上边框右边写着「应急 · 不落盘」。命令列表里没有 `/sessions`、`/config`、`/loop`、`/model`、`/effort`；`Ctrl+G` 的外部编辑器用不了。

## 一次修复

例子：你手改 `~/.miyu/etc/config.toml` 以后，敲 `miyu` 报「核心起不来：…」。

1. 敲：

   ```sh
   miyu rescue
   ```

   屏幕换成 LiteMiyu 的全屏界面，输入框上写着「应急 · 不落盘」。

2. 把情况告诉 AI，例如：

   ```text
   Miyu 的核心起不来。它的数据根是 ~/.miyu，系统配置是 ~/.miyu/etc/config.toml，核心日志是 ~/.miyu/var/log/core.log。
   看日志最后几十行找出原因，把配置改好，告诉我改了哪里。
   ```

3. AI 读日志、改文件，最后说改了什么。照着看一遍。
4. 按 `Ctrl+D`（输入框空着时）或敲 `/exit` 退出。
5. 敲 `miyu`，看核心起不起得来。

配置文件整份读不懂时，第 1 步先印「读不了 Miyu 的配置」的警告。LiteMiyu 自己也没配模型的话，它不进界面，印出缺什么和用法就退出（退出码 `2`），第一行是：

```text
litemiyu --rescue：没有能用的模型，缺 LITEMIYU_RESCUE_BASE_URL、LITEMIYU_RESCUE_MODEL。
```

这时自己把模型给它（地址、模型名换成你的供应商的，key 是假的）：

```sh
LITEMIYU_RESCUE_API_KEY=sk-example miyu rescue --base-url https://api.deepseek.com/v1 --model deepseek-chat
```

不给 `--api` 的按 `openai-chat`。没给 key 的照样进，界面上提示「没给 key（LITEMIYU_RESCUE_API_KEY），请求不带认证」。

想以后不用手敲：趁 Miyu 好着，跑一次 `litemiyu provider import`，把 Miyu 的供应商、key 和默认模型导进 LiteMiyu 自己的配置。

`miyu` 这个程序本身坏了、敲不了的，直接跑 `litemiyu --rescue`，模型照上面那样用环境变量和参数给。

## 别的系统

macOS、Windows 上没有 LiteMiyu。`miyu rescue` 找不到它，印：

```text
miyu: 未找到 litemiyu
提示：暂只支持 Linux
```

退出码 `1`。Linux 上找不到时，提示换成怎么装。

macOS、Windows 上核心坏了，自己照下面查：

1. 看核心日志的最后几十行，找出错的原因：

   ```bash
   tail -n 50 ~/.miyu/var/log/core.log
   ```

   ```powershell
   Get-Content -Tail 50 $HOME\.miyu\var\log\core.log
   ```

2. 手改坏了配置的，用编辑器打开 `~/.miyu/etc/config.toml`，把改坏的那几行改回去。核心起来时整份读不进来的配置照空的算，核心照样起；这时敲 `miyu check`，照它印的行号改。
3. 改好以后敲 `miyu core stop` 停掉在跑的核心，再敲 `miyu` 打开。

报错各是什么意思、怎么办，见 [出了问题怎么查](troubleshooting.md#核心起不来连不上)。

## LiteMiyu 平常怎么用

不带 `--rescue` 跑就是平常模式，会话、配置都存盘：

```sh
litemiyu            # 开对话
litemiyu -c         # 打开最近的会话
litemiyu config     # 进配置界面
litemiyu doctor     # 自检：配置、模型、目录、Miyu、终端
```

- 第一次跑先走一遍开头：接一家供应商（机器上有 Miyu 的，「从 Miyu 导入」排在最前）、选主对话的模型、选按键模式和明暗。
- 按键：`Enter` 发送，`Alt+Enter` 或 `Ctrl+J` 换行，`@` 补全文件路径，`Ctrl+C` 在回答时打断，`Ctrl+D` 在输入框空着时退出。敲 `/help` 看全部按键和命令。
- `litemiyu doctor` 有一项不好时退出码 `1`。它报找得到 Miyu、能导入几家供应商的，就能用 `litemiyu provider import`。
- 各条命令都有 `--help`，例如 `litemiyu provider --help`。

## 修不好的时候

带上 `~/.miyu/var/log/core.log` 的最后几十行，照 [出了问题怎么查](troubleshooting.md#报问题) 报问题。
