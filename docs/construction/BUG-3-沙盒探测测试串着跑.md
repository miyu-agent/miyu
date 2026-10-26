## 施工单 BUG-3：沙盒探测的测试串着跑，不再偶发红

状态：核心的分会话做。2026-10-26 主会话和记忆会话跑门禁时，`miyu-core` 的 `sandbox::tests::a_sandbox_that_cannot_confine_sockets_is_logged_and_keeps_others_off`（「成员、场所的会话不跑命令」）偶发红（本机 3 遍红 1 遍、10 遍红 3 遍）。MB-1 那一步留下的，主会话转来修。

### 查下来的

1. 核心起来时探一次沙盒，照探到的手段设进程里那一份「关起来的会话的 Unix 套接字管不管得住」（`miyu_sandbox::set_confines_sockets`，施工 MB-1）。真的核心一个进程只探一次，没问题。
2. 测试一个进程里并行探好几次：「管得住」那条（`landlock-sockets`）设成真，「管不住」那条（只有 `landlock`）设成假，各自探完再去读，读到的可能是另一条刚设的。连跑 `cargo test -p miyu-core --lib sandbox::tests` 200 遍红 24 遍，两条轮流红（`tests.rs` 第 152 行「管得住套接字」、第 169 行「成员、场所的会话不跑命令」）。
3. 同一个测试进程里别处碰不碰这个开关（主会话要查的）：改它的只有核心的 `sandbox::probe`，单元测试里只有 `sandbox/tests.rs` 调；起核心的 `main` → `start` → `run`（会探一次）没有单元测试调；读它的只有会话的守卫（`miyu-session/src/guard.rs`），核心的单元测试里不造带沙盒的会话，走不到。`miyu-session` 自己的单元测试只碰挂载开关（一条设真马上设回，别的不看规格里的 `mount`），`tests/sockets_off.rs` 是单独的进程、只有一条测试。所以锁只要在 `sandbox/tests.rs` 里。
4. 只是测试的毛病：开关本来就该一个进程一份（会话的守卫、规格照它定），不改成非全局的。

### 做了什么

- `miyu-core/src/sandbox/tests.rs`：探测一律在一把锁里跑（`serially`），探完在锁里读出关起来的会话能不能跑命令，再设回没设过的样子（管得住）；两条看开关的测试改看锁里读出来的值，不再探完以后自己去读全局的。

### 验收

1. 修之前连跑这组测试 200 遍红 24 遍；修以后连跑 400 遍红 0 遍。
2. `cargo xtask check` 全过、Windows 版 clippy、三平台 CI。
