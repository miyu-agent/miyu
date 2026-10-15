## 写一个 Miyu 软件包：给第三方作者

状态：2026-10-11 主会话起草；项目主人同一天照推荐定了第九节那四条，2026-10-12 照推荐定稿（技能、脚本补进第三节第 9 条以后）。英文版是 `README.en.md`（2026-10-13），两份一起改。写的是核心现在的样子；MCP 服务（施工 X-5 补进第三节第 8 条）、技能和脚本（施工 X-13 中补进第三节第 9 条）照它带。细节以蓝图为准，这一页只挑作者要知道的。

### 一、先知道的几件事

1. **一个文件夹就是一个包**，文件夹名就是编号。清单是文件夹里的 `package.toml`，别的文件跟它放在一起。
2. **包能带的东西**：一个程序（三选一：核心拉起、常驻的**扩展**；核心按需拉起的**小程序**；人打开的**界面**），现成的 **MCP 服务**（第三节第 8 条，不带程序也行），**技能**、**脚本**（第三节第 9 条，不带程序也行），一只终端界面的**吉祥物**，一个**后台页**（网页里打开的设置页面）。最常见的是扩展：给 AI 加工具、接一个聊天平台。
3. **扩展是一个独立的程序**，什么语言写都行。核心拉起它，经标准输入输出说 JSON-RPC 2.0，一行一条。扩展先握手，再把自己的工具登记进来；AI 调到这些工具时，核心反过来请扩展去做。
4. **装、卸照 pacman**：`miyu pkg install ./我的包`、`miyu pkg remove 编号`。装是整个文件夹拷进管理员的家目录（`~/.miyu/home/admin/packages/<编号>/`），当场生效，不用重启核心。卸就是清干净：包文件夹、包自己放状态的目录、设置里它的项一起删。
5. **你的程序能拿到什么，看清单声明、管理员批**：要哪些能力写在清单里，管理员打开它的时候批（第六节）。

### 二、包长什么样

```text
hello/
├── package.toml          # 清单
├── bin/
│   └── miyu-hello        # 程序；Windows 上是 miyu-hello.exe
└── page/
    └── index.html        # 后台页，可以没有
```

1. **编号**（文件夹名）：小写字母开头，只用小写字母、数字、`-`、`_`，最多 64 个。不能和随 Miyu 出厂的包撞（`basesystem`、`memory`、`roleplay`、`mermaid`、`net`、`onebot`、`tui`、`web` 这些）。
2. **程序放在 `bin/` 下**，名字照清单 `[command] program` 写的，Windows 上加 `.exe`。核心先在包的 `bin/` 里找，再在 `miyu` 本体旁边找，**不找 `PATH`**：别的程序冒充不了你的。Linux、macOS 上程序要有可执行权限；脚本要有 `#!` 开头那一行。
3. **包文件夹装好以后别往里写**：装的时候每个文件都记了哈希，`miyu pkg check` 会把你后来写进去的报成「多出」、改过的报成「已修改」。运行时的状态写进核心给你的工作目录（第四节第 2 条）。

### 三、清单

一个扩展的完整清单：

```toml
[package]
version = "0.1.0"                                   # 可以不写，只给人看
protocol = [1, 1]                                   # 你说得了的协议主版本 [最低, 最高]，现在只有 1
name = { en = "Hello", zh = "打招呼" }
summary = { en = "Greets people by name", zh = "照名字打招呼" }   # 可以不写
icon = "hand"                                       # Lucide 的图标名，可以不写

[command]                                           # 扩展必写
name = "hello"                                      # 敲 miyu hello … 时转给你的程序
program = "miyu-hello"                              # bin/ 下的程序名，不带路径
about = { en = "Hello extension", zh = "打招呼扩展" }

[process]                                           # 有它的就是扩展
args = ["serve"]                                    # 核心拉起时带的参数
start = "manual"                                    # manual：开关打开才拉起；always：核心起来就拉起
capabilities = ["tools"]                            # 要的能力，见第六节

[settings.greeting]                                 # 配置项，键是 hello.greeting
type = "text"
max = 40
default = "Hello"
layers = ["system"]
applies = "now"
name = { en = "Greeting", zh = "问候语" }
```

