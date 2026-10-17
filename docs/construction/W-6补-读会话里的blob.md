## 施工单 W-6 补：`blob.get` 读会话里的 blob

状态：2026-10-17 合了（f4eb1b1e）。接入QQ 的会话 2026-10-17 要的（`show` 发到 QQ）；图纸 `docs/blueprint/protocol.md` 的 `blob.get` 第 1 条。

### 起因

接入QQ 要把 `file.shown` 的内容发出去，得用 `blob.get` 读。以前 `blob.get` 一律照管理员名下找：群会话归接入QQ 的系统账号，`show` 存的、带进来的附件都在它名下，桥读不到。

### 做了什么

1. `blob.get` 多一个可写的 `session`：写了的照那个会话的属主名下读（`attach/reach.rs`）。
2. 谁读得到：会话的属主、管理员；别的系统账号只读得到它的平台的场所会话（`venues::owners`，同 `venue.sessions` 列给它的）里引用过的 blob。引用照结构认（消息、工具结果的图片块、文件块，`file.shown`），不在正文里搜：桥能以外部身份往主人的私聊里发字。不合的一律 `unknown_blob`。
3. 不写 `session` 的照旧照管理员名下。

### 测试

`crates/miyu-endpoint/tests/blob_session.rs`：
- 桥读得到群会话里引用过的（管理员名下那一份删了也读得到，证明照会话的属主读）、主人私聊里引用过的；正文里写着哈希的读不到；哈希不合写法 `bad_params`。管理员照会话的属主读，会话编号不合写法 `bad_params`。
- 主人的本机会话里引用过的桥读不到，管理员读得到。
- 手写变异 5 处全报红。
