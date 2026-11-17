## 施工单 LF-12 修：`/life` 那条路不吃光线程的栈

状态：施工中（2026-11-17）。

### 红在哪

main 04cd92807（LF-12、LF-13、LF-14 合了以后）的 CI，Windows 那一格红（run 44062113113），别的五项绿：`miyu-endpoint` 的 `life_command::inside_the_life_session_itself_it_works_too` 栈溢出（`STATUS_STACK_OVERFLOW`），整个测试进程退出。

### 根因

不是测试独有，是生产路径。这条测试在人生会话里连着打 `/life`、`/life kill`、`/life new`，是核心里调用层数最深的一条路：连接 → 方法分发 → 命令 → `/life` → 结束人生、新开会话。调试版里，一个 `async fn` 被调用时，调的一方那一帧要先放下它的整份状态，每一处 `.await` 各占一份；状态又是一层套一层的，越靠外的帧越大。

在本机（Linux，调试版）把测试线程的栈调小复现：1536 KiB 就溢出，2048 KiB（默认）刚好过，一直贴着线。用调试器看溢出那一刻各层的帧：

| 哪一层 | 占的栈 |
|---|---|
| 连接（`read_all`、`serve`、`serve_from` 等） | 约 350 KiB |
| 方法分发 `methods::call` | 130 KiB |
| `commands::run`、`fresh`、`resent::once` | 193 + 193 + 65 KiB |
| `/life` 命令那一块（`LifeCommand::run`） | 134 KiB |
| `commands::life::run` | 125 KiB |
| `life::act`（还没进到结束、新开里面） | 292 KiB |

通用的那几层本来就占九百多 KiB。`/life` 这条路自己五百多 KiB，其中 LF-12 在命令那一块里并排多放了「所有人生的列表」「没有人格的会话」两份状态，LF-14 给结束那一步加了查人格的名字，把它从「刚好够」推过了线。Windows 上帧再大一点就溢出。原来那个 `boxed()` 是在调用点才装箱，状态照样先在调的一方的帧上放一份，没省下来。

### 改了什么

深的那几步改成**在定义处就交回装好箱的**（`life::Heaped`）：调的一方只拿一个指针，自己那一帧不再放整份状态。

- `life::act`、`start`、`end`、`lives`；`commands::life::run`、`told`；`commands/life/bare.rs` 的 `run`。
- 通用的三处，一并改了（它们是这条路上剩下最大的）：`commands::fresh`、模板命令和 MCP 提示词命令的 `run`、`Sessions::create`。
- 去掉 `commands::life::boxed`。

行为一个字没变，只是这几步的状态放在堆上。

### 量出来的

同一条测试能过的最小栈（Linux，调试版）：改前 1536 KiB 溢出、2048 KiB 过；改后 896 KiB 过、768 KiB 溢出。768 KiB 时溢出的已经不是 `/life`，是开头的 `session.create`（新开会话本身在调试版里就要八百 KiB 上下，和这一步无关）。

### 守着它的

`life_command_stack.rs`：在一条 1536 KiB 栈的线程上把那一串走一遍。没改的时候在 Linux 上就溢出；溢出是进程退出，不会被当成一条普通的失败漏过去。

### 没管的

- 连接那一层的三百多 KiB、新开会话的八百 KiB 上下：所有请求、所有会话都走，不是这一步带来的；要再压另起一步。
- 发行版不受这个数影响（帧小得多）；会溢出的是调试版，开发、测试、CI 用的就是它。

### 验收

- 上面量的数；新测试改前溢出、改后过。
- `miyu-endpoint` 全套测试；门禁；三台 CI（这一步动的就是它红的那条路，先推分支跑）。
