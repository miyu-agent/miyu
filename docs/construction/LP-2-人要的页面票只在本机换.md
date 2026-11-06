## LP-2 人要的页面票只在本机换

状态：施工完，待 core1 审（2026-11-06）。来源 LP-1 施工单「要拍板的」第 3 条（推荐人要的 `via: "page"` 票也只在本机换），
core1 2026-11-06 照推荐定、派工。蓝图 `protocol.md`（`login.ticket` 一行、运行日志 `WARN` 那一行、守着它的两行）、
`extensions.md`（「替本机页面换票」第 5 条）；协议参考 `connection`（中英）`login.ticket`、`login.redeem` 一节；LP-1 施工单里
「人要的票不变」的三处改指这一张。

### 目的

LP-1 只把扩展替本机页面要的票记成「只在本机换」，人要的票一律哪儿都能换。网页软件的 `/ticket`（网页 W-1089：本机打开网页不要
凭据）出示本机令牌替本机页面要的 `login.ticket {via: "page"}` 也是同一种用处：页面拿到就在本机的 `/ws` 上握手用掉。票要是被带到
远程的连接上换，同样等于把主人的设备令牌交给了局域网上的人。两种 `page` 票该是同一道挡。

### `via` 有几种、谁在用（动手前查的）

| `via` | 谁要 | 在哪儿换 | 这一步 |
|---|---|---|---|
| `page` | 网页软件 `POST /ticket`（`miyu-web/src/ticket.rs`，用本机令牌）；核心拉起的扩展（LP-1，如 im 的页面服务） | 网页：页面在同一个本机地址上的 `/ws` 握手交 `ticket`；扩展：页面握手、扩展连接上 `login.redeem` | 人要的也改成只在本机换 |
| `open` | `miyu web`（网页软件 `open.rs`）；`--print`、`--format json`、`--ticket` 印出来交给桌面端外壳、托盘、终端、别的程序 | 印出来的 `#ticket=` 网址交给谁就是谁打开 | 不动：哪儿都能换 |
| 不写 | 网页软件后台页「在新标签页打开」im 的页面（`web-demo/packages/backstage/external.js`，`login.ticket {}`）；老的头 | im 的页面服务 `/login/ticket` 在它的扩展连接上 `login.redeem` | 不动：哪儿都能换 |

终端（`miyu-tui`）不要票。没有哪一种 `page` 票是设计成要在远程连接上换的：网页软件的 `/ticket` 对远程的（对端不是回环、带着反向
代理加的头）一律 403，页面只在本机地址上才去要（`served.js` 的 `onThisComputer()`），握手走的 `/ws` 对本机来的不写
`connection.remote`。所以人要的 `page` 票换的那一下本来就在本机的连接上，这一步不碍它（网页 origin/main `a3a38c39` 读过，只读）。

### 做了什么

- **票在哪儿能换只看 `via`**（`login/ticket.rs` 的 `Purpose::reach`）：`page` 的记成 `Reach::Here`，不管是人（本机令牌、设备令牌、
  密码连上的）还是扩展要的；`open`、不写的 `Reach::Anywhere`。谁能要哪种照旧（权利表、`issue` 的第二道都没动）。
- **换的时候照 LP-1 那道挡**：远程的连接（握手之前说过 `connection.remote`，不管地址，包括同机反向代理转来的回环）握手交它、
  `login.redeem` 它都回 `bad_ticket`，票当场作废，记一行 `WARN ticket for this machine refused off it`。
- **运行日志 `ticket issued` 的写法不变**：扩展的照旧多 `package`，人要的不带（原来照 `Reach` 分，改成照是不是扩展分）。
- 没加给模型看的字，没碰 `miyu-store`。

### 协议最终形状（core1 转两个头）

- `login.ticket {via: "page"}`：人要的、扩展要的都只在本机的连接上换得成；远程的连接拿它握手、`login.redeem` 都 `bad_ticket`，票作废。
- `login.ticket {via: "open"}`、`login.ticket {}`：照旧哪儿都能换。
- 回应、出错码、参数都不变。网页软件 `/ticket` → 本机页面 `/ws` 握手这一路不受影响。

### 定的技术题

1. **照 `via` 定在哪儿换，不再照谁要的**：扩展只能要 `page`，所以「扩展要的只在本机」是这条的特例，一处（`Purpose::reach`）管两种人。
2. **远程连接照样发 `page` 票**：说过 `connection.remote`、用密码登录着的连接要 `page` 票照旧给（不改谁能调），换的时候才挡。网页软件
   不会这样要（`/ticket` 只用本机令牌）；在发票处多挡一道要多一个拒绝的说法，不值。
3. **日志照是不是扩展分**：`by == "extension"` 的多记 `package`；人要的 `page` 票和原来一样一行 `purpose=page by=token`。

### 要拍板的

无。

### 怎么验

- `tests/login_ticket.rs`：
  - `a_persons_page_ticket_works_only_on_this_machine`：本机令牌要 5 张 `page` 票；本机握手、本机 `login.redeem` 换得到设备令牌；
    `192.168.1.30` 的远程握手 `bad_ticket`、回本机再握手也 `bad_ticket`（作废了）；`127.0.0.1` 的远程（同机反向代理）握手
    `bad_ticket`、回本机 `login.redeem` 也 `bad_ticket`；`::ffff:127.0.0.1` 远程用密码登录着的 `login.redeem` `bad_ticket`。
  - `a_persons_other_tickets_still_work_anywhere`：`open`、不写 `via` 的票，`127.0.0.1` 的远程握手、`192.168.1.30` 远程登录着的
    `login.redeem` 都换得到。
- `tests/ticket_extension.rs` 末尾「人要的票远程照旧换得到」改用 `via: "open"`（原来用的 `page`，这一步起不成立了）。
- 单测 `login/ticket/tests.rs`：`Purpose::reach` 三种。
- 现有的 `login_ticket`、`login`、`login_device`、`ticket_extension`、`login_log`、主程序的 `login_ticket` 照过。
- 变异（改坏、看对应测试变红、还原）：

| 改坏了什么 | 红的测试 |
|---|---|
| `page` 记成哪儿都能换（`Purpose::reach`） | `a_persons_page_ticket_works_only_on_this_machine`、`an_approved_extension_gets_page_tickets_that_work_only_on_this_machine`、`only_page_tickets_stay_on_this_machine` |
| 退回 LP-1：只有扩展要的记成只在本机 | `a_persons_page_ticket_works_only_on_this_machine` |
| `open` 记成只在本机 | `a_persons_other_tickets_still_work_anywhere`、`an_approved_extension_gets_page_tickets_that_work_only_on_this_machine`、`only_page_tickets_stay_on_this_machine` |
| 不写 `via` 的记成只在本机 | `a_persons_other_tickets_still_work_anywhere`、`only_page_tickets_stay_on_this_machine` |
| 换票不看本机（`Some(Reach::Here) if false`） | `a_persons_page_ticket_works_only_on_this_machine`、`an_approved_extension_gets_page_tickets_that_work_only_on_this_machine` |
