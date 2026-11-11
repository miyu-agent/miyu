## 施工单 BUS-5：收掉跨包的 `package.call`

状态：施工完，待主会话审（2026-11-11）。设计 `67-总线.md` 第八节、B5。不留过渡、不写旧版本兼容（2026-11-11 项目主人定：还没发版）。

### 目的

扩展照包编号只调得到自己，调别的包一律走总线（`bus.call`，照名字）。总线 BUS-1 到 BUS-4 做完以后，官方的包（im、赞助记录、渲染、表情包、语音三个）都改成照名字调了；旧路留着，社区作者照旧会把对方的包名写死，总线就白做了。协议 1 发出去以后只加不改，要收只能在发版前收。

### 改了什么

1. **扩展的 `package.call`**（`crates/miyu-endpoint/src/backstage/calls.rs` 的 `call`）：调的一方是核心拉起的扩展（`Caller::package` 有值）、`package` 不是它自己的编号的，回 `own_package_only`，`data.package` 是它写的那个编号。先于找包：那个包在不在、方法导没导出都是这一句，不告诉它别的。
   - 调自己照旧：要声明 `packages.call`、人批过（`connection/rights.rs` 没动），只调得到自己清单 `[export]` 里的（别的 `not_exported`），转回来的 `method.call` 带 `from`（自己的编号）。
   - `bus.call`、`bus.list`、`package.list` 的 `export` 没动。
2. **新的拒绝码 `own_package_only`**（`refusal/package.rs`、`refusal/message.rs`）。现有的码里没有合适的：`not_exported` 说的是方法，`local_only` 说的是只给人。名字照 `local_only`、`owner_only` 那一族。给人看的话（拒绝只有中文、英文两句，日文的连接照英文）：
   - 中文：扩展照包编号只能调自己，调别的软件包请用 bus.call。
   - 英文：An extension can call only itself by package id; use bus.call for other packages.
3. **清单 `[export] methods`**（`crates/miyu-config/src/package/export.rs`）：每一个都要在这个包自己 `provides`、`plugs` 的某个名字底下（`under`：`<名字>.<一段的短名>`）。不在的报 `bad_export`，`detail` 是那个方法、报在 `methods` 那一格；没声明名字却写了 `methods` 的也是。名字写错的先报名字。`package-problems/bad_export` 那一句三种语言跟着改（原来只说「要写方法名、不能重复」，BUS-1 加的名字没写进去）：
   - 中文：[export] 里的 {detail} 写法不对：名字写进 provides 或 plugs，方法写成 <名字>.<方法>；都是小写字母开头，不能重复
   - 英文：{detail} in [export] is badly written: declare names in provides or plugs and write methods as <name>.<method>; each starts with a lowercase letter and is listed once
   - 日文：[export] の {detail} の書き方が正しくありません：名前は provides か plugs に書き、メソッドは <名前>.<メソッド> と書きます。どれも小文字で始まり、重複できません

### 谁还能照包编号调

认的是连接：只有核心亲手拉起的那条（`connection::serve_spawned`）有 `Caller::package`，别的都是人的连接，照旧调得到哪个包登记了的全部（写没写进 `[export]` 都行），不带 `from`。

| 谁 | 哪条连接 | 受不受影响 |
|---|---|---|
| 终端、网页、托盘的界面块按钮、整页表单、`choices_from` 的下拉 | 头自己的连接 | 不受 |
| 网页里的后台页（框的 `call`） | 网页替它在人的连接上发，`package` 照框是谁的填 | 不受 |
| 核心自己起的宿主页（`backstage/host/bridge.rs` 的 `call`） | 进程里的管道、出示本机令牌握手（`host/link.rs`），不是 `serve_spawned` | 不受，`tests/page_host.rs` 守着 |
| `miyu pkg page`、包自己的命令行（`miyu im access`、`miyu say --voices`、`miyu listen`）、迁移程序 | 人在终端起的程序，出示本机令牌 | 不受 |
| 包自己起的网页服务替登录的人开的连接（im 的 `site/`） | 照密码、交接票握手的人的连接 | 不受 |
| 扩展的那条连接调自己 | 核心拉起的 | 照旧 |
| 扩展的那条连接调别的包 | 核心拉起的 | **拒 `own_package_only`** |

