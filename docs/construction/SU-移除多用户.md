## SU：移除多用户，改回一个人用

状态：主会话做，分 SU-1 到 SU-6。项目主人 2026-10-28 拍板：移除所有多用户设计，让占用降下来；网页局域网能用（http + 访问密码）；终端远程走 SSH；两个头、接入通讯平台（im）跟着移除。移除完统一量一次占用，中间不每步量。

### 一、为什么

空闲匿名内存从 10-22（多用户开工前）的 6.2 MB 涨到约 10 MB；多用户让每个功能都多一层「归谁、谁能看、成员能不能用」，开发难度和项目管理都失控了，而且没有一个真实的第二个用户。朋友要用就各跑一份 Miyu；跨机器的 agent 留言以后走 PT-4（A2A 扩展包）；聊天室留给第三方扩展。

### 二、留着的（不是多用户，是安全）

1. 场所里不可信的外部身份、角色、判官、场所会话（QQ 群这一套）；场所会话的命令沙盒、上网不碰本机和局域网。
2. 扩展包当独立的身份，能力照批过的挡；核心拉起的扩展照旧只碰批过的方法。
3. 主人自己在平台上的号的对应（私聊她时认出是主人），只剩主人一份。
4. 本机才能用的方法：远程连上来的照旧拒，名单以后照「路径在不在同一台机器上」再审。
5. 交接票（`login.ticket`、`login.redeem`）：im 的网页从主网页跳过去还要用。
6. 「你的」头像（原来的账号头像，只剩一张）。

### 三、拿掉的

| 块 | 拿掉什么 |
|---|---|
| 账号管理 | `account.disable`、`account.enable`、`account.invite`、`account.invite_revoke`、`account.invites`、`account.list`、`account.register`、`account.set_admin`、`package.members`；握手的 `user`、`invite`；回应的 `account`、`admin`；拒绝 `account_disabled`、`bad_invite`、`register_first`、`not_permitted`（成员闸） |
| 管理员模式 | 请求外壳的 `"scope": "system"`、`caps.scope`、`admin_mode_required`；命令行 `--system` |
| 组 | `system/groups.json` 的管理员组；能力组 `ext.<能力>` 换成 `extensions.json`（包 → 批过的能力） |
| 包的账号 | 带程序的包不再是一个账号（包编号和用户名互相占位、包的家目录）；包的数据放 `data/<包>/`，身份照旧是包 |
| 每人一份 | 配置、密钥（系统一层 + 个人一层 → 一份）；用量按人；记忆按账号分层（Y-7）；人格、预设、技能、脚本、提示词和工具说明覆盖的「系统 + 自己」两层 → 「你的 + 出厂」；软件包系统层 + 个人层、个人的软件源（关掉某个包留着：出厂的卸不掉，照它关，项目主人 10-24 要的）；扩展 `[process] scope = "account"`（每人一份的扩展进程、`copies`）；成员自己的平台号；每人一份的场所会话（IM-2）；会话索引、项目、定时任务的按账号分；定时任务每账号 200 条 → 全机 200 条 |
| 成员的限制 | 成员沙盒、成员上网闸、成员只装不带程序的包、「成员也能用」开关 |
| 存储布局 | `system/` + `home/<账号>/` 两层 → 数据根一层（第五节） |
| OOBE | 注册机主账号；admin 只当系统、不能登录这一套 |

### 四、新的登录（只有主人一个）

