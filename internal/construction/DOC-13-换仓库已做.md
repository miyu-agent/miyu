## 施工单 DOC-13：换仓库提前做了，设计 59 和一处旧地址跟着改

状态：待验收（2026-11-18）。主会话交来：项目主人定把设计 59 第三节第 3 条的「换仓库」提前做，当天做完；文档里写着「发版那天做」的地方和指旧核心仓库的地址照现状改。

### 改了什么

- `internal/designs/59-发版.md`：第三节第 3 条「在发版那天做」改成「2026-11-18 已提前做」；第五节第 1 步写明已经做了、发版那天不用再做；第九节加 2026-11-18 一条，记做了什么。
- `internal/designs/旧版证据索引.md`：旧版公开仓库的地址后面注一句它现在在哪。
- `internal/construction/DOC-12-安全渠道和打包说明.md`：两处「还没开」改成现状（私密漏洞报告、Issues 都开着了）。
- 顺带改一个撞了的编号：帮助页挂全那一步起的号 HP-1 和 2026-11-03「头自己的设置存进数据根」重了，改成 HLP-1（施工单文件名、README 一行、施工图一张、蓝图 `cli/main.md` 三处、`main.rs`、`help.rs` 注释里的五处）。那次提交的标题改不了，施工单里写明了。只改字，代码不动。

### 核实

照「`SHORiN-KiWATA/miyu-agent`、`SHORiN-KiWATA/Miyu`，后面不接字母、数字、连字符」在各仓库的 `origin/main` 上找真指旧核心仓库的地方：

| 仓库 | 找到 | 处置 |
|---|---|---|
| 核心 | `designs/59-发版.md` 两行、`designs/旧版证据索引.md` 一行 | 改，见上 |
| miyu-migrate | `crates/miyu-migrate/legacy/miyu.md:125` | 不改：这是旧版人格提示词的原文，迁移时照原文建人格、拿它认用户改没改过；旧地址会跳转 |
| miyu-kb-shorinwiki | `package/kb/shorinwiki/活用AI.md:29` | 不改：上游文章同步来的，下次同步会盖掉；要改改上游 |
| packages、miyu-skills-shorin-niri-use、终端、网页、托盘、im、sdk、voice、包模板等 | 没有 | — |

换完以后的现状（2026-11-18 查）：

- `https://github.com/SHORiN-KiWATA/miyu-agent` 回 301，跳到 `https://github.com/miyu-agent/miyu`；`gh api repos/SHORiN-KiWATA/miyu-agent` 回的是 `miyu-agent/miyu`。
- 新仓库 133 颗星，建于 2026-04-24（旧仓库的建仓日）；`main` 是 `fde886914`；有 `legacy-0.7` 分支（`5df19e2f1`）；Release 里重制版的 5 个（`v2026.11.0-rc.1`、`rc.2`，`v0.0.0-rc.2`、`rc.3`，`models-granite-embedding-97m-multilingual-r2`）和旧版到 `v0.7.4` 的都在。
- `miyu-agent/miyu-remake-old` 已归档。
- Issues、私密漏洞报告都回 `true`。

### 蓝图改哪几节

不改。只改文档。

### 验收

1. `cargo xtask check` 全过（见 CI）。
2. 核心仓库里搜「SHORiN-KiWATA/miyu-agent」只剩设计 59、旧版证据索引里讲来历的三处，都写明了现状。
