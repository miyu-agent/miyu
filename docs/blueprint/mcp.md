## MCP 适配器：现成的 MCP 服务接进来

### 是什么

MCP 服务只提供工具（`05-内核接口.md` 第四节）。核心里的 MCP 适配器把它接进来：照清单或个人设置拉起服务的进程，经标准输入输出说 MCP，问它有哪些工具，登记进核心的工具目录；她调到其中一件，适配器转成 MCP 的 `tools/call`，把回来的内容换成结果块。MCP 只是兼容入口：挂接点、命令、事件这些 Miyu 的本事，MCP 服务都没有，要的写成扩展（`05-内核接口.md` I1）。

已经定了的（不在这里重定）：

- 在核心里接，说 MCP 的是核心，不另起一个转译进程（`04-核心协议.md` 第一节的图、`05-内核接口.md` I1）。
- 工具目录缓存到磁盘，核心起来时不等服务就绪（`05-内核接口.md` I8，旧版启动等 MCP 卡过）；不可用的工具留在目录里，调到时说暂时不可用（I6）。
- 从哪来：软件包清单的 `[[mcp]]`，一段一个服务；本机自己放的写在个人设置的一张表里，一个名字一条命令，同名的自己放的赢（`31-软件包.md` 第二节、第五节）。
- 一个服务算一个功能，预设里一个开关，展开能逐件关（`30-插件框架.md` 第三节）；工具加载里一个服务一组（`25-工具加载.md` W3）。

