## 给界面用的门面 `miyu-client`

施工 S-1，设计 `32-仓库拆分.md` 第二节。终端、网页、接入QQ 这些头连核心要的几样都从这里拿：拆成各自的仓库以后，界面照 git 依赖钉住核心的一个提交，只依赖这一个 crate，核心里别的 crate 怎么改都碍不着它们。

### 在哪

| 文件 | 管什么 |
|---|---|
| `crates/miyu-client/src/lib.rs` | 门面：几组 `pub use`，`CoreCommand`，`testkit` 打开时的 `WORKSPACE_ROOT` |
| `crates/miyu-client/src/open.rs` | 照终端的样子连核心（出示本机令牌；施工 W-8 补起 `connect_login` 出示登录令牌、W-8 再补起 `connect_password` 照用户名密码握手并交回换到的登录令牌，都是替人做事的程序用，不读本机令牌；拒绝、连不上、没回一律交 `Refused {reason, message, retry_after_ms, data}`，`reason` 是核心的 `data.reason` 或 `unreachable`、`no_answer`、`disconnected`、`io`，`data` 是核心那份 `error.data` 原样、核心之外的是 `null`（施工 CA-2：接入通讯平台的页面要冲突时的 `current`、写错的 `problems`、续传的 `received`））、一问一答（`Core`；`Core::connect_waiting`、`Core::reconnect_waiting` 多收一个 `Slow`：拉起核心、等握手的回应各段过了 3 秒还没好各叫一次（施工 CS-6）；握手的回应照 `Patience` 最多等 10 分钟，过了是 `no_answer`（原来 30 秒）；`call` 一直没有一行到来时最多等 `WAIT` 30 秒，做得久的请求用 `call_waiting` 自己给时长，施工 CA-1：网页导出大会话的原始包），系统的办法开浏览器（`Browser`、`SystemBrowser`），这台机器的语言（`locale`）。从 `miyu-webserve` 挪来，行为不变（`webserve.md` 的「怎么用」照旧） |
| `crates/miyu-client/src/tests.rs` | 门面上的每一样用一遍：拿掉了哪一样当场编不过 |

### 门面上有什么

| 一组 | 转出什么 | 原来在 |
|---|---|---|
| `connect` | `connect`、`connect_or_start`、`connect_or_start_bare`、`reconnect_or_start`、`reconnect_or_start_bare`（施工 CS-5：头自己重连，人停的不拉）、`spawn_detached`、`Connection`、`Ready`、`ConnectError`、`StartError`；施工 CS-6 起另有带 `_waiting` 的五个（四个连法加 `spawn_detached_waiting`）多收一个 `Slow`（`&(dyn Fn() + Send + Sync)`，过了 3 秒还没好叫一次，页面照它显示「核心还在启动」），`Patience`（3 秒、10 分钟）、`patiently`（照它等一个 future）。不带的照旧、不叫 | `miyu-ipc` |
| `open_files` | `raise`、`Raised`：常驻的头、扩展起来时把打开文件数的软上限调高（施工 YC-1） | `miyu-ipc` |
| `open` | `Core`、`Browser`、`SystemBrowser`（Linux `xdg-open`、macOS `open`、Windows `rundll32 url.dll,FileProtocolHandler`，网址整个是一个参数、不经过 shell；施工 AU-014，原来 Windows 是 `cmd /C start ""`，`&` 后面的被当成另一条命令，`open/browser.rs`）、`locale`；连不上、拉不起核心的 `Refused`（`unreachable`）原话照 `locale` 的语言从资源目录取字（施工 EN-2，`open/unreachable.rs`），取不到的照底层的原话 | 本 crate |
| `places` | `DataRoot`、`ResourceRoot`、`Env`、`Platform`、`locale`、`cache_root`；施工 S-6 上补 `write`（生成的文件，`generated`）、`config_file`（读配置文件那一层）、`Packages`、`Issue`（两层的软件包清单） | `miyu-store` |
| `texts` | `Human`、`Face`、`clean`；施工 S-6 上补 `HumanError`。`Human::with_dir`（从包文件夹补读一份字）是正式接口，接入通讯平台起来时用它把自己 `data/human/` 的字并进核心的字（施工 FX-7 恢复：施工 AU-P1d 只看了核心仓库，把它放到了 `testkit` 后面）。读字表时不认识的格不管（施工 FX-7，`store/resources.md`「怎么走」第 3 条第 3 款）：头、扩展钉着旧核心，读的是新核心装好的字表 | `miyu-store` 的 `human` |
| `protocol` | `SessionId`、`Said`、`Template` | `miyu-kernel` |
| `manifest` | `read`、`Value` | `miyu-config` |
| `log` | `install`、`LevelFilter` | `miyu-log` |
| `ids`（施工 S-6 上） | `ExternalId`、`VenueId`、`Seq`、`ContentHash` | `miyu-kernel` 的 `id` |
| `time`（施工 S-6 上） | `Timestamp`、`UtcOffset` | `miyu-kernel` 的 `time` |
| `events`（施工 S-6 上） | `Event`、`Body`、`By`、`Block`、`FormatError`；`shape`（施工 ED-3：读、写、改文件的调用参数规整成标准写法，`kernel/tools.md`「标准写法」） | `miyu-kernel` |
| `tooltext`（施工 S-6 上；feature `tool`，默认不开） | `load`（`spec`、`text`，包文件夹自己带字的照 `spec_in`、`text_in`；`say`、`LoadError`）、`Spec`、`Access` | `miyu-tool`、`miyu-kernel` 的 `tool` |
| `settings`（施工 S-6 上） | `Kind`、`Secret`、`parse::{read, why}`、`problem::{At, Code, Severity, got, nearest, tell}` | `miyu-config` |
| `http`（施工 S-6 上；feature `http`，默认不开） | `get`、`fetcher`、`Get`、`Got`、`Proxy`、`is_loopback_url` | `miyu-http` |
| `view`（施工 V-D；feature `view`，默认不开，终端换成照视图流画要的） | `Entry`、`EntryId`、`Body` 和各种条目（`User`、`Reply`、`Group`、`Thought`、`Tool`、`End`、`Media`……）、`Notice` 和各种旁白、`Change`、`Status` 和它的几格（`Doing`、`JobRow`、`Waiting`……）、`NestedItem`、`VERSION`：都能读回来（`Deserialize`；视图流的推送照 `Change::read(method, params)` 读，施工 V-D 修），可以没有的格缺着照没有，不认识的格不管（核心加格子，旧的头照样读得懂） | `miyu-view` |
| `seeds` | `from_image`、`from_rgba`、`Seed`、`MAX`：从一张图挑主题色的候选，头算壁纸的用它，和核心算人格头像的（`persona.list` 的 `seeds`）同一份（施工 P-6 补，`personas.md`「主题色、背景图」第 4 条） | `miyu-seeds` |
| `heap` | `tune`、`trim`、`stats`、`Stats`、`usage`、`resident`、`Usage`：进程的堆，和核心的同一份；头起来时、还没起别的线程时调一次 `tune`（施工 S-1 补，终端画图量出来的；`internal/designs/23-性能预算.md` F3） | `miyu-heap` |
| 根 | `CoreCommand`；`testkit` 打开时 `WORKSPACE_ROOT`（钉住的那份检出的仓库根，测试照它读 `resources/`、`internal/designs/samples/`） | 本 crate |