1. **给人看的字**（`name`、`summary`、`about`、配置项的 `name`、`description`）写成「语言到一句话」，只认 `zh`、`en`、`ja`，界面照人的语言挑，没有的退到 `en`、`zh`、`ja`。
2. **`[command]`**：`name` 小写字母开头，只用小写字母、数字、`-`，最多 32 个。和内置子命令（`ask`、`config`、`pkg`、`web` 这些）撞了的，内置的赢，你的不转。人敲 `miyu hello 参数…`，核心把参数原样交给你的程序，退出码照你的；`miyu help hello` 转成 `--help`。所以你的程序除了 `serve`，也该认人敲的那些参数。
3. **配置项** `[settings.<名字>]`：`type` 是 `bool`、`int`（`min`、`max`）、`option`（`choices`）、`text`（`max`，默认 200）、`name`、`url`、`secret`、`list`（`element`）之一；`layers` 是 `system`、`personal` 里的一个或两个；`applies` 是 `now`、`new_session`、`next_turn`、`program_start` 之一。设置页的「软件包」那一页会有你一组。**不要自己读配置文件**：值由核心在握手时交给你，改了推给你（第四节）。
4. **功能** `[features.<编号>]`：预设里一个开关管一组工具。不写的整个包算一个功能；写了几个的，你登记的每一件工具都要列在某个功能下面，不然登记不进来。

   ```toml
   [features.greet]
   name = { en = "Greeting", zh = "打招呼" }
   tools = ["hello_greet"]
   ```

5. **包自己的检查** `[check] args = ["check"]`：`miyu check` 时跑 `<程序> check`，你在标准输出上一行一个 JSON 报问题（`kind`、`file`、`level`、`message` 必有，`line`、`column`、`code`、`key`、`rule`、`source` 可以有），只有警告时退出码 0，有错误 1。最多跑 30 秒。
6. **后台页** `[page] dir = "page"`：目录里要有 `index.html`。页面经网页软件给的通道调你登记的方法（第四节第 7 条）。
7. 读不成的清单不装：`miyu pkg install` 会说第几行哪里不对；`miyu check` 也查。
   带的名字和装着的别的包撞了的也不装（照 pacman 的冲突）：子命令、功能的编号、工具名（功能下列的、脚本）、技能、MCP 服务、知识库的编号、斜杠命令模板（也不能和 Miyu 自带的命令同名），`miyu pkg install` 一处一行说撞了什么、在哪个包里。名字建议带上包的编号做前缀。
8. **带 MCP 服务** `[[mcp]]`：一段一个服务，核心照它拉起服务的进程、经标准输入输出说 MCP，把服务的工具登记进来。只带服务、不带程序的包不用写 `protocol`、`[command]`、`[process]`；扩展包也能带，服务跟着扩展起停。

   ```toml
   [package]
   name = { en = "Reference MCP", zh = "参考 MCP 服务" }

   [[mcp]]
   name = "files"                                    # 服务名：小写字母、数字、-，1 到 32 个字符
   command = ["npx", "-y", "@modelcontextprotocol/server-filesystem", "/srv/notes"]
   env = { TOKEN = { secret = "notes" } }            # 可以不写；值能引用密钥 { secret = … }、环境变量 { env = … }
   venues = ["local"]                                # 可以不写，不写是只给本机的会话
   timeout_ms = 600000                               # 可以不写：一次调用等多久，1000 到 3600000
   need_load = true                                  # 可以不写：这个服务的工具都要先加载（下面「要先加载」）
   ```

   - **她看到的工具**叫 `mcp__<服务名>__<工具名>`，说明、参数照服务给的原样（说明超过 2048 个字的截断）。预设里一个服务算一个功能，编号 `mcp-<服务名>`，能整个关、也能一件件关。
   - **问不问人**：服务在工具的注解里写了 `readOnlyHint: true` 的哪一级都不问；别的照联网的一类，工作区那一级要人确认（`miyu ask` 里确认不了的直接不做）。你的服务里只读的工具请标上它。
   - **怎么起**：命令第一项先在包的 `bin/` 里找，再在 `miyu` 旁边找，最后找 `PATH`（`npx`、`uvx`、`docker` 这类）。Miyu 不替人装 Node、Python：用 `npx`、`uvx` 起的服务，说明里写清楚要先装什么。工作目录是 `~/.miyu/state/mcp/<服务名>/`，不进沙盒；标准错误在 `~/.miyu/state/logs/mcp-<服务名>.stderr`。
   - **开关和状态**：只带服务的包装好是关着的，在「软件包」页里打开；页上一个服务一行，看得到在跑（进程号）、在等着重启、还是停下了（原因、标准错误的最后几行），停下的点重启。起不来的退避着试 5 次就停下。
   - **说哪一版 MCP**：新旧两个时代都认（2026-07-28 版去掉了 `initialize`，之前的先握手），你的服务说哪一版都行；只接工具，资源、提示词、`sampling` 这些不接。
   - **不做成包的**：写进管理员的个人设置 `~/.miyu/home/admin/settings.toml`，一个服务一张 `[mcp.<服务名>]`，格子同上、不写 `name`；改了当场生效，和包里带的同名时这里的赢（施工 F-9）。