状态：施工中（2026-10-11 主会话写；项目主人同一天照推荐定了下面「定了的」四条）。X-1（客户端）做了；X-2（清单的 `[[mcp]]`、`bundle`、一个服务一个功能，`packages.md`「MCP 服务」）做了；X-3 上（`bundle` 带的服务接进核心：起、登记、调用、开关）、X-3 下（扩展包带的服务、目录缓存、`ttlMs`、卸包清干净）做了；X-4（每个服务的状态、`package.restart`、预设开关的契约测试）做了；X-5（官方参考服务、真模型走一遍，作者说明补上怎么带）做了，MCP 这一段做完。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-mcp/src/client.rs` | 客户端：认时代、握手（「怎么走」第一条）；列工具（翻页、`ttlMs`）、调工具、工具变了的次数（施工 X-1） |
| `crates/miyu-mcp/src/link.rs` | 一条连接：写、读两个任务，请求照编号等回应；没等到就不等了的发取消；服务反过来的请求当场回（施工 X-1） |
| `crates/miyu-mcp/src/wire.rs` | 一行一条的 JSON-RPC：读一行（最长 64 MiB）、认成回应、请求、通知（施工 X-1） |
| `crates/miyu-mcp/src/content.rs` | 列出来的工具（名字、说明、参数、注解、原文）、调用回来的几种内容（施工 X-1） |
| `crates/miyu-mcp/src/http.rs`、`src/testkit/http.rs` | 网址的服务（施工 X-32 上，「网址的服务怎么说话」）：把一行一条接到 Streamable HTTP 上，交回一对读写给客户端；测试用的假网址服务照请求答（`miyu-http` 的假服务器照剧本回、答不了带编号的请求，所以另写一份） |
| `crates/miyu-mcp/src/testkit.rs` | 测试用的假服务（`testkit` 开关）：在进程里经内存管道照剧本演两个时代、不好好说话的几种（施工 X-1） |

| `crates/miyu-config/src/package/mcp.rs` | 清单的 `[[mcp]]`：名字、命令、环境变量、给哪种会话、时限（施工 X-2） |
| `crates/miyu-endpoint/src/mcp.rs` | 服务的表：一个服务一格（状态、连着的客户端、看管的任务）；核心起来时起开着的 `bundle` 的服务、停时一起关；装卸时跟着停、起；包的状态照它的服务合起来算（施工 X-3 上） |
| `crates/miyu-endpoint/src/mcp/launch.rs` | 怎么起一个：程序照命令第一项找（「怎么走」第二条第 2 条）、环境变量取出引用的、工作目录、标准错误的文件（施工 X-3 上） |
| `crates/miyu-endpoint/src/mcp/supervise.rs` | 看管一个：起、认时代、列、登记、等退出或叫停；工具变了再列；退避着重启、停下（施工 X-3 上） |
| `crates/miyu-mcp/src/prompt.rs`、`crates/miyu-endpoint/src/mcp/prompts.rs`、`src/commands/prompts.rs` | 提示词（施工 X-39）：适配器读 `prompts/list` 的一项、`prompts/get` 的消息（`Client::prompts`、`Client::prompt`，`offers_prompts` 看本事）；端点清理成命令名、撞名的进 `skipped`、取回来的照工具结果换块；斜杠命令那一头的菜单和 `command.run` |
| `crates/miyu-mcp/src/resource.rs`、`crates/miyu-endpoint/src/mcp/resources.rs`、`resources/core/mcp/{list_resources,read_resource}.json` | 资源（施工 X-41，「怎么走」第七条）：适配器列一页（`Client::resources`、`resource_templates`，只留规范里那几格）、读（`Client::read_resource`），`offers_resources` 看本事；端点照服务登记那两件、列的回一行 JSON、读的图换成图 |
| `crates/miyu-mcp/src/oauth.rs`、`src/oauth/{flow,callback}.rs`、`crates/miyu-endpoint/src/mcp/login.rs`、`src/mcp/login/flow.rs`、`crates/miyu-cli/src/mcp.rs` | 网址的服务登录（施工 X-42 上，「怎么走」第八条）：适配器照规范的纯逻辑（挑战、规范网址、两种 well-known、元数据、PKCE、授权页、回调、`iss`、`scope`）和几次 HTTP（发现、动态注册、换令牌）、只听 `127.0.0.1` 的回调；转接记下 `401` 的挑战；端点 `needs_login`、`mcp.login`、令牌存进 `mcp-oauth-<服务名>`、连的时候当 `bearer`、删服务时清掉；`miyu mcp login` |
| `crates/miyu-mcp/src/http.rs` 的 `Credentials`、`oauth/flow.rs` 的 `refresh`、`crates/miyu-endpoint/src/mcp/login/credentials.rs`、`src/mcp/login/manage.rs` | 登录以后（施工 X-42 下，「怎么走」第八条第 10 到 13 条）：转接每个请求现场问令牌，快过期的先续、`401` 续一次重发，续不成的、`403 insufficient_scope` 的记下挑战关掉；续期写回密钥、记下摘要不重起；`mcp.logout`、`mcp.login_finish`；`miyu mcp logout`、贴回来的一行 |
| `crates/miyu-endpoint/src/mcp/instructions.rs` | 服务的说明拼成 system 尾部的一段（施工 X-39）：外壳、截到 `APPEND_MAX`、结束标签转掉；`hooks.rs` 的 `appended` 照它和扩展放的一起交给会话，会话照工具面挑（`miyu-session` 的 `appended.rs`） |
| `crates/miyu-endpoint/src/mcp/spec.rs` | 列出来的换成目录里的工具：名字、说明、访问类别、给哪种会话；查不过的那一件不登记（施工 X-3 上），交回没登记的几件，列完的记进看管那一格（施工 X-37） |
| `crates/miyu-endpoint/src/mcp/tool.rs` | 一件 MCP 工具（`Tool` 的实现）：照这时连着的客户端调，时限、暂时不可用、出错（施工 X-3 上） |
| `crates/miyu-endpoint/src/mcp/content.rs` | 回来的内容换成结果块、截断（施工 X-3 上） |
| `crates/miyu-endpoint/src/mcp/switch.rs` | `bundle` 的开关：和扩展同一份文件、同一把锁（施工 X-3 上） |
| `crates/miyu-endpoint/src/mcp/methods.rs` | `mcp.list`、`mcp.restart` 和推送 `mcp.changed` 的那一项（施工 F-9 补）；每一项带服务那一级的 `need_load`（`by` 是 `source` 或 `setting`）和全局开关 `on`（施工 X-27） |
| `crates/miyu-endpoint/src/mcp/editing.rs`、`mcp/written.rs`、`config/rewrite.rs` | `mcp.set`、`mcp.remove`：查写法、在个人设置的字上加上、换掉、删掉一张，照 `config.set` 的规矩写（施工 X-26） |
| `crates/miyu-endpoint/src/mcp/home.rs` | 个人设置里的服务：起来时定下谁盖住谁、挂在 `~home` 下起，跟着配置换（新写的起、删掉的停、改了的重起、包里被盖住的停下或起回来）（施工 F-9 下） |
| `crates/miyu-config/src/package/mcp/personal.rs` | 个人设置的 `[mcp.<名字>]` 读成服务，写错的那一个不收（施工 F-9 上） |
| `crates/miyu-endpoint/src/check/mcp.rs` | `miyu check` 报个人设置里写错的、盖住了包里同名的（施工 F-9 上） |
| `crates/miyu-tool/src/catalog.rs` 的 `replacing_service` | 目录里一个服务的工具单独换，和包自己登记的互不冲掉（施工 X-3 上）；脚本的整份换是同一个做法（`replacing_scripts`，施工 X-12 中，`skills.md`） |
| `crates/miyu-endpoint/src/bin/miyu-test-mcp.rs` | 测试用的服务：真起进程走一遍，照参数演新旧时代、几件工具、记下收到的每一行（施工 X-3 上；不随发行带出去） |

| `crates/miyu-endpoint/src/mcp/cache.rs` | 目录缓存 `state/mcp/<服务名>.json`：写、照命令和环境变量的写法认、卸包时清掉（施工 X-3 下） |
| `crates/miyu-endpoint/src/packages/services.rs` | 软件包列表上每个服务一格的状态，写法同 `extension.status` 的一项（施工 X-4） |
| `crates/miyu-endpoint/src/packages/switch.rs` 的 `restart` | `package.restart`：扩展的进程照 `extension.restart`，再重启带的服务（`Core::restart_services`）；`bundle` 重启带的服务（施工 X-4） |

### 对外的样子

**清单里的一段**（`31-软件包.md` 第二节已经定了样子，这里补齐格子）：

```toml
[[mcp]]
name = "github"                                   # 服务名：小写字母、数字、-，1 到 32 个字符
command = ["npx", "-y", "@modelcontextprotocol/server-github"]
env = { GITHUB_TOKEN = { secret = "github" } }    # 可以不写；值照配置的写法，能引用密钥
venues = ["local"]                                # 可以不写，不写是只给本机的会话
timeout_ms = 600000                               # 可以不写：一次调用等多久，1000 到 3600000，不写是 600000
```

**网址的服务**（施工 X-32 上；写法 2026-10-20 项目主人照推荐定）：`command`、`url` 写一个。写 `url` 的不起进程：

```toml
[[mcp]]
name = "github"
url = "https://api.githubcopilot.com/mcp/"        # https:// 开头；http:// 只给本机地址（连的时候认，第五条）
bearer = { secret = "github" }                    # 可以不写：发成 Authorization: Bearer <值>，写法同 env 的值
headers = { X-Team = "core" }                     # 可以不写：别的请求头，写法同上；Authorization、Mcp-Session-Id、MCP-Protocol-Version 不收
```

`env` 只跟着 `command`，`bearer`、`headers` 只跟着 `url`，混着写、两个都没写的报 `bad_mcp`。`venues`、`timeout_ms`、`need_load`、`clicall` 照旧。

**个人设置里的一张表**（施工 F-9，`31-软件包.md` 第五节）：管理员的个人设置 `home/<管理员>/settings.toml` 里一个服务一张 `[mcp.<名字>]`，名字照清单的写法，格子同上、不写 `name`（名字就是表名）：

```toml
[mcp.github]
command = ["npx", "-y", "@modelcontextprotocol/server-github"]
env = { GITHUB_TOKEN = { secret = "github" } }
```

1. 只认管理员的：服务一个核心一份、照核心的身份跑，同家目录里放的技能、脚本只认管理员的家目录（`skills.md`）。成员的个人设置里写了不起。
2. 不进配置清单（2026-10-13 主会话定）：一个服务是一张有结构的表，配置清单的类型装不下；`config.get`、`config.set` 不管它，读个人设置时这一张表跳过、不报 `unknown_key`。手写，或者经头的编辑页：`mcp.set` 加上、整条换掉一张，`mcp.remove` 删掉一张（施工 X-26，`protocol.md`「`mcp.*`」）；写之前照读这张表的规矩查，写法同 `config.set`（带版本、不覆盖同时的手改），只动那一张的字。网址的服务一样（施工 X-32 下）：`mcp.set` 交 `url`、`bearer`、`headers`，不交 `command`、`args`、`env`；交了 `args` 没交 `command` 的写不成，拒 `bad_params`，别的混写照读表的规矩拒。随包来的服务（个人设置里没写这个名字的）不经 `mcp.set` 改。
3. 写错了的：只丢那一个服务（G8），`miyu check` 照行报（`kind` 是 `mcp`，`code` 照清单的 `bad_mcp`），核心记一行 `WARN mcp settings problem`。
4. 同名的盖住包里带的（`31-软件包.md` 第五节第 3 条）：包里那个不起、它的功能不算；`miyu check` 报一句被谁盖住了（`mcp_covered`）。删掉个人设置里的，包里那个照包的开关起。
5. 挂在 `~home` 下（同家目录里放的脚本）：工具 `mcp__<名字>__…`，功能 `mcp-<名字>`，预设照它开关。写进去就算开着，没有包的开关；不想用的删掉，或者在预设里关。
6. 当场生效（施工 F-9 下）：核心起来时和包里带的一起起；个人设置手改了、核心重读时，新写的起，删掉的停下、工具出目录，改了的（命令、环境变量、给哪种会话、时限）停下再起、重新列；只动了注释、行号的不动。密钥变了的（施工 X-32 补，2026-10-21 项目主人定）：`secret.set`、`secret.delete` 落了盘、手改密钥文件被看到，配置服务换上新的一份，同一个循环醒来，拿这一刻每个密钥值的摘要和上一刻的比（摘要只在内存里，不落盘、不进日志）；`env`、`bearer`、`headers` 里引用了变了的名字、开着的服务（看管的那一格不是关着的：在跑、在起、在等、连不上停下了的）停下再起，连续失败从零数，个人设置里的一个个重起，包里带的照 `package.restart` 连同这个包的别的服务一起；人关掉的、包没开的不因此被拉起来。记一行 `INFO mcp restarted … why="secret changed" secret=<名字>`，只有名字。现在只有管理员一个账号、一份密钥文件；多用户那一段照账号分开。不轮询：醒的是配置服务换新的那一下。包里带的服务都被盖住了的包，软件包列表上照关着算（不卡在「起着」）。
7. 程序照命令第一项找：绝对路径照它；`~/` 开头的照系统的家目录换成绝对路径；带路径分隔符的相对路径照家目录接；只有名字的先找家目录下的 `bin/`，再照 `PATH`。工作目录、标准错误、目录缓存同包里带的：`state/mcp/<名字>/`、日志目录里的一份、`state/mcp/<名字>.json`。
8. 头上的状态（施工 F-9 补，网页的会话要的）：它们不是包，`package.list` 不列；另给 `mcp.list`（状态同软件包列表上服务的那一格，另带 `covers`；知道有哪些工具的带 `tools`，施工 X-26；列出来却没登记的带 `skipped`，见「怎么走」第三条第 4 条，施工 X-37；编辑页要的原来写的 `env`、`timeout_ms`，施工 X-27 补）、`mcp.restart`，状态变了推 `mcp.changed`（订阅软件包列表的连接收）。包里被盖住的那一格、全被盖住的包带 `covered: true`，头照它写「被个人设置里的同名服务替代」，分得出是被盖住还是关着（`protocol.md`「`mcp.*`」）。

**她看到的工具**：

1. 名字是 `mcp__<服务名>__<工具名>`（Claude Code 的写法：模型见得多，一看就知道是哪个服务的；`opencode` 是 `<服务名>_<工具名>`）。工具名里不合 `[A-Za-z0-9_-]` 的字换成 `_`，拼起来超过 64 个字符的截到 64 个：两件截成一样的，后一件不登记，记一行 `WARN`。
2. 说明照服务给的原文，超过 2048 个字的截断（Claude Code 同样截）；参数照它的 `inputSchema` 原样交。第三方的字不进登记簿（`26` 第十节）：她装了什么就看到什么，和自己写的技能一样。服务给了 `title` 的（空白的不算）当给人看的名字，不分语言、记在 `en` 下，调的那一刻记进结果，时间线上写它（施工 9-8 六补，`view.md`「标题那一句」）；没给的照工具名。
3. 访问类别：服务在工具的注解里写了 `readOnlyHint: true` 的算 `read`，哪一级都不问；别的一律算 `network`：工作区那一级问人，人能批「这一件以后都行」（`session/guard.md` 第四条）。注解是服务自己报的，只用来少问，不用来放宽别的。
4. 给哪种会话照 `venues`，不写只给本机的会话：群里、私聊里的会话碰不到装在这台机器上的 MCP 服务，要给的明着写。

**协议**：功能、开关照现有的 `preset.get`、`package.info` 出现（一个服务一个功能，编号 `mcp-<服务名>`）。每个服务的状态在软件包列表那一项的 `mcp` 里，一格同 `extension.status` 的一项：`state`（`off`、`starting`、`running`、`waiting`、`stopped`）、`failures`，在跑的带 `pid`，在等的带 `retry_in`，停下的带 `reason` 和标准错误的最后几行 `stderr`；状态变了推 `package.changed`（施工 X-4）。手动重启是 `package.restart`（施工 X-4，`package-pages.md`「开关」）：停下了的、想换个进程的都用它，连续失败从零数。头怎么显示归头。

### 怎么走

**一、说哪一版**（2026-10-11 主会话照 MCP 规范定）

MCP 在 2026-07-28 版去掉了 `initialize` 握手，每个请求在 `_meta` 里自己带版本和客户端的本事；之前的版本（2025-11-25 及更早）先握手、再说话。现在装得到的服务两种都有，所以适配器两种都说（规范叫「两个时代都认的客户端」）：

1. 进程起来，先发 `server/discover`（带 `_meta`：`io.modelcontextprotocol/protocolVersion` 是 `2026-07-28`、`clientInfo` 是 `miyu` 和版本、`clientCapabilities` 是空的）。
2. 回来 `DiscoverResult`：新时代，从它的 `supportedVersions` 里挑我们认得的最新的一版，之后每个请求都带它；回来 `UnsupportedProtocolVersionError`（`-32022`）的照它列的挑一版再发一次。
3. 回来别的错、5 秒没回：旧时代，发 `initialize`（版本 `2025-11-25`，本事是空的），服务答的版本在我们认得的里面（`2025-11-25`、`2025-06-18`、`2025-03-26`、`2024-11-05`）就照它说，再发 `notifications/initialized`；都不认得的这个服务起不来，原话记下来。
4. 认出来的时代跟着这个服务的配置一起缓存（命令、环境变量没变就还当它是那个时代），下次起来直接照它说；照它说不通了再从第 1 条重认。

两个时代在工具上差得不多：都是 `tools/list`（带 `cursor` 翻页）、`tools/call`、`notifications/cancelled`。旧时代的结果没有 `resultType`，当是 `complete`。新时代的 `input_required`（要客户端补信息再问一次）、旧时代服务反过来向客户端要东西（`sampling/createMessage`、`roots/list`、`elicitation/create`）都不支持：前者当这次调用出错，后者回「没有这个方法」（`-32601`）。这几样规范自己也在弃用（roots、sampling、logging，2026-07-28 版）。

**二、进程**（网址的服务没有进程，见第五条）

1. 什么时候起：核心起来时，开着的服务各起一个，在后台起，不挡 `ready`（I8）；之前没缓存过目录的，目录要等它起来、列完才进来。新装、新写进个人设置、在预设里打开的，当场起。
2. 起在哪：工作目录是核心的状态目录里这个服务的一格（`state/mcp/<服务名>/`），环境变量是核心的环境加上配置里写的；不进沙盒（照 Claude Code、opencode：MCP 服务多半要联网、要碰自己的文件，人装它就是让它做这些）。每次调用照访问类别问人。见下面「定了的」第 1 条。
3. 一个核心一份，所有会话共用（同 Claude Code 的「一个会话一套」不同：会话退下、载入不重启服务，不然有状态的服务——浏览器、数据库连接——每轮都要从头来）。
4. 退出了照扩展的办法退避着重启（`extensions.md`「怎么走」第 5 条）：退避 1、2、4、8、16 秒（最多 60 秒），连续 5 次停下；连着跑满 60 秒的下一次从 1 数。停下的原话、最后几行标准错误给头看。重启以后重新列目录。
5. 标准错误照扩展那样写进日志目录里它的一份，滚动着留（`extensions/stderr.rs`）。
6. 核心停下、关掉它时先关它的标准输入，等 5 秒，不退的杀掉（同扩展，也是 MCP 规范的 stdio 关法）。

**三、目录**

1. 列完的写进 `state/mcp/<服务名>.json`：时代、版本、每件工具的原文（名字、说明、参数、注解）。一样的不写。照什么认这个服务：命令的是命令和环境变量的写法；网址的是网址和请求头的名字（有令牌的另记一个 `Authorization`），不认值，换了令牌照旧能用（施工 X-32 上，记在缓存文件原来的两格里，以前的缓存照旧认得）。核心起来时照它先登记，进程还没好时调到的说暂时不可用。
2. 服务发 `notifications/tools/list_changed`（旧时代），或者新时代的 `ttlMs` 过了、下一次要用它时：重新列，变了换掉这个服务的那几件、写缓存。开着的会话下一个回合换工具面（`30-插件框架.md` 第九节），不在回合中间换。
3. 关掉（包的开关关掉、卸包、个人设置里删掉）：它的工具出目录，进程停掉。预设里关掉只管那个会话的工具面（整个关掉 `[features] mcp-<服务名> = false`，单件关掉 `[tools]`），不停进程：服务一个核心一份、所有会话共用，别的会话的预设可能开着它（施工 X-4 更正，原来写的是预设里关也停进程）。
4. **没登记的给人看**（施工 X-37：原来只在运行日志里记一行，人只会发现工具少了）：这一次列出来却没登记的，照原名排记在看管那一格。为什么三种：`name`（没写名字）、`parameters`（参数格式不是对象：列的时候就不收的，和登记时查出 `type` 不是 `object` 的）、`name_taken`（名字被占了：截到 64 个字以后和同一个服务的撞了，或者别的包、自带的工具已经叫这个）。`mcp.list` 的一项、软件包列表上服务那一格带 `skipped: [{"tool", "why"}]`（`tool` 是服务那边的原名，没写名字的是空的），没有的不写；`miyu check` 一个服务有没登记的报一条警告 `mcp_tool_skipped`，说哪几件、为什么，个人设置里的报在个人设置上，包带的报在那个包的清单上。只说这一次列出来的：照缓存先登记的那一回不记，每次列完照新的换；停下了的留着上一次列的（它的工具也留在目录里，第二条第 4 条）。
5. **服务的说明**（施工 X-39，2026-10-21 项目主人定重开）：连上以后服务交的 `instructions`（新时代在 `server/discover` 的回应里，旧时代在 `initialize` 的回应里，空白的当没有）记在看管那一格、写进目录缓存（多一格 `instructions`，以前的缓存没有照样认得），照缓存先登记的那一回也有。照扩展经 `system.append` 放的那几段走（`26-提示词.md` 第四节第 8 块）：一个服务一段，键是 `mcp:<服务名>`（包的编号里没有 `:`），核心的外壳 `core/mcp/instructions.txt` 圈着服务的原文（`<mcp-instructions server="…">`）；原文先去掉首尾的空白（施工 X-39 补：everything 的原文以换行结尾，外壳又换一行，结束标签前多一个空行），再截到 4000 个字符（`APPEND_MAX`，按字符）；已经钉下的会话照旧，工具面变的那一轮重拼时换成新的，原文里出现外壳的结束标签的，那一处的 `<` 写成 `\u003c`（同技能列表的转义），别的照原样。工具面上有这个服务的工具（先加载的也算）的会话才带；只跟工具面一起变：工具面变了的那一轮照这一刻的说明换，没变的照快照里钉的，服务升级换了说明原文也不读（appended 在 system 里，变了整个前缀的缓存都断）。Claude Code 同样把服务的说明放进 system（每个截到 4096 个字），工具先加载以后她照它知道什么时候去拿这个服务的工具。

**四、调用**

1. 她调到 `mcp__github__create_issue`：照权限策略判（访问类别见上）；放行了，适配器给那个服务发 `tools/call {name, arguments}`（新时代带 `_meta`），等回应。
2. 时限照 `timeout_ms`：到了没回的发 `notifications/cancelled`，这次交回「没在 N 秒内答完、可能做了一部分」（同提供者的说法）。人停了这一轮、工具被掐掉也发 `notifications/cancelled`。
3. 回来的内容换成结果块：`text` 照原文；`image` 存进会话属主的 blob、成图片块；`resource_link` 写成一行（名字和链接）；带文字的 `resource` 照文字，二进制的写一行说有多大、什么类型；`audio` 写一行说收到一段声音、没交给她。只有 `structuredContent` 没有文字的，照紧凑的 JSON 写成文字。`isError: true` 的算出错，原话照交。
4. 结果太长的截：文字加起来超过 10 万个字的，留前面 10 万个，结尾写一行截掉了多少（照 Claude Code 给 MCP 结果设上限：浏览器这类服务一次能回几万 token）。
5. 服务这时不在（起不来、退出了、正在重启）：交回暂时不可用，不等它。

**五、网址的服务怎么说话**（施工 X-32 上，`crates/miyu-mcp/src/http.rs`）

1. **转接**：一对内存管道，一头交给客户端（`Client::connect` 照常认时代、列、调、取消），另一头由转接读：客户端写出的每一行 POST 到 `url`，几条同时在路上的各发各的；`Accept: application/json, text/event-stream`，带上服务给过的 `Mcp-Session-Id`、协议版本（`MCP-Protocol-Version`：先照这一条自己 `_meta` 里写的，新时代的每个请求都写，施工 X-32 补，GitHub 的远程服务没有它回 `400`；没写的照握手回应里认出来的，握手那一次还没有，不带），和清单写的请求头（令牌已经换成 `Authorization: Bearer …`）。回来的是 JSON 的照一行写回去，是 SSE 的照 `data:` 一条一条写回去，`202` 的什么都不写；整段读完再写，不开流式读。一段回应至多读 64 MiB（同管道一行的上限，施工 X-32 下），再长的不往下读，当对方回错（第 2 条）。
2. **对方回错**（4xx、5xx、连不上、读到一半断了）：有编号的请求回一条 JSON-RPC 错误（`-32000`，原话是 `HTTP <状态>: <正文的头 200 个字>`），客户端照常了结；往这个服务的日志（`state/logs/mcp-<服务名>.stderr`，同进程的标准错误那一份）写一行。握手时 401、403 的连不上，照进程起不来那样退避、连续 5 次停下，软件包列表上照旧给日志的最后几行。
3. **会话过期**（带着会话编号回 `404`）：关掉这一对读写，交出一个「是过期断的」标记（`Opened::expired`）。看管的那一头：服务已经跑起来过（握了手、列完工具）的，这一次不算失败、不退避，马上重连、从认时代重来，「连续 5 次停下」不数它（主会话审时提）；还没跑起来就过期的照一次失败算，一直回 `404` 的服务不会原地打转。
4. **不另开 GET 的长连接**：服务主动推的只在 SSE 回应里收得到；工具变了靠新时代的 `ttlMs`、重连时重列。
5. **`http://` 只给本机地址**（`miyu_http::is_loopback_url`）：别的机器的不连，令牌一个字节都不发出去，日志里写 `http:// is only for this machine, use https://`，记成停下 `cannot_start`。日志里一律不写网址：有的服务把密钥放在网址的查询串里。本机地址不走代理，别的照环境变量（同下载软件包）。
6. **状态**：没有进程号（`State::Running`、`Starting` 的 `pid` 是空的），`mcp.list`、软件包列表上不带 `pid`；带 `url`，不带 `command`。

