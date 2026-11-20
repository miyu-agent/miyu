## 施工单 QU-3：额度写明是哪一家

状态：做完，等合（分支 `step/CB-1`）。2026-11-17 终端会话要的，本机验收第 6、34 条。

### 为什么

终端、网页的用量框写「{供应商} 额度情况」「{供应商} 余额」，供应商名是头照自己记的当前模型猜的：换了模型、池里换了成员、模型名和供应商名对不上时就写错。`session.quota` 查的是哪一家只有核心知道。

### 做什么

1. `session.quota` 有 `windows` 或 `balance` 的回应多一格 `provider: {id, name}`：`id` 是配置里那一家的编号（会话接下来请求的 `model.endpoint` 那一家），`name` 给人看：配置写了 `providers.<id>.name` 的照它（去掉两头空白），没写的照目录里对上的那一家的名字，再没有的照编号，同 `model.list` 的 `name`（施工 8-21）。回 `{}` 的不带。
2. 文档：蓝图 `protocol.md`「`session.quota`」第 5 条；协议参考 `sessions` 中英。

### 不做什么

- 头怎么画由终端、网页各自改。

### 验收

1. 改前红：`miyu-endpoint` `session_quota` 两条（opencode Go 没写显示名的照目录写 `OpenCode Go`、配置写了显示名的照它）、`session_balance::an_official_deepseek_session_gets_its_balance_as_written`（main 上回应没有 `provider`）。
2. 门禁十一项、`random::` 长跑、Windows 版 clippy、`cargo xtask logs`。
3. 实测：查额度要真的官方 key（opencode Go、DeepSeek 官方），手边没有；集成测试照真核心、真路由、假的额度接口走一遍。沙箱里地址写成环境变量的 `dev` 那一家照旧回 `{}`、不带 `provider`。
