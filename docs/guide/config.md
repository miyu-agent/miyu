# 配置

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

Miyu 的设置都写在 TOML 文件里。这一页讲文件在哪、怎么改、改了什么时候生效、给一个仓库单独设、存密钥，和一项设置为什么没生效。每个键是什么、能写什么，查[配置键全表](config-keys.md)。

## 文件在哪

| 文件 | 位置 | 谁来写 |
|---|---|---|
| 系统配置 | 数据根（默认 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`）下的 `etc/config.toml`，即 `~/.miyu/etc/config.toml` | 你手改，`miyu config set`，终端、网页的设置页 |
| 项目配置 | 仓库里的 `.miyu/config.toml` | 只能手改 |
| 密钥文件 | `~/.miyu/etc/private/secrets.toml` | `miyu provider login`，也能手改 |

`miyu config path` 印系统配置的完整路径，`miyu config path --project` 印当前目录的项目配置的。文件还没有的，照样印出它该在的位置，再说一句「文件尚不存在」。

一份系统配置的样子：

```toml
#:schema ../var/cache/config/config.schema.json

[ui]
language = "zh"

[models]
chat = "deepseek/deepseek-v4-flash"

[providers.deepseek]
key = { secret = "deepseek" }

[permission.confirm]
timeout = 600

[jobs]
max_running = "45m"