**六、提示词**（施工 X-39，2026-10-21 项目主人定重开，`slash.md`「是什么」第三种）

1. **列**：服务的本事里声明了 `prompts` 的（新时代看 `server/discover` 的回应，旧时代看 `initialize` 的回应），列完工具接着 `prompts/list`（照 `nextCursor` 翻页）；跟着工具一起再列（服务说工具或者提示词变了，适配器数成同一个；`ttlMs` 照工具的）。列不出来的记一行 `WARN mcp prompts not listed`，照没有算，不拖垮工具。不进目录缓存：服务不在时提示词也取不了。没有名字、参数没有名字的那一项不收（适配器那一层，名字照原样交上来）。
2. **名字**：`<服务名>:<提示词名>`，恰好一个 `:`（两个头的斜杠菜单照这个形状认）。提示词名照工具名的办法清理：不合 `[A-Za-z0-9_-]` 的字（空白、斜杠、`:`）换成 `_`，整个超过 64 个字符的截到 64 个；清理以后撞了同一个服务前面那个的不登记，记一行 `WARN mcp prompt skipped`，进看管那一格的 `skipped`（`{"prompt", "why": "name_taken"}`，第三条第 4 条）。取的时候照原名问服务。
3. **菜单、跑**：`command.catalog` 列在跑的服务的（`protocol` 参考 `packages`）；`command.run` 名字里有 `:` 的照它跑（`commands/prompts.rs`）：只在本机的会话里认（服务默认只给本机，通讯平台那边也没有菜单），谁能用同模板，预设跟着服务的功能 `mcp-<服务名>`，不另设功能；人跟的字照「切词」（`slash.md`「展开」第 3 条，带引号的是一个词，施工 X-40）一个一个照声明的先后对上，最后一个参数拿剩下的整段（正好是一个带引号的词的去掉引号），空的值（`""`）当没给，没声明参数的接在末尾；缺必填的 `command_failed`、`data.why` 照连接的语言说缺哪几个（`commands/failed/missing-arguments`，施工 X-39 补）。照服务的 `timeout_ms` 等 `prompts/get`，最多一分钟（同扩展的命令，`commands::ANSWER_WAIT`；`timeout_ms` 是给调工具的，可以长到一小时，人敲了命令在等；施工 X-39 补，2026-10-21 主会话照推荐定），到时没答的 `command_unavailable`、不带 `why`；服务回错的 `command_failed`，`why` 是原话。
4. **取回来的**：`messages` 照工具结果的规矩换成块（第四条第 3、4 款：文字照原文、太长的截、四种图存成会话属主的 blob、声音和二进制的资源写一行），`assistant` 的照原文并进来、前面不另加标记（会丢掉谁说的，Claude Code 也是这么并的），并成这个人说的一句，之后同模板命令（`message.user` 带 `command`，时间线上是人敲的那一行）。
5. **出错**：服务不在跑、没列出这一个的 `command_unavailable`；服务回错（`-32602` 这类）、回 `input_required`、到时没答的 `command_failed`，`data.why` 照原话；没有这个服务的 `unknown_command`。

