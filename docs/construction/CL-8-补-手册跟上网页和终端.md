## 施工单 CL-8 补：核心钉上新的网页、终端，手册跟上

状态：已完成（2026-11-18）。

### 为什么

项目主人 2026-11-18 定的那一套交互，网页这一半做完了（网页仓库 main `06bcf0b6`，施工 W-1214）：

- `miyu web`：网页服务没起的先在后台起（核心没跑一并拉起），开浏览器、印地址，马上返回、不占终端；浏览器关了服务照样在。
- 网页服务不再闲置退出：`web.idle_seconds` 出厂从 `600` 改成 `0`，键留着，设置页照旧不露。
- `miyu web stop`、`miyu web status` 取代 `--stop`；`--stop` 直接去掉。

终端那边同一天把两个开关改成正着说（终端仓库 `53e38905`）：`tui.skip_pick` 改名 `tui.auto_pick`（值不反）；挂载位部件（`tui.side`、`tui.footer`、`tui.above`、`tui.top`）每一项的 `hidden` 改成 `shown`（值反过来，出厂 `true`）。

核心仓库钉着两个界面的提交号（`heads.toml`），手册里讲它们的句子也在核心仓库。施工 CL-8 只带了核心自己的命令，没动这些句子：那时核心钉的还是旧的网页，写了就和实际对不上。这一步把提交号和手册一起挪过去。

### 改成什么样

1. `heads.toml`：`[webui]` 钉 `70339229165bf7f079b2df3f6bdfa82ca296ab2d`（带着 `miyu web stop | status` 的 `06bcf0b6` 往后，网页拆掉推演人生的那几步也在里面：核心施工 RM-1 以后没有 `life.*` 了，头要钉拆过的）；`[tui]` 钉 `cc6f38a10d4ff432909ade0ac065c78e638044f7`：带着改名的 `53e38905` 往后，终端拆掉推演人生的那几步也在里面，它在终端 main 上的 CI 四项全绿。
2. 手册（中英一起）：
   - `heads/web`：`miyu web` 在后台起、马上返回、关了浏览器服务还在；选项表去掉 `--stop`；闲置退出那一段改成出厂不退；只关网页用 `miyu web stop`，看在不在用 `miyu web status`；「出了问题」表里连不上的原因照实改。
   - `cli` 的 `miyu web` 一节：用法多 `stop`、`status`，各自印什么、印在哪、退出码（`status` 0 在跑、3 没在跑，和 `miyu core status` 同一套）。
   - `service`：只关网页的命令；当系统服务跑网页的那一节去掉「先关掉空闲退出」那一步（出厂就不退），提醒别改成非 0；配置表的默认值；怎么停的表。
   - `data`：备份前停网页的两处命令。
   - `heads/applet`、`dev/architecture`：托盘「停止」调的是 `miyu web stop`。
   - `config-keys`：`web.idle_seconds` 的默认值；`tui.auto_pick`；部件的 `shown`。`config`、`heads/tui`、`personas-presets` 里的例子和键名跟着改。
3. `packaging/README.md`、`packaging/systemd/system/miyu-web.service` 的注释：出厂就不退，不用先设 `idle_seconds = 0`。
4. `bench/rivals/rivals.toml`：对比基准里给 Miyu 配的那一条改成 `tui.auto_pick`；蓝图 `perf.md` 那一句跟着改。

### 顺手改的

终端设置页里这一项给人看的名字，手册原来写的是「新会话不再询问人格预设」，终端包里一直是「自动选择人格和预设」。照包里的改了（`heads/tui`、`personas-presets` 两对页）。

### 托盘

托盘不在 `heads.toml` 里，只改文档。它的「停止」「退出 Miyu」改调 `miyu web stop`（再 `miyu core stop`），「启动」照旧，在托盘仓库 main `87e2f4d`；手册 `heads/applet`、`dev/architecture` 照它写。

### 验收

- 门禁（提示词、死链、打包的测试：`packaging.rs` 断言打包进去的 README 里有 `idle_seconds = 0`）。
- 实测：照钉的提交装上网页包，敲 `miyu web`、`miyu web status`、`miyu web stop`、`miyu web --stop`，对手册写的。

### 做下来的

- 文档里写的每一句都对着两个仓库钉的那个提交的源码核过（网页：帮助页、给人看的字、停和看状态的代码、包清单里 `idle_seconds` 的出厂值；终端：包清单里 `auto_pick`、四个挂载位的 `shown`）。
- 实测（2026-11-18，沙箱数据根和家目录；开浏览器的程序换成只记一行的假程序，不在桌面上弹窗；不用模型）：把网页仓库检出到钉的 `70339229`（钉的提交换过两次：`06bcf0b6`、`ecdf89f8`、`70339229`，每换一次重编、重走一遍，三遍结果一样），自己编 `miyu-web`，照它的 `tools/pack-web.py --stage` 搭成包，`miyu pkg install` 装进沙箱。`miyu web --version` 是 `2026.11.0`（这台机器系统里装着旧的候选版，不装进沙箱的话转交到的是那一份，头一次就是这样、`status` 报「未知选项」）。
  1. 核心、网页都没在跑时敲 `miyu web` → 印「已在浏览器中打开：http://127.0.0.1:8304/」，不到 0.2 秒返回，退出码 0；假浏览器记下了那个地址；`miyu core status` 是「核心正在运行」（一并拉起了）。
  2. `miyu web status` → 「网页界面正在运行：http://127.0.0.1:8304」，退出码 0；英文环境没在跑时印 `the web interface is not running`。
  3. `miyu web status extra` → 「miyu: 'status' 后面不能再写别的参数」和一行提示，退出码 2。
  4. `miyu web --stop` → 「miyu: 未知选项 '--stop'」和一行提示，退出码 2。
  5. `miyu web stop` → 「已停止网页界面」，退出码 0；`miyu web status` → 「网页界面没有在运行」，退出码 3；再 `stop` 一次还是那一句、退出码 0。这时 `miyu core status` 还是「核心正在运行」。
  6. `miyu config get web.idle_seconds` → `0`。
  7. `miyu web --help`：用法多一行 `miyu web <命令>`，末尾一节「命令」列 `stop`、`status`。
- 没实测的：终端的两个键（没编终端包），只对了源码。

