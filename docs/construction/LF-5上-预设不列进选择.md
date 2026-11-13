## 施工 LF-5 上：预设多认一个键 `hidden`

状态：已完成（2026-11-14）。设计 `70-推演人生.md` 第三节第 7 条。

### 目的

预设文件的 `[preset]` 多认一个键 `hidden`（布尔）。写了 `hidden = true` 的预设，头的「新建会话选哪个预设」里不列，预设管理页照列、照改。它是个通用的键，谁的预设都能写；给以后出厂的「推演人生」预设用，这一步不带那个预设。

### 蓝图改哪几节

- `presets.md`「格式」那一段的样例和第 1 到 7 条：加 `hidden`，只收 `true`、`false`，别的报错写明第几行；几层叠起来照 `icon` 的叠法（上一层写了的盖下面的）；没写的是 `false`。
- `presets.md` 讲 `preset.list`、`preset.get`、`preset.set` 的几条：一项多一格 `hidden`（布尔，没写的给 `false`）；`preset.set` 能写、能去掉。
- 它不影响会话：不进叠好的文件的指纹、不进策略快照（照 `icon` 现在的处理核对；`icon` 要是进了指纹，`hidden` 也不许进，写清为什么）。
- 协议参考 `docs/reference/personas.md`、`personas.en.md` 的 `preset.list`、`preset.get`、`preset.set`。

### 不做什么

- 核心不拦拿 `hidden` 的预设新建会话、不从 `preset.list` 里拿掉它：不列是头的事，核心只带上这一格。
- 不加出厂的「推演人生」预设，不动出厂四个预设。
- 不改终端、网页的头（它们在别的仓库）。
- 不动已有的 `unlisted`。

### 验收

1. `cargo test -p miyu-policy preset`：`hidden = true`、`false` 读得出；写成字、数的报错带第几行；两层叠起来上一层的盖住下面的；没写的是 `false`。
2. `cargo test -p miyu-endpoint --test all preset`：`preset.list`、`preset.get` 的一项带 `hidden`；`preset.set` 写 `hidden = true` 以后再读是 `true`，去掉以后回到 `false`；出厂四个都是 `false`。
3. 写了 `hidden` 和没写的两份预设，造出来的会话的快照、指纹一样（除了本来就不同的地方）。
4. 已有的预设测试全过；其中两条把列表逐字比对的，期望里各项补一格 `hidden`（照当年加 `icon` 的做法），别的断言不改。
5. 门禁（`cargo xtask check`）全过；Windows 版 clippy 过。

### 风险

- 指纹：以前造的会话的快照要照旧对得上，`hidden` 进了指纹就全对不上了。验收第 3 条守着。
- 配置样本、协议方法清单这类生成出来的文件：门禁哪一项红了照它说的重新生成。
