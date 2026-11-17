## 施工单 ML-1：调扩展的方法带上调用方的界面语言

状态：做完，等验收（分支 `step/VS-batch`）。网页 W-1208 要的；语音会话「say 交付方案」同日转来。

### 目的

扩展的后台页、界面块、总线方法回给人看的字（报错、提示）不知道该用哪种语言：`method.call` 只带 `method`、`params`、`from`。斜杠命令那一条（`command.call`）早就带 `language`。照它的写法，`method.call` 多带 `language`：调的那个连接这一刻的界面语言。头经 `package.call`、`bus.call` 调的是头的，包经 `bus.call` 调的是那个包的连接的；`ui.language` 是 `auto` 的照那个连接握手报的 `locale`（`hello::Shaken::now`，同 `command.call`）。核心自己调的（SD-1 兜底放语音）不带。

SDK 那一半（调用期间 `Core::language()` 交回它）语音会话在 `miyu-sdk` 做。

### 蓝图改哪几节

- `package-pages.md`「`package.methods`、`package.call`、`method.call`」第 2 条。
- `protocol.md` 的 `package.call` 那一行。
- 协议参考 `extensions.md` 的 `method.call`（中英）。

### 做法

`methods/later.rs` 把这一刻的 `peer.language` 交给 `backstage::call`、`backstage::bus::call`；`backstage::calls::forward` 收 `(from, language)`，`language` 有的写进 `method.call` 的参数；核心自己调的（SD-1 的 `bus::call_by_core`）交 `None`。

### 验收

1. 改前红：`miyu-endpoint` 的 `backstage::a_page_calls_the_methods_its_program_registered`（转过去的参数带 `"language":"zh"`）、`bus::names_are_listed_and_called_without_package_ids`（同）、`package_call_ext`（扩展经总线调带它自己连接的 `"language":"en"`，人调的带 `"zh"`）。
2. 实测：沙箱核心，假扩展收 `method.call`，看 `language`（见提交说明）。
3. 门禁十一项、Windows 目标的 clippy。
