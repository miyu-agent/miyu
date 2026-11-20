# 模型

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

Miyu 要接上一家模型供应商才能对话。这一页讲怎么接入供应商、怎么更换模型、密钥存放在哪里，以及模型出错时 Miyu 会怎么处理。

## 快速配置：miyu setup

`miyu setup` 是配置模型最快的方法，它会一步一步带你选好供应商、填好密钥、选好模型：

1. 运行 `miyu setup`。
2. 在 `选择供应商：` 下面输入一个序号。列表里有常用的几家供应商、本机正在运行的模型服务（标着 `本机 <地址>`），最后一行是 `自定义`。已经配置过的供应商标着 `已配置`，环境变量里有密钥的供应商标着 `已找到密钥`。
3. 粘贴密钥，粘贴时屏幕上不会显示。如果已经在环境变量里找到了密钥，就不会再问。按 `Ctrl+C` 可以取消；在 Linux、macOS 上，还没有输入任何字时按 `Ctrl+D` 也可以取消。
4. 等待 Miyu 测试连接。连接成功会显示 `已连通`；如果列不出模型，会让你填写模型名。如果测试不通，会问 `仍然保存？[y/N]`，选择不保存就回到上一步。
5. 在 `选主对话的模型：` 下面输入序号，直接回车会使用标着 `（推荐）` 的那个模型。
6. 看到 `已设置 models.chat = …` 就配置完成了。

常用的几家供应商如下，按列表里的先后排列：

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

如果要用的供应商不在表里，就把模型目录里它的编号写在 `--provider` 后面。有三个参数可以各跳过一步：`--provider <编号>` 跳过选择供应商；`--env <变量>` 从核心环境里的这个变量读取密钥，不需要粘贴；`--model <模型>` 跳过选择模型。不在终端里运行时，必须写 `--provider`，密钥从管道读取：

```bash
echo "$KEY" | miyu setup --provider deepseek
```

```powershell
$key | miyu setup --provider deepseek
```

如果选择 `自定义`，Miyu 会依次询问 `Base URL：`、`接口协议：`（`OpenAI 兼容`、`Anthropic`、`OpenAI Responses`）和密钥（密钥可以留空）。测试连通以后，会询问这一家供应商的编号，直接回车会使用根据地址起的那个编号。

