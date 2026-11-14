# 模型

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

Miyu 接上一家模型供应商才能对话。这一页讲怎么接、怎么换、key 放哪，和模型出错时 Miyu 怎么办。

## 最快：miyu setup

1. 敲 `miyu setup`。
2. 在「选择供应商：」下面敲一个序号。列的是常用的几家、本机跑着的模型服务（标「本机 <地址>」），最后一行「自定义」。配过的标「已配置」，环境变量里有 key 的标「已找到密钥」。
3. 贴 key。贴的时候不显示。找到了环境变量的不问。按 `Ctrl+C` 取消；Linux、macOS 上一个字都没敲时按 `Ctrl+D` 也是取消。
4. 等它试。通了印「已连通」；列不出模型的，让你填模型名。试不通的问「仍然保存？[y/N]」，不保存就回到上一步。
5. 在「选主对话的模型：」下面敲序号，直接回车用标「（推荐）」的那个。
6. 看到「已设置 models.chat = …」就好了。

常用的几家，照列出的先后：

| 名字 | 编号 |
|---|---|
| OpenCode Zen | `opencode` |
| OpenCode Go | `opencode-go` |
| ClinePass | `cline-pass` |
| DeepSeek | `deepseek` |
| Anthropic | `anthropic` |
| OpenAI | `openai` |
| OpenRouter | `openrouter` |
| Moonshot 国际版 | `moonshotai` |
| Moonshot 国内版 | `moonshotai-cn` |
| 智谱 GLM 国内版 | `zhipuai` |
| Z.AI 国际版 | `zai` |
| 通义千问 国际版 | `alibaba` |
| 通义千问 国内版 | `alibaba-cn` |
| MiniMax 国际版 | `minimax` |
| MiniMax 国内版 | `minimax-cn` |

不在表里的，用模型目录里的编号写 `--provider`。三个参数各跳过一步：`--provider <编号>` 不选供应商，`--env <变量>` 照核心环境里的这个变量取 key、不贴，`--model <模型>` 不选模型。不在终端里时必须写 `--provider`，key 从管道读：

```bash
echo "$KEY" | miyu setup --provider deepseek
```

```powershell
$key | miyu setup --provider deepseek
```

选「自定义」的，依次问 `Base URL：`、「接口协议：」（`OpenAI 兼容`、`Anthropic`、`OpenAI Responses`）、密钥（可以留空），试通以后问这一家的编号，直接回车用照地址起的那个。