### 规矩

- **重的两组放在 feature 后面**（施工 S-1 再补，2026-10-17 网页量出来的）：`tooltext`（feature `tool`）、`http`（feature `http`）拉进 `miyu-tool`、`miyu-http`，连带 reqwest、rustls、ring、沙盒那一套；界面用不上，默认不开，只有接入QQ 打开。门面测试查这两个依赖是可选的、没有默认打开的 feature。接入QQ 搬出核心仓库以后（施工 S-6 下）核心里没有别的 crate 打开它们，门面在开发依赖里照打开两组依赖自己：测试、clippy 照旧编、测这两组，界面拿到的照旧默认不开；文档注释里这两组不写成链接（`cargo doc` 照默认的 feature 编）。

1. **代码不搬**：转出去的留在原来的 crate，门面只 `pub use`；`open` 是唯一挪进来的，因为它本来就只给头用。
2. **门面的样子是协议的一部分**：加、改、拿掉门面上的东西，照改协议的做法告诉两个头（提交号、改了什么）；两个头 2026-10-11 报的用法是这一版的全部。施工 S-6 上照接入QQ（桥和群聊内核）2026-10-16 数的用法多 `ids`、`time`、`events`、`tooltext`、`settings`、`http` 六组，补 `places`、`texts`（设计 32 第九节第 1 条）：接入QQ 拆出去以前，桥只依赖门面；`events` 的 `Block`、`tooltext` 的 `Access` 是桥用到、施工单没数到的，一起转出。
3. **层序**：第 3 层（`01-架构.md` 第九节），只引同层、下层。
4. **钉核心的仓库也是调用方**（施工 FX-7，2026-11-20 主会话定）：核心的 crate 不止门面有人用，接入通讯平台、终端、网页这些仓库照 git 依赖钉着核心的某一个提交，连同 `miyu-store`、`miyu-core` 这些。判一个公开的函数「只有测试用」、要放到 `testkit` 后面以前，先在这些仓库里也查一遍：施工 AU-P1d 只查了核心仓库，把接入通讯平台正式在用的 `Human::with_dir` 放到了 `testkit` 后面，终端的测试用的 `Packaged::as_released` 也是，施工 FX-7 恢复。它们读的资源目录又是装着的那一版核心带的，所以读资源的结构遇到不认识的格不管（上面 `texts` 那一行）。
