# 配置

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

Miyu 的设置都写在 TOML 文件里。这一页讲配置文件在哪里、怎么修改、修改之后什么时候生效、怎么为一个仓库单独设置、怎么存放密钥，以及一项设置没有生效时怎么排查。每个键是什么、可以写什么值，可以查 [配置键全表](config-keys.md)。

## 配置文件的位置

Miyu 会读下面三个文件：

| 文件 | 位置 | 谁来写 |
|---|---|---|
| 系统配置 | 数据根（默认是 `~/.miyu`，Windows 上是 `%USERPROFILE%\.miyu`）下的 `etc/config.toml`，也就是 `~/.miyu/etc/config.toml` | 你手动修改，或者通过 `miyu config set`、终端界面和网页界面的设置页修改 |
| 项目配置 | 仓库里的 `.miyu/config.toml` | 只能手动修改 |
| 密钥文件 | `~/.miyu/etc/private/secrets.toml` | `miyu provider login`，也可以手动修改 |

`miyu config path` 会输出系统配置的完整路径，`miyu config path --project` 会输出当前目录对应的项目配置的完整路径。如果文件还不存在，命令照样会输出它应该在的位置，然后提示 `文件尚不存在`。

一份系统配置的样子如下：

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

```

键按照 `.` 分段，前面几段是表头，最后一段写在表里。例如 `permission.confirm.timeout` 就是上面 `[permission.confirm]` 下面的 `timeout`。时长写成带引号的字符串，数字后面跟 `s`、`m`、`h`，不跟单位时按秒计算。配置文件最大 1 MiB。

## 修改设置

下面三种方法修改的都是同一份文件 `~/.miyu/etc/config.toml`：

- 命令行：运行 `miyu config set <键> <值>`。
- 设置页：在终端界面或网页界面里输入 `/settings`。在终端里直接运行 `miyu config`，会打开默认界面（`ui.head`）的设置页。
- 手动修改文件，或者运行 `miyu config edit`。

命令行和设置页只修改那一项，注释、空行、先后顺序和不认识的键都会原样保留。手动修改的文件一保存，核心就会重新读取，不需要重启。

1. 修改一项设置，运行：

   ```sh
   miyu config set permission.confirm.timeout 600
   ```

   标准错误上会输出一行 `已设置 permission.confirm.timeout = 600（下一轮生效）`。如果这一项本来就是这个值，会输出 `已经是 600，未修改`。

2. 查看现在使用的值，运行：

   ```sh
   miyu config get permission.confirm.timeout
   ```

3. 查看这个值从哪里来：运行 `miyu config explain permission.confirm.timeout`（见下面的「排查没有生效的设置」）。

如果不再需要这一项，运行 `miyu config unset permission.confirm.timeout`，会输出 `已删除 permission.confirm.timeout，现在是 300（默认值）`。

### 生效时机

每个键都登记了一种生效时机，`miyu config set` 和 `miyu config explain` 都会告诉你：

| 输出 | 意思 | 例如 |
|---|---|---|
| `立即生效` | 修改之后马上使用新的值 | `ui.language`、`log.level`、`core.idle_exit`、模型的冷却 |
| `下一轮生效` | 开着的会话从下一轮开始使用新的值，正在运行的这一轮仍然使用原来的值 | 供应商的地址和 key、受保护的命令、压缩 |
| `新会话生效` | 以后开始的会话使用新的值，已经开着的会话不变 | `models.chat`、`persona.default`、`permission.start_read_only` |
| `重启后生效` | 界面（终端界面、网页界面）下次启动时才会读取 | `ui.startup`，以及软件包里没有写生效时机的键 |
| `重启核心后生效` | 核心下次启动时才会读取 | `core.workers` |

核心怎么停止、怎么重新启动，见 [常驻和服务器](service.md)。

### 在命令行上写值

不同类型的值在命令行上的写法如下：

| 类型 | 写法 | 例子 |
|---|---|---|
| 开关 | `true`、`false` | `miyu config set compaction.prepare false` |
| 数 | 直接写 | `miyu config set compaction.line 70` |
| 字、选项、模型、时长 | 直接写，也可以加双引号 | `miyu config set jobs.max_running 45m` |
| 列表 | TOML 的数组，整个用单引号括起来；写的是整个列表，原来的列表会被替换掉 | `miyu config set skills.external.dirs '["~/.agents/skills", "~/my-skills"]'` |
| 密钥引用 | TOML 的行内表 | `miyu config set providers.acme.key '{ secret = "acme" }'` |
| 每一项有几个字段的列表 | 行内表的数组 | `miyu config set tui.footer '[{ part = "usage" }, { part = "model", shown = false }]'` |

列表和行内表按照 TOML 读取，TOML 的字符串用双引号、单引号都可以。在 PowerShell 里，外面要用双引号，里面的字符串用 TOML 的单引号写法。因为 Windows PowerShell 5.1 把参数交给程序时会吃掉里面的双引号，而下面这样写，在 5.1 和 PowerShell 7 里都可以直接照抄：

```powershell
miyu config set skills.external.dirs "['~/.agents/skills', '~/my-skills']"
miyu config set providers.acme.key "{ secret = 'acme' }"
miyu config set tui.footer "[{ part = 'usage' }, { part = 'model', shown = false }]"
```

如果键里有一段带着 `.`、`:` 这类字符，就给那一段加上双引号，再用单引号把整个键括起来：

```bash
miyu config set 'providers.ollama.models."qwen3:8b".window' 32768
```

在 Windows PowerShell 5.1 里，这种键的双引号同样会被吃掉。此时把键里的引号换成单引号，外面用双引号：`miyu config set "providers.ollama.models.'qwen3:8b'.window" 32768`；或者用 `miyu config edit` 手动修改。

`miyu config set` 一次只能修改一个键。整张表（例如一个 MCP 服务 `[mcp.<名字>]`）需要手动修改文件。

## miyu config 的全部命令

`miyu config` 有下面这些子命令：

| 命令 | 做什么 | 例子 |
|---|---|---|
| `miyu config` | 在终端里运行时打开默认界面的设置页；不在终端里运行时输出帮助 | `miyu config` |
| `get [<键>…]` | 输出最终值。只给一个键时只输出值；给几个键或者不给键时，每行输出一个 `键 = 值` | `miyu config get models.chat` |
| `set <键> <值>` | 修改系统配置里的一项 | `miyu config set ui.language zh` |
| `unset <键>` | 从系统配置里删除一项，让它回到默认值 | `miyu config unset ui.language` |
| `edit` | 用编辑器打开系统配置，保存之前先检查 | `miyu config edit` |
| `explain <键>` | 显示这一项在每一层写了什么、哪一层生效 | `miyu config explain log.level` |
| `path` | 输出系统配置的完整路径 | `miyu config path` |
| `trust` | 查看当前目录的项目配置会修改什么，然后决定是否信任它 | `miyu config trust` |

| 选项 | 用在 | 做什么 |
|---|---|---|
| `--project` | `edit`、`path` | 改为针对当前目录的项目配置 |
| `--format <text\|json>` | `get`、`explain` | 输出格式，默认是 `text` |
| `--yes` | `trust` | 直接信任，不询问 |
| `--no` | `trust` | 直接不信任，不询问 |

`set` 和 `unset` 只修改系统配置。如果给 `set` 加上 `--project`，它不会修改任何文件，而是报错 `项目配置只能手动编辑`，退出码是 `2`。

`miyu config edit` 使用 `VISUAL` 里的编辑器；如果没有设置，就使用 `EDITOR`；如果两个都没有设置（或者都是空的），Linux、macOS 上使用 `vi`，Windows 上使用 `notepad`。编辑器的那一串命令在 Linux、macOS 上通过 `sh -c` 运行，在 Windows 上通过 `cmd /c` 运行，所以带参数的写法在两边都可以使用，例如 `code --wait`：

```bash
export VISUAL='code --wait'
```

```powershell
$env:VISUAL = 'code --wait'
```

`miyu config edit` 会在原文件旁边放一份副本给你修改。关掉编辑器以后：

- 如果没有修改：输出 `未修改`。
- 如果有错误：每处错误输出一行，然后询问 `有 N 处错误，未保存。按回车继续编辑，输入 q 放弃：`。如果只有警告，照样保存。
- 如果你编辑的时候文件被别处修改了：不保存，输出 `编辑期间文件已被修改，未保存`，副本会保留下来，并告诉你它在哪里。

`edit` 需要在终端里运行。

## 为一个仓库单独设置

项目配置是仓库里的 `.miyu/config.toml`，只对在这个仓库里开始的会话起作用，并且只能把限制收紧。目前能写的只有一项：

```toml
[permission]
start_read_only = true          # 在这个仓库里开始的新会话一开始就是只读
```

写 `false` 不算数。其他的键写进项目配置也不算数，`miyu config explain <键>` 会在那一行标出 `← 不算：不能写在这一层`。

1. 在仓库里运行 `miyu config edit --project`，或者直接创建这个文件。如果还没有这个文件，它会创建在有 `.git` 的那一层目录下；如果往上都没有 `.git`，就创建在当前目录下。
2. 在仓库里运行 `miyu config trust`。它会列出 `<文件> 会修改以下设置（项目配置只能收紧限制）：` 和每一项设置，然后询问 `信任 '<文件>'？[y/N]`。
3. 输入 `y`，会输出 `已信任（文件变化后会再次询问）`。

没有信任的项目配置不起作用。在这样的仓库里，`miyu ask` 开头会输出 `项目配置 <文件> 尚未信任，本次没有使用`。终端界面和网页界面里不会询问，需要到命令行里运行 `miyu config trust`。文件内容一旦变化，就要重新信任。如果不在终端里运行，需要加上 `--yes` 或 `--no`。如果你回答了不信任，以后不会再提醒，直到文件发生变化。

Miyu 从会话的工作目录开始，一层层往上找 `.miyu/config.toml`，只使用最近的那一份。找到有 `.git` 的那一层就不再往上找；家目录本身和它的上级目录不找，数据根里面也不找。

## 叠加顺序

一个键的最终值从下往上叠加，上面一层会覆盖下面一层：

1. 默认值。
2. 系统配置。
3. 项目配置：只有信任过、并且是收紧限制的，才算数。
4. 环境变量：只有 `log.level`（`MIYU_LOG`）和 `models.catalog.update`（`MIYU_CATALOG_UPDATE`）读取环境变量，并且只对这一次启动的核心有效。

如果 `miyu config set` 修改的值被上面一层覆盖着，会多输出一行警告，例如 `log.level 被环境变量覆盖，使用 "debug"`。

## 密钥

配置里不写 key 本身，只写引用：

- `{ secret = "名字" }`：从密钥文件 `~/.miyu/etc/private/secrets.toml` 里读取。Linux、macOS 上，Miyu 每次写这个文件都会把权限设成 `0600`，只有你能读写；Windows 上不另外设置权限，沿用数据根继承的访问控制。三个平台都不使用系统的钥匙串，key 以原文写在这个文件里。
- `{ env = "变量" }`：从核心启动时的环境变量里读取，不写进任何文件。由服务启动的核心看不到你 shell 里的变量，见 [常驻和服务器](service.md)。

存储、查看、删除密钥都使用 `miyu provider`。它不只管理模型供应商的 key，任何名字的密钥都可以存：

| 命令 | 做什么 |
|---|---|
| `miyu provider login [<名字>]` | 存储一个密钥。粘贴时不显示内容，也可以从管道读取。不写名字时，从配置里用到的、已经存过的名字里选择 |
| `miyu provider list [--format text\|json]` | 列出已经存储的密钥，以及配置里用到了但还没有存储的密钥，并标出谁在使用。不显示值 |
| `miyu provider logout [<名字>]` | 删除一个密钥 |

名字以小写字母开头，只包含小写字母、数字、`-` 和 `_`，最长 64 个字符。例如给网络搜索存储 Tavily 的 key：

```bash
echo "$TAVILY_KEY" | miyu provider login tavily
miyu config set net.search.tavily.key '{ secret = "tavily" }'
```

```powershell
$tavilyKey | miyu provider login tavily
miyu config set net.search.tavily.key "{ secret = 'tavily' }"
```

密钥文件也可以手动修改，每行一个 `名字 = "值"`，保存之后就生效。

可以写引用的键如下：

| 键 | `{ secret }` | `{ env }` |
|---|---|---|
| `providers.<编号>.key`，`net.search.exa.key`、`net.search.deepseek.key`、`net.search.tavily.key`、`net.search.brave.key`，软件包声明成密钥的键 | 能 | 能 |
| `providers.<编号>.base_url`、`models.catalog.url`、`net.search.searxng.url`、`packages.source`、`packages.sources.<编号>.url` | 不能 | 能 |
| MCP 服务的 `env`、`bearer`、`headers` 里的值 | 能 | 能 |

如果密钥类型的键写成了明文，会报类型不对，这一项不算数。如果引用的密钥还没有存储，或者环境变量没有设置，只会报警告，用到这一项时它没有 key。

## 环境变量

下面这些环境变量可以由你设置：

| 变量 | 做什么 |
|---|---|
| `MIYU_HOME` | 数据根放在哪里，要写绝对路径，开头的 `~` 会被换成家目录。不设置时是 `~/.miyu` |
| `MIYU_LOG` | 这一次启动的核心的运行日志级别，会覆盖 `log.level`，可以写 `error`、`warn`、`info`、`debug`、`trace`、`off` |
| `MIYU_CATALOG_UPDATE` | 写成 `false` 时，这一次启动的核心不在后台拉取模型目录，会覆盖 `models.catalog.update` |
| `MIYU_RESOURCES` | 把资源目录指到其他地方，开发 Miyu 时使用 |
| `MIYU_DISTRO_PACKAGES` | 只在这个绝对路径下寻找系统包管理器安装的 Miyu 软件包，做试验时使用 |

在 shell 里设置的环境变量，只对这一个窗口和从这个窗口启动的核心有效：

```bash
export MIYU_LOG=debug
```

```powershell
$env:MIYU_LOG = 'debug'
```

核心按照它启动那一刻的环境读取变量，所以如果核心已经在运行，需要先运行 `miyu core stop`。如果想让以后打开的窗口都读到这个变量，或者想让开机启动的服务和系统托盘带上 `MIYU_HOME`，见 [数据和备份](data.md#更换数据根的位置)。

下面这些变量是 Miyu 启动扩展、软件包带的命令、脚本、MCP 服务和 AI 运行的命令时传给它们的。写软件包的人会用到它们，你不需要设置：

| 变量 | 是什么 |
|---|---|
| `MIYU_PROGRAM` | 主程序 `miyu` 的真实位置 |
| `MIYU_HOME` | 数据根 |
| `MIYU_RESOURCES` | 资源目录 |
| `MIYU_APP_DIR` | 软件包的文件夹 |
| `MIYU_CONFIG_DIR` | 软件包自己的配置目录 `~/.miyu/etc/<编号>/` |
| `MIYU_STATE_DIR` | 软件包、脚本、MCP 服务的状态目录 |
| `MIYU_CACHE_DIR` | 可以删除的缓存目录 |
| `MIYU_LOG_DIR` | 软件包的日志目录 `~/.miyu/var/log/<编号>/` |
| `MIYU_TMP_DIR` | 软件包的暂存目录，每次启动前清空 |
| `MIYU_RUNTIME_DIR` | 软件包的运行时目录，用来放锁和套接字 |
| `MIYU_IMAGES_DIR` | 脚本存放要交给 AI 的图片的目录 |
| `MIYU_PICTURES_DIR`、`MIYU_VIDEOS_DIR`、`MIYU_DOCUMENTS_DIR`、`MIYU_DOWNLOADS_DIR` | `~/.miyu/root/` 下给你查看的几个目录 |
| `MIYU_ARGS_JSON` | 调用脚本时的参数，格式是 JSON |
| `MIYU_CALLER_VENUE` | 这次调用来自哪一种会话 |
| `MIYU_CALLER_OWNER` | 说话的是不是你本人：`1` 或 `0` |
| `MIYU_SETTINGS_JSON` | 这个软件包的设置项的最终值，格式是 JSON |
| `MIYU_MODEL`、`MIYU_EFFORT` | 只在 AI 运行的命令里有：这一轮的模型和思考程度 |
| `MIYU_SESSION_LOG` | 只在 AI 运行的命令里有：这个会话的日志目录 |
| `MIYU_CALL_SOCK`、`MIYU_CALL_TOKEN` | 只在 AI 运行的命令里有：`miyu call` 连回这个会话时使用的地址和令牌 |

每个变量的用法见 [写一个 Miyu 软件包](../../authors/README.md)。

## 编辑器补全

核心启动时会生成两个文件，放在 `~/.miyu/var/cache/config/` 里：

- `config.schema.json`：系统配置的 JSON Schema，包含每个键的名字、说明、可以写的值和默认值。
- `reference.toml`：全部的键和默认值，供你翻阅。修改它不会有任何作用。

这两个文件使用 `ui.language` 设置的语言。修改界面语言、安装或卸载软件包以后，它们会被重新生成。

核心新建的系统配置，第一行是：

```toml
#:schema ../var/cache/config/config.schema.json
```

如果你的编辑器装了能识别这一行的 TOML 插件（例如 Taplo 这一类），就可以使用补全、悬停说明，写错时也会被标出来。如果你自己创建的文件没有这一行，把它照抄到第一行即可。项目配置没有 Schema。

## 排查没有生效的设置

如果一项设置没有生效，可以按下面的步骤排查：

1. 查看哪一层在生效，运行：

   ```sh
   miyu config explain permission.start_read_only
   ```

   ```text
   新会话开局只读（permission.start_read_only）：打开以后，新会话一开始就是只读。AI 只能查、写计划，要改文件时你再关掉只读。项目配置里只能把它打开。新会话生效。
     true   项目配置  ~/src/app/.miyu/config.toml:2  ← 生效
     false  系统配置  ~/.miyu/etc/config.toml:5
     false  默认值
   ```

   输出的第一行是这一项的名字、说明和生效时机。下面从上往下每一层占一行：值、哪一层、写在哪个文件的哪一行。生效的那一行标着 `← 生效`，被环境变量覆盖的标着 `← 生效，只管这一次启动`。写了但不算数的会标出原因：`← 不算：还没信任`、`← 不算：比下面几层宽`、`← 不算：不能写在这一层`。

2. 检查有没有写错，运行 `miyu check`。

3. 核对生效时机：`新会话生效` 的设置需要开始一个新会话，`重启后生效` 的设置需要重新打开终端界面或网页界面。

### 写错时的结果

配置写错时，不同的错误有不同的结果：

| 写错的情况 | 结果 |
|---|---|
| 不认识的键 | 警告，并给出最接近的键名。这个键留在文件里，不起作用 |
| 值的类型不对、不在选项里、超出范围、写法不对 | 错误。只丢弃这一项，改用下面一层的值，下面一层也没有时使用默认值 |
| 写在了不允许写的那一层 | 错误。这一项不算数 |
| TOML 语法错误 | 错误。整份文件都不算数：核心启动时把它当作空文件，核心运行中则继续使用上一次成功读取的那一份 |
| 引用的密钥没有存储、环境变量没有设置 | 警告 |

`miyu config set` 和设置页遇到不对的值会直接拒绝，文件一个字都不会改动。`miyu config edit` 在有错误时不保存。如果文件里有错误，`miyu ask` 开头会输出 `配置中有 N 处错误`，并提示运行 `miyu check`。

### miyu check

`miyu check` 用来检查你手写的配置文件有没有写错。可以使用：

```sh
miyu check                 # 检查全部文件
miyu check ~/.miyu/etc/config.toml   # 只检查这一个文件
```

它按照磁盘上现在的内容，检查你手写的、Miyu 要读取的文件：系统配置（包括里面的 MCP 服务）、当前目录的项目配置、密钥文件、每个人格的 `persona.toml` 和 `prompts/examples.md`、每一份预设、每个软件包的清单，还有被跳过的脚本、技能，以及定时任务里读不懂的行。如果给出了文件，就只检查那一个文件。每个问题输出一行：

```text
~/.miyu/etc/config.toml:12:3 错误：…
```

最后一行是合计，例如 `1 处错误，2 处警告`，没有任何问题时输出 `没有问题`；之后还会输出语义模型和记忆、知识库的索引进度，这些不算问题。有错误时退出码是 `1`。加上 `--format json` 会以 JSON 格式输出。

## 软件包的设置

软件包可以在清单里声明自己的设置项，这些设置项写在同一份 `~/.miyu/etc/config.toml` 里。键的形式是 `<前缀>.<名字>`：前缀是软件包清单里写的 `prefix`，没有写时使用软件包的编号。终端界面的前缀是 `tui`，网页界面的前缀是 `web`：

```toml
[tui]
keys = "vim"

[web]
port = 8400
```

这些键在设置页里也可以修改，也可以用 `miyu config set tui.keys vim` 修改。如果软件包里没有写生效时机，这些键在界面下次启动时生效。卸载软件包时，这些键默认留在文件里但不起作用，重新安装后仍然生效；只有 `miyu pkg remove --purge` 才会把它们一起删除。出厂的终端界面、网页界面的键见 [配置键全表](config-keys.md)。
