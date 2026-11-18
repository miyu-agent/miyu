## 施工 DR-2：扩展驱动可以声明等得久一点

状态：做完，等验收（分支 `step/CB-3`）。核心主会话派（2026-11-18），agy-cli 会话实测报来。编号：DR-1 是「整理会话记下结果」（dream），这一张接着用 DR-2（driver），两件不相干。

### 目的

agy-cli 实测：agy（扩展驱动）medium 档每次调用 100～150 秒才出第一个字，离 180 秒的空闲时限很近；agy 不流思考的字，这段时间一点增量都没有。让 `[[driver]]` 能声明多等一会儿，核心对这个驱动的请求照它等；没写的照旧。

### 改了什么

- 清单 `[[driver]]` 多一格 `quiet_secs`：多久没动静才算断，秒，整数 1 到 900（`QUIET_SECS_MOST`，十五分钟：总时限照它的十倍，再长的卡住了要等太久才知道）。写错的报 `bad_driver`，`detail` 是 `driver.<驱动名>.quiet_secs`。
- 端口的 `Declared` 多 `quiet`，挑候选时备进 `Reach`；发的时候这一次的空闲时限是「这一轮的空闲时限（照思考强度放大过的）」和它里长的那个：等第一个字、两段之间、最后一段到回答都照它，总时限跟着是它的十倍。比核心的短的不收紧。
- 键名用 `quiet_secs` 不用 `first_token_timeout_secs`：核心只有一个「多久没动静」的时限，第一个字和两段之间是同一个，名字照它说。

### 给 agy-cli 的写法

```toml
[[driver]]
name = "antigravity"
quiet_secs = 300   # medium 档 100～150 秒才出第一个字；high 档照思考强度另放大（2 倍）
```

要配合这一版以后的核心：旧核心不认这一格，报不认识的键（VER-1 起后面补「这个包可能要更新的 Miyu」）；包里 `[package] miyu` 写这一版的版本号。

### 蓝图改哪几节

- `extensions.md`「对外的样子」的「扩展提供的模型驱动」第 1 条（交的声明多 `quiet_secs`）；`models.md`「驱动、地址从哪来」那一条扩展驱动那一段（时限）；`packages.md`「出错」`bad_driver` 那一行。
- 协议参考 `extensions.md`（中英）`driver.call` 的时限；包作者文档 `manifest.md`（中英）`[[driver]]` 的表、`extensions.md`（中英）的时限。

### 验收

1. 改前红（新加的格，main 上读清单报不认识的键、端口和执行器没有这一格，编不过）：
   - `miyu-config` `package::driver::tests::a_driver_may_wait_longer_for_its_first_token`：300、900 读得出，0、-1、901、1.5、`"300"` 报 `bad_driver`。
   - `miyu-session` `route::extension::tests::quiet::a_driver_that_declares_a_longer_quiet_is_waited_for`（时钟停着，不真等）：假驱动每段隔 200 秒（第一个字也是），空闲时限 180 秒：声明 300 的四段全收、说完；没声明的 180 秒按卡住断、一段都没收；空闲时限 300、声明 60 的照 300 等。
   - `miyu-session` `route_extension_driver::a_declared_quiet_lets_a_slow_first_token_through`：从端口的声明一路交到执行器（空闲时限 200 毫秒、第一个字晚 600 毫秒、声明 5 秒，说成了）；把 `reach` 里交 `quiet` 那一句拿掉，它红。
2. 门禁十一项、长跑随机、Windows 目标的 clippy。

### 不做什么

- 不给内置驱动、供应商的配置加这一格：只有扩展驱动报来要。
- 不分开「第一个字」和「两段之间」两个时限。
