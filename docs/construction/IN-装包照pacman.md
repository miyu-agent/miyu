## IN 装包照 pacman

状态：施工中（2026-10-30 开工）。照设计 49。样子项目主人 10-30 定了，拆法、协议是技术细节：CI 全绿、测试全过就合；IN-3 合之前在虚拟机里真装一遍给项目主人看。

### 分步

| 步 | 做什么 |
|---|---|
| IN-1 | 索引一个包多 `recommends`、`author`、`upstream`、`installed` 四格：`miyu-index` 收录时照清单、包文件算；以前生成的索引照旧读得进来，没有的格是空的 |
| IN-2 | `package.info` 多 `source` 参数、能照软件源答没装的；装着的多 `required_by`；`miyu pkg info` 写成「键 : 值」，`-Si` 只查软件源 |
| IN-3 | 看一眼多 `recommends`、`installed`；`package.progress` 带 `stage`；`miyu pkg install` 照设计 49 第二节分阶段印、终端里画进度条、问推荐的 |

### 照的图纸

设计 49、设计 37（软件源）、设计 47（命令行文案）；蓝图 `packages.md`「软件源」、`cli/pkg.md`、`protocol.md` 的 `package.info`、`package.install`、`package.progress`；参考 `reference/packages*.md`。

### 不做什么

- 网页、终端界面的软件包页怎么显示这几格：协议加了，各自的头自己定。
- 不改从包目录、包文件装的流程，只是印法跟着分阶段。

### 验收

- IN-1：新索引带四格、老索引读得进来；收录测试里 `installed` 等于解开以后的字节数。
- IN-2：没装的照软件源答、装着的照本地答、两处都没有的报错；对齐照显示宽度（中文两格）；`-Si`、`-Qi` 各查各的。
- IN-3：看一眼的回应、进度的三个阶段有测试；命令行在伪终端里画进度条、管道里只印行；`--yes` 不装推荐的。
