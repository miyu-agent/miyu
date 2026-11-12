## PV-2 已安装的包带提供什么

状态：施工完，待审（2026-11-12）。项目主人 2026-11-12 定（经网页会话转来）：`package.list` 的每一项、连同 `package.changed` 推的那一项，
加一格 `provides`，形状照软件源索引里的 `provides`，好让终端、网页「已安装」里也能标「技能」「脚本」。接 PV-1（下载软件详情页的「提供」）。

### 目的

「已安装」和「下载软件」用同一格分得出一个包带了技能、脚本、MCP 服务这些：装着的包照本机的包文件夹现算，写法和索引那一格一模一样。

### 蓝图改哪几节

- `packages.md`「对外的样子」`package.list` 那张表多 `provides` 一行；「守着它的」多 `packages_provides.rs` 一行。
- `protocol.md`「`package.list`」一段末尾一句：读成了的每一项多 `provides`，推送、回应「同 `package.list` 的一项」的同样有。
- 参考 `packages`（中英）`package.list` 的表多 `provides` 一行。

### 怎么做

- 收录里算 `provides` 的那份规则（`miyu-source/src/collect.rs` 的 `provides`、`listed`）原样挪成 `miyu_source::provides::of(清单, 包文件夹)`，
  收录照旧调它；端点本来就依赖 `miyu-source`（PV-1 的 `tools`），层序不变。
- 端点 `packages/item.rs` 的 `listed`：读成了的照 `Found::files_dir` 调它，空的也写 `{}`（同索引）。`package.list`、`package.changed`、
  装卸开关的回应都是这一份，自然都有；写错的、被盖住的照旧不带。
- `package.info` 装着的那一份的 `provides` 是装之前看一眼的「包含」（`program`、`page` 这些），是另一样东西，这一步不动。

### 不做什么

- 不改 `package.info`、看一眼的「包含」的形状；不给 `provides` 加 `tools`（PV-1 那一格照旧只在 `package.info`、索引里）。
- 不改终端、网页：它们照这一格画，各自的会话做。

### 验收

- `cargo test -p miyu-endpoint --test all packages_provides packages_push`：
  - 装一个带技能的、一个带脚本的、一个带 MCP 服务的，装的回应、`package.changed` 推的、`package.list` 列的 `provides` 各自是
    `{"skills": ["draw"]}`、`{"scripts": ["hello.sh"]}`、`{"mcp": ["probe"]}`；
  - 出厂的 `base-authoring` 有 `skills: ["package-creator", "skill-creator"]`，`base-persona-engineer` 有 `personas`，`base-shell` 有
    `features: ["shell"]`；什么都没带的是 `{}`。
  - 改前红：不改 `item.rs` 时这两条测试红。
- `cargo test -p miyu-source`：收录的 `provides` 照旧（规则只是挪了地方）。

### 风险

- `package.list` 每一项多读几个目录（`skills/`、`scripts/`、`personas/`、`presets/`），装卸时前后各算一遍列表也是：几十个包、每个四次
  `read_dir`，比清单读一遍轻得多，不另做缓存。
