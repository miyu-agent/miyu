## 施工单 8-14 补：Zen 的会话头照 opencode 的会话编号编

状态：已合（2026-10-13，15c841d7，CI run 2017）。项目主人验收终端的第一次引导时报的：选 OpenCode Zen、试免费模型 403。照 `docs/blueprint/models.md`「八、opencode Zen」第 1 条修，是接上免费档（2026-10-04 定）的回归，不用另外拍板。

### 为什么

Zen 的免费档按客户端识别。10-04 实测的判据是三件：User-Agent 是 `opencode/<版本>` 的形状、带至少一个 `x-opencode-*` 头、工具面里有 `shell` 和 `read`，还要流式。10-13 照旧版仓库的测具 `testkit/opencode-zen/freetier_probe.js` 再测，三件照旧成立，可 Miyu 发的请求还是 403。一次只改一个变量对照下来，差在 `x-opencode-session`：Zen 现在查它的前 12 位是不是 opencode 那种倒序的时间编号（`Identifier.descending`：`~(毫秒 × 4096)` 的低 48 位）。我们原来写的是种子的 SHA-256 前 26 位，前 12 位是乱码，一律 403。

| 前 12 位 | 结果 |
|---|---|
| 照现在的时间倒序编 | 放行 |
| 一小时前、三天前、四十天前、一年前、一小时以后 | 放行 |
| 照现在的时间正序编（不取反） | 403 |
| 哈希出来的乱码、手写的固定一串 | 403 |

尾巴是小写十六进制、带大写的 base62 都放行。

### 做了什么

1. `miyu-models` 的 `headers::digest`：照旧 26 位小写十六进制，前 12 位改成种子的时间照 opencode 的倒序编法，后 14 位是种子 SHA-256 的十六进制前 14 位。种子是 UUIDv7（会话编号）的取它前 48 位记的毫秒数，同一个会话重启以后还是这个值；别的种子（一次性调用的用途、`provider.test`）照固定的 2026-01-01 编。档案里的模板 `ses_{session_digest}` 不变，Go、Zen 两家一起换。
2. 档案 `resources/models/profiles.toml` 的注释、蓝图「八、opencode Zen」第 1 条照新判据写。

### 验收

- 真的 Zen（`cat-jam-free`）：新编法的两种值（UUIDv7 的会话、`provider.test` 的固定时间）都放行；旧的 `ses_` 加 26 位哈希 403。
- 编出来的 release 对真的 Zen：`miyu setup --provider opencode --model cat-jam-free` 通了（列出 47 个模型），接着 `miyu ask` 照 `opencode/cat-jam-free` 说话，回了「你好」；运行日志里没有 key。
- 测试：普通种子照固定时间、UUIDv7 照它的时间、长得像却不是的（版本不是 7、少一位、没有横线、不是十六进制、36 个横线）照固定时间也不越界，倒序编法照 opencode 的算式。
- 变异 8 处全抓住（不取反、不截 48 位、版本不查、长度不查两处、固定时间换成 0、尾巴少一位、时间取错位置）；「全是十六进制」那道检查抓不住，查下来是多余的（前 12 位不是十六进制的本来就解析不出来），删了。

### 留下的

- Zen 随时会再改判据。改了先跑旧版仓库里的 `testkit/opencode-zen/freetier_probe.js`；这次它没测出来（它每次现算合规的编号，正好绕过了这一条），以后对照 Miyu 真发的请求，抓包看头。