`miyu setup` 会把配置写进数据根（默认是 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`）下的 `etc/config.toml`，也就是 `~/.miyu/etc/config.toml`，写入后立即生效。不同的选择写入的内容如下：

| 你选的 | 写的 |
|---|---|
| 贴了密钥 | `providers.<编号>.key = { secret = "<编号>" }`，密钥存进密钥文件，名字就是编号 |
| 环境变量里有密钥 | `providers.<编号>.key = { env = "<变量>" }` |
| 本机的服务 | `providers.<编号>.local = true` |
| 自定义 | `providers.<编号>.driver`、`providers.<编号>.base_url`，如果贴了密钥，还会写 `key = { secret = "<编号>" }` |
| 已配置的一家 | 不修改这一家的配置 |

每一种情况都会另外写一行 `models.chat = "<编号>/<模型>"`。如果还没有配置模型就运行 `miyu ask`，在终端里会先进入一遍 `miyu setup`。

## 更换默认模型

`models.chat` 是新会话默认使用的模型，写成 `供应商/模型` 或者 `@池`。例如：

```sh
miyu config set models.chat deepseek/deepseek-v4-pro
```

修改后对新会话生效，已经打开的会话不会改变。

## 存放密钥

配置文件里不能直接写密钥本身，只能写一个引用。引用有两种写法：

- `{ secret = "名字" }`：从密钥文件 `~/.miyu/etc/private/secrets.toml` 里读取。在 Linux、macOS 上，这个文件只有你能读写（`0600`）；在 Windows 上，Miyu 不另外设置权限，文件继承数据根的访问控制。三个平台都不使用系统的钥匙串，密钥以原文写在这个文件里。
- `{ env = "变量" }`：从核心启动时的环境变量里读取，不会写进任何文件。

如果使用环境变量，先在一个终端窗口里设置好变量，再在同一个窗口里运行 `miyu setup`。已经在运行的核心看不到之后才设置的变量，所以要先停掉它：

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

这样设置的变量，只有从这个窗口里启动的核心才看得到。由 `miyu service enable` 安装的服务启动的核心看不到你 shell 里的变量；如果要让常驻的核心用上密钥，就使用密钥文件，见 [服务和环境变量](service.md#服务和环境变量)。

保存、查看、删除密钥使用 `miyu provider`：

| 命令 | 做什么 |
|---|---|
| `miyu provider login [<名字>]` | 保存一个密钥。如果不写名字，可以从配置里用到的名字和已经保存过的名字里选择，也可以输入一个新名字。粘贴时不会显示，也可以从管道读取 |
| `miyu provider list [--format text\|json]` | 列出已经保存的密钥，以及配置里用到了但还没有保存的密钥（灰色显示），同时显示谁在使用它们。不会显示密钥本身 |
| `miyu provider logout [<名字>]` | 删除一个密钥。如果不写名字，可以从已经保存的密钥里选择 |

名字以小写字母开头，只包含小写字母、数字、`-`、`_`，最长 64 个字符。`login` 只负责保存密钥，配置里还需要有引用它的那一行。例如：

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

一家供应商只有一个密钥。如果要把几份额度一起使用，就把它们配置成几家供应商，再放进同一个池。

## 手写一家供应商

如果要手动配置供应商，打开 `~/.miyu/etc/config.toml`（或者运行 `miyu config edit`），一家供应商写一张表 `[providers.<编号>]`。编号以小写字母开头，只包含小写字母、数字、`-`、`_`，最长 32 个字符。引用模型时写成 `<编号>/<模型>`，模型名使用供应商那边的叫法。

| 格 | 说明 |
|---|---|
| `base_url` | 接口地址，驱动会把路径接在它后面。认得出的供应商可以不写。也可以写成 `{ env = "变量" }` |
| `driver` | 使用的驱动，见下一张表。认得出的供应商使用它自己的驱动；认不出的供应商不写时就是 `openai-chat` |
| `key` | `{ secret = … }` 或 `{ env = … }`。没有写时，请求不带认证头 |
| `name` | 界面上显示的名字。没有写时使用模型目录里那一家的名字，目录里也没有时使用编号 |
| `catalog` | 对应模型目录里的哪一家，例如只转发 DeepSeek 的中转服务写 `deepseek` |
| `local` | 表示这是本机的服务，价格按 0 计算。没有写时根据地址判断：`127.0.0.1`、`localhost`、`::1` 算作本机 |
| `price_multiplier` | 价格倍率，没有写时是 1 |

其余的格见[配置键全表](config-keys.md)。

| 驱动 | 发到 | 认证头 |
|---|---|---|
| `openai-chat` | `<地址>/chat/completions` | `Authorization: Bearer <key>` |
| `anthropic` | `<地址>/messages` | `x-api-key: <key>` |
| `openai-responses` | `<地址>/responses` | `Authorization: Bearer <key>` |

DeepSeek 官方的供应商默认使用 `openai-chat`。如果想使用 DeepSeek 自己的联网搜索，需要把它的 `driver` 改成 `openai-responses`，见 [联网搜索和抓取网页](web-search.md#使用-deepseek-responses-接口内置的搜索)。

软件包也可以带驱动，这时 `driver` 写成 `<包编号>/<驱动名>`。例如社区的软件包 `antigravity`（只支持 Linux、macOS）带有驱动 `antigravity/antigravity`，它把请求交给本机的 Antigravity CLI，使用你在 Antigravity CLI 里的登录；安装和配置的方法见这个软件包自己的说明（`miyu pkg info antigravity` 输出里 `上游` 那一行）。想自己写一个驱动，可以看[写一个模型驱动](../dev/extensions.md#写一个模型驱动)。

模型不需要一个个列出来。Miyu 会向供应商请求模型列表（`<地址>/models`），再加上模型目录里对应那一家的模型和配置里写到的模型，合在一起给你选择。引用里的模型名不会被检查，所以列表里没有的模型也可以使用。

一个 OpenAI 兼容的第三方服务可以这样写：

```toml
[providers.acme]
name = "Acme"
base_url = "https://api.acme.example/v1"
driver = "openai-chat"                  # 可以不写：认不出的供应商默认就是它
key = { secret = "acme" }               # 用 miyu provider login acme 保存

[models]
chat = "acme/acme-large"
```

本机的 Ollama 可以这样写：

```toml
[providers.ollama]
base_url = "http://127.0.0.1:11434/v1"  # 编号是 ollama 时，这一行和下一行可以不写
driver = "openai-chat"
local = true

[providers.ollama.models."qwen3:8b"]
window = 32768                          # 不知道窗口大小的模型，Miyu 不会提前压缩上下文