9. **带技能、脚本**（`docs/blueprint/skills.md`）：放在包文件夹的 `skills/`、`scripts/` 下面，清单里不用写；只带它们的包清单只写一张 `[package]`（不写 `protocol`），没有开关，每个技能、脚本在预设里各自开关。

   ```text
   notes/
   ├── package.toml          # 只有 [package]
   ├── skills/
   │   └── take-notes/
   │       └── SKILL.md
   └── scripts/
       └── dice.py
   ```

   - **技能**照公开的 Agent Skills 格式：`skills/<名字>/SKILL.md`，开头的 YAML 写 `name`（小写字母、数字、`-`，和文件夹同名）、`description`（给模型看，英文，说清是什么、什么时候用）；给人看的名字、一句说明写在 `metadata` 的 `display_name`、`summary` 里（英文），中文、日文另写 `display_name_zh`、`summary_zh`、`display_name_ja`、`summary_ja`。`description` 用双引号包起来：里面有「冒号加空格」的不加引号读不成。AI 平时只看到名字和说明，用得上时自己读正文；文件夹里别的文件（参考资料、带的脚本）她也读得到，改不了。
   - **脚本**一个文件一件工具，工具名照文件名（去掉扩展名，不合 `[A-Za-z0-9_-]` 的换成 `_`）。开头的注释写说明、参数：

     ```python
     #!/usr/bin/env python3
     # Display name: Dice
     # 显示名称：掷骰子
     # Description: Roll dice and return each face and the total.
     # 描述：掷骰子，交回每一颗和总数。
     # Timeout: 10
     # Need load: yes
     # Parameters:
     # {"type": "object", "properties": {"count": {"type": "integer"}, "sides": {"type": "integer"}}, "required": ["count", "sides"]}
     ```

     参数是一份 JSON，在环境变量 `MIYU_ARGS_JSON` 和标准输入里各一份；标准输出就是交给 AI 的结果，退出码不是 0 的算失败；能写缓存的目录在 `MIYU_SCRIPT_CACHE_DIR`。照 `#!` 起（Windows 上照扩展名找解释器：`.py` 用 `python`，`.ps1` 用 PowerShell，`.js` 用 `node`）。跑的规矩同 AI 跑命令：在沙盒里跑（写只在工作区，Miyu 的数据目录读不到，脚本自己那个文件单独放开只读），沙盒用不了的要人确认。说明、参数写短：每一轮都跟着请求发。
   - **斜杠命令模板**照 Claude Code 的写法：`commands/<名字>.md`，人敲 `/<名字> 参数` 时把正文当这个人说的一句发给 AI，正文里的 `$ARGUMENTS` 换成参数（`$ARGUMENTS[0]` 是第一个），没写的参数接在末尾。开头可以写一段 YAML：`description` 是命令菜单里那一句（英文），`argument-hint` 是名字后面跟什么的提示，中文、日文的说明写在 `metadata` 的 `summary_zh`、`summary_ja`。名字同子命令名的写法；和 Miyu 自带的命令同名的不算（自带的赢）。时间线上显示人敲的那一行（`docs/blueprint/slash.md`）。
   - **交给程序办的斜杠命令**（扩展包）：清单一段一条 `[[slash]]`，写 `name`、`summary`（语言到一句话）、可以写 `argument`（名字后面跟什么的提示），`[process] capabilities` 里要有 `commands`。人敲了，核心发给你的程序 `command.call {command, argument, text, session, by, owner, language}`，你答 `{"said": "…"}`（可以多行），这段话就是给人看的回执；答 JSON-RPC 错误的，人看到「命令执行失败」和你的原话。你的程序起来以后要 `provide` 一次（没有工具的写空的），核心才认得你的连接。谁能用和 Miyu 自带的命令一样（本人、群里的主人和管理的人），要再挡的照 `by` 自己挡。
   - **回合开始加一段字**（施工 X-16）：`[process] capabilities` 里要有 `context.inject`，程序起来以后 `provide` 过一次，而且这个会话的工具面上有你的工具（预设关了你就不问）。每一轮开始核心发 `turn.start {"session", "venue", "text"?}`：`venue` 是 `local`（终端、网页）、`private`（平台私聊）、`group`（群），`text` 是触发这一轮的人话（不是人开的一轮不带）。300 毫秒内答 `{"inject": [{"kind", "text"}]}`，至多 3 块，`kind` 小写字母、数字、`-`、`_`，`text` 1 到 4000 个字符，原样进她的上下文、排在人说的话前面，记进会话日志以后不再变。没得加就答 `{"inject": []}`；超时、出错、写法不对的这一轮当没交。概率、开关你自己管。你交的字是给模型看的：写英文短句、外面套一层 XML 标签，自己记账（量 token、写明为什么要）。
   - **界面块**（扩展包）：往终端、网页上放自己的东西。清单一段一块 `[[widget]]`：`id`、`place`（`home` 首页、`sidebar` 侧边栏、`status` 底栏的一小段、`page` 整页）、`title`（语言到一句话，侧边栏、整页必写）、`scope = "session"`（只给侧边栏：跟着会话，一个会话一份）、`command`（只给整页：打了这个斜杠命令就打开这一页）。一个包最多 16 块，`[process] capabilities` 里要有 `widgets`。界面自己画，你只给内容（第四节第 8 条）；换不掉界面自带的东西，时间线、输入框、设置页也放不进去（设计 `33-界面扩展.md`）。
   - 写错了的不登记，`miyu check` 说哪一个、为什么；和别的工具撞了名的也说。
   - 不想做成包的，直接放进 `~/.miyu/home/admin/skills/`、`scripts/`、`commands/` 也一样用（同名的这里赢），不归包管理。

