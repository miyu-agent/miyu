## 项目

状态：施工 H-1 做了项目这一样东西（存、`project.*`、`projects` 流）；H-2 会话在项目里、H-3 附加提示词和留言的范围照设计 `38-项目.md` 接着做。协议见 `docs/reference/projects.md`。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-store/src/projects.rs` | `home/<账号>/projects.json` 的写法：一项的各格、查写法（名称、说明、颜色、工作目录、附加提示词）、读、照读到的版本写（0600） |
| `crates/miyu-endpoint/src/projects.rs` | `project.list`、`create`、`update`、`delete`：排着改（`Core` 的 `projects` 锁）、人格和预设照造会话那一套找、写成了报一声 |
| `crates/miyu-endpoint/src/projects/changes.rs` | 「哪个账号的哪个项目变了」的广播：一条是「账号、换行、编号」，订阅的连接只认自己账号的 |
| `crates/miyu-endpoint/src/subscriptions/listed.rs` | `projects` 流：和软件包、扩展的推送共用一个转发任务，回应是整张表，之后推 `projects.changed` |

### 怎么走

1. 改的几个方法先拿 `projects` 锁，读、改、照读到的版本写：两个连接同时改不会互相盖掉；有人这一瞬间手改了文件的写不进、回 `internal_error`。
2. 建、改都先查：各格写法不对的 `bad_params` 带 `data.field`，人格、预设照造会话的找法（`personas::resolve`、`presets::resolve`），没有的照它们的原因码；拒了的什么都不写。
3. 写空字符串的格当清掉；`unset` 列的回到「没写」；名称不能清。
4. 只给人（`connection.rs` 的 `people_only`：方法名 `project.` 开头的）。

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-store/src/projects/tests.rs` | 没有的是空的，写了读回来一样，没写的格不写，人格三种状态分得开；改过的不写、版本不对说是坏了；各格的写法；照编号找、改、拿掉 |
| `crates/miyu-endpoint/tests/projects.rs` | 建、列、改（`unset`、空字符串清掉）、删，编号照造的先后，存在家目录；写错的各格、没有的人格和预设、没有的项目拒、什么都不写；订阅的回应是整张表、之后推变了的整项和删了的、取消了不推 |
| `crates/miyu-endpoint/src/projects/changes/tests.rs` | 一条只推给自己账号的连接 |
| `crates/miyu-endpoint/src/connection/tests.rs` | `project.*` 只给人 |
