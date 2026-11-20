## 施工单 VW-1：主人这边的连接能只读地看场所会话

状态：做完，等主会话审、合（2026-11-14 写；项目主人 2026-10-23 定场所会话只读、不从头上交互，2026-11-12 定「查看会话只看不发」）。

### 为什么

主人在 im 网页点「查看会话」，网页用 `#session=<核心会话编号>` 打开群或私聊的场所会话，核心回 `session_not_found`（「会话不存在。」）。

根因在第 4 道（`connection/checks.rs` 的 `others_session`）：照 `sessions/confined.rs` 的 `reachable`、`reaches` 判，人的头（没有包的连接）只放 `venue == local || !confined`。群、陌生人的私聊这类关着的场所会话，带它编号的请求一律被拒，读的也拒。`blob.get`、`session.shown` 照 `attach/reach.rs` 的 `reachable` 判，同样拒。

网页那头（W-1168）已经能把「订到了、会话表里没有」画成只读、收起输入框，核心放开以后网页不用改。

### 改了什么

1. **一张只读方法的白名单，默认拒**（`crates/miyu-endpoint/src/sessions/peek.rs`）：`READS` 列人的头对关着的场所会话能调的方法，`peeks(caller, method)` 判「人的头（`caller.package` 为空）、方法在表里」。表外的一律照旧当没有这个会话。
2. **三处共用这一个判法**：
   - 第 4 道 `others_session`：`peeks` 成的放行。
   - `attach/reach.rs` 的 `reachable` 多一个参数 `method`：不是桥的连接原来只读得到本机的、没关的，现在 `peeks` 成的也读得到。`blob.get` 传 `"blob.get"`、`session.shown` 传 `"session.shown"`，拒的说法照旧（`unknown_blob`、`unknown_shown`）。
3. **`reaches` 不动**：`schedule.add` 的 `target.session`、HK-2 挑 `tool.after` 收件人还照它，语义不变（人的头照旧不能往关着的场所会话里排定时任务）。
4. **平台桥、扩展不变**：`peeks` 只认没有包的连接。桥照旧只碰得到它的平台的场所会话，读的也一样。

白名单（判据：会不会改会话的状态、日志，会不会往会话里交东西）：

| 方法 | 为什么算读 |
|---|---|
| `subscribe` | 订这个连接自己的事件流、视图流（`after` 补发照常），会话里什么都不变 |
| `unsubscribe` | 退这个连接自己的订阅；不放的话订了退不掉 |
| `view.page` | 按页读日志里的历史 |
| `view.detail` | 读一次调用的完整结果、差异，一次压缩的摘要原文 |
| `session.prompts` | 只读日志列问过的话，不载入会话（问话跳转） |
| `session.tools` | 照这一刻的配置算她下一轮的工具，不影响会话 |
| `session.context` | 照这一刻算上下文一块块各占多少，「只在调用时算，不影响会话」 |
| `session.usage_parts` | 读用量账本，照块分开 |
| `job.output` | 读后台命令的输出 |
| `blob.get` | 读会话里的附件（图片要显示得出来） |
| `session.shown` | 读她展示过的文件 |

后八个是 `connection/rights.rs` 早就归在「读会话里的东西」里的（扩展调要 `events.read`），去掉了 `session.list`（不带会话）和 `session.recap`（要请求模型、花钱、记 `model.called`）。

**读的、但没放的**（看会话用不上，拿不准就不放；要放的话往表里加一行）：

- `session.viewing`：只记这个连接开着哪个会话，不碰会话日志；但会改会话列表的 `viewed`、影响谁来弹后台通知。网页调它出错不报（`.catch(() => {})`）。
- `command.catalog`：读这个会话能用哪些命令；只读看的会话输入框收起，用不上。网页调它出错照不带会话的那份列。
- `usage.query`：读用量账本，`session` 只是筛选；不带 `session` 的本来就查得到。
- `session.export`：不改会话，但往硬盘上写文件，整份带走群聊；要不要给主人导出群会话是产品题。
- `session.recap`：见上，要请求模型。
- `subscribe` 的 `memory` 流带关着的会话：一并放了（同在 `subscribe` 里），场所会话没有记忆，回 `memory_unavailable`。

### 头要做什么

- 不用改：网页 W-1168 已经照「订到了、会话表里没有」画成只读。
- 写的方法对关着的场所会话照旧回 `session_not_found`，头照「会话不存在」说。

### 蓝图、参考改了哪几处

- `connection/checks.rs`、`attach/reach.rs`、`shown.rs` 的头注释、函数注释。
- 蓝图 `protocol.md`「关着的场所会话只给桥」（「怎么走」第 4 条）、`blob.get` 第 1 条、`session.shown` 第 2 条；`venues.md`「不在本机的头上」，「在哪」加一行。
- 参考 `venues`（中英）加「主人看场所会话」一段，`sessions`（中英）「谁能调」、`blob`（中英）`blob.get`、`session.shown` 的「谁能调」跟着改。

### 不做什么

- 不动 `reaches`、`schedule.add`、HK-2。
- 不放写的方法，一个都不放：说话、`command.run`、重做、撤销、打断、压缩、清空、换配置、`set_*`、分叉、改标题、删。
- 不改网页、im。

### 怎么验

- 改前红（把三处 `peeks` 临时去掉）：`owner_views_venue::the_owner_reads_a_confined_group_but_cannot_drive_it`（订阅被拒）、`owner_views_venue::every_method_outside_the_reads_is_refused_on_a_confined_group`（白名单里的回了 `session_not_found`）、`blob_session::the_bridge_reads_blobs_its_sessions_refer_to_and_nothing_else`（人的头取群里的图回 `unknown_blob`）。变异：往 `READS` 里加一个写的方法（`session.interrupt`），方法表全覆盖那一条报红。
- `crates/miyu-endpoint/tests/owner_views_venue.rs`：
  - 接 QQ 的桥开一个群、以群友身份发一张图；人的头订阅（`after: 0` 补来群友那一句）、`view.page` 读到条目、带会话 `blob.get` 取到原图；`session.send`、`session.redo`、`session.configure` 回 `session_not_found`，群会话日志一条不多。
  - 主人在 `tg:` 上的私聊：接 QQ 的桥 `view.page`、订阅回 `session_not_found`，`blob.get` 回 `unknown_blob`。
  - 方法表全覆盖：`docs/reference/methods-v1.txt` 冻结的每一个方法（门禁照代码查它全不全），加上 `subscribe`、`unsubscribe`，都带上群会话的编号调：白名单里的不回 `session_not_found`，别的一律回；群会话日志一条不多。测试里的白名单写一份同 `READS`，两边要一起改：表多了、少了，或者以后加的方法绕过了这一道，都报红。
- `bridge_venues.rs`、`audit_sec_1030.rs`（头往群里排定时任务那一条的前提）原来断言人的头订不到群，改成断言人的头说不了话，`schedule.add` 照旧被拒；`blob_session.rs` 人的头取群里的图从 `unknown_blob` 改成取得到。