`miyu setup` 写进数据根（默认 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`）的 `etc/config.toml`，即 `~/.miyu/etc/config.toml`，写了就生效：

| 你选的 | 写的 |
|---|---|
| 贴了 key | `providers.<编号>.key = { secret = "<编号>" }`，key 存进密钥文件，名字就是编号 |
| 环境变量里有 key | `providers.<编号>.key = { env = "<变量>" }` |
| 本机的服务 | `providers.<编号>.local = true` |
| 自定义 | `providers.<编号>.driver`、`providers.<编号>.base_url`，贴了 key 的再写 `key = { secret = "<编号>" }` |
| 已配置的一家 | 不动这一家 |

每种都另写 `models.chat = "<编号>/<模型>"`。还没配模型就敲 `miyu ask` 的，在终端里会先走一遍 `miyu setup`。

## 换默认模型

`models.chat` 是新会话默认用的模型，写成 `供应商/模型`，或者 `@池`：

```sh
miyu config set models.chat deepseek/deepseek-v4-pro
```

改了对新会话生效，开着的会话不变。

## key 放在哪

配置里写不了 key 本身，只能写引用：

- `{ secret = "名字" }`：到密钥文件 `~/.miyu/etc/private/secrets.toml` 里取。Linux、macOS 上这个文件只有你能读写（`0600`）；Windows 上 Miyu 不另设权限，照数据根继承的访问控制。三个平台都不用系统的钥匙串，key 照原文写在这个文件里。
- `{ env = "变量" }`：照核心起来时的环境变量取，不进任何文件。

用环境变量的，先在一个窗口里设好，再在这个窗口里跑 `miyu setup`。已经在跑的核心看不到后设的变量，先停掉它：

```bash
export DEEPSEEK_API_KEY='sk-假的'
miyu core stop
miyu setup --provider deepseek --env DEEPSEEK_API_KEY
```

```powershell
$env:DEEPSEEK_API_KEY = 'sk-假的'
miyu core stop
miyu setup --provider deepseek --env DEEPSEEK_API_KEY
```

这样设的变量只有这个窗口里拉起的核心看得到。`miyu service enable` 装的服务起的核心看不到你 shell 里的变量，要常驻的核心用得上 key，用密钥文件，见 [服务和环境变量](service.md#服务和环境变量)。

存、看、删密钥用 `miyu provider`：

| 命令 | 做什么 |
|---|---|
| `miyu provider login [<名字>]` | 存一个 key。不写名字的，从配置里用到的、存过的里面选，也能敲一个新名字。贴的时候不显示，也能从管道读 |
| `miyu provider list [--format text\|json]` | 列出存了的，和配置里用到了、还没存的（灰字），带谁在用。不显示 key |
| `miyu provider logout [<名字>]` | 删一个。不写名字的，从存过的里面选 |

名字小写字母开头，只有小写字母、数字、`-`、`_`，最长 64 个字符。`login` 只存 key，配置里还要有引用它的那一行：

```bash
echo "$KEY" | miyu provider login acme
```

```powershell
$key | miyu provider login acme
```

```toml
[providers.acme]
key = { secret = "acme" }
```

一家只有一个 key。几份额度要一起用的，配成几家，放进同一个池。

## 手写一家供应商

打开 `~/.miyu/etc/config.toml`（或者 `miyu config edit`），一家一张表 `[providers.<编号>]`。编号小写字母开头，只有小写字母、数字、`-`、`_`，最长 32 个字符。引用模型时写 `<编号>/<模型>`，模型名照供应商那边的叫法。

| 格 | 说明 |
|---|---|
| `base_url` | 接口地址，驱动把路径接在后面。认得出的供应商可以不写。也能写 `{ env = "变量" }` |
| `driver` | 驱动，见下表。认得出的供应商照它的；认不出的不写就是 `openai-chat` |
| `key` | `{ secret = … }` 或 `{ env = … }`。不写的不带认证头 |
| `name` | 界面上显示的名字。不写的照目录里那一家的名字，再没有的照编号 |
| `catalog` | 对应模型目录里的哪一家，例如只转 DeepSeek 的中转写 `deepseek` |
| `local` | 本机的服务，价格当 0。不写的照地址：`127.0.0.1`、`localhost`、`::1` 算本机 |
| `price_multiplier` | 价格倍率，不写是 1 |

其余的格见[配置键全表](config-keys.md)。

| 驱动 | 发到 | 认证头 |
|---|---|---|
| `openai-chat` | `<地址>/chat/completions` | `Authorization: Bearer <key>` |
| `anthropic` | `<地址>/messages` | `x-api-key: <key>` |
| `openai-responses` | `<地址>/responses` | `Authorization: Bearer <key>` |

模型不用一个个列。Miyu 向供应商要模型列表（`<地址>/models`），再加上目录里对上的那一家的、配置里写到的，合在一起给你挑。引用里的模型名不查，列表里没有的也能用。

一个 OpenAI 兼容的第三方服务：

```toml
[providers.acme]
name = "Acme"
base_url = "https://api.acme.example/v1"
driver = "openai-chat"                  # 可以不写：认不出的供应商默认就是它
key = { secret = "acme" }               # 用 miyu provider login acme 存

[models]
chat = "acme/acme-large"
```

本机的 Ollama：

```toml
[providers.ollama]
base_url = "http://127.0.0.1:11434/v1"  # 编号是 ollama 的，这一行和下一行可以不写
driver = "openai-chat"
local = true

[providers.ollama.models."qwen3:8b"]
window = 32768                          # 不知道窗口的模型，Miyu 不会提前压缩上下文

