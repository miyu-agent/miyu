## 施工单 LT-1：握手认交接票，`login.ticket` 带账号

状态：核心的分会话做。接入通讯平台设计 07 第四节 4.6 第 4 样（`miyu-im/docs/designs/07-多平台和Telegram.md`，2026-10-27 项目主人定「一套登录」），形状主会话和 f4 定的。主会话派时编号写 LG-1，那个编号已经是 `LG-1-日志隐私.md`，改叫 LT-1（交接票）。

### 为什么

项目主人要「一套登录」：终端登录了，`miyu web` 打开就是登录的；主网页登录了，进 im 网页也是登录的（设计 07 第二节第 6 条、3.6）。原来的交接票（施工 W-8 补）要接的一方先 `login.redeem` 换登录令牌、再拿令牌握手，两步；浏览器里直连核心的页面一步连上更好，网址里还要写票是谁的（`as`）。

### 做了什么

1. 握手的凭据多一种 `ticket`，待遇同密码（`hello.rs`、`login/ticket.rs` 的 `handshake`）：票对得上的当场作废（和 `login.redeem` 同一张表，`core.identity.tickets` 的 `take`），给票的账号造一个登录令牌，回应带 `login: {token, expires}`；连接照登录令牌连上的算（`Via::Login`，带新令牌的哈希和票的账号），登出那个令牌时连接跟着断。没有、过期、用过的 `bad_ticket`，回完断开，运行日志记 `bad ticket`；和别的凭据一起写的 `bad_params`，票不动。
2. `login.ticket` 的回应多带 `account`（票是谁的），给网页跳转网址的 `as`。
3. `login.ticket` 本来就收本机令牌的连接（`issue` 认 `Via::Token`），代码不改；设计 21 的 X6 补一条「交接票不是未选的那一条」：不替代密码，只把已经认出来的人从一处带到另一处，和本机令牌能要一次性码是同一份信任。
4. `login.redeem` 照旧保留：老的头还在用。`redeem` 和握手共用「用掉票、造令牌」那一段（`exchange`）。
5. 文档：参考文档 `connection.md` 中英两份（握手的凭据表、回应的 `login`、出错多 `bad_ticket`，交接票一节写两种接法、`login.ticket` 带 `account`）；`protocol.md`（握手第 3 条、方法表、握手被拒的运行日志、测试表）；设计 21 X6。

### 验收

1. 修前：新测试三条都红（`login.ticket` 没有 `account`；握手不认 `ticket`）。
2. `tests/login_ticket.rs` 三条：握手交票连上的是票的账号、带新的登录令牌、换出的认得、同一张票只能用一次（再握手、再 `login.redeem` 都是 `bad_ticket`、断开）；成员的票换出的是成员、不是管理员；不认得的票 `bad_ticket`；和本机令牌、密码、登录令牌一起写的 `bad_params`、票没用掉。登录的老测试照过。
3. 手写变异 6 个都红：票用了不作废、连接记成管理员、凭据互斥漏了 `ticket`、`login.ticket` 不带 `account`、认票后算成本机令牌、回应不带登录令牌。
4. `cargo xtask check` 全过、Windows 版 clippy、三平台 CI。