1. **本机**：照旧凭只有你读得到的本机令牌（`token`），不用密码。
2. **访问密码**：只在要从局域网连时需要。`account.setup {"password"}` 设、改，只给本机令牌、一次性码连上的连接；`account.setup_code` 只给本机令牌的连接，交回一次性码，网页第一次设密码用（`#setup=` 照旧）。密码 8 到 1024 个字节，argon2id 存。改了密码，全部设备令牌作废。
3. **登录**：握手写 `password`，对了交回 `login: {"expires", "token"}`（设备令牌，30 天）；之后握手写 `login`。`code`、`ticket` 照旧。
4. **吊销**：`account.logout {}` 作废这一个连接用的设备令牌；`account.logout {"all": true}` 全部作废，用着它们的连接断开。
5. **限流**：60 秒内错 5 次，剩下的时间先拒，`login_throttled`（`data.retry_after_ms`，照旧）；密码不对 `bad_password`。原来照用户名数，改成照来源数：网页软件转过来的连接带着来源地址的照地址分，不知道来源的算一份。
6. **网页局域网**：网页软件照配置听局域网的地址，http；没设访问密码时只认本机来的连接。

### 五、新的数据根（SU-5）

```text
<数据根>/
├── config.toml  secrets.toml  journal.jsonl  trust.toml  access.json（访问密码的哈希）  logins.json（设备令牌）
├── personas/ presets/ skills/ commands/ scripts/ prompts/ tools/   你的，同名盖出厂的
├── packages/  extensions.json  data/<包>/
├── sessions/ trash/ index/ blobs/ workspace/ memory/
├── state/  run/
```

主人的部署在 SU 合进 main 以后由一次性脚本搬（停核心 → 整份备份 → 搬 → 逐项核数，赞助账本专门核），onebot 会话执行，时间项目主人定。不在产品里做兼容。

### 六、分步

| 步 | 内容 | 头、im 要跟着切 |
|---|---|---|
| SU-1 | 管理员模式、成员闸、账号管理、邀请、管理员组拿掉；新登录（第四节）；OOBE 不建账号 | 两个头：登录、OOBE、账号页、管理员模式 |
| SU-2 上 | 软件包每人自己那一层、个人软件源、`package.members` 和「成员也能用」、`miyu pkg --system` | 两个头：软件包页（`members`、`available`、`layer: "user"` 没了） |
| SU-2 中 | 批过的扩展能力从组表 `groups.json` 的 `ext.<能力>` 挪进 `extensions.json`，组表删掉 | 无（形状不变） |
| SU-2 下 | 每人一份的扩展（`[process] scope = "account"`、按人拉起的那一份） | 两个头：扩展页；im：每人一份 |
| SU-3 上 | 配置只剩一层：系统配置和个人设置并成一份（`personal` 层拿掉），每人一份的配置（`config/people.rs`）拿掉；密钥只剩一份（`secret.*` 不再有 `layer`；原排在 SU-3 中，同 PL-1 的共用那份缠在一起，并进来一步做） | 两个头：设置页、`/model` 的两份、密钥页；命令行 |
| SU-3 下 | 人格、预设、技能、脚本、提示词和工具说明的覆盖「系统 + 自己」两层并成一层；每人一个工具架子；`external.me` 并进主人对应表（用量、项目、定时任务按账号存放挪到 SU-5，跟存储布局一起改） | 两个头；im：平台号 |
| SU-3 末 | 并进 SU-4（第十八节第 6 条）：成员已经没有了，人格那一间只归主人；「间」带账号的那一格是存放的路径，归 SU-5 | 无 |
| SU-4 | 场所会话归包（IM-2 已随 SU-2 下拿掉）；包的账号（带程序的包是一个账号、包编号和用户名互相占位、包的家目录）跟着场所会话一起拿掉；成员沙盒、成员上网闸收成只管场所会话 | im |
| SU-5 | 存储布局一层；搬家脚本 | 无（头不碰路径） |
| SU-6 | 剩下的账号参数收尾、蓝图和设计（06 标作废）、作者文档；量占用 | 无 |

每一步：协议改了的，合之前把形状发给两个头和 im，大家同一步切；本机门禁、变异照常。

### 七、SU-1：做了什么

