## LP-1 扩展替本机页面换登录票

状态：施工完，待 core1 审（2026-11-06）。来源 im 网页「本机打开不要密码」（项目主人 2026-11-05 定）；派工单 core1 2026-11-06；
项目主人 11-06 定 B：只给本机免密（im 不另设密码，局域网照旧输核心存的访问密码；「或者在它那里核过密码的页面」那条作废）。
蓝图 `extensions.md`（能力、谁能调、「替本机页面换票」）、`protocol.md`（`login.ticket` 一行、运行日志、守着它的、能力个数）、
`packages.md`（能力个数、出厂的例外）；协议参考 `connection`、`extensions`（中英）；作者文档 `manifest`（中英）的能力表；设计 66
`model.driver` 改成第二十项。

### 目的

im 的网页想做到本机打开不用输密码，要替页面向核心换一张交接票。`login.ticket` 原来对扩展一律 `local_only`；直接放开等于任何扩展
都能换主人的设备令牌。所以加一项要人批的能力 `login.page`，出厂的也不算批过；核心这头再挡一道：扩展要的票只在本机的连接上换。

### 做了什么

- **能力 `login.page`**（`miyu-config/src/package/capability.rs`）：第十九项，排在 `act_for_external` 后面（同是「替人做事」的一类）。
  名字「替本机页面换登录票」、说明「它能让本机打开的页面不用密码登进来」，英、日对应，进 `resources/core/human` 三份。作者文档
  `manifest`（中英）的能力表加一行（`every_capability_is_in_the_authors_docs` 守着）。
- **出厂的不算批过**：`Capability::needs_person()`（只有 `login.page` 是真），`extensions/approval.rs` 的 `holds` 照它：出厂的包算批过
  声明的全部，这几项除外。所以出厂的包声明了它，`extension.status` 列在 `unapproved` 里，照「还有没批的不拉起」停着，人在
  `extension.enable` 带 `approve` 批了才起。装的时候批（PK-3）照旧认它（`valid` 认得十九个名字）。
- **谁能调**（`connection/rights.rs`）：`login.ticket` 的 `via` 是字 `"page"` 的要 `login.page`，没批的 `needs_approval`，
  `data.capabilities` 是 `["login.page"]`；不写 `via`、别的值（`"open"`、大小写不同、不是字、`null`）照旧只给人，`local_only`。
- **发票**（`login/ticket.rs`）：连接交进来 `Asker {via, package, here}`。人（本机令牌、设备令牌、密码）要的照旧，记成哪儿都能换
  （`Reach::Anywhere`）；核心拉起的扩展只在 `via` 是 `page` 时发，记成只在本机换（`Reach::Here`），别的组合 `local_only`（权利表挡在
  前面，这里是第二道）。每发一张记一行 `INFO ticket issued purpose=page by=extension package=<编号>`，人要的那一行不变；不记票。
- **换票**（`login/ticket.rs` 的 `exchange`）：照旧先用掉票；是 `Here` 的、换的连接不是本机的，回 `bad_ticket`（和不认得的一样，
  不说为什么），票已经作废，记一行 `WARN ticket for this machine refused off it`（不记票）。本机不本机照连接的 `Reach`：没说过
  `connection.remote` 的是本机。握手（`hello.rs`，`source.is_none()`）、`login.redeem`（`connection.rs`，`reach.is_local()`）两条路
  都交进来。人要的票哪儿都能换，不变。
- **设计 66**：第十节第 1 条、第十二节第 5 条、第十四节第 1 条的 `model.driver` 改成第二十项，写明第十九项被 LP-1 先占了。

### 协议最终形状（core1 转 im 和两个头）

- 扩展在扩展连接上：`{"method": "login.ticket", "params": {"via": "page"}}` → `{"ticket": "<64 位十六进制>", "expires": "<时刻>"}`，
  60 秒、一次性，同人要的。
- 没批 `login.page`（清单没声明、声明了没批、出厂的没批）：`needs_approval`，`data.capabilities: ["login.page"]`。
- 不写 `via`、写别的：`local_only`（照旧）。参数多别的格：`bad_params`（照旧）。
- 换：页面握手交 `ticket`，或者扩展在扩展连接上 `login.redeem {"ticket"}`，都只在本机的连接上成。网页软件说过 `connection.remote`
  的连接（不管地址）拿它握手、`login.redeem` 都回 `bad_ticket`，票作废。
- 人要的票（终端、网页、`miyu web`）形状、行为都不变。
- 头那边：能力列表多一项 `login.page`（`extension.status`、`package.install` 预览的 `capabilities`，名字和说明照连接的语言）；出厂的
  包声明了它也会出现在 `unapproved` 里。

### 定的技术题

1. **排第十九、`model.driver` 挪到第二十**：LP-1 先落地，设计 66 是首个正式版以后的步子；`login.page` 排在 `act_for_external` 后面，
   都是替人做事的一类。设计 66 三处改了编号，`model.driver` 在表里的位置（`search` 后面）不变。