[models]
chat = "ollama/qwen3:8b"
```

模型名里有 `.`、`:`、`/` 的，表头里那一段加双引号。

供应商的列表、目录里缺的资料，在 `[providers.<编号>.models."<模型>"]` 里补，写了的压过别处的：

| 格 | 说明 |
|---|---|
| `window` | 上下文窗口，单位 token |
| `max_output` | 一次最多输出多少 token |
| `inputs` | 能收什么：`text`、`image`、`pdf`、`audio`、`video` |
| `effort` | 默认的思考程度，写它的一档，例如 `high`；能关思考的写 `off` |
| `temperature` | 默认的温度，0 到 2 |
| `price_multiplier` | 这个模型的倍率，压过供应商上写的 |
| `price.input`、`price.output`、`price.cache_read`、`price.cache_write`、`price.currency` | 每一百万 token 的价，币种不写是 `USD`。写了一格就整份用手写的 |

这些键只认系统配置，项目配置里写不了。供应商这几格改了，下一轮生效。

## 三个用途

| 键 | 管什么 | 能写 |
|---|---|---|
| `models.chat` | 新会话默认用的模型 | `供应商/模型`、`@池` |
| `models.vision` | 对话的模型看不了图时，替它看图 | `供应商/模型`、`@池` |
| `models.embedding` | 算向量，用来找记忆、知识库 | `local`（默认，本机的 `miyu-embed`）、`off`（只照关键词找）、`供应商/模型` |

```toml
[models]
chat = "deepseek/deepseek-v4-flash"
vision = "acme/acme-vision"
embedding = "local"
```

## 池：便宜的干杂活，贵的干正事

池是几个模型编成一组，引用时写 `@池名`。有三个池一直在、删不掉，开头是空的：

| 池 | 界面上叫 | 谁用 |
|---|---|---|
| `lite` | 轻量池 | 起会话标题 |
| `normal` | 日常池 | 整理记忆 |
| `flagship` | 旗舰池 | `advisor`：AI 卡住时向更强的模型求助 |

`lite`、`normal` 是空的，或者成员都在冷却、都出错了，就用会话自己的模型。`flagship` 是空的，`advisor` 答「旗舰池里没有模型」。

往池里放模型，写 `pools.<池名>.models`。成员只能是 `供应商/模型`，照写的先后排。也能自己建池：

```toml
[pools.lite]
models = ["deepseek/deepseek-v4-flash"]

[pools.flagship]
models = ["deepseek/deepseek-v4-pro"]

[pools.daily]
models = ["deepseek/deepseek-v4-flash", "acme/acme-large"]
strategy = "ordered"
subagent = true
description = "Everyday work, cheap"

[models]
chat = "@daily"
```

`strategy` 是分法：

| 值 | 终端里叫 | 怎么分 |
|---|---|---|
| `pin` | 按会话固定 | 一个会话一直用一个成员，新会话轮到下一个 |
| `rotate` | 每次请求轮换 | 每次请求换下一个成员 |
| `ordered` | 不可用时回退 | 每次请求都从第一个排起，第一个出错才用后面的 |

不写时是 `pin`；成员所在的供应商全写了 `cache = "per_request"` 的，是 `rotate`。

`subagent = true` 的池，AI 派子代理时能选它，`description` 是 AI 看到的一句说明，用英文写。三个固定池默认开着，自己建的默认关着。改了对新会话生效。

网页里，池在设置「模型」页的「模型池」分页，一个池一张卡片，最下面一行是「子代理能选」开关。点卡片下面的「＋ 新建模型池」，写名字，名字下面就有「子代理能选」开关（默认关），回车建好。改了当场存进 `~/.miyu/etc/config.toml`。

终端里，设置页「供应商和模型」的「模型池」页打开一个池或者新建一个，「调用方式」下面一格「子代理能选」，选「开」或「关」；新建的停在「关」。

## 临时换模型

- 命令行：`miyu ask --model deepseek/deepseek-v4-pro "…"`，也能写 `--model @flagship`。和 `-c`、`-s` 一起用的，那个会话换过去，以后一直用它。
- 终端：敲 `/model`，在「模型」框里选一个，回车。下一轮开始生效。在冷却的、没设 key 的选不了。`/config` 打开设置页，进「供应商和模型」，里面有「供应商」「默认模型」「模型池」三页；`/connect` 直接进「供应商和模型」。
- 网页：点输入框底栏上的模型名，菜单里分「模型」「模型池」两页。也能敲 `/model`，后面直接写 `供应商/模型` 或 `@池` 就不开菜单。`/config` 打开设置的「模型」页。

## 模型目录

窗口、价格、能收什么、认得出的供应商的地址和驱动，都照 [models.dev](https://models.dev) 的目录。装好就带一份，核心在后台拉新的：

| 键 | 默认 | 说明 |
|---|---|---|
| `models.catalog.update` | `true` | 关掉只用装好时带的和已经拉过的。环境变量 `MIYU_CATALOG_UPDATE=false` 压过它 |
| `models.catalog.url` | `https://models.dev/api.json` | 从哪拉。也能写 `{ env = "变量" }` |
| `models.catalog.every` | `24h` | 手上那份旧过这么久才再拉，1 小时到 30 天 |

