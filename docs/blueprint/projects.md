## 项目

状态：施工 H-1 做了项目这一样东西（存、`project.*`、`projects` 流）；H-2 做了会话在项目里（进出、新会话照项目的默认、删项目挪出来）；H-3 做了附加提示词进她的 system、留言照项目筛。协议见 `docs/reference/projects.md`。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-store/src/projects.rs` | 数据根里 `projects.json` 的写法：一项的各格、查写法（名称、说明、颜色、工作目录、附加提示词）、读、照读到的版本写（0600） |
| `crates/miyu-endpoint/src/projects.rs` | `project.list`、`create`、`update`、`delete`：排着改（`Core` 的 `projects` 锁）、人格和预设照造会话那一套找、写成了报一声 |
| `crates/miyu-endpoint/src/projects/changes.rs` | 「哪个项目变了」的广播：一条是项目编号，订阅着的连接都推 |
| `crates/miyu-endpoint/src/projects/sessions.rs` | 会话在项目里（H-2）：项目在不在、这个会话能不能进（本机的主会话：没有父会话）、建好了移进去、删项目时挪出来 |
| `crates/miyu-kernel/src/session/meta.rs` | 会话现在在哪个项目：`session.meta_changed` 的 `project` 一路盖上去（空的是移出来），`Session::project` 交出来（H-2） |
| `crates/miyu-store/src/index/row.rs` | 会话列表的索引多一列 `project`（版本 7，H-2） |
| `crates/miyu-policy/src/project.rs`、`resources/core/project.txt` | 附加提示词那一块（H-3）：模板包一层，接在「关于你」后面，记进快照的 `project` |
| `crates/miyu-session/src/actor/persona/project.rs` | 回合开始照会话现在在哪个项目，读 `projects.json` 拼这一块；和快照里的不一样这一轮换快照（同扩展放的那几段）；子代理进不了项目、不带，分叉出来的跟着父会话带 |
| `crates/miyu-tool/src/sessions.rs` 的 `reaches`、`crates/miyu-session/src/sessions.rs` | 留言的范围（H-3）：列会话照项目筛，认会话、读、发话都照列出来的那一批 |
| `crates/miyu-endpoint/src/subscriptions/listed.rs` | `projects` 流：和软件包、扩展的推送共用一个转发任务，回应是整张表，之后推 `projects.changed` |

### 怎么走

1. 改的几个方法先拿 `projects` 锁，读、改、照读到的版本写：两个连接同时改不会互相盖掉；有人这一瞬间手改了文件的写不进、回 `internal_error`。
2. 建、改都先查：各格写法不对的 `bad_params` 带 `data.field`，人格、预设照造会话的找法（`personas::resolve`、`presets::resolve`），没有的照它们的原因码；拒了的什么都不写。
3. 写空字符串的格当清掉；`unset` 列的回到「没写」；名称不能清。
4. 只给人（`connection/rights.rs` 的表，施工 X-30：`project.list`、`create`、`update`、`delete`，订阅 `projects` 流也是）。
5. 会话在哪个项目只记在会话日志里（H-2）：`session.set_meta` 的 `project` 换成内核的 `SetMeta`，`null` 记成空的；移进的项目要在，会话要是本机的主会话（分叉出来的也有父会话）。`session.create` 带 `project` 的，没写的人格、预设照项目的，建好了再交一条 `SetMeta`（命令编号是请求的编号接 `/project`），回应的 `events` 是 `[1, 2]`。
6. 删项目（H-2）：照会话列表的扫法找出在这个项目里的本机会话，一个个移出来，再从文件里拿掉，回 `moved`。
7. 附加提示词（H-3）：回合开始照内核记的这个会话在哪个项目（`Session::project`），读那个项目现在的 `prompt` 拼一块，和快照里的比，变了整段 system 重算一次（下一轮生效）；读不成的照旧用快照里的、记一行。
8. 留言（H-3）：端点列主会话时照 `projects.json` 给每一个填项目、开没开；执行器照列表里自己那一行和每一个比（`reaches`）。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-store/src/projects/tests.rs` | 没有的是空的，写了读回来一样，没写的格不写，人格三种状态分得开；改过的不写、版本不对说是坏了；各格的写法；照编号找、改、拿掉 |
| `crates/miyu-endpoint/tests/projects.rs` | 建、列、改（`unset`、空字符串清掉）、删，编号照造的先后，存在数据根里；写错的各格、没有的人格和预设、没有的项目拒、什么都不写；订阅的回应是整张表、之后推变了的整项和删了的、取消了不推 |
| `crates/miyu-endpoint/src/connection/tests.rs` | `project.*` 只给人 |
| `crates/miyu-endpoint/tests/projects_sessions.rs` | 在项目里建的照项目的人格、预设、日志第二条移进去，明着写的照写的，没有的项目拒；移进、移出、列表带 `project`，没有的项目、空字符串、分叉出来的拒；删项目挪出来、回 `moved` |
| `crates/miyu-kernel/src/session/tests/meta.rs` | 移进、移出项目记下来，一样的不记，载入照日志算回来 |
| `crates/miyu-store/src/index/tests.rs` | 索引的 `project` 照最后一次移动，只改标题的照旧 |
| `crates/miyu-endpoint/tests/projects_scope.rs` | 她调 `sessions`：没开的项目只看得到同一个项目里的，开了的和外面互相看得到；附加提示词第一轮就带上、项目改了下一轮换上、移出项目下一轮没有 |
| `crates/miyu-tool/src/sessions/tests.rs` | `reaches` 的几种：都在外面、同一个项目、没开的和外面、开了的和外面、两个开了的、一边没开的 |
| `crates/miyu-policy/src/project/tests.rs` | 空的提示词没有那一块、字节一样；有的照模板、去掉首尾空白，接在 system 后面 |
