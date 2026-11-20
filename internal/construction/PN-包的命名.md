## PN 包的命名

状态：施工中（2026-10-30 开工；2026-11-12 项目主人改定编号，加 PN-5）。照设计 48。只有技术细节，照第一节第 1 条直接做：CI 全绿、测试全过就合。

### 目的

包编号改成设计 48 的写法（2026-11-12 起：不带 `miyu-`、不带接入方式，出厂的叫 `base-<名字>`；10-29 到 11-12 是 `miyu-<归属>-<名字>`），配置键和扩展的事件种类靠清单的 `prefix` 和编号脱开。2026-10-30 普查过：同一个字符串在代码里同时是包编号、功能编号、资源目录名、界面文字键前缀、内核模块编号。只改目录和常量，编译照样过，但记忆、角色扮演、后台任务、联网这些会悄悄被当成没装、没开。所以先把这几样拆开，再改名。

### 分步

| 步 | 做什么 |
|---|---|
| PN-1 | 清单的 `[package] prefix`。包声明的设置项的键、发给扩展的配置、扩展从设置页改自己的配置、卸包时删的键、设置项的文字，都照前缀。扩展记事件的 `ext.<x>` 认前缀，不认编号。前缀撞了装的时候拒绝。作者文档、参考文档、蓝图跟着改。还不改任何包的编号 |
| PN-2 | 功能编号、资源目录和包编号脱钩，还不改编号、行为不变：<br>- 没写 `[features]` 的六个出厂包（clicall、goal、knowledge、memory、project-docs、roleplay）把功能写明，编号照旧；authoring 只带技能，没有包自己的功能；<br>- 按功能查开没开（`PresetFile::opens`）先照 `homes` 找功能所在的包；<br>- 记忆、长期目标、知识库、联网、定时、循环的 `PACKAGE` 一类常量拆成包编号、资源目录（`SOFTWARE`）两个，加载资源、拼日志里存着的字的键只认资源目录。<br>基础系统「工具 → 包」靠工具名等于包编号对上的那一处，和出厂预设的 `[software]` 一起留给 PN-3：改名时换成对照表 |
| PN-3 | 出厂包改名成 `miyu-base-*`（PN-5 再改成 `base-*`）：<br>- 28 个目录改名（`ask_user`、`session_usage` 的下划线换成连字符），清单写 `prefix = "<旧编号>"`；<br>- `miyu_basesystem::PACKAGES` 换新编号，`package_of` 照对照表 `TOOLS`（工具名到包编号）；<br>- 记忆、长期目标、知识库、联网、定时、循环、注入环境信息、人设遵循提醒、画 mermaid 的包编号常量换新编号；端点斜杠命令的所属包、定时任务到点看的包换新编号，长期目标拆成功能编号 `GOAL_FEATURE`、包编号 `GOAL_PACKAGE`；<br>- 端点认人格记忆装没装原来用的是功能编号 `MEMORY`，改照包编号（`memory::PACKAGE`）；<br>- 设置页 `ui.package`、`ui.group`（记忆、联网、展示那几组的组编号是包编号）、`OWNED`、`groups.toml` 的 `order`、出厂预设的 `[software]`、配置样本、蓝图跟着改。<br>撞名的先后：出厂的照旧先于装上的；出厂的包彼此照编号排，加了同一个前缀、`_` 换成 `-` 以后先后不变 |
| PN-4 | 界面和扩展的编号：<br>- `ui.head` 默认 `miyu-tui`，`heads.toml` 和 xtask 的头表照新编号（`miyu-tui`、`miyu-webui`）；<br>- 语义模型 worker 改 `miyu-embed`，记忆包的 `recommends` 一起改；<br>- 命令行没装时的提示照新编号（`miyu pkg install miyu-webui`、`miyu-embed`，终端界面照 `ui.head`）；<br>- 合完通知终端、网页、im、embed、表情包各自改清单、包文件夹，官方源认领新编号 |
| PN-5 | 编号去掉 `miyu-`（2026-11-12，见下面「PN-5」一节；PN-4 改成的 `miyu-tui`、`miyu-webui`、`miyu-embed` 也跟着去掉）：<br>- 出厂的 30 个包 `miyu-base-*` 改名成 `base-*`；<br>- `ui.head` 出厂 `tui`，`heads.toml`、xtask 的头表、发版取的包文件夹照 `tui`、`webui`，两个头的分包叫 `tui-*`、`webui-*`；<br>- 语义模型小程序包叫 `embed`，记忆包的 `recommends` 一起改；<br>- 没装时的提示照新编号（`miyu pkg install webui`、`embed`，终端界面照 `ui.head`）；<br>- 合完通知终端、网页、im、embed、表情包、语音各自改清单、包文件夹，官方源认领新编号 |