[sandbox.private]
add = ["~/notes/keys"]
```

键照 `.` 分段，前几段是表头，最后一段写在表里：`permission.confirm.timeout` 就是上面 `[permission.confirm]` 下的 `timeout`。时长写成带引号的字，数字后面跟 `s`、`m`、`h`，不跟的是秒。文件最大 1 MiB。

## 改一个设置

三种办法改的是同一份文件 `~/.miyu/etc/config.toml`：

- 命令行：`miyu config set <键> <值>`。
- 设置页：终端、网页里敲 `/settings`。在终端里直接敲 `miyu config`，打开默认界面（`ui.head`）的设置页。
- 手改文件，或者 `miyu config edit`。

命令行和设置页只动那一项，注释、空行、先后、不认识的键都照原样留着。手改的文件一存盘，核心就重新读，不用重启。

1. 改：

   ```sh
   miyu config set permission.confirm.timeout 600
   ```

   标准错误上印一行「已设置 permission.confirm.timeout = 600（下一轮生效）」。值本来就是这个的，印「已经是 600，未修改」。

2. 看现在用的值：

   ```sh
   miyu config get permission.confirm.timeout
   ```

3. 看它从哪来：`miyu config explain permission.confirm.timeout`（见下面「设置为什么没生效」）。

不要了：`miyu config unset permission.confirm.timeout`，印「已删除 permission.confirm.timeout，现在是 300（默认值）」。

### 什么时候生效

每个键登记了一种生效时机，`miyu config set`、`miyu config explain` 都会说：

| 印的 | 意思 | 例如 |
|---|---|---|
| 立即生效 | 改了马上照新的 | `ui.language`、`log.level`、`core.idle_exit`、模型的冷却 |
| 下一轮生效 | 开着的会话从下一轮起照新的，正在跑的这一轮照旧 | 供应商的地址和 key、受保护的命令、私密目录、压缩 |
| 新会话生效 | 以后开的会话照新的，开着的不变 | `models.chat`、`persona.default`、`permission.start_read_only` |
| 重启后生效 | 头（终端、网页）下次启动时才读 | `ui.startup`，软件包里没写生效时机的键 |
| 重启核心后生效 | 核心下次起来时才读 | `core.workers` |

核心怎么停、怎么再起来，见[常驻和服务器](service.md)。

### 命令行上怎么写值

| 类型 | 写法 | 例子 |
|---|---|---|
| 开关 | `true`、`false` | `miyu config set compaction.prepare false` |
| 数 | 照写 | `miyu config set compaction.line 70` |
| 字、选项、模型、时长 | 照写，可以加双引号 | `miyu config set jobs.max_running 45m` |
| 列表 | TOML 的数组，整个用单引号括起来；写的是整份，原来的被换掉 | `miyu config set skills.external.dirs '["~/.agents/skills", "~/my-skills"]'` |
| 密钥引用 | TOML 的行内表 | `miyu config set providers.acme.key '{ secret = "acme" }'` |
| 每项几个字段的列表 | 行内表的数组 | `miyu config set tui.footer '[{ part = "usage" }, { part = "model", hidden = true }]'` |

列表、行内表照 TOML 读，TOML 的字符串用双引号、单引号都行。PowerShell 里外面用双引号，里面的字用 TOML 的单引号字符串：Windows PowerShell 5.1 把参数交给程序时会吃掉里面的双引号，这样写 5.1 和 PowerShell 7 都能照抄：

```powershell
miyu config set skills.external.dirs "['~/.agents/skills', '~/my-skills']"
miyu config set providers.acme.key "{ secret = 'acme' }"
miyu config set tui.footer "[{ part = 'usage' }, { part = 'model', hidden = true }]"
```

键里有一段带 `.`、`:` 这类字的，那一段加双引号，整个键再用单引号括起来：

```bash
miyu config set 'providers.ollama.models."qwen3:8b".window' 32768
```

这种键在 Windows PowerShell 5.1 里双引号同样会被吃掉，改用 `miyu config edit` 手改。

`miyu config set` 一次改一个键。整张表（例如一个 MCP 服务 `[mcp.<名字>]`）要手改文件。

## miyu config 的全部命令

| 命令 | 做什么 | 例子 |
|---|---|---|
| `miyu config` | 在终端里打开默认界面的设置页；不在终端里印帮助 | `miyu config` |
| `get [<键>…]` | 印最终值。只给一个键的只印值；给几个、不给的，一行一个 `键 = 值` | `miyu config get models.chat` |
| `set <键> <值>` | 改系统配置里的一项 | `miyu config set ui.language zh` |
| `unset <键>` | 从系统配置里删掉一项，回到默认值 | `miyu config unset ui.language` |
| `edit` | 用编辑器打开系统配置，存之前先查 | `miyu config edit` |
| `explain <键>` | 这一项每一层写的什么、哪一层生效 | `miyu config explain log.level` |
| `path` | 印系统配置的完整路径 | `miyu config path` |
| `trust` | 看当前目录的项目配置会改什么，决定信不信任 | `miyu config trust` |

| 选项 | 用在 | 做什么 |
|---|---|---|
| `--project` | `edit`、`path` | 换成当前目录的项目配置 |
| `--format <text\|json>` | `get`、`explain` | 输出格式，默认 `text` |
| `--yes` | `trust` | 信任，不问 |
| `--no` | `trust` | 不信任，不问 |

`set`、`unset` 只改系统配置。`set` 写了 `--project` 的不改，报「项目配置只能手动编辑」，退出码 `2`。

`miyu config edit` 用 `VISUAL` 里的编辑器，没有的用 `EDITOR`，都没有（或者是空的）的 Linux、macOS 上用 `vi`，Windows 上用 `notepad`。编辑器那一串在 Linux、macOS 上经 `sh -c` 跑，在 Windows 上经 `cmd /c` 跑，所以带参数的写法两边都行，例如 `code --wait`：

```bash
export VISUAL='code --wait'
```

```powershell
$env:VISUAL = 'code --wait'
```

它在原文件旁边放一份副本给你改，关掉编辑器以后：

- 没改：印「未修改」。
- 有错：一处一行印出来，问「有 N 处错误，未保存。按回车继续编辑，输入 q 放弃：」。只有警告的照样存。
- 你改的时候文件被别处改了：不存，印「编辑期间文件已被修改，未保存」，副本留着，说它在哪。

`edit` 要在终端里跑。

## 给一个仓库单独设

项目配置是仓库里的 `.miyu/config.toml`，只对在这个仓库里开的会话起作用，只能把限制收紧。现在能写的只有一项：

```toml
[permission]
start_read_only = true          # 在这个仓库里开的新会话一开始就是只读
```

写 `false` 不算。别的键写进项目配置也不算，`miyu config explain <键>` 里那一行标「← 不算：不能写在这一层」。

1. 在仓库里敲 `miyu config edit --project`，或者直接建这个文件。还没有这个文件的，建在有 `.git` 的那一层下面；往上都没有 `.git` 的，建在当前目录下面。
2. 在仓库里敲 `miyu config trust`。它列出「<文件> 会修改以下设置（项目配置只能收紧限制）：」和每一项，问「信任 '<文件>'？[y/N]」。
3. 敲 `y`。印「已信任（文件变化后会再次询问）」。

没信任的项目配置不起作用。`miyu ask` 在这样的仓库里开头印「项目配置 <文件> 尚未信任，本次没有使用」。终端、网页里不问，要到命令行里敲 `miyu config trust`。文件内容一变，要重新信任。不在终端里跑的，写 `--yes` 或 `--no`。答了不信任的，以后不再提醒，直到文件变了。

Miyu 从会话的工作目录往上一层层找 `.miyu/config.toml`，只认最近的那一份。到有 `.git` 的那一层就不再往上，家目录本身和它的上级不找，数据根里面也不找。

## 叠加顺序

一个键的最终值从下往上叠，上面的压下面的：

1. 默认值。
2. 系统配置。
3. 项目配置：信任过，又是收紧的，才算。
4. 环境变量：只有 `log.level`（`MIYU_LOG`）和 `models.catalog.update`（`MIYU_CATALOG_UPDATE`）认，只管这一次启动的核心。

`miyu config set` 改了、上面一层又压着的，多印一行警告，例如「log.level 被环境变量覆盖，使用 "debug"」。

## 密钥

配置里不写 key 本身，只写引用：

- `{ secret = "名字" }`：到密钥文件 `~/.miyu/etc/private/secrets.toml` 里取。Linux、macOS 上 Miyu 每次写它都把权限设成 `0600`，只有你能读写；Windows 上不另设权限，照数据根继承的访问控制。三个平台都不用系统的钥匙串，key 照原文写在这个文件里。
- `{ env = "变量" }`：照核心起来时的环境变量取，不进任何文件。用服务起的核心看不到你 shell 里的变量，见[常驻和服务器](service.md)。

存、看、删密钥都用 `miyu provider`。它不只管模型供应商的 key，什么名字的密钥都能存：

| 命令 | 做什么 |
|---|---|
| `miyu provider login [<名字>]` | 存一个。贴的时候不显示，也能从管道读。不写名字的，从配置里用到的、存过的里面选 |
| `miyu provider list [--format text\|json]` | 列出存了的，和配置里用到了、还没存的，带谁在用。不显示值 |
| `miyu provider logout [<名字>]` | 删一个 |

名字小写字母开头，只有小写字母、数字、`-`、`_`，最长 64 个字符。例如给联网搜索存 Tavily 的 key：

```bash
echo "$TAVILY_KEY" | miyu provider login tavily
miyu config set net.search.tavily.key '{ secret = "tavily" }'
```

```powershell
$tavilyKey | miyu provider login tavily
miyu config set net.search.tavily.key "{ secret = 'tavily' }"
```

密钥文件也能手改，一行一个 `名字 = "值"`，存盘就生效。

能写引用的键：

| 键 | `{ secret }` | `{ env }` |
|---|---|---|
| `providers.<编号>.key`，`net.search.exa.key`、`net.search.deepseek.key`、`net.search.tavily.key`、`net.search.brave.key`，软件包声明成密钥的键 | 能 | 能 |
| `providers.<编号>.base_url`、`models.catalog.url`、`net.search.searxng.url`、`packages.source`、`packages.sources.<编号>.url` | 不能 | 能 |
| MCP 服务的 `env`、`bearer`、`headers` 里的值 | 能 | 能 |

密钥类型的键写成明文，报类型不对，这一项不算。引用的密钥还没存、环境变量没设的，只报警告，用到时这一项没有 key。

## 环境变量

你能设的：

| 变量 | 做什么 |
|---|---|
| `MIYU_HOME` | 数据根放在哪，要写绝对路径，开头的 `~` 照家目录换开。不设是 `~/.miyu` |
| `MIYU_LOG` | 这一次启动的核心的运行日志级别，压过 `log.level`：`error`、`warn`、`info`、`debug`、`trace`、`off` |
| `MIYU_CATALOG_UPDATE` | 写 `false` 时这一次启动的核心不在后台拉模型目录，压过 `models.catalog.update` |
| `MIYU_RESOURCES` | 把资源目录指到别处，开发 Miyu 时用 |
| `MIYU_DISTRO_PACKAGES` | 只在这个绝对路径下找系统包管理器装的 Miyu 软件包，试验时用 |

在 shell 里设，只管这一个窗口和从它拉起的核心：

```bash
export MIYU_LOG=debug
```

```powershell
$env:MIYU_LOG = 'debug'
```

核心照它起来那一刻的环境读，已经在跑的先 `miyu core stop`。要以后开的窗口都认、开机启动的服务和托盘怎么带上 `MIYU_HOME`，见 [数据和备份](data.md#换一个位置)。

Miyu 拉起扩展、包带的命令、脚本、MCP 服务、AI 跑的命令时给它们的，写软件包的人用得着，你不用设：

| 变量 | 是什么 |
|---|---|
| `MIYU_PROGRAM` | 主程序 `miyu` 的真实位置 |
| `MIYU_HOME` | 数据根 |
| `MIYU_RESOURCES` | 资源目录 |
| `MIYU_APP_DIR` | 包的文件夹 |
| `MIYU_CONFIG_DIR` | 包自己的配置目录 `~/.miyu/etc/<编号>/` |
| `MIYU_STATE_DIR` | 包、脚本、MCP 服务的状态目录 |
| `MIYU_CACHE_DIR` | 能删的缓存目录 |
| `MIYU_LOG_DIR` | 包的日志目录 `~/.miyu/var/log/<编号>/` |
| `MIYU_TMP_DIR` | 包的暂存目录，每次拉起前清空 |
| `MIYU_RUNTIME_DIR` | 包的运行时目录，放锁、套接字 |
| `MIYU_IMAGES_DIR` | 脚本放要交给 AI 的图的目录 |
| `MIYU_PICTURES_DIR`、`MIYU_VIDEOS_DIR`、`MIYU_DOCUMENTS_DIR`、`MIYU_DOWNLOADS_DIR` | `~/.miyu/root/` 下给你看的几个目录 |
| `MIYU_ARGS_JSON` | 调脚本时的参数，JSON |
| `MIYU_CALLER_VENUE` | 这次调用来自哪种会话 |
| `MIYU_CALLER_OWNER` | 开口的是不是你本人：`1`、`0` |
| `MIYU_SETTINGS_JSON` | 这个包的设置项的最终值，JSON |
| `MIYU_MODEL`、`MIYU_EFFORT` | AI 跑的命令里：这一轮的模型、思考程度 |
| `MIYU_SESSION_LOG` | AI 跑的命令里：这个会话的日志目录 |
| `MIYU_CALL_SOCK`、`MIYU_CALL_TOKEN` | AI 跑的命令里 `miyu call` 连回这个会话用的地址和令牌 |

各自的用法见[写一个 Miyu 软件包](../authors/README.md)。

## 编辑器补全

核心起来时生成两份文件，放在 `~/.miyu/var/cache/config/`：

- `config.schema.json`：系统配置的 JSON Schema，带每个键的名字、说明、能写什么、默认值。
- `reference.toml`：全部键和默认值，给你翻着看。改它没用。

两份的字照 `ui.language`。改了界面语言、装卸软件包以后会重写。

核心新建的系统配置第一行是：

```toml
#:schema ../var/cache/config/config.schema.json
```

编辑器装了认这一行的 TOML 插件（Taplo 这一类）就有补全、悬停说明、写错就标。你自己建的文件没有这一行的，照抄放到第一行。项目配置没有 Schema。

## 设置为什么没生效

1. 看哪一层在生效：

   ```sh
   miyu config explain permission.start_read_only
   ```

   ```text
   新会话开局只读（permission.start_read_only）：打开以后，新会话一开始就是只读。AI 只能查、写计划，要改文件时你再关掉只读。项目配置里只能把它打开。新会话生效。
     true   项目配置  ~/src/app/.miyu/config.toml:2  ← 生效
     false  系统配置  ~/.miyu/etc/config.toml:5
     false  默认值
   ```

   第一行是名字、说明、什么时候生效。下面从上往下每一层一行：值、哪一层、写在哪一行。生效的那一行标「← 生效」，环境变量压着的标「← 生效，只管这一次启动」。写了却不算的标原因：「← 不算：还没信任」「← 不算：比下面几层宽」「← 不算：不能写在这一层」。

2. 查有没有写错：`miyu check`。

3. 对一下生效时机：「新会话生效」的要开新会话，「重启后生效」的要重开终端或网页。

### 写错了会怎样

| 写错的 | 结果 |
|---|---|
| 不认识的键 | 警告，给出最像的键名。留在文件里，不起作用 |
| 值的类型不对、不在选项里、超出范围、写法不对 | 错误。只丢这一项，照下面一层的值用，没有的照默认值 |
| 写在了不能写的那一层 | 错误。这一项不算 |
| TOML 语法错 | 错误。整份文件不算：核心起来时当它是空的，开着时照上一次读好的那份用 |
| 引用的密钥没存、环境变量没设 | 警告 |

`miyu config set` 和设置页碰到不对的值直接拒，文件一个字都不动。`miyu config edit` 有错误时不存。文件里有错误的，`miyu ask` 开头印「配置中有 N 处错误」，提示运行 `miyu check`。

### miyu check

```sh
miyu check                 # 查全部
miyu check ~/.miyu/etc/config.toml   # 只查这一份
```

它照磁盘上现在的字查你手写的、Miyu 要读的文件：系统配置（连同里面的 MCP 服务）、当前目录的项目配置、密钥文件、每个人格的 `persona.toml` 和 `prompts/examples.md`、每一份预设、每个软件包的清单，还有被跳过的脚本、技能和定时任务里读不懂的行。给了文件的只查那一份。一处问题一行：

```text
~/.miyu/etc/config.toml:12:3 错误：…
```

最后一行是合计，例如「1 处错误，2 处警告」，都没有的印「没有问题」；再印语义模型和记忆、知识库的索引进度，不算问题。有错误的退出码是 `1`。`--format json` 印成 JSON。

## 软件包的设置

软件包能在清单里声明自己的设置项，写在同一份 `~/.miyu/etc/config.toml` 里。键是 `<前缀>.<名字>`：前缀是包清单里写的 `prefix`，没写的是包的编号。终端界面的前缀是 `tui`，网页的是 `web`：

```toml
[tui]
keys = "vim"

[web]
port = 8400
```

这些键设置页里也改得到，也能用 `miyu config set tui.keys vim` 改。软件包里没写生效时机的，头下次启动时生效。卸包时这些键默认留在文件里、不起作用，重装照旧生效；`miyu pkg remove --purge` 才一起删。出厂的终端、网页的键见[配置键全表](config-keys.md)。