**七、资源**（施工 X-41，2026-10-21 项目主人定：照规范、照推荐，先给她两件工具，只改核心）

1. **认**：服务的本事里声明了 `resources`（`capabilities.resources` 是对象；新时代看 `server/discover` 的回应，旧时代看 `initialize` 的回应）。目录缓存多一格 `resources`（以前的缓存没有的照没声明），照缓存先登记的那一回也登这两件。
2. **两件工具，跟着这个服务登记**：`spec::register` 那一回在服务的工具后面多放 `mcp__<服务名>__list_resources`、`mcp__<服务名>__read_resource`，同一个包、同一个前缀，所以预设照功能 `mcp-<服务名>`、场所照 `venues`、先加载照 `need_load`、`miyu call` 照 `clicall`、等多久照 `timeout_ms`、服务停了一起撤、工具面照「提供的工具跟着登记走」换，都不另开特例。只开放资源、没有工具的服务也上了工具面，它的说明（第三条第 5 条）跟着带。访问类别是只读。服务自己有同名工具的，服务的赢，这一件不登记，记一行 `WARN mcp resource tool skipped`。说明、参数照 `core/mcp/list_resources.json`、`read_resource.json`。不做全局一对、带服务名参数的（Claude Code 是那样）：那要给执行器开一条到 MCP 的端口、工具面另开特例。
3. **`list_resources {cursor?}`**：一次一页，回一行 JSON `{"resources", "resourceTemplates"?, "nextCursor"?}`，结构照 MCP 原样、不另写给她看的话。资源每项只留 `uri`、`name`、`title`、`description`、`mimeType`、`size`，模板每项只留 `uriTemplate`、`name`、`title`、`description`、`mimeType`，没有地址的那一项不收。模板只在第一页（不带 `cursor` 的）列，只列模板的第一页；服务没实现模板（回 JSON-RPC 的错）的照没有。结构化的那一份同时交 `miyu call`。
4. **`read_resource {uri}`**：`resources/read`，每一段照工具结果换块（第四条）：文字原样；类型是图的二进制换成图，照工具交的图收（张数、尺寸上限同别的工具，主会话 2026-10-21 定）；别的二进制写成 `core/mcp/not-shown.txt` 那一行，不灌原文；加起来超过 10 万个字的截。服务回错的原话交回。
5. **不做**：订阅（`resources/subscribe`、`notifications/resources/updated`）；资源变了的通知（`notifications/resources/list_changed`：列是现取的）；参数补全（`completion/complete`）；人在输入框里 `@服务:地址` 附资源。

**八、网址的服务登录**（施工 X-42 上，2026-10-21 项目主人定重开 OAuth，照 MCP 规范 2026-07-28 的 Authorization；`crates/miyu-mcp/src/oauth.rs`、`crates/miyu-endpoint/src/mcp/login.rs`）

