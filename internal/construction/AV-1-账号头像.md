## AV-1：账号头像

状态：主会话做，一步，2026-10-28 做完。接入通讯平台（im）设计 07 第 4.6 节第 8 样（核心编号「核心-戊」）。

### 一、为什么

主网页、im 网页的登录页要照 QQ 电脑版「选择账号登录」做：一排这台浏览器登录过的账号的头像。账号现在没有头像，只能画名字的首字。

### 二、定下的（技术题，主会话定；登录页只列这台浏览器登录过的是项目主人 10-27 定的）

1. **放哪**：账号家目录 `home/<账号>/avatar.<png|jpg|webp>`。删账号跟着走，不另开存储。
2. **规矩照人格头像**：PNG、JPEG、WebP，最大 1 MiB、宽高不超过 1024，照内容认。从 `personas/picture.rs` 挪出写、读两个函数共用（`replace`、`encoded`），不另抄一份。
3. **谁能改、谁能读**：
   - 只改自己的（`account.set_avatar`，带 `expect` 对照版本，同 `persona.set`）。
   - 读自己的谁都行；别人的只有管理员（`account.avatar`）。
   - 都不在管理员模式那一串里，因为都只碰自己的。成员名单放行这两个。
4. **头怎么拿**：握手回应、`account.list` 有头像的带版本 `avatar`，头照版本换缓存，图另取。没登录的人看不到这台 Miyu 上都有谁：这台浏览器登录过哪些、它们的头像，网页登录时自己记下。核心不加「不登录也能列账号」的口子。
5. 新的拒绝原因 `avatar_conflict`，其余沿用人格头像的 `avatar_not_image`、`avatar_too_big`。

### 三、做了什么

1. `login/avatar.rs`：`version`、`set`、`read`；`connection/accounts.rs` 分到它；权限表、成员名单、`methods-v1.txt`。
2. `personas/picture.rs`：挪出 `replace`、`encoded`、`Checked::expect`，人格头像改用它们，行为不变。
3. `hello.rs` 握手回应、`login/members.rs` 的 `account.list` 带版本。
4. 蓝图 `protocol.md`，协议参考 `connection.md`、`errors.md` 中英两份。

### 四、怎么验

1. 端点 `account_avatar.rs`：
   - 机主握手不带 `avatar`。
   - 成员换上一张，回版本，读回来原样。
   - 成员读机主的 `not_permitted`，管理员读成员的照给。
   - `account.list` 那一行带版本；成员再连上，握手带版本。
   - 版本对不上 `avatar_conflict`、不是图 `avatar_not_image`、太宽 `avatar_too_big`，都不动原来那张。
   - 删掉以后读是 `null`。
2. 原有人格头像、成员、登录、握手测试照过。
3. 手写变异 5 个全杀：读别人的不查管理员、不对照版本、握手不带、列表不带、照背景图的上限查。
4. 本机门禁全过。