### 出厂的包、别的仓库

- 出厂的包（`resources/packages/*/package.toml`）没有一个写 `[export]`，没有靠旧导出互调的，不用改。`docs/designs/samples` 里也没有。作者文档 `manifest` 的例子原来是 `methods = ["greet"]`，改成 `provides = ["hello"]`、`methods = ["hello.greet"]`。
- 别的仓库照本机的 `origin/main` 只读查的（2026-11-11，没有 `git fetch`）：

  | 仓库（`origin/main`） | 扩展的连接发 `package.call` 调别的包 | 清单里不在名字底下的导出 | 别的 |
  |---|---|---|---|
  | miyu-im `c7ede70` | 没有：跨包的几处都是 `bus.call`（`core/hooks.rs`、`core/route/long.rs`、`crates/miyu-im-sponsor/src/main.rs`） | 两份清单都合（`chat`、`sponsor`） | 测试夹具 `crates/miyu-im-app/tests/hooks.rs` 第 68 行装的 `other` 包导出 `qq.other`，`support/hooked.rs` 只给 `chat.hook.` 开头的补 `plugs`，这一份清单换钉以后读不成；注释还写着「别的包经 `package.call` 调」的有 `core/route/notes.rs`、`core/route/exports.rs`、`core/route/outreach.rs`、`miyu-im-sponsor/src/methods.rs`、`miyu-im/package.toml` 第 71 行 |
  | miyu-render `89b6afd` | 没有 | 合（`render`） | |
  | miyu-memes `b899088` | 没有：`src/save.rs` 走 `bus.call` | 合（`memes`） | |
  | miyu-voice `1cf496e` | 没有：跨包的都是 `bus.call` | `miyu-say`（`tts`）、`miyu-voice-input`（`stt`）都合，`miyu-voice-assistant` 没写 | |
  | miyu-migrate `6031e03` | 不是扩展；赞助记录走 `bus.call` | 没有清单 | |

- 头照包编号调的（人的连接，不受影响）：

  | 仓库 | 哪里 | 调的什么 |
  |---|---|---|
  | miyu-tui `99e0c7a8` | `src/core/asides.rs` | 界面块的动作：放这一块的包的 `action.call` |
  | | `src/settings/pages/runtime_choices.rs` | `choices_from` 写的那个包的那个方法 |
  | miyu-web `bf4ae007` | `web-demo/packages/backstage/external.js` | 自己起服务的包的 `status` |
  | | `web-demo/packages/backstage/frame.js` | 后台页的 `call`：框是谁的就调谁 |
  | | `web-demo/packages/settings/rows.js` | `choices_from` 写的那个包的那个方法 |
  | | `web-demo/packages/widgets/index.js` | 界面块的动作、整页的保存和加一项：放这一块的包 |
  | miyu-desktop `27c76d6` | 没有 | |
  | miyu-im（`site/`、`access.rs`） | `site/venues.rs`、`access.rs` 调 im 自己；`site/plugin_settings.rs`、`site/plugin_widgets.rs` 调插件包 | 都在登录的人的连接上 |
  | miyu-voice | `miyu say --voices` 调 `miyu-say` 的 `voices`；`miyu listen` 调 `miyu-voice-assistant` 的 `listen` | 命令行出示本机令牌 |

### 蓝图改哪几节

- `protocol.md`：`package.call` 那一条（扩展只调得到自己、`own_package_only`）；出错表 `not_exported` 改写、加 `own_package_only`；给人看的话那张表加一行；`blob.get` 第 1 条、测试表里「经 `package.call` 拿回来的」改成经 `bus.call`。
- `package-pages.md`：「`package.methods`、`package.call`、`method.call`」第 2 条；「软件之间互相调」第 1、2 条改成总线的写法；「守着它的」加 `package_call_ext.rs`。
- `packages.md`：`package.list` 的 `export` 一格；「依赖」第 5 条；`bad_export` 一行；「守着它的」加 `export/tests.rs`。
- `extensions.md`：「谁能调」的表里 `package.call`、`bus.call` 那一格。
- 参考 `packages`、`extensions`、`bus`、`errors`、`blob`，作者 `manifest`、`extensions`，中英都改；设计 67 第五节、第八节、B5。