1. **要登录**：网址的服务回 `401`、`WWW-Authenticate` 是 `Bearer`，这张表又没写 `bearer`、`headers` 里也没写 `Authorization` 的（`login::explicit`），转接记下挑战的原文（`Opened::challenged`），看管的一头照扩展的 `needs_approval` 那一路停下：`state: "stopped"`、`reason: "needs_login"`，不算失败、不退避（主会话 2026-10-22 同意，不另开状态），挑战记在看管那一格。写了令牌的照旧当连不上。
2. **`mcp.login {service}`**（只给人，远程连进来的也行：施工 X-42 下补，主会话 2026-10-22 定）：找服务（个人设置里的先，再找包里带的）；照挑战找受保护资源元数据（先用头里的 `resource_metadata`，没有的依次试 `/.well-known/oauth-protected-resource/<路径>`、根上的那一个），取 `authorization_servers` 的第一个，授权服务器元数据照规范的先后试（`issuer` 带路径的三种、不带的两种），`issuer` 一模一样、认 `S256` 才用；元数据写的 `resource` 要是这个服务或它的上一级。回 `{"url"}`（授权页）。
3. **客户端和回调的端口**：存着的那一份（同一个授权服务器的）照用，回调先绑记着的端口；没有的动态注册（RFC 7591：`native`、`token_endpoint_auth_method: none`、带 `refresh_token`，`redirect_uris` 写 `http://127.0.0.1:<真端口>/callback`）；记着的端口被占了的换一个随机端口，动态注册来的照新端口重新注册一次（主会话 2026-10-21 同意：授权服务器认为回调地址不对时只在浏览器里报错、不跳回来，核心收不到，所以照服务记住端口）。授权服务器不认动态注册、又没有客户端的 `oauth_unregistered`。回调只听字面的 `127.0.0.1`，路径 `/callback`，别的路径回 `404` 接着等。
4. **授权页**：`response_type=code`、`client_id`、`redirect_uri`、PKCE（`S256`，`code_verifier` 32 个随机字节的 base64url）、`state`（同样 32 个字节，一次一用）、`resource`（服务的规范网址：去掉 fragment、末尾的 `/`）、`scope`（`WWW-Authenticate` 写了的照它，没写的照受保护资源元数据的 `scopes_supported`，都没有的不写；有了范围、授权服务器认 `offline_access` 的接上）。
5. **等回调**（最多 10 分钟，同一个服务又登录的停掉上一个在等的）：`state` 对不上的回一页「没成」、接着等；`iss` 照 RFC 9207 那张表判，对不上的不换令牌、不显示它带的错误说明；人拒了的回「没成」。对上了的拿授权码换令牌（带 `code_verifier`、`resource`），换到的连同 `issuer`、客户端、端口写成一行 JSON，照 `secret.set` 那一路存进 `mcp-oauth-<服务名>`（整份重写、0600、照版本防并发改，记在登录的这个人名下，`secrets::store`）；回浏览器的那一页照登录的人的语言（`mcp/login-done`、`mcp/login-failed`）。
6. **连的时候**：这张表自己没写令牌、存着 `mcp-oauth-<服务名>` 的是登录来的（`launch::plan` 的 `login`），转接每个请求现场问令牌（施工 X-42 下，`miyu_mcp::http::Credentials`，端点的 `login/credentials.rs`），存着的令牌不写进连接计划；密钥变了照第五条那一路（施工 X-32 补）重起，所以登录完服务自己连上。
7. **只核心自己用**：密钥的值本来就不经协议交出去（`secret.list` 只列名字），扩展读不到；运行日志只写 `mcp oauth login started`、`logged in`、`token not given`（带 OAuth 的 `error` 码）、`issuer mismatch`、`refreshed`、`refresh failed`（带 `error` 码，施工 X-42 下）、`logged out` 这几行，带服务名，不写令牌、授权码、`code_verifier`、`state`（`tests/oauth_log.rs` 守着）。授权服务器的错误说明截到 200 个字，只进 `oauth_failed` 的 `data.why`。
8. **跟着清**：个人设置里删了这个服务、卸了带它的包（个人设置里还有同名服务的不清），`mcp-oauth-<服务名>` 连同端口一起删（`secrets::discard`，记在内核名下）。
9. **命令行**：`miyu mcp login <服务>` 调 `mcp.login`，印授权页的地址、试着打开浏览器（Linux `xdg-open`、macOS `open`、Windows `rundll32 url.dll,FileProtocolHandler`），照 `mcp.list` 等个人设置里的服务连上（最多 10 分钟）；包里带的印了地址就算；标准输入是终端的同时读一行，贴回来的交给 `mcp.login_finish`（施工 X-42 下）。`miyu mcp logout <服务>` 调 `mcp.logout`（施工 X-42 下，`cli/mcp.md`）。
10. **续期**（施工 X-42 下，主会话 2026-10-21、22 审过）：离 `expires_at` 不到 60 秒的请求前先续；服务回 `401` 的续一次、照新令牌把那一次请求重发一遍。续一次只跑一个（几个请求同时撞上的等它的结果：有的授权服务器续一次就作废旧的 `refresh_token`），照存着的 `issuer` 再取一次授权服务器元数据，`grant_type=refresh_token` 带登录时的 `resource`（存在那一份的 `resource` 里，X-42 上存的没有的照服务的规范网址）、`scope`、客户端（有密钥的照 `client_secret_post`），不另开定时器、用到才续。成了的照密钥现有的写法存回（新给了 `refresh_token` 的换上，没给的留旧的；整份重写、照版本防并发改，撞了重读再写，人手改的不丢），**不重起服务**：看管的一头先记下自己写进去的那一份的摘要（`Servers::wrote`），跟着个人设置的那一圈比密钥时这个名字的摘要和记下的一样的不算变了；人自己 `secret.set` 改了的、`mcp.login` 存的照旧重起。没有 `refresh_token`、续不成的（`invalid_grant` 这类）：转接记下挑战、关掉这一对读写，看管的一头停在 `needs_login`，运行日志 `WARN mcp oauth refresh failed service=… error=<OAuth 的 error 码>`。连之前就过期了的（核心停了一阵），第一个请求前先续。写了 `bearer`、`headers` 的服务没有这一套，回 `401` 照旧只了结那一次请求。
11. **权限范围不够**（施工 X-42 下）：服务回 `403`、`WWW-Authenticate` 的 `error="insufficient_scope"` 的，那一次调用照服务回错交给她，转接记下挑战、关掉这一对读写，服务停在 `needs_login`（规范的 step-up，不自动弹浏览器：核心不能替人点同意）；下一次 `mcp.login` 要的 `scope` 是存着的加上挑战里的（去重、照先后），授权页照它写。
12. **登出**（施工 X-42 下）：`mcp.logout {service}`（只给人）：在等回调的那一次登录一起取消，删掉 `mcp-oauth-<服务名>` 那一份（客户端、端口一起：「端口和 `client_id` 同进退」，主会话 2026-10-21）；服务照密钥变了那一路重起、回 `401`、停在 `needs_login`。没登录过的 `not_logged_in`。不吊销令牌（RFC 7009 规范没要求，授权服务器多半不给吊销的端点），只删本机的一份。`mcp.list` 的那一项存着这一份时带 `logged_in: true`，没有的不写（施工 X-42 下再补，主会话 2026-10-22 同意：终端、网页照它显示「登出」）；只看密钥文件里有没有这个名字，配置照最近换上的那一份读，推 `mcp.changed` 时同一处算。
13. **手贴回调**（施工 X-42 下）：浏览器不在核心那台机器上的（网页从别的机器连进来、远程连进来的头，施工 X-42 下补；在服务器上用 ssh 跑命令行、浏览器在自己的笔记本上），授权完跳到打不开的 `http://127.0.0.1:<端口>/callback?…`；`mcp.login_finish {service, url}`（只给人）把那一整段交给在等的那一次登录，读法同本机收到的（`miyu_mcp::oauth::pasted` 取路径加查询串、照 `callback` 读），判法同一处（`state`、`iss`，主会话补的第 3 条）：对上了、换到令牌存好了回 `{}`；`state` 对不上的、读不出来的回 `oauth_failed`，那一次登录照旧等着；`iss` 对不上的同本机回调一样拒、那一次登录结束；没有在等的 `not_waiting`。

### 出错

| 情形 | 怎么办 |
|---|---|
| 命令找不到、起不来 | 退避着重试，`WARN mcp not started`（服务名、原话）；头上看得到 |
| 两个时代都认不出（`initialize` 也失败、版本都不认得） | 停在起不来，原话记下来 |
| 工具名撞了（截断后、和别的包、和自带的） | 撞的那一件不登记，`WARN mcp tool skipped`（服务名、工具名、撞了谁）；`mcp.list`、软件包列表带 `skipped`，`miyu check` 报 `mcp_tool_skipped`（施工 X-37） |
| 参数格式不是对象、没写名字 | 那一件不登记，同上 |
| 调用时进程退出、回应写法不对 | 这次交回暂时不可用 / 出错，原话照交 |