[models]
chat = "ollama/qwen3:8b"
```

如果模型名里有 `.`、`:`、`/`，表头里的那一段要加上双引号。

供应商的模型列表和模型目录里缺少的资料，可以在 `[providers.<编号>.models."<模型>"]` 里补充，这里写了的值会覆盖其他来源的值：

| 格 | 说明 |
|---|---|
| `window` | 上下文窗口的大小，单位是 token |
| `max_output` | 一次最多输出多少 token |
| `inputs` | 能接收哪些输入：`text`、`image`、`pdf`、`audio`、`video` |
| `effort` | 默认的思考程度，写这个模型的一档，例如 `high`；能关闭思考的模型可以写 `off` |
| `temperature` | 默认的温度，范围是 0 到 2 |
| `price_multiplier` | 这个模型的价格倍率，会覆盖供应商上写的倍率 |
| `price.input`、`price.output`、`price.cache_read`、`price.cache_write`、`price.currency` | 每一百万 token 的价格，没有写币种时是 `USD`。价格里只要写了一项，就整份使用你手写的价格 |

这些键只在系统配置里有效，不能写进项目配置。供应商的这几格修改后，从下一轮开始生效。

## 三个用途

Miyu 在三种用途上使用模型，每种用途由一个键决定：

| 键 | 管什么 | 能写 |
|---|---|---|
| `models.chat` | 新会话默认使用的模型 | `供应商/模型`、`@池` |
| `models.vision` | 对话的模型看不了图片时，由这个模型替它看图 | `供应商/模型`、`@池` |
| `models.embedding` | 计算向量，用来查找记忆和知识库 | `local`（默认，本机的 `miyu-embed`）、`off`（只按关键词查找）、`供应商/模型` |

```toml
[models]
chat = "deepseek/deepseek-v4-flash"
vision = "acme/acme-vision"
embedding = "local"
```

## 池：便宜的干杂活，贵的干正事

池是把几个模型编成一组，引用时写 `@池名`。有三个池一直存在、不能删除，它们一开始是空的：

| 池 | 界面上叫 | 谁用 |
|---|---|---|
| `lite` | 轻量池 | 起会话标题 |
| `normal` | 日常池 | 整理记忆 |
| `flagship` | 旗舰池 | `advisor`：AI 卡住时向更强的模型求助 |

如果 `lite`、`normal` 是空的，或者成员都在冷却、都出错了，就使用会话自己的模型。如果 `flagship` 是空的，`advisor` 会回答 `旗舰池里没有模型`。

往池里放模型，需要写 `pools.<池名>.models`。成员只能写成 `供应商/模型`，按写的先后排列。你也可以自己建池，例如：

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

`strategy` 决定请求怎么在成员之间分配：

| 值 | 终端里叫 | 怎么分 |
|---|---|---|
| `pin` | 按会话固定 | 一个会话一直使用同一个成员，下一个新会话轮到下一个成员 |
| `rotate` | 每次请求轮换 | 每次请求都换到下一个成员 |
| `ordered` | 不可用时回退 | 每次请求都从第一个成员开始，第一个出错时才使用后面的成员 |

没有写时是 `pin`；如果成员所在的供应商全部写了 `cache = "per_request"`，就是 `rotate`。

`subagent = true` 的池，AI 派子代理时可以选择它；`description` 是 AI 看到的一句说明，要用英文写。三个固定的池默认打开这一项，自己建的池默认关闭。修改后对新会话生效。

在网页界面里，池在设置的 `模型` 页的 `模型池` 分页，一个池一张卡片，卡片最下面一行是 `子代理可用` 开关。点击卡片下面的 `＋ 新建模型池`，填写名字，名字下面会出现 `子代理可用` 开关（默认关闭），回车即可建好。修改会立即保存进 `~/.miyu/etc/config.toml`。

在终端界面里，进入设置页 `供应商和模型` 的 `模型池` 页，打开一个池或者新建一个池，`调用方式` 下面的一格是 `子代理可用`，选择 `开` 或 `关`；新建的池默认停在 `关`。

## 临时更换模型

如果只想临时换一个模型，可以使用下面几种方式：

- 命令行：`miyu ask --model deepseek/deepseek-v4-pro "…"`，也可以写 `--model @flagship`。如果和 `-c`、`-s` 一起使用，那个会话会换到这个模型，以后一直使用它。
- 终端界面：输入 `/model`，在 `模型` 框里选择一个模型，回车，从下一轮开始生效。正在冷却的模型和没有设置密钥的模型不能选择。`/config` 会打开设置页，进入 `供应商和模型` 后，里面有 `供应商`、`默认模型`、`模型池` 三页；`/connect` 会直接进入 `供应商和模型`。
- 网页界面：点击输入框底栏上的模型名，菜单里分为 `模型`、`模型池` 两页。也可以输入 `/model`，后面直接写 `供应商/模型` 或 `@池`，这样就不会打开菜单。`/config` 会打开设置的 `模型` 页。

## 模型目录

模型的窗口、价格、能接收的输入，以及认得出的供应商的地址和驱动，都来自 [models.dev](https://models.dev) 的目录。安装时就带有一份目录，核心会在后台拉取新的目录：

| 键 | 默认 | 说明 |
|---|---|---|
| `models.catalog.update` | `true` | 关掉以后，只使用安装时带的目录和已经拉取过的目录。环境变量 `MIYU_CATALOG_UPDATE=false` 的优先级高于它 |
| `models.catalog.url` | `https://models.dev/api.json` | 从哪个地址拉取目录。也可以写成 `{ env = "变量" }` |
| `models.catalog.every` | `24h` | 手上的目录旧过这个时间才会再次拉取，范围是 1 小时到 30 天 |