1. 端点的身份（`login.rs`）只剩主人：`Via::Login`、`Via::Password` 不再带账号，`Revoked` 只有「全部」「这一个」；`account.setup {password}`（本机令牌、一次性码连上的），一次性码连上的另交设备令牌；访问密码握手照来源（`connection.remote` 的地址，本机来的算一份）限流；主人那一行密码是空的算没设，回 `password_not_set`；设备令牌只认主人的 `logins.json`。删掉 `admins.rs`、`invites.rs`、`members.rs`、`oobe.rs`。
2. 握手（`hello.rs`）：凭据去掉 `user`、`invite`，`caps.scope` 不认；回应去掉 `account`、`admin`、`register`；远程来的什么都没写、又没设密码的回 `password_not_set`。`Peer.admin` 照「替主人干活」算，`admin_mode` 就是它。
3. 闸门（`connection/gate.rs`）只剩一次性码那一道；成员名单、管理员模式两份删掉。请求外壳的 `scope` 照收不理。
4. 交接票不再绑账号，回应不带 `account`；头像只有主人一张，`account.avatar` 不收 `account`。
5. 核心起来（`miyu-core/src/owner.rs`）：没有主人当场照系统用户名建一个、不设密码；「只能设置」的那一段（`Core::in_setup`）删掉。
6. 软件包：会话里属主是主人的装进系统那一层（`packages/port.rs`），不再提示进管理员模式；给模型看的 `software/basesystem/package/admin-mode.txt` 删掉（登记簿、蓝图跟着删）。
7. 命令行去掉管理员模式（`Rpc::in_admin_mode`、握手的 `caps.scope`、「加 --system」的提示）；`--system` 选层留到 SU-2、SU-3。客户端库 `miyu-client` 的 `connect_password` 去掉用户名，`account()` 删掉。
8. 协议 1 冻结的方法清单删掉 8 个账号管理方法（`account.disable`、`enable`、`invite`、`invite_revoke`、`invites`、`list`、`register`、`set_admin`）：项目主人 10-28 拍板移除多用户，协议还没发过版。`package.members` 留到 SU-2。
9. 测试：删掉只测成员、管理员、每人一份扩展的 21 个文件；登录、交接票、头像、软件包工具、包名占位照单人化重写。
10. 文档：参考 `connection`、`errors` 中英两份，蓝图 `protocol.md`、`core.md`，方法清单、扩展能调什么的表。

### 八、SU-1：怎么验

1. `miyu-core` 单测：第一次起来照系统用户名建主人、不设密码，有了照旧；用户名换成账号的写法，换不成的叫 `me`。
2. 端点 `login.rs`：一次性码只给本机令牌、只能设访问密码、设好换设备令牌、写在主人那一行；设备令牌连上的改不了密码；本机令牌改密码以后以前的设备断开；还没设的密码登录、远程空凭据回 `password_not_set`，设了以后空凭据 `bad_token`；同一个来源错 5 次先拒、别的来源和本机各算各的；主人那一行密码是空的照样算没设。交接票、头像、软件包工具（主人装进系统那一层、装回出厂的、打开扩展先问再开）各一组。
3. 总程序的测试：交接票、密码登录（`connect_password` 只带密码）、拉起核心。
4. 手写变异 11 个全杀；「主人那一行密码是空的」头一轮活了，补了照真机第一次起来写好的那一行才杀掉。
5. 本机门禁全过。

### 九、SU-2 上：做了什么