### 照的图纸

- 设计 48，设计 30（包），设计 44（基础系统拆包），设计 37（软件源）。
- 蓝图：`packages.md`、`config.md`、`presets.md`、`chat.md` 第七节第 3 条、`cli/main.md`。参考：`reference/packages*.md`、`reference/extensions*.md`。作者文档：`authors/manifest*.md`。

### 不做什么

- 不认旧编号，不写迁移代码。主人部署里的旧数据由仓库外的工具搬（设计 48 第五节）。
- 不改功能编号、资源目录名、界面文字的键、内核模块编号、子命令名。
- `packages.<编号>.off` 和数据根里按编号放的目录照旧按编号。

### 验收

- 每一步本机门禁全过，三台 CI 全绿。
- PN-1：
  - 测试包的编号和前缀不同（编号 `x-long`、前缀 `x`），它的设置项是 `x.*`，扩展收到 `x.*`，能记 `ext.x.*`，记 `ext.x-long.*` 被拒；
  - 两个包同一个前缀，后装的被拒。
- PN-2：功能编号和包编号不同时，开没开照功能所在的包算（关包、组、`[software]` 写包编号）；`homes` 照清单列全；整套测试照旧过（行为不变）。功能真的换了包编号以后照旧开着，由 PN-3 改完名的整套测试验。
- PN-3、PN-4、PN-5：预设开的功能、设置页的分组、按包隐藏的设置项和改前一样（样本逐字节比）。

### 风险

- 资源目录和界面文字的键存在日志里，改错了旧会话显示不出字，测试不一定逮得到：PN-2 的常量拆开以后，资源目录这边一个字都不动。
- 撞名判定的先后（子命令、功能撞名谁赢、预设和人格的先后）是照编号排的，改名会让先后变。PN-3 查一遍现在有没有撞的。
- 打包和软件源跟着改编号，头、扩展各自的仓库要钉核心以后改，旧编号的包装不上新核心。部署时核心、头、扩展一起换。

### PN-5：编号去掉 `miyu-`（2026-11-12）

项目主人 2026-11-12 改定命名（设计 48 第一节）：包编号不带 `miyu-`、不带接入方式，出厂的叫 `base-<名字>`；仓库名、程序名、子命令、前缀规矩、`miyu` 本身不变；接入方式做成界面标签，核心不加清单字段。PN-4 改成的 `miyu-tui`、`miyu-webui`、`miyu-embed` 一起去掉 `miyu-`。

- **改了什么**：
  - `resources/packages/miyu-base-*` 30 个目录改名成 `base-*`，`miyu_basesystem::PACKAGES`、`TOOLS`，记忆、长期目标、知识库、联网、定时、循环、注入环境信息、人设遵循提醒、画 mermaid、待办、展示的包编号常量，设置页的 `ui.group`、`ui.package`，`groups.toml`，出厂预设的 `[software]`，界面文字里组的名字（`resources/core/human/*.json`）跟着改；
  - `ui.head` 出厂 `tui`（`miyu-endpoint` 的设置和握手里的默认、`config/unused.rs` 认的终端包）；
  - 语义模型小程序包 `embed`（`miyu-core` 的 `WORKER`，记忆包 `[recommends] workers`），没装时的提示 `miyu pkg install embed`；
  - `heads.toml` 两张表 `[tui]`、`[webui]`，xtask 的头表、发版取的包文件夹（头仓库里的 `tui/`、`webui/`）、版本盖章的路径、资源目录里的 `packages/tui/`、`packages/webui/`；两个头的分包照编号叫 `tui-<版本>-<系统>.tar.gz`、`webui-<版本>-<系统>.tar.gz`，`release.yml` 收包、算哈希、出处证明、草稿 Release 的通配跟着加上；
  - 系统源模板：下载的分包名、解开以后的包文件夹（`/usr/share/miyu/packages/tui/`、`share/miyu/packages/webui/`）；系统源里的包名（`miyu-tui`、`miyu-webui`）不变；
  - 三家对比测具（`bench/rivals`）认终端仓库里的 `tui/`；
  - 测试、样本、蓝图、参考、作者文档里的编号；作者文档的编号建议改成「短名、不带 `miyu-` 和种类、别用 `base-` 打头」。
