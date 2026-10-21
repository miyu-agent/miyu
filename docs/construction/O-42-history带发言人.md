## 施工单 O-42：`history` 在场所会话里写发言人

状态：合了（2026-10-21）。通讯平台的会话 2026-10-21 拿旧群聊记录实测查出、项目主人定了转来：她在群里查「刚才谁推荐了 bitwarden」，结果是 `#39 2026-10-21 12:35 user: bitwarden`，没有名字，她答不上，还编了一句「群记录压缩了」。

### 做了什么

- `crates/miyu-basesystem/src/history/entry.rs`：带 `venue` 的 `message.user`，「谁」那一格写发的人，同群里那一行（`miyu-assemble` 的 `group::sender`）：名字，没有的写平台身份；这一条 `show_ids` 又有名字的带 `id=`；对应表里有的标 `owner`、桥报的管理员标 `manager`；名字照模板的规矩转义成一行。基础系统不依赖组装那一层，这十几行照同一个规矩另写一份。
- 筛 `by: user` 照样算人说的；本机的头上说的照旧写 `user`。

### 验收

1. `crates/miyu-basesystem/src/history/tests/venue.rs`：几种发的人、转义、筛 `user`。
2. 变异 4 个都报红。
3. `cargo xtask check` 八道全过。