1. 软件包只剩出厂的、装上的两层：`miyu_store::packages` 去掉 `Layer::User`、`of_account`、`read_over` 和盖过系统那一份的 `overrides.rs`；端点删掉 `packages/scope.rs`（`Scope`）、`mine.rs`、`Core::packages_seen`，`package.list`、`info`、`files`、`logo`、`search`、升级都照 `Core::system_packages`。
2. `Core::packages_of(账号)` 是全部去掉关了的（`packages/own.rs` 的 `Visible`，一格缓存，全部或关了的换了当场重造）；包的账号照全部。关掉某个包（`packages.<编号>.off`）留着。
3. 谁能装：协议上 `package.install`、`remove`、`upgrade` 只给人（`connection/rights.rs`），人就是主人；会话里的 `package` 工具照属主，不是主人的（场所里替外人的会话）装、卸、升级、开扩展、看一眼要装的一律 `not_permitted`（`Port::managing`）。
4. 软件源：系统配置、主人设置里加的都认（同名的认系统配置的），索引都在 `state/packages/.sources/<名字>/`；替成员只连公网的那道闸（`Feed::guarded`、`Trust.testing`、`Core::with_source_guard`、下载的 `Wanted.guarded`）删掉。
5. 「成员也能用」：清单的 `[package] members`、开关文件的 `members`、`package.members`、`Core::members_may`、`barred_from_members`、列表的 `members`、`available` 一起删；后台页 `package.file`、`package.call` 的成员闸（`backstage::opened_to`）删掉；扩展的能力不再和「替谁干活」的那个人取交集（`extensions::held`）。不替主人干活的连接（平台桥）拿到的列表照旧裁掉路径、程序、MCP 写法（`for_others`）。
6. `miyu pkg --system` 删掉（没有管理员模式以后它什么都不做）。
7. 方法清单删掉 `package.members`；参考 `packages` 中英两份、蓝图 `protocol.md`、`packages.md`、`extensions.md`、`package-pages.md`、`tools/package.md`、`cli/pkg.md` 跟着改。

### 十、SU-2 上：怎么验

1. 会话端口：属主不是主人的会话看一眼、装、卸、升级、开扩展一件一件拒 `not_permitted`，什么都没装；列得出；主人的会话看得了（`packages/port/tests.rs`）。
2. 软件源（`tests/package_feeds.rs`，照单人重写）：主人设置里加的源搜得到、`package.sync` 列着、装进装上的那一层、本地库记源名；源删了升级时进 `gone`、再装 `unknown_source`；和系统配置同名的那一条不用。
3. 软件包列表不再带 `members`（`tests/packages.rs`）；关掉某个包照旧，再关一个当场跟着换（`package_off`）。
4. 手写变异 9 个：头一轮活了 5 个，补了 4 处测试（再关一个包、技能列表当场换；不替主人干活的会话列表里没有 `state`、`package.info` 没有 `source`；重装时已经装着的依赖不再装）以后全杀。剩下的一个是等价的：包的账号的设置那一层没人写，`packages_off` 对它先交空的和照读的一样（SU-4 连包的账号一起拿掉）。
5. 本机门禁全过。

### 十一、SU-1 补：网页改访问密码、知道设没设

网页切 SU-1 时提的两处（miyu-web 10-28）：从 `miyu web` 打开的网页走交接票换来的设备令牌，按 SU-1 的形状设不了、改不了访问密码；页面也不知道设没设（「允许局域网访问」没设时要灰着）。

1. `account.setup` 设备令牌连上的也能调（`login::setup`）：已经设过的要带对现在的 `old`，不带 `bad_params`，错的 `bad_password`，照来源限流同密码登录（`login::verify`，密码登录也走它）；还没设的不用带。改好了这个连接换一个新的设备令牌、照旧用得了，别的设备断开。扩展拉起的照旧 `local_only`。
2. 握手回应多 `password_set`（布尔）。
3. 主人对应表 `external.bindings`：写了非空值的身份就是主人，值不再查是哪个账号（头照旧写 `admin`）；`venue.binding`、`venue.session`、桥列会话都照它（`venues::owners_ids`）。SU-3 改成只列身份。
4. 怎么验：新的 `tests/login_device.rs` 两条（设备令牌改密码：不带、带错、带对、别的设备断开、旧令牌不认、新令牌认得、同一个来源错 5 次先拒；票换来的设备第一次设不用带、握手照实写设没设）；场所的测试夹具把对应表的值写成 `admin`；桥碰得到哪些会话补一条单测（`venues/sessions/tests.rs`：主人的碰得到，别的桥名下的、对应表里没有这个平台的、本机的碰不到）。手写变异 5 个，头一轮活了「属主得是主人」那一个，补了单测以后全杀；本机门禁。