### 四、写扩展程序

1. **怎么被拉起**：程序是 `bin/` 下那一个，参数是 `[process] args`。环境照核心的，另有 `MIYU_HOME`（数据根）、`MIYU_RESOURCES`（资源目录）。工作目录是这个包放状态的目录 `<数据根>/state/packages/<编号>/`：你的数据库、缓存都放这里，卸载时一起删。标准错误接到 `<数据根>/state/logs/<编号>.stderr`，出了问题先看它。
2. **一行一条**：每条消息是一行紧凑的 JSON，`\n` 结尾，一行最长 1 MiB。你发的请求 `id` 要是字符串（1 到 128 字节）。
3. **握手**：第一条发 `hello`：

   ```json
   {"jsonrpc":"2.0","id":"h1","method":"hello","params":{"protocol":[1,1],"head":{"kind":"hello","version":"0.1.0"}}}
   ```

   不用出示凭据：标准输入输出是核心亲手给的。回应里的 `config` 是你这个包的配置项的最终值（`{"hello.greeting":"Hello"}`，密钥是真值，没设的键不放）；`language` 是给人看的字该用哪种。之后配置变了，核心推 `extension.config {"keys": {"hello.greeting": "Hi"}}`，只放变了的键，变成没设的是 `null`。连上 10 秒还没握手成的会被断开。
