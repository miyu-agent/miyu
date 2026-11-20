## 施工单 DOC-28：作者教程并进开发者文档，开发者文档和协议参考整个用 Apache-2.0

状态：待验收（2026-11-20）。项目主人定整理文档：`docs/authors/` 和 `docs/zh/dev/` 是两棵讲同一类事的树，合成一棵。文档大搬家三步里的最后一步。许可证的范围跟着目录变，项目主人 2026-11-20 定：整个 `docs/*/dev/` 和 `docs/*/reference/` 都用 Apache-2.0，按目录划。

### 改了什么

- `docs/authors/` 五页（中英各一份）搬进 `docs/zh/dev/`、`docs/en/dev/`，页名两边相同：`extensions.md`、`manifest.md`、`mcp.md`、`scripts-skills.md`，原来的目录页 `README.md` 改名 `packages.md`（写一个 Miyu 软件包）。`docs/` 下现在只有 `README.md`、`zh/`、`en/`。
- 相对链接 122 处、写明的路径 82 处跟着改；每页的语言跳转句去掉（10 处），标题下面加署名行。
- 目录页合成一份：开发者文档的目录页（`dev/README.md`）本来就按「我想做的事」列了全部教程，作者教程自己的目录页改成其中一篇教程。开头「文档分在三个地方」改成两个地方。
- 和用户手册的分工写明：`dev/scripts-skills.md` 开头说明它是完整的格式、写给要做进软件包的人，只给自己用的先看手册的「扩展 Miyu」；手册那一页末尾本来就指到开发者文档。两页的内容没有互相删：`manifest.md`、`mcp.md`、`scripts-skills.md` 三页是有测试守着的格式出处（每一格、每个环境变量都要写到），手册那一页是按步骤的用法。
- 中英对齐：「写一个软件包」中文页有八行硬换行，接回上一行；「写扩展」英文页代码块里折成两行的一条注释并成一行。
- 代码：
  - 三处 `include_str!`（`miyu-basesystem` 核对脚本的环境变量、`miyu-config` 核对清单每一格和 MCP 每一格）读新位置，报错里的文件标签跟着换。
  - CI 的路径规则去掉 `!docs/authors/**` 两行：这几页有测试读，改了应该跑 CI（原来排除在外是个漏洞）。
- 许可证：
  - 蓝图 `licenses.md`「对外的样子」：Apache-2.0 管的目录写成 `docs/zh/dev/`、`docs/en/dev/`、`docs/zh/reference/`、`docs/en/reference/` 和 `resources/packages/base-authoring/`。比原来多出来的是开发者文档里原有的六页（目录、架构、贡献代码、写一个头、写主题和外观、往界面上放东西）。
  - 两份仓库首页没有动：它们是项目主人手写的。`AGENTS.md`、`base-authoring` 的清单都写着「见 README 许可证」，而首页重写以后没有说明 Apache-2.0 范围的那一段了；要不要补、补什么，另问项目主人，点头了单独一步加。
  - 两份文档总目录原来写 `guide/` 也用 Apache-2.0，不对，改掉：用户手册和仓库的其他内容一样是 AGPL-3.0-or-later。
  - `AGENTS.md`、开发者文档目录页、架构页里列范围的地方跟着改。

### 没做的

- 并进来的五页和协议参考二十页的写法还没照「文档的规矩」改（省略短句、称 AI 用「她」；文档检查还报着英文 16 页的 she/her）。另排一步逐页改写。
- 别的仓库里指到 `docs/authors/…`、`docs/reference/…` 的链接（接入通讯平台 12 处、软件包模板 6 处）要在这一步合进 main 以后各开一个 PR 改。

### 核实

- 文档线的检查脚本：中文 62 页、英文 62 页一页对一页，1561 个链接都指得到；剩下的 16 个问题都是英文页里的 she/her。
- 全仓库搜 `docs/authors`：只剩施工单里的历史记录，和指到通讯平台仓库自己的 `docs/authors/` 的一处外链。
- `cargo xtask check` 全过（三处读文档的测试在「测试」一项里）。

### 蓝图改哪几节

`licenses.md`「对外的样子」：Apache-2.0 管的目录，见上。

### 验收

1. `docs/` 下只有 `README.md`、`zh/`、`en/`；`docs/zh/dev/`、`docs/en/dev/` 各 11 页，页名相同。
2. `cargo xtask check` 全过，三台 CI 全绿。
3. 给清单加一格、给脚本加一个环境变量而不写进 `docs/zh/dev/`、`docs/en/dev/` 的对应页，测试照旧报红。
