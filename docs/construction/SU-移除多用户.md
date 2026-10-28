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
| SU-3 | 配置、密钥、用量、记忆、项目、定时、平台号、头像改成一份 | 两个头：设置页的层；im：平台号 |
| SU-4 | 场所会话归包、IM-2 拿掉；包的账号（带程序的包是一个账号、包编号和用户名互相占位、包的家目录）跟着场所会话一起拿掉；成员沙盒、成员上网闸收成只管场所会话 | im |
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
