## 施工单 CS-5：人停的核心，头不再自动拉起

状态：做完，等验收（分支 `step/OA-1`）。项目主人 2026-11-17 本机验收第 18 条定「停了就停，卡片带启动」。派工时叫 CS-1，和「等核心起来的时限放宽到 60 秒」撞号，接 CS-4 叫 CS-5。

### 目的

网页开着时 `miyu core stop`，核心断一瞬又被网页拉起来。人停的（`miyu core stop`、头上的「停止」、托盘的「退出」，都走 `core.stop`）头不再自动拉起，等人点「启动」或者敲命令；崩了、空闲退出、收到停的信号停的照旧自动拉起。

### 怎么做

1. 记号 `run/stopped`（`miyu-ipc` 的 `stopped.rs`）：核心收到 `core.stop`、回应以前写（`miyu-endpoint` 的 `connection.rs`）；核心起来拿到锁就删（`open_locked`）。
2. 拉起分两种（`miyu-ipc` 的 `start.rs`）：人要用的照旧 `connect_or_start`、`connect_or_start_bare`；头断线以后自己重连用新的 `reconnect_or_start`、`reconnect_or_start_bare`，没在跑又有记号的交回 `StartError::StoppedByUser`，不拉。拿到拉起锁以后再看一眼。
3. `StartError::StoppedByUser` 的说法 `core/start/stopped_by_user`：「核心已停止」「The core was stopped」「コアは停止されました」。
4. `miyu-client`：门面多 `reconnect_or_start`、`reconnect_or_start_bare`；`open::Core::reconnect`，人停的交回 `Refused { reason: "stopped_by_user", message: 照这台机器的语言 }`。
5. 命令行的命令（`miyu ask`、`miyu pkg …`、`miyu check` 这些）都照旧 `connect_or_start`：人要用的。

为什么记号由核心写、不由 `miyu core stop` 写：托盘的「退出」、头上的「停止」也走 `core.stop`，都是人停的，一处写全管到；回应以前写，别的头断线时记号一定已经在了。

### 给头的接口

| 什么时候 | 用哪个 | 人停的核心 |
|---|---|---|
| 人要用：头刚打开、点了「启动」 | `connect::connect_or_start`、`connect_or_start_bare`，`open::Core::connect` | 拉起 |
| 断线以后自己重连 | `connect::reconnect_or_start`、`reconnect_or_start_bare`，`open::Core::reconnect` | 不拉：`StartError::StoppedByUser`，说法 `core/start/stopped_by_user`；`Core::reconnect` 的 `Refused.reason` 是 `stopped_by_user` |

头见到它画「核心已停止」的卡片带「启动」按钮，点了照第一行拉。网页、终端、托盘要换成这一套才见效（现在重连用的 `connect_or_start*` 照旧拉起）。

### 蓝图改哪几节

- `ipc.md`：文件表、新加「人停的不自己拉起」一节、说法那一条。
- `client.md` 门面表；`core.md`「停下」第 10 条、测试表；`protocol.md` 方法表 `core.stop`。
- 协议参考 `connection.md`（中英）`core.stop` 一节：记号、两种拉起、说法。

### 验收

1. 改前红：`miyu-core` 的 `serve.rs` 的 `a_head_asking_it_to_stop_stops_the_running_sessions_first` 加了「回应时 `run/stopped` 已经在了」，在 `origin/main` 上红。`miyu` 的 `core_stopped.rs`（新）用到新接口，在 `origin/main` 上编不过：`miyu core stop` 以后记号在，`reconnect_or_start` 交回 `StoppedByUser`、`starting` 还是一次；`connect_or_start` 拉起、记号没了；它空闲退出以后 `reconnect_or_start` 照旧拉起。
2. 门禁十一项、长跑随机、Windows 目标的 clippy、`cargo xtask prompts`、`cargo xtask logs`。

### 不做什么

- 头（网页、终端、托盘）换接口、画卡片：各自的会话做，照上面「给头的接口」。
- 收到停的信号（`kill`、关机）不算人停的：分不清是人还是系统，照旧自动拉起。