### 十二、SU-2 中：批过的扩展能力挪回开关文件

1. 组表 `system/groups.json`（`miyu_store::groups`、端点 `login::read_groups`、`change_groups`）整个删掉：SU-1 拿掉管理员组以后它只剩扩展能力的 `ext.<能力>` 组。
2. 批过的能力记在开关文件 `system/extensions.json` 的 `approved`（包的编号到能力，照名字排；一样都没批的不写）：`extension.enable` 批的时候在开关那把锁里读好、记下这时声明的全部、和开关一起写回（`approval::approve` 改成直接改要写回的那一份）；卸包时开没开、批过的一起删（`approval::forget`，一次写）。出厂的照旧算全批过、不记。
3. 9-4 下上的开关文件本来就是这一格的写法，以前写下的 `approved` 照样认（Y-5 下上起读的时候扔掉）。主人部署的组表里的 `ext.*` 由 SU-5 的搬家脚本挪进来。
4. 怎么验：`tests/extension_approval.rs` 批了以后开关文件写着 `approved`、没有组表，升级多了的只问多的、记的是这时声明的全部；开关文件里记着批过的，起来直接拉起；`tests/packages_purge.rs` 卸了开没开、批过的一起清掉；`miyu-store` 开关文件读写的单测换成 `approved`。手写变异 5 个全杀；本机门禁。

### 十三、SU-2 下：一个包只有一份进程

1. 清单的 `[process] scope` 拿掉（`miyu_config::package::Scope`、`bad_scope` 和它的三种语言的话）：写了的照不认识的键报 `unknown_key`。已知的包（im、赞助）都没写。
2. 端点的扩展进程只认包（`extensions::Instance` 只剩 `package`）：每人一份的（`per_account`、`scoped`、`Core::start_for`、用到时拉起的 `provide/reach.rs`、`Provided::wait`、`copies_of`）删掉；扩展起来照开关一律拉起（`Core::launch`）。`extension.status` 不再有 `scope`、`copies`，`package.list` 的 `process` 不再有 `scope`；`port.list` 不再有 `account`。
3. `process.retire`、`process.keep` 和它们背后的 `retired` 状态、`system/kept.json`（`miyu_store::kept`）删掉：只给每人一份的那一份用，没有包调它们。方法清单跟着删。测试扩展的 `retire`、`if:` 两步删掉。
4. 握手交的配置、提供者表、后台页的 `package.call`、挂接点、搜索来源、声明的斜杠命令、到点推给扩展、界面块（`Key.account`、`Item.owner`）都照包找那一份，不再照（包，账号）。
5. 场所：每人一份的那一份在场所里替人做事（IM-2，`Bridge::Personal`）拿掉，`Bridge` 只剩平台桥；主人对应表在哪都认（`binds_for` 删掉）。`external.me`（BD-1）照旧，SU-3 和主人对应表并成一份。
6. 怎么验：编译、clippy 过；端点、存储、配置、core 的测试；清单测试改成 `scope` 照不认识的键报；反向调用、界面块的单测去掉每人一份的那几条。参考 `extensions`、`packages`、`venues`、`schedules` 中英两份，蓝图 `extensions.md`、`providers.md`、`package-pages.md`、`widgets.md`、`venues.md`、`schedules.md`，方法清单。手写变异 5 个杀 4 个，活的那个是等价的：推给扩展时跳过已经断了的连接，不跳过的话发也发不出去，照样记 `skipped`。本机门禁。

### 十四、SU-3：设置只剩一份（设计）

**定了的**（主会话 10-28，技术题）：