4. **登记工具**：握手以后发 `provide`，`tools` 是你现在提供的全部工具，每发一次整份换掉：

   ```json
   {"jsonrpc":"2.0","id":"p1","method":"provide","params":{"tools":[{"name":"hello_greet","description":"Greet someone by name.","input_schema":{"type":"object","properties":{"name":{"type":"string"}},"required":["name"]},"access":"read","venues":["local"]}]}}
   ```

   - `name`：英文字母、数字、`_`、`-`，1 到 64 个，不能和核心自带的、别的包的撞。建议带上包的编号做前缀。
   - `description`、`input_schema` 里的说明是给模型看的：写英文短句，一件工具最多三句（是什么、什么时候用、最容易踩的那个坑）；调用以后才用得上的知识写进工具的输出，不写进说明。这些字每一轮都跟着请求发，越短越省钱。
   - `access`：这件工具会碰什么。`read`（只看，不碰这台机器、不出 Miyu）、`write`（改文件）、`execute`（跑命令）、`network`（上网）、`outbound`（往 Miyu 外面发东西）、`venue`（在聊天平台上做动作：撤回、禁言这些）。权限策略照它决定问不问人，**照实写**：往外发东西的写成 `read` 是在骗管理员。
   - `venues`：给哪种会话，`local`（本机的）、`private`（平台私聊）、`group`（群）里的一个或几个。
   - `timeout_ms`：可以不写，1000 到 600000，默认 60000。
   - `display_name`：可以不写，给人看的名字，照语言写：`{"zh": "打招呼", "en": "Greet"}`。时间线上照用的人的语言写它，没写的照 `name`。这是给人看的，不给模型。
   - `need_load`：可以不写，`true` 是要先加载（下面「要先加载」）。
   - 写错的整份不收，回应的 `data` 说哪一件、哪里不对（`{"tool":"hello_greet","problem":"name"}`）。
5. **被调**：AI 调到你的工具时，核心发给你 `tool.call`：

   ```json
   {"jsonrpc":"2.0","id":"core-1","method":"tool.call","params":{"session":"…","call_id":"…","tool":"hello_greet","args":{"name":"Ann"},"cwd":"/home/me/src","owner":true}}
   ```

   `args` 已经照你的 `input_schema` 修正过。`by` 是谁让她做的（没有的不写），`owner` 是核心认不认这个人是主人本人：在群里做动作的工具，谁能用要你自己照它们挡。回应：

   ```json
   {"jsonrpc":"2.0","id":"core-1","result":{"blocks":[{"type":"text","text":"Hello, Ann!"}],"error":false}}
   ```

   `blocks` 是内容块（文字 `{"type":"text","text":…}`；图片先经 `blob.put` 传上去，再放 `{"type":"image","blob":…,"media_type":…,"width":…,"height":…}`）。失败的照样回结果、`error` 写 `true`，把原因写给模型看；回 JSON-RPC 的错误也算这次失败。同一时间可能来好几个 `tool.call`，照 `id` 对着回。