- **名字校验**：核心对包编号只查写法（小写字母打头，`[a-z0-9_-]`，最多 64 个）和数据根保留名（`miyu_config::package::RESERVED`，`private`、`models`、`scripts` 这些，设计 55 第三节第 3 条），没有「`miyu-` 打头」「`base-` 保留给出厂」这类规矩，这一步不加（只改名，不加规矩）。新编号没有一个撞保留名。出厂的和装上的同编号照旧认出厂的（`base-*` 实际上就是出厂的）。
- **不做什么**：不认旧编号、不留别名、不挪旧配置键和旧目录（没发版，项目主人 2026-11-11 定）；不改仓库名、程序名、子命令、系统源的包名；不加「接入方式」字段；施工图不另画（PN 线一直没画）。
- **验收**：整套测试、两种 clippy、`fmt --check`、`cargo doc`、`cargo xtask prompts`；配置样本重新生成，`config.md` 里嵌着的样本同步；`git grep miyu-base-` 只剩历史叙述。合完通知终端、网页、im、embed、表情包、语音各自改清单和包文件夹、`heads.toml` 钉改完名的头；官方源认领新编号。

#### 附录：`miyu-*` 字样的分类（2026-11-12 普查）

普查用 `git grep` 找 `miyu-base-`、`miyu-tui`、`miyu-webui`、`miyu-embed`、`miyu-im`、`miyu-memes`、`miyu-render`、`miyu-migrate`、`miyu-ocr`、`miyu-scripts-`、`miyu-skills-`、`miyu-voice`、`miyu-say`，改前 1970 处：改成新编号的约 830 处；留下的 1139 处里，代码、测试、打包、CI、测具里约 240 处是程序名、仓库网址、系统源的包名、临时目录名，文档里约 900 处是历史叙述和来历（含这一节、设计 48 新写的来历）。

| 类 | 怎么办 | 例子 |
|---|---|---|
| 包编号 | 改成新编号 | 出厂包目录和常量、`ui.head`、`[recommends] workers`、`heads.toml` 的表、头的包文件夹、`[content.<编号>]`、测试里的官方包、提示里的 `miyu pkg install <编号>` |
| 程序名 | 不改 | `miyu-tui`、`miyu-web`、`miyu-embed` 程序（`[command] program`、`[worker] program`、`bin/`、`miyu upgrade` 换的程序表、`miyu-tui shellhook`） |
| 仓库名、网址 | 不改 | `miyu-agent/miyu-tui`、`miyu-agent/miyu-embed`、`miyu-agent/miyu-memes`、链到头仓库蓝图的网址 |
| 系统源的包名 | 不改 | AUR、Homebrew 的 `miyu-core`、`miyu-tui`、`miyu-webui`、`miyu-agent`、`miyu-applet`，`.cargo/config.toml` 的注释 |
| 历史叙述 | 不改 | 别的设计、施工单、评审、性能记录里写当时的编号；蓝图里「PN-3 起是 `miyu-base-*`」这类来历改成写到 PN-5 |
| 其他 | 不改 | 临时目录名（`miyu-images-test-`、`miyu-skills-shelves-`、`miyu-base-8272-0`）、xtask 的临时目录前缀 `miyu-scripts-`、测试用的二进制 `miyu-test-embed`、模型二进制文件里碰巧有的字节 |