1. 配置层只剩出厂默认值 → 系统配置 → 项目配置（信任过、只收紧）。`personal` 层（`home/<主人>/settings.toml`）拿掉：每一项原来能写个人层的都改成写系统层（清单 `layers` 里 `personal` 换成 `system`），只能写个人层的那几项（`packages.<编号>.off`、`external.me` 这些）也一样。文件照旧是 `system/config.toml`，SU-5 挪到数据根。主人部署的个人设置由搬家脚本并进系统配置（个人设置里写了的盖过系统配置的同名项，同以前叠的先后）。
2. 协议：层的名字只剩 `system`、`project`。`config.get` 的 `files` 没有 `personal`；`config.set` 的 `layer` 可以不写（就是 `system`），写 `personal` 的 `bad_params`；`config.check` 的 `layer` 是 `system`、`project`；`config.schema` 每一项的 `layers` 里没有 `personal`；`provider.list` 每家没有 `layer`；`/model` 这类「用户 / 系统」两份的地方只剩一份。
3. 密钥只剩 `system/secrets.toml`：`secret.set`、`secret.delete` 不收 `layer`，`secret.list` 每项没有 `layer`。
4. 命令行的 `--system`（`miyu config`、`miyu provider login/logout`、`miyu mcp`、`miyu kb` 这些）一起拿掉：只有一份，不用选。
5. 人格、预设、技能、脚本、提示词和工具说明的覆盖：系统区（`system/…`）和主人家目录（`home/<主人>/…`）两层并成「你的」一层，同名的照旧盖出厂的、包带的；SU-3 下定放哪一处（SU-5 统一挪到数据根）。
6. 不在产品里做兼容：开发用的数据根里个人设置写的东西换了核心以后不生效，照搬家脚本手动并。

### 十五、SU-3 上：配置、密钥只剩一份（做了什么）

1. `miyu_config::Layer::Personal` 拿掉，清单 `layers` 里的 `personal` 换成 `system`（包清单 `[settings]` 的 `layers` 只认 `system`）；合并（`merge::Layers`）只剩系统配置、项目配置；生成的 Schema 只剩 `config.schema.json`（`settings.schema.json` 不再生成）。
2. PL-1 的「两层都配了的供应商另合一份共用的 `<编号>@system`」整套拿掉：`merge/system_copy.rs`、`key::system_copy`、`Reference::SystemSecret`、`Values::alias`、`model.list` 每家的 `layer`。主人部署里写成 `<编号>@system/<模型>` 的引用由搬家脚本改成 `<编号>/<模型>`。
3. 端点：`Config` 只有系统配置一份文件、一份密钥文件（`personal`、`own_secrets` 拿掉）；每人一份的配置（`config/people.rs` 的 `People`、`of_member`、`spread`）拿掉，`Core::mine` 只是交回核心手里那一份（还带着账号参数的调用处 SU-6 收掉）；MCP 服务从个人设置挪到系统配置的 `[mcp.<名字>]`；软件源的「和系统配置同名」（`SourceShadowed`）拿掉。
4. 协议：`config.set` 的 `layer` 可以不写，只认 `system`；`config.check` 的 `layer` 是 `system`、`project`；`config.get` 的 `files` 没有 `personal`；`secret.set`、`secret.delete` 不再有 `layer`（带了也不看）、`secret.list` 每项没有 `layer`；`model.changed` 的 `effort.from` 只有 `system`。
5. 命令行：`miyu config path/set/unset/edit`、`miyu provider login/logout`、`miyu mcp` 的 `--system` 拿掉；第一次设置写系统配置。
6. 给人看的字、样本：三种语言里「个人设置」的说法拿掉（参考文件开头、软件源说明、包清单 `layers` 的报错、第一次引导那一项）；系统配置里 MCP 服务的读法改名 `configured_mcp`；生成的样本重出、`settings.schema.*.json` 删掉，蓝图的样本块跟着换；`miyu config`、`provider`、`mcp`、`check` 的帮助页去掉 `--system` 和个人设置。
7. 怎么验：测试里两层叠的照一层改写（配置合并、供应商、思考强度、温度、命令行的写进哪一层的话、第一次设置写进系统配置）；只剩一层就没意义的删掉（两份供应商 `provider_copies`、个人设置盖系统配置的几条、和系统配置同名的软件源）；新加 `config.set` 不写 `layer` 写进系统配置、写 `personal` 参数不对；手写变异见 `~/.cache/miyu-remake/SU3a-mut`；本机门禁。
8. 搬家脚本（SU-5）要做的：个人设置并进系统配置（同名的个人设置赢）、个人密钥并进系统那一份、`@system` 引用改写。