2. **出厂例外放在能力上**（`Capability::needs_person`），不在批准的那一处写死名字：以后还有这类能力加一行就行；全部走 `holds`
   一处，`unapproved`、`held`、`approve` 自动跟着。
3. **票记着在哪儿能换**（`Reach::Here`、`Anywhere`），不另开一张表：同一个 16 张的上限、同一个 60 秒。
4. **远程换票当场作废**：拿到票的远程一方本来就不该有它，留着不如烧掉；拒的说法同不认得的 `bad_ticket`，不说「这是本机票」。
5. **核心只看 `Reach`，不另外解析地址**：核心只监听本机套接字、命名管道，没有 TCP 对端；唯一的地址是网页软件 `connection.remote`
   里说的。网页软件对回环的、不带反向代理头的连接不说这一句（`miyu-webserve` 的 `remote_request`：`to_canonical` 以后
   `is_loopback`，IPv4 映射进 IPv6 的照 IPv4 算），带反向代理头的回环也说（R3S-2），这时 `from` 可能就是 `127.0.0.1`。所以「说过
   就拒，不管地址」比「解析 `from` 看是不是回环」严，也不用重复一套判法。
6. **发票处的第二道**：权利表已经挡了 `via` 不是 `page` 的；`issue` 里扩展只在 `page` 时发，其余 `local_only`，防表以后被改松。
7. **INFO 那一行的格子沿用 LT-2 的**：`purpose`（就是 `via`）、`by`（这个连接怎么认出来的，扩展的是 `extension`），扩展的多 `package`。

### 要拍板的（报告里同样列给 core1）

1. **核心这一道挡的范围**：只挡「票被带到网页软件转来的远程连接上换」。扩展自己在扩展连接上 `login.redeem` 永远算本机，核心看不到
   扩展的 HTTP 对端；im 的 `/login/ticket` 那一路就是这样换的，所以本机页面这件事的真正把关在扩展（参考里写成义务）。
2. **反向代理在本机时**：对端是回环。网页软件靠反向代理加的头认出来、标远程，核心照拒；扩展自己的 HTTP 服务也要照这个认（参考
   义务第 1 条），认不出的反向代理（不加任何这类头的）在扩展那头会被当成本机——这是扩展侧的边界，核心挡不住。
3. **人要的 `via: "page"` 票要不要也只在本机换**（网页软件 `/ticket` 替本机页面要的，用本机令牌）：这一步没改（派工单「非扩展连接
   的行为不变」）。推荐以后也改成只在本机换：那张票本来就只给本机页面，换的那条路（网页软件转来的本机连接）也是本机的，改了不碍事、
   多一道防。

### 怎么验

- `tests/ticket_extension.rs`：没声明的要 `page` 的票 `needs_approval`（`data.capabilities` 对）、不写 `via` 和 `open` 的
  `local_only`；批过的要得到 5 张，本机握手换得到设备令牌、令牌认得，本机 `login.redeem` 换得到；局域网地址的远程握手 `bad_ticket`、
  回本机再用也 `bad_ticket`（作废了）；`127.0.0.1` 的远程（同机反向代理）握手 `bad_ticket`；`::ffff:127.0.0.1` 远程用密码登录着的
  `login.redeem` `bad_ticket`；批过了不写 `via`、写 `open` 的照旧 `local_only`；人要的票远程照旧换得到。
- `tests/extension_approval.rs`：出厂的夹具扩展多声明 `login.page`，`unapproved` 只有它；开关文件记上批准以后不列。
- `tests/login_log.rs`：两张票两行 `ticket issued`，带 `purpose=page`、`package=pages`；远程那次有 `ticket for this machine refused
  off it`；整份日志（`TRACE` 起）搜不到这两张票。
- 单测：`connection/tests.rs`（权利表：`page` 要能力，`open`、大小写、数组、`null`、不写都只给人）、`login/ticket/tests.rs`（票记着
  在哪儿换）、`miyu-config` 能力表先后和只有 `login.page` 要人批、`packages/approve/tests.rs` 装时批认它。
- 变异（改坏、看对应测试变红、还原）：

| 改坏了什么 | 红的测试 |
|---|---|
| `rights.rs` 去掉 `via: "page"` 那一行（扩展一律只给人） | `ticket_extension` 两条 |
| 出厂的也算批过 `login.page`（`holds` 去掉 `needs_person`） | `a_shipped_extension_still_asks_for_login_page` |
| `needs_person` 一律假 | 同上 |
| 换票不看本机（`Some(Reach::Here) if false`） | `an_approved_extension_gets_page_tickets_that_work_only_on_this_machine` |
| 扩展要的票记成哪儿都能换 | 同上 |
| 握手总当本机（`handshake(…, true)`） | 同上 |
| `login.redeem` 总当本机（`here: true`） | 同上 |
| 发票处不认扩展的 `page`（照旧 `local_only`） | 同上 |
| 日志那一行不记包编号 | `login_log` |
| 日志那一行记了票 | `login_log` |

另：只把 `issue` 里扩展那一格放宽到任何 `via`，测试不红：前面权利表照样挡成 `local_only`，那一格是第二道（技术题第 6 条）。