### 改了哪些现有的测试

| 测试 | 原来 | 改成 | 为什么 |
|---|---|---|---|
| `miyu-endpoint` `tests/package_call_ext.rs` | 扩展照包编号调别的包导出的调得到、没导出的 `not_exported`、没有的包 `unknown_package` | 三样都是 `own_package_only`、没有转过去；加经 `bus.call` 调同一个方法、照包编号调自己（导出的、没导出的）、人调导出的和没导出的 | 这一步改的正是它 |
| `miyu-endpoint` `tests/blob_grants.rs` | `bridge` 经 `package.call` 调 `memes` 的 `give` 拿回 blob | `memes` 声明 `provides = ["pics"]`、方法 `pics.give`，`bridge` 经 `bus.call` 调 | 旧路没了；守的还是「答的包拿得到的才转」 |
| `miyu-endpoint` `tests/package_depends.rs` | `relay` 的清单 `[export] methods = ["qq.inbound", "qq.groups"]`，列表里 `export` 只有 `methods` | 加 `provides = ["qq"]`，列表里多 `provides` | 原来的清单现在读不成 |
| `miyu-config` `package/export/tests.rs` | 断言不在名字底下的 `own` 照收 | 断言读不成；加一个测试列五种写法 | 规矩反过来了 |
| `miyu-config` `package/links/tests.rs` | `methods = ["add", "search.by-meaning"]` 没有名字 | 加 `provides = ["pics"]`、方法带前缀；写错的几种、内置包不能写照旧 | 原来的清单现在读不成 |

测试用的扩展（`miyu-test-extension`）加两样：`ask` 等回应时先来的 `method.call` 照 `serve` 答、接着等（照包编号调自己要它）；`method.call` 的方法以 `.give` 结尾的照 `give:` 交。

### 不做什么

- 扩展调自己不放宽：照旧要 `packages.call`、要写进 `[export]`。
- `packages.call` 不改名、不拆（B7：批的单位第一版照旧）。
- 别的仓库不动，查到的列在上面，交各自的会话。
- `not_exported` 在蓝图 `protocol.md` 的话表里有一句，代码的 `refusal/message.rs` 里没有（回的是「已拒绝」）；`no_provider`、`package_required` 反过来，代码里有、蓝图的两张表里没有。这是以前留下的出入，这一步没碰，交主会话定。

### 验收

1. 先红：`cargo test -p miyu-config export`，改之前 `methods_outside_every_declared_name_are_refused` 红（清单读成了）；`cargo test -p miyu-endpoint --test all package_call_ext`，改之前红在第一条（扩展照包编号调别的包导出的方法，回的是 `{"answered": "pics.add"}`）。
2. 改完这两条都绿，`cargo test -p miyu-endpoint --test all -- blob_grants package_depends bus:: backstage page_host widget_content` 绿：经总线拿回来的 blob、清单带名字、总线、人照包编号调后台页的方法、宿主页的 `call`、界面块的动作都照旧。
3. `cargo test --workspace --no-fail-fast`、`cargo clippy --workspace --all-targets -- -D warnings`（另加 `--target x86_64-pc-windows-gnu` 一遍）、`cargo fmt --check`、`RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps` 全过；`cargo xtask prompts` 没有差异（没碰给模型看的字）。

### 风险

- **别的仓库的测试夹具**：im 的 `hooks.rs` 里那一份 `qq.other` 清单换钉以后读不成，那一条测试原来守的是「没接进 `chat.hook` 的包不算插件」，清单读不成也会让它「不算」，可能绿得不对。已列给 im 的会话。
- **社区的包**：现在没有第三方的包；作者文档的例子、参考都改成总线的写法了。
- **误伤人的连接**：认的是 `Caller::package`，只有 `serve_spawned` 的连接有；人的几条路上面的表逐条对过，`page_host.rs`、`backstage.rs`、`widget_content.rs` 的测试没改、照旧绿。