### 守着它的

| 测试 | 守什么 |
|---|---|
| `crates/miyu-mcp/src/wire/tests.rs` | 一行认成回应、请求、通知；认不出的说为什么；写出去的一条一行；行尾去掉、太长的认出来 |
| `crates/miyu-mcp/src/content/tests.rs` | 工具照写法读、注解；没名字、参数不是对象的不收，分得出是哪一种（施工 X-37）；几种内容照类型读，缺格子的原样留着 |
| `crates/miyu-mcp/tests/era.rs` | 新时代答探、之后带 `_meta`；没有共同版本的照它列的报；规范的错照报不退回；旧时代回错、不回的退到握手，认得的旧版本照说、不认得的报；探的时候退出是断了；缓存说旧时代的不探，握不成再探；说明两个时代都留着 |
| `crates/miyu-mcp/tests/tools.rs` | 翻页列全、照先后；写法不对的那一项不收、说是哪一项；`ttlMs`；工具变了数一次 |
| `crates/miyu-mcp/tests/calls.rs` | 几种内容原样回来；工具自己出错不算出错、不认识的工具是服务的错；要补信息的不支持；服务反过来问的回 `-32601` 接着答，`ping` 回空的；等不及丢掉的发取消、连接照常；服务退出是断了；丢下客户端就关了服务的输入（施工 X-3 上） |
| `crates/miyu-endpoint/tests/mcp.rs` | 真起 `miyu-test-mcp`：开了起、会话里调得到，几种结果、服务回的错、写法不对、超时（发了取消）各自交回，新旧两个时代都通；关了工具出目录、标准输入关了它自己退出；核心重启照开关起；一直起不来的试 5 次停下、标准错误留着；调用中途退出说暂时不可用、重启后又能调；工具变了再列、换上；包自己登记的不许占 `mcp__` 的名字（施工 X-3 上） |
| `crates/miyu-endpoint/tests/view_tool_names.rs`（施工 9-8 六补） | 服务给了 `title` 的工具调的那一刻记进结果（记在 `en` 下），没给的不记；视图的标题照它、没给的照工具名；服务关掉以后再翻照旧 |
| `crates/miyu-endpoint/src/mcp/*/tests.rs` | 名字照写法换、截断；说明截断；访问类别只认 `readOnlyHint: true`；给哪种会话；结果换块的每一种、截断、只有结构化的；程序照绝对路径、包目录、`bin/`、`PATH` 找（施工 X-3 上） |
| `crates/miyu-tool/src/catalog/tests.rs` 的三条 `…service…` | 包自己的和服务的分开换；查不过的那一件跳过、别的照登；卸掉又装回的服务不算卸掉（施工 X-3 上） |
| `crates/miyu-endpoint/tests/mcp_cache.rs`、`src/mcp/cache/tests.rs` | 核心重启以后服务起不来，照缓存登记的在工具面上、调到暂时不可用；缓存记着旧时代的直接握手不再探；命令、环境变量的写法变了不用缓存，坏了的不用；`ttlMs` 过了用一次就再列；扩展开着服务就起、关了停下出目录；卸包清掉缓存和工作目录，用过的会话调到说已卸载（施工 X-3 下） |
| `crates/miyu-endpoint/tests/mcp_settings.rs` | 个人设置里的服务（施工 F-9）：`miyu check` 报写错的、被盖住的，`[mcp.<名字>]` 不算不认识的键；真起测试用的服务：写了就起、调到的是它、包里同名的不起、那个包照关着算、预设里一个功能；手改个人设置当场换：新写的起、包里同名的停，改了的照新写的重起，删掉的停、包里的照开关起回来 |
| `crates/miyu-endpoint/tests/mcp_settings_status.rs` | 头上的状态（施工 F-9 补）：被盖住的服务、全被盖住的包带 `covered`；`mcp.list` 列状态和盖住了谁，`mcp.restart` 换一个进程、失败从零数，没有的拒；订阅的连接收 `mcp.changed`，删掉的 `server` 是 `null`，不推叫 `~home` 的包 |
| `crates/miyu-endpoint/tests/mcp_skipped.rs`（施工 X-37） | 测试用的服务列几件登记不上的（没名字、没参数格式、参数不是对象、截断以后撞名）：软件包列表上那一格、`mcp.list` 带 `skipped`，照原名排，都登记上了的不写；`miyu check` 一个服务一条 `mcp_tool_skipped`，个人设置里的、包带的各报在自己的文件上；核心重启以后服务起不来、照缓存登记的不带 |
| `crates/miyu-endpoint/tests/mcp_instructions.rs`、`src/mcp/instructions/tests.rs`、`crates/miyu-session/src/appended/tests.rs`、`src/mcp/cache/tests.rs` 的 `an_older_cache_without_instructions_still_reads`（施工 X-39） | 服务的说明在 system 的最后、外壳带服务名、伪造的结束标签转掉、截到上限；工具面没变时换了说明原文，请求的 system 逐字节不变，工具面变了的那一轮换上；照缓存先登记的会话里也有；以前的缓存没有说明那一格照样认得；MCP 工具照服务带那一段，扩展放的照旧照说的 |
| `crates/miyu-mcp/tests/prompts.rs`、`src/prompt/tests.rs`、`crates/miyu-endpoint/tests/mcp_prompts.rs`、`src/mcp/prompts/tests.rs`、`src/commands/prompts/tests.rs`（施工 X-39） | 声明了 `prompts` 的才认，翻页列全、`ttlMs`，旧时代握手里的也认，提示词变了和工具变了同一个数；取回来的照 `role` 收；命令名恰好一个 `:`、清理、截到 64、撞名的进 `skipped`；参数照先后对上、最后一个拿剩下的、缺必填的说缺哪几个；真核心：菜单列出、参数拼好，跑出来的字和图并成一句、她说的照原文并进来、时间线是人敲的那一行，缺参数、服务回错的 `command_failed`，没列出的 `command_unavailable`，没有的服务 `unknown_command`，服务停了不列，预设关了服务的拒；施工 X-39 补：缺参数的 `why` 照连接的语言（中、英各断一次，三种语言的字都有），不答的照 `timeout_ms` 等（最多一分钟）、回 `command_unavailable` 不带 `why`，说明原文首尾的空白去掉 |
| `crates/miyu-mcp/tests/resources.rs`、`crates/miyu-endpoint/src/mcp/resources/tests.rs`、`tests/mcp_resources.rs`、`src/mcp/cache/tests.rs`（施工 X-41） | 声明了 `resources` 的才认、新旧两个时代；一次一页、照 `nextCursor` 拿下一页、只留那几格、没地址的不收；读回字、二进制，没有的照原话；图的二进制换成图；真核心：声明了的两件上工具面、没声明的没有，第一页带模板和下一页、第二页不带模板，读的字原样、图成图、别的二进制一行、没有的照原话，只开放资源的服务说明照带，服务自己的同名工具赢，预设关了的没有，照缓存先登记的也有 |
| `crates/miyu-mcp/src/oauth/tests.rs`、`tests/oauth.rs`、`src/http/tests.rs`、`crates/miyu-endpoint/src/mcp/login/tests.rs`、`tests/mcp_login.rs`、`tests/oauth_log.rs`、`crates/miyu-cli/src/mcp/tests.rs`（施工 X-42 上） | 规范和 RFC 里的例子（挑战、规范网址、两种 well-known 的先后、`issuer`、不认 `S256` 的拒、RFC 7636 的 `S256`、授权页、回调、RFC 9207 那张表、`scope`）；假授权服务器走一遍：发现、注册、PKCE、换令牌、令牌服务认，只照 well-known 也找得到，要精确端口的同一个端口照过、换了端口不跳回来，`code_verifier` 不对的换不到；`401` 记下挑战；真核心：要登录的停在 `needs_login`、不算失败，`mcp.login` 的授权页带 PKCE、`state`、`resource`、`scope`，点了同意令牌进密钥文件（0600）、服务连上带 `Bearer`、协议上只有名字，`state` 对不上的不认、对的照样等得到，第二次登录绑同一个端口、不重新注册，删了服务存的那一份清掉，写了令牌的不走登录；运行日志、系统日志里搜不到令牌、授权码、`code_verifier`、`state` |
| `crates/miyu-mcp/src/http/tests/credentials.rs`、`tests/oauth.rs` 的续期那一条、`crates/miyu-endpoint/tests/mcp_relogin.rs`、`tests/oauth_log.rs` 的续期那一段（施工 X-42 下） | 被拒的续一次、照新令牌重发，续不成的关掉、记下挑战，要更多范围的同样；续期换来新的一对、带同一个 `resource`，拒了的照 OAuth 的错误回答报；真核心：快过期的请求前续、不重起，`401` 的续一次重发，续不成的停在 `needs_login`，续期期间在跑的调用不断、人自己 `secret.set` 改了的照旧重起，要更多范围的停下、下次登录的范围并上，`mcp.logout` 删掉那一份、没登录过的 `not_logged_in`，贴回来的对不上的 `oauth_failed`、那一次登录还在等、贴对的连上，没在等的 `not_waiting`；运行日志里续成了、续不成的各一行，新的令牌、`refresh_token` 搜不到 |
| `crates/miyu-endpoint/tests/script_pages.rs` 的 `set_loading_moves_the_source_to_the_setting_and_the_next_face`（施工 X-27） | `mcp.list` 的 `need_load` 照 `loading.mcp.<服务名>` 从 `source` 变 `setting`，`on` 照 `features.mcp-<服务名>` |
| `crates/miyu-endpoint/tests/page_extras.rs`（施工 X-27 补） | 软件包列表上包带的服务带 `need_load`、`on`，改了跟着变；`mcp.list` 带回原来写的 `env`、`timeout_ms`，照它存回去个人设置一个字不变，没写的不带 |
| `crates/miyu-endpoint/tests/mcp_edit.rs`、`src/mcp/written/tests.rs` | 头改个人设置里的服务（施工 X-26）：`mcp.set` 加上、整条换掉，别的字不动、正在跑的照新写的换；写错的、随包来的拒、什么都不写；`mcp.remove` 删掉、没有的拒；`mcp.list` 起好列完的、照缓存登记的带 `tools`，不知道的不带；表写成点号、行内表、子表的照样换、删，`mcp` 写成放不下的样子的不改；网址的服务（施工 X-32 下，`tests/mcp_url.rs`）经 `mcp.set` 写进去、连上，`mcp.list` 照写的交回 `url`、`bearer`、`headers`，原样交回一字不变；混写的、留给核心的头拒 `mcp_invalid`，有参数没程序的拒 `bad_params` |
| `crates/miyu-mcp/src/http/tests.rs`、`crates/miyu-endpoint/tests/mcp_url.rs`（施工 X-32 上） | SSE 拆成一条条；经网址认时代（不认 `server/discover` 的退回握手）、列、调，JSON、SSE 两种回法都通，令牌和请求头每次都带、握手以后带会话编号和版本；401 连不上、日志里一行；连上以后回 500 的那一次马上拿到带原话的错；新时代的假服务（照 GitHub，`_meta` 的版本和头对不上回 `400`）认成新时代、列、调都通，每个请求的头和 `_meta` 一样，老时代的握手那一次不带版本头（施工 X-32 补）；回应超过 64 MiB 的那一次拿到「太长」的错、不往下读，连接照旧能用（施工 X-32 下）；会话过期连接断开、标记亮着。真核心：个人设置里写 `url` 的服务连上、`mcp.list` 带 `url` 不带 `pid`、会话里调得到；`http://` 别的机器的停下 `cannot_start`、日志说清；连着过期三次，每次照样回到 `running`、`failures` 一直是 0、重连了三次。缓存照网址和请求头的名字认、换了令牌照旧（`src/mcp/cache/tests.rs`）。清单：网址、令牌、请求头的写法，和命令混写、写错的报 `bad_mcp`（`miyu-config` 的 `package/mcp/tests.rs`） |
| `crates/miyu-endpoint/src/mcp/secrets/tests.rs`、`tests/mcp_url.rs` 的 `a_server_follows_the_secret_it_uses`、`tests/secrets_log.rs`（施工 X-32 补） | 密钥变了重起谁：新存的、删掉的、改了值的都算；引用照 `env`、`bearer`、`headers` 的 `{ secret = … }` 认。真核心：先配服务、没存令牌的连不上，`secret.set` 存进去以后自己连上、失败从零数；换了值的照新令牌重连；删了的停下重连；没引用的只握一次手；包没开的一个请求都收不到。运行日志有 `mcp restarted … secret=<名字>`，整份搜不到 key |
| `crates/miyu-endpoint/tests/mcp_status.rs` | 软件包列表上每个服务一格的状态：起不来的停下带原因、失败几次、标准错误，推送里也是；`package.restart` 把停下的拉回来、失败从零数，在跑的换个进程、工具照常；关着的、没有能重启的、没有的拒；预设整个关掉、单件关掉照常换工具面、写进「装了没开」那一行，`preset.get` 列出服务的工具（施工 X-4） |

