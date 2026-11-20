## 施工单 E-M 修：子代理第一块模型事实写对预设

状态：主会话做。终端集成的会话 2026-10-23 转来：项目主人在 Windows 虚拟机里实测，她答「挂了两个预设（空的和 full）」。

### 根因

模型那一块事实的预设由会话补（路由不知道预设，`actor/model.rs` 的 `hand`）。造会话、载入时第一次交给内核的那份限额（`open.rs`、`open/draft.rs`、`open/load.rs`）直接用路由的，预设是空的。子代理一造好就开第一轮，回合开始的那一块写成 `preset=""`；会话随后补上预设、再交一次，下一个边界又注入一块 `preset="full"`。主会话要等人说话，那时已经补上，所以只有一块。

### 做了什么

- `snapshot.rs` 多 `stamp_preset`：限额里模型那一块补上快照的预设，不用预设的 `none`。会话的 `hand` 和造会话、载入、草稿三处第一次交限额都照它。

### 验收

1. `crates/miyu-endpoint/tests/subagent_model_fact.rs`：真核心，主会话派一个子代理，模型端口报得出身份；子代理日志里模型那一块只有一条、写 `preset="full"`，主会话也是一条。撤掉造会话那一处的补，这一条照报来的样子红（先 `preset=""` 再 `preset="full"`）。
2. `cargo xtask check` 全过。
