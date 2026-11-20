## IM-4：平台包在自己平台的场所里不过预设

状态：主会话做，一步，2026-10-28 做完。接入通讯平台（im）设计 06 第 4.6 节第 3 样。

### 一、为什么

im 设计 06 把功能分两层：核心的预设是基础层，管 shell、文件、记忆、联网；im 自己的开关是场所层，管 QQ 群里她能做什么。现在 QQ 会话里 im 的工具也过预设开关：预设一关，群里她连话都说不了，im 后台的开关也说了不算。im 卡片要把「预设」改名「基于预设」，等的就是这一步。

### 二、定下的（技术题，主会话定）

1. **写成通用规矩，不写 im 的名字**：清单 `[connection] platform` 里有场所平台的包，在这个场所的会话里整包开着。场所平台是场所编号第一个 `:` 前面那段。
2. **整包，不只是工具**：`[features]`、`[software]`、`[tools]`、全局开关、`unlisted` 都不管它。回合开始照问它、它的 `system.append` 照拼、它的技能照放开、单件工具关了也照给。不算进没开的那几个，不进预设指纹。
3. **别的几道筛照旧**：
   - 显示不了图片扣下的照旧扣下，是场所显示不了，不是开关。
   - 这个人关了的包（`packages.<编号>.off`）照旧不开，他自己说不要。
   - 场所、子会话、能不能确认照工具面自己的规矩。
   - 场所插件（O-60）、平台后台的开关在平台那一侧，照旧管。
4. **本机的会话、别的平台的场所照预设**：终端里 im 的工具照「在终端里也能用」走 `feature.set`（第 4 样）。
5. **放哪**：和「显示不了图片扣下的包」放在一处。`PresetFile` 多一格 `own`，`Chosen::blind` 换成 `Chosen::situated(features, images, venue)`，一次定两样；`Features` 多记接平台的包（`connecting`、`connected_to`）。造会话、载入时找技能、回合开始换快照、换人格四处都照它。
6. **缓存**：现有 QQ 会话里预设关了 im 的，下一轮工具面多出 im 的工具，冷启动一次，照设计 06 第 4.5 节可以接受。没关的字节不变。

### 三、做了什么

1. `miyu-policy`：`Features::{connecting, connections, connected_to}`；`PresetFile::own`、`situate`，`opens_in`、`keeps` 认它；`Chosen::situated` 取代 `blind`。
2. `miyu-session`：`open/setup.rs` 的 `situated`、`PresetPlaces::file` 带场所；`open/face.rs`、`open/load.rs`、`actor/persona.rs`、`actor/persona/recast.rs` 照场所定。
3. `miyu-endpoint`：`packages::features` 记接平台的包，`scripts::features` 照传。
4. 蓝图 `presets.md`、`venues.md`。

### 四、怎么验

1. `miyu-policy` 单测 `a_platform_package_ignores_the_preset_in_its_own_venues`：预设关了 `qq` 功能、单件关了 `send`。
   - `qq:`、`tg:` 的场所里照开、照留，不算进没开的。
   - 本机的、`discord:` 的照预设。
   - 指纹不变。
   - 显示不了图片的、这个人关了的照旧不开。
   - 原来扣图那两条改照 `situated`。
2. 端点 `platform_tools.rs` 的 `a_platform_package_s_tools_skip_the_preset_in_its_own_venues`：系统预设 `off` 关了 `bridge` 包、单件关了 `note`。
   - `tg:` 的群工具面照有 `note`、`qqnote`、`tg_send`，快照的 `preset.off` 不带 `bridge`。
   - 群里说一句（回合开始照这一轮重新定预设、换快照）以后，最新的快照照旧有这几件、`preset.off` 照旧不带 `bridge`。
   - 本机的会话一件都没有。
3. 原有预设、扣图、场所、人格、每人一份的测试照过。
4. 手写变异 8 个杀了 7 个：不认 `own`、单件关了照扣、不放进 `own`、`connected_to` 不看平台、端点两处不记接平台的包、回合开始换快照照本机算都红。最后这条头一轮活了（原来没跑回合），补了群里说一句才杀掉。活下来的是载入会话时找技能那一处照本机算：平台包的技能在它自己的群里重启以后不放开。走的是同一个 `situate`，测它要重启核心、再让她读技能的文件，没补，记在这里。
5. 本机门禁全过。
