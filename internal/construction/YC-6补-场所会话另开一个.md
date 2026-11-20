## 施工单 YC-6补：场所会话另开一个

状态：施工中（2026-11-10）。

### 为什么

YC-6 以后，日志中间坏了的会话回 `session_read_only`，只能看、不能接着聊。项目主人 11-10 定：im 的场所会话撞上它，自动另开一个接着用，旧的照旧看得到。可 `venue.session` 找的是这个场所最新的主会话，坏了的那个照样是最新的，im 再问一次拿回来的还是它，绕不出去。

### 做什么

`venue.session` 多一格 `fresh`（布尔，默认 `false`）：写 `true` 的不找已有的，直接造一个新的；新的就是最新的，以后照常找回它。旧的照旧在列表里。参考文档 `venues.md`、`.en.md` 加一行。im 那边 YC-10 照它写：`session.send` 撞上 `session_read_only`，同样的参数加 `fresh: true` 调一次 `venue.session`，拿新会话重送这一条；一条消息最多新开一次。

### 验收

- `tests/venues.rs`：`fresh: true` 造出一个新的，之后不写 `fresh` 找回的是新的，旧的还在。改之前参数表不认 `fresh`（`deny_unknown_fields`），这条报红。
- 本机门禁全过。
