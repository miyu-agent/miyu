## 施工单 ACP-9：`miyu acp` 的模型列表跟着变、核心断了再连上

设计 73 第五节第 5 条、第八节。项目主人在 Zed 里实测 ACP-6 撞到的两处（2026-11-26）。

### 为什么

1. **ACP 的 agent 不在编辑器的模型选择器里**：沙箱装了 `acp` 包，`miyu ask --model acp/claude-acp` 跑得通、供应商 `acp` 的列表也存下了，
   Zed 的模型选择器却只列 magpie 的。查下来不是核心滤掉的：核心的 `model.list` 照常列包带的、不要密钥的这一家（不是 `no_key`），只是
   头一次问的时候它的列表还没拉过，核心照规矩先答空的、在后台拉，拉到了往订着配置的连接推 `models.changed`。沙箱的运行日志对得上：
   `miyu acp` 17:54:39.679 连上，`acp.json` 17:54:39.741 写盘，前后差 60 毫秒。`miyu acp` 一个进程只问一次 `model.list`、不订配置的推送，
   之后一直列不出。终端界面、网页本来就照 `models.changed` 重问（终端界面 `src/core/config.rs`、网页 `model-menu.js`），不受影响。
2. **核心重启以后 `miyu acp` 直接退出**：`miyu core stop` 换了核心，编辑器下一次新建会话报「Failed to Launch: Incoming transport closed
   (session/new)」。主循环读到核心断了就回退出码 1。

### 改了什么

- `acp/choosing.rs`（新）：每次 `session/new`、`load`、`resume` 接上会话时重问 `model.list`；握手以后订配置的推送（`subscribe {stream: "config"}`），
  收到 `models.changed`、`config.changed` 里有 `providers.`、`models.`、`pools.` 开头的键、配置的 `resync`（先重新订阅）也重问。问到的和原来
  不一样的，开着的会话各报一次整份 `config_option_update`；问不到的照旧用上一份。`Choices` 多了比较。
- `acp/relink.rs`（新）：断的那一下等着回的 `session/prompt` 照出错回（`-32603`，规范里的 `cancelled` 只给编辑器自己打断的），问编辑器的确认、
  提问作废；当场照头自己重连的连法试一次（`reconnect_or_start`，人停的不拉，施工 CS-5）；没连上的，编辑器下一次要用核心（`initialize`、
  `session/close` 以外的请求）时照 `miyu ask` 的连法连（`connect_or_start`，核心没开就拉起），连不上的那一条回出错、之后再试；主循环还没
  顾上的断开先办完（`Rpc::closed`）。连上了重新握手、订配置，开着的会话照掉队的 `resync` 一个个重新订阅，再重问能选的模型。
  怎么连交给 `Reconnect`（`miyu acp` 用本机数据根的 `Local`，测试给自己的），`acp_on` 多这一个参数；`acp()` 拉起核心的命令改成可以用几次的 `Fn`。
- `server.rs`：断了不退出（编辑器关了管道才退）；不属于哪个会话的推送交给 `choosing.rs`；握手挪进 `relink.rs`。
- 测试夹具：`tests/support/restart.rs` 在同一个数据根上重启核心（照收到停的信号那样停下会话、丢掉监听器和连接、再起一份）。
- 文档：蓝图 `cli/acp.md`（「核心断了」一节、模式和配置项、测试表）、设计 73 第七、八节、用户手册 `guide/editors.md`（中英）。协议没变，协议参考
  不用改。

### 定下的

- 断了以后自己重连只试一次、不轮询（基本原则「避免轮询」）；人停的核心不替人拉，等编辑器要用时再拉，同终端界面、网页（CS-5）。代价：人停了
  核心又自己起了一个以后，编辑器什么都不发时开着的会话收不到推送，发了第一条请求才接上。
- 重问 `model.list` 不带 `refresh`：照核心的规矩，列整份的每家一分钟最多在后台拉一次，同终端界面开 `/model` 的框。

### 怎么验

- `crates/miyu-cli/tests/acp/choosing.rs`：不要密钥的本机供应商，列表是头一次开会话以后才拉到的：编辑器先只见配置里点名的 `near/a`，拉到以后
  收到带 `near/b` 的 `config_option_update`，再开的会话一开就有。把重问改回「只问一次」：六十秒等不到，红。
- `crates/miyu-cli/tests/acp/relink.rs` 两条：正在说的一轮遇上核心重启回出错、之后照常开新会话、原来的会话在别处切的模式报得到、说得了话、
  编辑器关了管道才退出（自己重连的那一次不连，走「编辑器要用时再连」）；自己重连连得上的，编辑器什么都不发，开着的会话照样报别处切的模式。
  把「断了就退出」改回来：两条都红。
- `crates/miyu-endpoint/tests/package_providers.rs` 多一条：包带的、不要密钥的扩展驱动供应商，头一次 `model.list` 答空的，推 `models.changed`
  以后再问列得出、都是 `ok`、没有 `key`。核心本来就对，这条守着 `miyu acp` 靠的那份约定，修之前也是绿的。
- 真程序在自己的沙箱里（装了本地的 `acp` 包，删掉 `acp.json`）：新开会话时模型分组只有 magpie，随后收到带「ACP 智能体」41 个模型的
  `config_option_update`；`miyu core stop` 三秒后 `miyu acp` 还在、核心没被拉起；再 `session/new` 成功、核心被拉起；编辑器关管道退出码 0。
  同一套换成 main 编的程序：等不到配置项更新，`core stop` 以后 `session/new` 读到管道关了。
- 门禁。