## 限流、额度用完时

`models.chat` 写的是一个模型的：

- 限流、连不上、5xx、卡住：同一步最多再试 5 次。供应商说了要等多久的照它等，要等 2 分钟以上的不等，这一轮报错结束；没说的等 1、2、4、8、16 秒。
- 认证失败、额度用完：不再试，这一轮报错结束。

写的是池的，出错的那个成员记冷却，马上换下一个不在冷却的成员。终端里会出一行「↻ 已切换模型：A → B」。成员都在冷却的，等最早恢复的那个。`pin` 的池换过去以后，这个会话接着用换到的那个。

冷却第 n 次连着失败是 `base × 2^(n−1)`，最多 `max`；成了一次就清零。供应商说了要等更久的照它，也不超过 `max`。冷却只在内存里，核心重启就清掉。

| 出了什么错 | 键 | 默认 `base` / `max` |
|---|---|---|
| 限流 | `models.cooldown.rate_limited.base`、`.max` | `30s` / `10m` |
| 连不上、5xx、卡住 | `models.cooldown.retryable.base`、`.max` | `10s` / `5m` |
| 认证失败 | `models.cooldown.auth.base`、`.max` | `10m` / `2h` |
| 额度用完 | `models.cooldown.quota.base`、`.max` | `30m` / `30m` |

额度用完、供应商说了什么时候重置的，冷却到那时，最多一天，不看 `max`。冷却只停这一家的这个模型，同一家别的模型照用。

## 用量和花费

- 终端：`/usage` 看这台机器上的用量和花费，按天、按模型；`/context` 看这个会话的，按用途分。
- 网页：设置里的「用量」页，能按模型、会话、用途、场所看。

金额照 `usage.currency`（默认 `USD`）显示，按每一笔那天的汇率换算，取不到汇率的不显示金额。价格照目录；本机的服务算 0；中转站加价、打折的写 `price_multiplier`。

## 出了问题

| 看到的 | 怎么办 |
|---|---|
| 「未配置模型」、`no model configured: set models.chat` | 跑 `miyu setup`，或者写 `models.chat` |
| `miyu setup` 说「核心未读到 XXX_API_KEY（在核心启动后设置）」 | 核心比这个变量先起来。`miyu core stop` 以后再跑 `miyu setup`，或者直接贴 key |
| `provider "acme" has no usable key` | 密钥没存，或者核心的环境里没有那个变量。`miyu provider list` 看哪个没设 |
| `provider "acme" needs base_url: it matches nothing in the catalog` | 目录里认不出这一家，写 `base_url` |
| 「未找到：请检查端点地址和模型名」 | `base_url` 要写到驱动接的路径前面那一截，模型名要和供应商那边的一样 |

`miyu check` 查配置写没写错。