6. **超时和叫停**：过了 `timeout_ms` 没回的、人打断的，核心发通知 `tool.cancel {"session","call_id"}`（没有 `id`，不用回）。收到就放下那一件；已经做了的收不回，回的时候照实说。
7. **后台页的方法**（有后台页的、界面块里有按钮的才要）：发 `package.methods {"methods":[{"name":"status"}]}` 登记；页面、按钮调到时核心发 `method.call {"method","params"}`，你回什么页面就拿到什么，不认识的方法回 JSON-RPC 的 `-32601`。
8. **界面块的内容**（声明了 `[[widget]]` 的才要）：
   - 有人在看你的一块时，核心发通知 `widget.watch {"widget", "session"?}`；最后一个人不看了发 `widget.unwatch`。握手以后马上就可能来，和你的请求的回应穿插着，照 `id` 分开。只在有人看时取数据、算内容，别定时刷。
   - 推内容：`widget.set {"widget", "session"?, "title"?, "content"}`，跟着会话的写 `session`，`title` 能换掉清单里的标题（「构建 3/5」这种会变的）。内容是几段，只有六种：

     ```json
     [{"type":"heading","text":"本周"},{"type":"text","text":{"zh":"晴 23°C","en":"Sunny 23°C"},"tone":"accent"},{"type":"pairs","items":[{"label":"湿度","value":"40%"}]},{"type":"progress","value":0.4,"text":"索引 40%"},{"type":"list","items":[{"title":"构建 #128","right":"3m","state":"doing","action":{"call":"build.open","params":{"id":128}}}]},{"type":"buttons","items":[{"label":"刷新","action":{"call":"refresh"}}]}]
     ```

   - 只有字：不认 Markdown、HTML、颜色码，字可以按语言给几份。语气 `tone`：`normal`、`dim`、`accent`、`good`、`warn`、`bad`；状态 `state`：`todo`、`doing`、`done`、`failed`、`off`。动作三种：`{"call": <方法>}` 调你登记的方法（第 7 条），`{"open": <整页的编号>}` 打开你的一整页，`{"copy": <字>}` 人点了写进剪贴板。底栏只收一段一行的 `text`，最宽 40 列（中文算两列）。
   - 写错的整块不收，回 `bad_widget_content`，`data.path` 说错在哪，原来的照旧显示。同一块推得再快，界面最多每 250 毫秒变一次，最后一份不丢。`widget.clear` 回到没有内容。程序停了，你推过的全清；重新起来、握了手，核心再告诉你谁在看。
9. **退出**：标准输入读到头就是「请退出」，三个平台一样；5 秒内不退会被杀掉。
10. **崩了怎么办**：你自己退出（退出码 0 也算）、握手不成、标准输出关了，都算一次失败，核心隔 1、2、4、8、16 秒（最多 60 秒）再拉起，连续 5 次停下。**退出码 1 留给「配置错、端口被占」这类重启也没用的**：核心直接停下、不重试，人改好再开。连着跑满 60 秒的，下次失败从头数。
11. **别做的**：别读写数据根里别的地方（会话、别人的包、配置文件、密钥文件）；要信息走协议。扩展调 `extension.*`、`package.install` 这类只给本机的人用的方法会被拒（`local_only`）。

### 五、一个最小的扩展

`bin/miyu-hello`（Python；Linux、macOS 上 `chmod +x`）：

```python
#!/usr/bin/env python3
import json
import sys

TOOLS = [{
    "name": "hello_greet",
    "description": "Greet someone by name.",
    "input_schema": {"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]},
    "access": "read",
    "venues": ["local"],
}]


def send(message):
    sys.stdout.write(json.dumps(message, ensure_ascii=False) + "\n")
    sys.stdout.flush()


def serve():
    send({"jsonrpc": "2.0", "id": "h1", "method": "hello",
          "params": {"protocol": [1, 1], "head": {"kind": "hello", "version": "0.1.0"}}})
    greeting = "Hello"
    for line in sys.stdin:  # 读到头就是核心请你退出
        message = json.loads(line)
        if message.get("id") == "h1":
            if "error" in message:
                sys.exit(1)  # 握手被拒：重启也没用
            greeting = message["result"].get("config", {}).get("hello.greeting", greeting)
            send({"jsonrpc": "2.0", "id": "p1", "method": "provide", "params": {"tools": TOOLS}})
        elif message.get("method") == "extension.config":
            greeting = message["params"]["keys"].get("hello.greeting") or "Hello"
        elif message.get("method") == "tool.call":
            name = message["params"]["args"].get("name", "there")
            send({"jsonrpc": "2.0", "id": message["id"],
                  "result": {"blocks": [{"type": "text", "text": f"{greeting}, {name}!"}]}})


if __name__ == "__main__":
    if sys.argv[1:] == ["serve"]:
        serve()
    else:
        print("usage: miyu hello serve (started by the Miyu core)", file=sys.stderr)
        sys.exit(2)
```

Windows 上 `bin/` 里要放 `.exe`：用 PyInstaller 打包，或者换一种编得出可执行文件的语言。

