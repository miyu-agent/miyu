## 施工单 DOC-9：手册跟上默认人格预设的警告、英文报错、循环和迁移包的英文

状态：待验收（2026-11-17）。跟核心的 CK-1、EN-1、EN-2、LO-2、LO-3 和迁移包的 MG-21。

### 改了什么

中文页、英文页同一处一起改。

- `guide/personas-presets`：默认人格、默认预设指着没有的，`miyu config set` 照收并接一行警告，`miyu check` 也警告（DOC-6 实测时程序还不报，手册那时照实写了不报，现在改回来）。
- `guide/troubleshooting`、`guide/rescue`、`guide/data` 英文页：连不上核心的五句、数据根不归自己的两句、`MIYU_HOME` 不是绝对路径的一句，程序有英文了，从照意思写换成引原文。命令行印的是去掉句号、句首小写的样子，照那个样子引。
- `guide/automation`、`commands`、`sessions`、`heads/web`：`/stop` 连这个会话的循环一起停；关循环 `off` 写成 `clear`、`stop` 也行；AI 正在干活时敲的 `/loop` 等这一轮结束再跑头一次。
- `guide/migrate`：英文界面下目录叫 `Old Miyu migration`、两份文件叫 `report.md`、`for-agent.json`、报告是英文的；英文页整页换成英文名，报告里的节名、状态照迁移程序英文的原文。`~/.local/bin/miyu` 是这一版的时报告自己写「别删」，手册那句兜底收成一句。写进新版的东西不跟语言变的（`旧版产物/`、`旧版/`、兜底会话的名字）英文页照旧留中文。

没改的：`miyu check` 的示例路径、`miyu schedule --cwd` 的默认值，程序的帮助改成了手册原来写的样子；换模型当场显示新的（请求照旧下一轮才换，手册写的是「下一轮开始生效」）。

### 实测

沙箱数据根，当前 main 编的 `miyu`，Linux，中文、英文界面各一遍（这一轮不用模型）：

- `miyu config set preset.default gone`：「已设置 …」，下一行「miyu: 警告：preset.default 写的预设 gone 现在没有：建了这个预设以后就能用」；`persona.default` 同样。`miyu check` 两条警告，退出码 0。英文是 `miyu: warning: preset.default names preset gone, which doesn't exist yet; it works once the preset is created`。
- `MIYU_HOME=relative/dir miyu check`：「miyu: MIYU_HOME 要写绝对路径，写的是 relative/dir」，英文 `miyu: MIYU_HOME must be an absolute path, got relative/dir`。
- `MIYU_HOME` 指到一个放着别的文件的目录：「… 里有别的东西，认不出是 Miyu 的数据根，没有动它。…」，英文 `… has other things in it and isn't a Miyu data root, so it was left alone. …`。
- 套接字的位置上放一个普通文件：「miyu: 核心起不来：… 上有别的东西：不是套接字的文件，或者别的程序正在听。不动它」；数据根和临时目录的路径都很长：「miyu: 核心起不来：套接字的路径 … 太长，临时目录下也放不下」。英文同字表。`guide/troubleshooting` 这两行照它改（EN-2 起这两句前面多了「核心起不来：」，也有了英文）。
- `MIYU_RESOURCES` 指到不在的目录：中英界面印的都是「miyu: 核心起不来（resources_missing）：…」，取不到字表时的退路，手册没写这一句。
- `miyu help check`、`miyu help schedule`：示例路径带 `root/`，`--cwd` 写「默认是新会话自己的工作区」。

没实测：

- 循环和 `/stop`：斜杠命令要在终端、网页里敲（`miyu ask "/loop off"` 是把这几个字当话发给模型），照核心 LO-2、LO-3 的施工单和字表写。
- 连不上核心的另外几句（等了太久、别的程序正在拉起、拉不起、没说为什么）：没造出那几种情形，照字表引。
- 迁移包：没有 0.7 旧版的数据可迁，照 `miyu-migrate` main 的字表和它的测试写。它的最新发布版还是 0.1.3，不带这些，要等它发新版手册才对得上。
- Windows、macOS。

### 蓝图改哪几节

不改。只改文档。

### 验收

1. `cargo xtask check` 全过（见 CI）。
2. 改过的每页中英行数、标题层级、表格行列、代码块个数相等；英文页的链接和锚点都指得到。
3. 引的原文对过实测的输出或者字表（`resources/core/human/{zh,en}.json` 的 `start/…`、`commands/usage/loop/off`，`miyu-migrate` 的 `src/words/`）。
