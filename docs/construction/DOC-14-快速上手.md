## 施工单 DOC-14：快速上手换成项目主人的手写稿

状态：待验收（2026-11-18）。`guide/quickstart` 一直是占位，项目主人 2026-11-18 写好了稿子，这一步放进来，出英文版。

### 改了什么

- `docs/guide/quickstart.md`：换成项目主人的稿子。框架和写好的字不动；稿子里留给我示意的两处填上了：
  - 「快速安装」下面五条问句（Windows 一个 exe？Linux 一条命令？MacOS 用 Homebrew 还是安装包？npm 做不做？）换成现在真有的装法：Windows 的 `.msi`、Linux 的 `.deb`、`.rpm` 或安装脚本、MacOS 的 Homebrew 或 `.dmg`、Arch 的 AUR，后面加一句指到安装页。npm 没有，不写。
  - 「可以阅读：」后面空着的，接上作者文档和软件包模板的链接。
  - 前面加页标题「# 快速上手」和指到英文版的一行（手册每页都有这两样）。
  - 别的地方逐行和原稿相同（脚本比过）。
- `docs/guide/quickstart.en.md`：英文版，照稿子逐句翻，和中文一页对一页。
- `docs/guide/heads/web`（中英）「第一次打开」第 6 步：「AI 怎么称呼你」「更多关于你的描述（可选）」是改名前的叫法，照现在的写成「称呼」「更多描述（可选）」，上面还有一行「回复语言（可选）」。这一处是手册自己的页，不是稿子里的。

快速上手是项目主人写的，不带生成署名。

头一版我放进去时照字表和现有的装法改了十几处、补了一步和几个链接。项目主人定：框架用他写好的，他写好的、给用户看的字不改，只有留给我示意的地方由我填。写好的字已恢复成原稿；查出来的对不上的地方列在下面，改不改由项目主人定。

### 稿子和程序对不上的地方（没改）

照各仓库 `origin/main` 的字表、手册和现有的装法查的：

| 稿子里 | 程序里、现在的情况 |
|---|---|
| 「跟随系统（自动）」 | 界面上是「跟随系统」 |
| 「示例对话」 | 界面上是「示范对话」 |
| 「AI 怎么认识你」里的「名称」 | 界面上是「称呼」 |
| 「自定义预设」 | 界面上那一项叫「自定义」 |
| 小节标题「向局域网开放」 | 那一屏的标题是「在别的设备上用」 |
| 「软甲包」 | 「软件包」（笔误） |
| 接入通讯平台（`miyu-im`） | 包编号是 `im`，`miyu-im` 是仓库名 |
| 整合包包含终端界面 | 托盘安装包（`.msi`、`.dmg`、`.deb`、`.rpm`）不带终端界面，AUR、Homebrew 的带 |
| Mac 和 Linux 还会装上 `litemiyu` | 现在哪种装法都不带；项目主人定了发版前带进去，等 `litemiyu` 发版和核心的 `packaging/` 跟上 |
| Linux 或者 MacOS 可以仅安装核心 | 只装核心的包现在只有 AUR、Homebrew 的 `miyu-core` |
| 引导的几步 | 「创建人格」后面还有一屏「配置外观」，稿子没写 |

### 核实和实测

- 上表的界面文字逐个对过字表：网页引导（`miyu-web` 的 `web-demo/packages/welcome/manifest.json`、`setup/manifest.json`、`settings/manifest.json`）、托盘菜单（`miyu-applet` 的 `resources/text/zh.toml`）、终端设置（`miyu-tui` 的 `resources/text/zh.json`）、通讯平台后台（`miyu-im` 的 `im/site/texts.js`）、核心的 `resources/core/human`。
- 「只装核心，再 `miyu pkg install webui`」实测过：只有核心的程序（当前 main 编的）加沙箱数据根，`miyu web` 回「未知命令 'web'」；`miyu pkg install webui --yes` 从软件源下载、核对、装上，之后 `miyu web --print` 印出地址；`tui` 同样。Linux。
- 「使用技巧」两条对过手册 `models`「池」一节和 `automation`「循环 /loop」，和程序一致。
- 推演人生已由 RM-1 拆掉，手册和开发者文档里搜过没有残留；页数 38 变 37，中英对齐、锚点、链接三个脚本照基线过。

### 蓝图改哪几节

不改。只改文档。

### 验收

1. `cargo xtask check` 全过（见 CI）。
2. 快速上手中英两页行数、标题层级、代码块个数相等，链接指得到。