真服务走一遍（施工 X-5，2026-10-11，`docs/construction/X-5-MCP真服务走一遍.md`）：官方的 `everything`、`filesystem` 两个参考服务（npm 上的版本，都说旧时代 2025-11-25），装成一个 `bundle`，真模型照只读注解不问人地调了读文件、求和、小图、结构化内容、资源链接，写文件照联网的一类要人确认、`miyu ask` 里不做；起来到能用约 1.5 秒；`everything` 起来就说一次工具变了，照常重列。

### 施工拆分（草案）

| 步 | 做什么 |
|---|---|
| X-1 | `miyu-mcp` 客户端和假服务：两个时代、列、调、取消、改目录 |
| X-2 | 清单的 `[[mcp]]`（连同 `miyu check`）；个人设置的 `[mcp.<名字>]` 随 F-9 |
| F-9 上 | 个人设置的 `[mcp.<名字>]`：读、个人设置跳过这一张表、`miyu check` 报写错的和盖住了的 |
| F-9 下 | 核心起个人设置里的服务：挂在 `~home` 下、盖住包里同名的、手改了当场起停、功能和预设；真服务走一遍 |
| F-9 补 | 头上的状态：被盖住的标 `covered`，`mcp.list`、`mcp.restart`、`mcp.changed` |
| X-3 | 接进核心：看管进程、登记、缓存、调用、暂时不可用、标准错误 |
| X-4 | 功能和状态：一个服务一个功能、逐件关（契约测试）；头上每个服务的状态、`package.restart`。工具加载一组随存根那一步 |
| X-5 | 真服务、真模型走一遍；作者说明里写怎么接 MCP |

### 还没有的

- 工具加载：W9 起不按组（`25-工具加载.md`），`[[mcp]]`、个人设置的服务写 `need_load` 的照它，这个服务的工具都算（施工 L-1）；没写的先加载，要全量的写 `false` 或 `"no"`（施工 L-3，2026-10-20 项目主人改，原来没写的全量）。个人设置的 `loading.mcp.<服务名>` 盖过它，单件工具的 `loading.tools.<工具名>` 再盖过服务那一项；头经 `tool.set_loading` 改，`mcp.list` 的 `need_load.by` 说是服务表写的（`source`）还是人改的（`setting`）（施工 X-27）。每个服务还有全局开关 `features.mcp-<服务名>`（`presets.md`「全局开关」），经 `feature.set` 改，`mcp.list` 的 `on` 是它现在的值。
- 脚本里能调（施工 M-2，设计 `39-脚本调工具.md` 第三节）：`[[mcp]]`、个人设置的服务写 `clicall` 的照它，这个服务的工具都算；没写的能，写 `false`、`"no"` 不能（写法同 `need_load`）。个人设置的 `clicall.mcp.<服务名>` 盖过它，单件工具的 `clicall.tools.<工具名>` 再盖过服务那一项。只改这一格也算改了，那个服务照旧重起。工具交回的 `structuredContent` 不管有没有文字都原样交给脚本（`Done.data`），出错的不交。`mcp.set` 没交 `clicall` 的照原来写的 `clicall = false` 留着：网页的编辑页还不认这一格，编辑一次别的格不会把它丢了。

- 网址的服务要浏览器登录的（OAuth）：2026-10-20 项目主人定推迟，2026-10-21 重开（「命令行先，头也做」）；登录施工 X-42 上做了，续期、权限不够、登出、手贴回调施工 X-42 下做了（「怎么走」第八条）；两个头的按钮由两个头做，CIMD 等项目主人定网址，见下面「待定（施工 X-42）」；令牌、请求头那一种施工 X-32 上做了。
- 补全参数（`completion/complete`）、Tasks 扩展、MCP Apps、资源的订阅：都先不接。服务的 `instructions`、提示词施工 X-39 接了（「怎么走」第三条第 5 条、第六条），资源施工 X-41 接了（第七条），都是 2026-10-21 项目主人定重开。
- 每个账号各一份服务进程（`24-威胁模型.md`）：等多用户那一段。

### 待定（施工 X-42：网址的服务的 OAuth）