### 十六、SU-3 下：你的东西只放一处（设计）

**定了的**（主会话 10-28，技术题）：

1. 人格、预设、技能、脚本、斜杠命令模板、提示词覆盖、工具说明覆盖：读的时候只认「出厂的 → 包带的 → 你的」，你的就是主人家目录 `home/<主人>/…`（写一直写在这里）。系统区的 `system/personas`、`system/presets`、`system/skills`、`system/scripts`、`system/prompts`、`system/tools` 不再读；主人部署里那几处的东西由搬家脚本并进家目录（同名的家目录那份赢；`persona.toml`、预设照原来叠的规矩逐项合）。SU-5 再整体挪到数据根。
2. 跟着拿掉：`layers::Layer::System`；人格的 `system`、`keeper` 和取「谁的家目录」的那一圈（`scope.rs`）；每人一个工具架子（`shelves.rs`、`machine_side`）；改动监视看系统区的那一半；文件围栏（`Fence`）和卸包剩下的检查里系统区那几处。
3. 协议不变：技能、脚本的 `from` 原来系统区的也写 `home`；人格、预设的 `remove`（`delete`、`restore`）照旧。
4. `external.me` 并进 `external.bindings`（样子 10-28 和 im 对过了，im 设置页只填号、写 `owner`）：只留对应表，键是平台身份，值不空就算主人本人；搬家脚本把 `external.me` 的每一项写成一条、值 `owner`。
5. 用量、项目、定时任务、知识库、会话这些按账号存放的，是存储布局，挪到 SU-5 一起改。


### 十七、SU-3 下：你的东西只放一处（做了什么）

1. 存储：`layers::Layer` 拿掉 `System`；人格、预设只叠出厂的、包带的、主人家目录的；技能、脚本只找主人家目录的和包带的（`skills::system_dir`、`scripts::system_dir`、`DataRoot::system_places` 拿掉）；列目录的围栏只认家目录几处和装上的包那一处。
2. 端点：工具说明、提示词的同名覆盖只读主人家目录（`ToolOverride::over` 两份叠的那一步拿掉）；改动监视只盯家目录；卸包剩下的检查只看家目录；`scope.rs`、每人一个工具架子（`shelves.rs`）、`machine_side` 拿掉，会话一律拿核心那一个工具目录。
3. `external.me` 拿掉，并进主人对应表 `external.bindings`（im 10-28 确认：设置页只填号、写 `owner`）；三种语言里那一项的字改成「我的平台账号」。
4. 协议不变：技能、脚本的 `from`、人格、预设的 `remove` 照旧。
5. 测试：样本人格装进主人家目录（`install_sample_persona` 多一个主人的参数）；要「下面一层」的测试改用装着的样本包（端点测试的 `packaged::samples`、`sample_persona_below`）；只剩一个人就没意义的（成员的架子、乙家目录里的人格、系统区盖出厂的）删掉，新加「系统区不再读」一条。手写变异见 `~/.cache/miyu-remake/SU3b-mut`（6 个，5 个逮住；把系统区的技能目录加回去那一个逮不住：列目录的围栏本来就不认系统区，加回去也读不到，等价）。
6. 搬家脚本（SU-5）要做的：`system/personas`、`system/presets`、`system/skills`、`system/scripts`、`system/prompts`、`system/tools` 并进主人家目录（同名的家目录那份赢，`persona.toml`、预设逐项合）；`external.me` 的每一项写进 `external.bindings`、值 `owner`。

