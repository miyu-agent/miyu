## 施工单 BUG-2：扩展调模型带的图先照它自己的账号找

状态：核心的分会话做。2026-10-26 onebot 实测偷表情包撞上、主会话转来，修法主会话定的。

### 为什么

扩展先 `blob.put` 一张图，再 `model.call` 带这张图，回 `unknown_attachment`。`blob.put` 存在连接自己的账号名下（`methods.rs` 的 `caller.account`），`model.call` 却照它替谁干活的账号找图（`methods/later.rs` 的 `caller.acting_for()` → `models::call` → `attach::images`）。平台桥替外人干活时这两个是同一个（它自己），替机主干活的扩展不是：存在扩展名下，去机主名下找。表情包后台的「让 AI 写」「重新描述」（`miyu-memes` 的 `describe.rs`）走的是同一条路，一起坏、一起好。

### 做了什么

- `model.call` 交下去两个账号：连接自己的、替谁干活的。
- `attach::images`：先照连接自己的账号找，找不到再照替谁干活的；在自己名下找到、两个不是同一个账号的，拷一份进替谁干活的那个人名下（一次性入口照他取图编码，请求照他的配置发、记他的账）。`blob.put` 存在哪不改。
- 图纸 `protocol.md`（`attach.rs` 那一行、测试表），协议参考 `config.md` 中英 `model.call` 的 `images`。

### 验收

1. `blob_account.rs`：替机主干活的扩展 `blob.put` 再 `model.call` 带图找得到、拷进了机主名下；只在机主名下的图照样找得到；只在别的账号名下的照旧 `unknown_attachment`。平台桥那一条照旧。
2. 手改：不看自己名下、不拷进那个人名下、不退到替谁干活的，各至少红一条。
3. `cargo xtask check` 全过、Windows 版 clippy、三平台 CI。

### 合了以后

- sha 发给 onebot（偷表情包卡在它上面）。