## 限流、额度用完时

模型出错时，Miyu 的处理方式取决于 `models.chat` 写的是单个模型还是池。如果 `models.chat` 写的是一个模型：

- 限流、连接不上、5xx、卡住：同一步最多再重试 5 次。如果供应商说明了要等多久，就按它说的时间等待，但是要等 2 分钟以上时不再等待，这一轮报错结束；如果供应商没有说明，就依次等待 1、2、4、8、16 秒。
- 认证失败、额度用完：不再重试，这一轮报错结束。

如果写的是池，出错的那个成员会进入冷却，Miyu 马上换到下一个不在冷却的成员。终端界面里会显示一行 `↻ 已切换模型：A → B`。如果成员都在冷却，就等待最早恢复的那个成员。`pin` 的池换过去以后，这个会话会继续使用换到的那个成员。

连续第 n 次失败时，冷却时间是 `base × 2^(n−1)`，最多是 `max`；成功一次就清零。如果供应商要求等待更久，就按供应商的要求，但也不会超过 `max`。冷却只记在内存里，核心重启后就会清掉。

| 出了什么错 | 键 | 默认 `base` / `max` |
|---|---|---|
| 限流 | `models.cooldown.rate_limited.base`、`.max` | `30s` / `10m` |
| 连不上、5xx、卡住 | `models.cooldown.retryable.base`、`.max` | `10s` / `5m` |
| 认证失败 | `models.cooldown.auth.base`、`.max` | `10m` / `2h` |
| 额度用完 | `models.cooldown.quota.base`、`.max` | `30m` / `30m` |

如果额度用完、并且供应商说明了什么时候重置，冷却会持续到那个时候，最多一天，这种情况不受 `max` 限制。冷却只停用这一家供应商的这个模型，同一家的其他模型照常使用。

## 用量和花费

可以在下面两个地方查看用量和花费：

- 终端界面：`/usage` 查看这台电脑上的用量和花费，按天、按模型分；`/context` 查看当前会话的用量，按用途分。
- 网页界面：设置里的 `用量` 页，可以按模型、会话、用途、场所查看。

金额按 `usage.currency`（默认 `USD`）显示，每一笔按当天的汇率换算，取不到汇率时不显示金额。价格来自模型目录；本机的服务按 0 计算；如果中转站加价或者打折，就写 `price_multiplier`。

## 排查问题

下面是配置模型时常见的问题和解决方法：

| 看到的 | 怎么办 |
|---|---|
| `未配置模型`、`no model configured: set models.chat` | 运行 `miyu setup`，或者写上 `models.chat` |
| `miyu setup` 提示 `核心未读到 XXX_API_KEY（在核心启动后设置）` | 核心比这个变量先启动了。运行 `miyu core stop` 以后再运行 `miyu setup`，或者直接粘贴密钥 |
| `provider "acme" has no usable key` | 密钥没有保存，或者核心的环境里没有那个变量。运行 `miyu provider list` 查看哪个没有设置 |
| `provider "acme" needs base_url: it matches nothing in the catalog` | 模型目录里认不出这一家，需要写 `base_url` |
| `未找到，请检查端点地址和模型名` | `base_url` 要写到驱动拼接的路径之前的那一部分，模型名要和供应商那边的一样 |

运行 `miyu check` 可以检查配置有没有写错。