### 十八、SU-4：场所会话归包，包不再各有一个账号（设计）

**定了的**（主会话 10-28，技术题；协议的形状等 im 回了再定）：

1. 会话只有一个属主：主人。场所会话（群、私聊）的属主也是主人，另记是哪个包、哪个场所开的（`session.created` 已经带场所）；存放先照旧在主人的会话目录里（SU-5 整体挪到数据根）。包不再各有一个账号：`[process] system_account`、「带程序的包是一个账号」、包的家目录、包账号的会话索引、`account_taken` 一起拿掉。
2. 谁关在沙盒里（原来照「属主是不是主人」）：改成照场所。本机的会话、对方在主人对应表里的私聊（主人本人）不关；别的场所会话（群、陌生人的私聊）照场所会话的沙盒、上网闸关着。这条照会话自己记的场所和对方算，载入时照样算出来，不另存。
3. 平台桥照包认：连接带着包编号，有 `act_for_external` 能力的就是桥（原来照「是不是系统账号」）。场所会话能碰哪些：这个包接的平台前缀的场所（原来照属主是不是自己）。
4. 存处：扩展的 `blob.put`、`blob.open` 和主人共用一个存处，照旧按 `blob_grants` 管；包之间、包和会话之间拷 blob 的那几处（`hand_over`、`copy_over`、包调用的回应）不用再拷。
5. 协议（10-28 和 im 对过）：`venue.binding` 回 `{"owner": true|false}`；`venue.session`、`venue.sessions` 回应不再带 `account`；拒绝 `no_system_account` 拿掉，没批 `act_for_external` 的照能力的拒法拒；清单里还写着 `system_account` 的照不认识的键，整份不收（im 换钉那一步同时删）。事件里外部身份的 `account`（对应表认出是主人的带主人账号）不动：日志要逐字节回放。`venue.session` 的 `profile` 照第 2 条的「信任」认。
6. 分步：SU-3 末（记忆按账号分层）并进这一步：成员已经没有了，人格那一间早就只归主人；剩下的「间」带账号的那一格是存放的路径，归 SU-5。SU-4 上 沙盒和上网闸照场所（不动协议）；SU-4 中 包账号拿掉、场所会话归主人、桥照包认、协议照第 5 条；SU-4 下 收尾（`acting_for`、`serving`、`memory_owner` 这些只剩主人一个的参数）。
7. 搬家脚本（SU-5）：`home/<包>/sessions`、`blobs`、索引、定时任务、用量里属主是包账号的，并进主人那一份。

### 十九、SU-4 上：关不关起来照一处算（做了什么）

1. 会话：`Create`、`Load` 的 `owner_is_admin` 换成 `confined`（外部身份的会话关起来）；会话日志、放行出沙盒管得住的地方都照它；端点只在 `sessions::confined` 一处算（这一步还是「属主不是主人」，SU-4 中改成照场所认，只改这一处）。
2. 成员的本机会话那一种拿掉：`Own`、`Reach::Own`、`Confined::only`、工作目录不在自己几处里的拒（`astray`）、命令沙盒只到自己几处；本机的会话都是主人的，`Callers` 的 `admin` 跟着拿掉（本机的就是主人）。
3. `Guard::for_session` 不再另传「是不是机主」，照造它时交的关不关算。
4. 协议不变。
5. 怎么验：成员本机会话的几条测试删掉，群会话的照旧；新加 `sessions::confined` 一条。手写变异见 `~/.cache/miyu-remake/SU4a-mut`（5 个：3 个逮住；`admin` 那一个随代码拿掉；关起来的会话也给机主自己的几处那一个等价：关起来的会话命令本来就拒，放行表多几处也跑不了）。本机门禁。