### 六、能力和审批

1. 清单 `[process] capabilities` 写你要哪些，只认这十三个：`tools`（往工具目录里登记工具）、`commands`（提供斜杠命令）、`widgets`（在界面上显示自己的内容）、`context.inject`（回合开始时往上下文里加内容）、`tool.guard`（工具执行前放行、拒绝或要求问人）、`tool.rewrite`（工具执行后改写结果）、`events.read`（读会话的原始事件）、`events.write`（追加自己名下的事件）、`sessions.drive`（发消息、造子会话这类驱动会话的事）、`act_for_external`（代表聊天平台上的人发命令，只给平台的桥）、`network`（上网）、`fs.read`、`fs.write`（读、写指定的路径）。现在协议上第三方用得上的是登记工具（`provide`）、斜杠命令、后台页的方法、界面块和回合开始加的字（`context.inject`，第四节）；`tool.guard`、`tool.rewrite` 还没开放。
2. 管理员打开你的扩展时批：一次批你这时声明的全部。升级以后多要的，下次打开只问多的那几个。没批完的不拉起。
3. 照批过的能力挡方法、在沙盒里跑，随多用户、沙盒那一段做，现在还没强制。**照实声明**：以后没声明的会被挡掉。

### 七、装、试、查

1. 装：`miyu pkg install ./hello`。先印一份要做什么（带了什么程序、加什么命令、要什么能力、多大），问一句 `[Y/n]`；`--yes` 不问。升级就是再装一次：先停下在跑的那一个，换文件，再照开关拉起。
2. 打开：装好以后问一句「现在打开？[Y/n]」，答是就把上面列的能力一并批了、打开（`--yes` 当是）；答不的，以后在终端界面或网页的「软件包」页里打开、批（`manual` 的要打开才拉起；`always` 的批了就拉起）。
3. 看：`miyu pkg info hello`（名称、版本、大小、装的时间）、`miyu pkg files hello`（装了哪些文件）、`miyu pkg check hello`（装好的文件改没改）。「软件包」页上看得到它在跑、在等着重启、还是停下了（停下的写原因、标准错误的最后 20 行）。
4. 日志：你的标准错误在 `~/.miyu/state/logs/hello.stderr`（拉起前超过 1 MiB 的挪成 `.old`）；核心自己的运行日志在 `~/.miyu/state/logs/core.log`，拉起、握手、退出、退避、停下都各记一行。
5. 开发时反复改：改完再 `miyu pkg install ./hello` 一次就换上了。也可以在一份单独的数据根里试（`MIYU_HOME=/tmp/miyu-try miyu …`），不碰自己平常用的。
6. 卸：`miyu pkg remove hello`。设置里它的项、`state/packages/hello/` 一起删；密钥文件里的条目不删（别处可能也在用）。

### 八、三个平台

1. 程序名照清单写，Windows 上核心找的是 `<名字>.exe`。
2. Linux、macOS 上程序要有可执行权限；装的时候照原样拷，权限跟着走。
3. macOS（只考虑 Apple 芯片）：从网上下载的程序带隔离标记，Gatekeeper 会拦没签名、没公证的程序。发出去的程序请签名并公证；自己编、自己装的不受影响。
4. 「请退出」三个平台都是标准输入读到头，别靠信号。

### 九、定了的（2026-10-11 项目主人照推荐定）

1. **命令行里打开扩展、批能力**：`miyu pkg install` 装好以后问一句「打开？」，印的那一份列了要的能力，答是就一并批、打开（施工 F-8 四补做了，第七节第 2 条照它改了）。
2. **这一页的语言**：先中文定稿，0.0.1 发出去以前出一份英文的（2026-10-13 出了，`README.en.md`）。
3. **清单的 `author`、`license`、`url`**：随软件源那一段一起加。
4. **技能、脚本、MCP 服务怎么放进包**：`skills.md`、`mcp.md` 定了，随 X-2、X-10 起的施工补进这一页。MCP 服务施工 X-5 补进了第三节第 8 条，技能、脚本施工 X-13 中补进了第三节第 9 条。