第 1、2（本机回调）、3、4、7 题施工 X-42 上照推荐做了，第 5、6、8 题和第 2 题的手贴施工 X-42 下做了，见「怎么走」第八条；下面留着的是头（第 9 题）和 CIMD 的，做过的几题留着原来的推荐当来由。照 MCP 规范 2026-07-28 的「Authorization」（OAuth 2.1、RFC 9728 受保护资源元数据、RFC 8414 / OIDC 发现、RFC 8707 `resource`、RFC 9207 `iss`、PKCE）。每一题附推荐，主会话审过、要项目主人定的问项目主人。

1. **怎么知道要登录**。推荐：网址的服务回 `401`、`WWW-Authenticate: Bearer`，这张表又没写 `bearer`、也没在 `headers` 里写 `Authorization` 的，照规范找受保护资源元数据：先用头里的 `resource_metadata`，没有的依次试 `/.well-known/oauth-protected-resource/<服务网址的路径>`、`/.well-known/oauth-protected-resource`。找到了的，服务记成新的一种状态 `needs_login`：不重连、不算失败次数，`mcp.list`、软件包列表写出来，推 `mcp.changed`。写了 `bearer` 的照旧当令牌不对，回 401 就停（人写了令牌就是不走 OAuth）。
2. **登录从哪发起、回调谁收**。推荐：核心收。新方法 `mcp.login {service}`（只给人）：核心做发现、注册，起一个只听 `127.0.0.1` 的随机端口等回调，回 `{url}`（授权页）。头打开浏览器：命令行 `miyu mcp login <服务>` 用系统的打开命令，打不开的印出网址叫人自己点。核心最多等 10 分钟，收到回调就换令牌、存、重连服务、推 `mcp.changed`。浏览器不在核心那台机器上的（网页从别的机器连进来）回调到不了：授权完浏览器跳到打不开的 `http://127.0.0.1:…` 那一页，把地址栏整段贴回来（`mcp.login_finish {service, url}`）。
3. **客户端注册**。推荐照规范的先后：
   - 个人设置这张表写了 `oauth = { client_id = "…", client_secret = { secret = "…" } }` 的照用（预先注册，`client_secret` 可以不写）；
   - 授权服务器有 `registration_endpoint` 的动态注册（RFC 7591）：`application_type: "native"`、`token_endpoint_auth_method: "none"`、`grant_types` 带 `refresh_token`、`redirect_uris` 写 `http://127.0.0.1/callback`；拿到的照授权服务器的 `issuer` 记，授权服务器换了重新注册；
   - 都没有的，`mcp.login` 回 `oauth_unregistered`，说请人去服务那边注册、把 `client_id` 写进这张表。
   - 规范更推荐的「客户端元数据文档」（CIMD：一个固定的 https 网址放 Miyu 的客户端信息）**这一步不做**：要先定网址、开项目的 GitHub Pages，是对外发布的事，项目主人定了再单开一步。规范把动态注册标成了旧办法，但现在的服务多半还认它。
4. **令牌存哪**。推荐：存进密钥文件（`system/secrets.toml`，0600，同 `miyu login` 存的），名字 `mcp-oauth-<服务名>`，值是一段 JSON：`access_token`、`refresh_token`、`expires_at`、`scope`、`issuer`、`client_id`、`client_secret`（动态注册给了的）。服务表里不用写引用，核心照名字找。日志、事件、协议的回应里都不出现令牌、授权码、`code_verifier`；`secret.list` 照旧只列名字，扩展读不到值。
5. **续期**。推荐：`access_token` 到期前 60 秒、或者服务回 401 时，拿 `refresh_token` 换新的（同一个 `resource`、`scope`），成了存回、重发那一次请求。没有 `refresh_token` 或换不成的（`invalid_grant` 这类），服务记成 `needs_login`，推 `mcp.changed`，日志写一行 `WARN mcp oauth refresh failed service=… error=<OAuth 的 error 码>`，不写令牌。
6. **权限范围不够**（服务回 `403`、`error="insufficient_scope"`）。推荐：记成 `needs_login`，下次登录要的 `scope` 是以前要的加上这次要的（规范的 step-up），不自动弹浏览器：核心不能替人点同意。第一次登录的 `scope` 照规范：`WWW-Authenticate` 里写了的照它，没写的照受保护资源元数据的 `scopes_supported`，都没有的不写。授权服务器的 `scopes_supported` 里有 `offline_access` 的加上（要 `refresh_token`）。
7. **安全**。推荐全照规范，不降级：
   - PKCE 只用 `S256`；授权服务器元数据的 `code_challenge_methods_supported` 里没有 `S256` 的拒。
   - `state` 随机 32 字节，一次一用；回调里的 `state` 对不上的不认。
   - `iss` 照 RFC 9207 那张表校验；对不上的不换令牌，也不显示它带回来的错误说明。
   - 授权服务器元数据的 `issuer` 和拼网址用的一样才认。
   - `resource` 写服务的规范网址：去掉 fragment、末尾的 `/`，主机名小写。
   - 授权服务器只认 `https`（测试用的 `127.0.0.1` 例外）。
   - 回调只听 `127.0.0.1`、随机端口、只收一次，10 分钟后关。令牌只放 `Authorization` 头，不进网址。
8. **登出**。推荐：`mcp.logout {service}`（只给人），删掉密钥里的那一份，服务记成 `needs_login`；命令行 `miyu mcp logout <服务>`。
9. **头**。推荐：施工 X-42 只做核心和命令行。终端、网页的「软件包」页上 `needs_login` 的服务显示「登录」，点了调 `mcp.login`、打开回来的网址，由主会话和两个头排期另做。

### 施工时定的（主会话，技术题照推荐）

- 施工 X-3 上：先只接 `bundle` 带的，扩展包带的牵扯扩展的批能力，单做 X-3 下；目录分开管这一步就做好（`Catalog::replacing` 只换包自己的，`replacing_service` 只换一个服务的）。包自己经 `provide` 登记的工具不许用 `mcp__` 开头的名字（`bad_tool`，`problem` 是 `name`）。
- 施工 X-3 上：命令第一项只有名字的先找包目录的 `bin/`、再照 `PATH`：包自己带的服务程序和别的包的程序一样放 `bin/`；带路径分隔符的照包目录算。Windows 上 `npx` 这类是 `.cmd`，要经 `cmd /c` 起，留到 X-5 真服务走一遍时做。
- 施工 X-3 上：被人停下的调用不另看叫停的旗：内核到点掐掉这次调用，丢掉的那一刻客户端发取消，和超时同一条路。客户端丢下就关服务的标准输入（`miyu-mcp` 读的一头只拿弱的发送端）。
- 施工 X-3 上：服务回 JSON-RPC 的错照原话交（第三方的字，同服务给的说明）；写法不对、要补信息的没有原话，用已有的「内部错误」那一句（`core/tool-results/crashed.txt`），不新加字。新加的两句只有「没给她看的」（`core/mcp/not-shown.txt`）和「截掉了」（`core/mcp/cut.txt`），登记在 `26` 第十节。
- 施工 X-3 下：扩展包带的服务跟着扩展起停，`extension.restart` 只重启扩展进程（手动重启服务随 X-4）。缓存认命令和环境变量的写法、不认取出来的值。缓存、工作目录照图纸放 `state/mcp/` 下，不挪进包的状态目录（那是包自己的地方），卸包时照服务名另删。`ttlMs` 过了不当场列：先照旧调，同时叫任务列，下一个回合换工具面。
- 施工 X-3 上：图跟着 `Done::images` 交给执行器存成 blob，排在文字后面（同 `read` 交图）；收不了的（不是那四种、解不开、超上限）写一行。链接写成 Markdown 的 `[名字](地址)`。
- 施工 X-32 下：搜索来源里走 MCP 的几家（Exa、Parallel、Firecrawl、You.com）不改走网址的转接（2026-10-20 项目主人照推荐定）：`miyu-net/src/search/mcp.rs` 那一份只调一件工具、只要文字，跑得稳、有测试守着；改了要重写搜索的测试和假服务器，第一次连还多探一次 `server/discover`。两份各自守着，`miyu-net` 那份不另加功能。

### 定了的（2026-10-11 项目主人照推荐定）

1. **MCP 服务不进沙盒**：照 Claude Code、opencode，服务多半要联网、要读写自己的数据，关起来大半用不了。靠「人装的才有」「默认只给本机会话」「没标只读的每次调用都问人、可以批这一件以后都行」守着。
2. **工具名带前缀**：`mcp__<服务名>__<工具名>`，模型一看就知道是外接的。
3. **服务给模型的 `instructions` 先不放进去**：第三方的字会常驻 system；等实测哪个常用服务离了它用不好再加，加的时候放在那个服务的工具组里、跟着开关走。
4. **群聊、私聊的会话默认不能用**：清单或个人设置里写了 `venues` 的才能。
