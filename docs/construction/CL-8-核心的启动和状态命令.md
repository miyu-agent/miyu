## 施工单 CL-8：`miyu core start`、`miyu core status`

状态：已完成（2026-11-18）。

### 为什么

项目主人 2026-11-18 定（核心主会话转达）命令行上核心和网页各管各的：

- `miyu core start | status | stop` 只管核心。`start` 拉起核心就回来，不开网页、不开浏览器；`status` 说在不在跑、版本、数据根，`--format json` 也给；`stop` 照现在。
- `miyu web` 管网页（网页服务没起的在后台起、开浏览器、马上回来；`miyu web stop | status`；不再闲置退出）。那是网页包自己的命令，另一边做。

原来只有 `miyu core stop`（施工 CL-4，给桌面端的「退出」用），不写进帮助。想让核心先起来只能随便跑一条会连核心的命令；想知道核心在不在跑、是哪一版、用的哪个数据根，没有一条命令答得出。

### 改成什么样

1. **`miyu core start`**：核心没在跑就拉起来，等它好了再回来；已经在跑的什么都不做。成了什么都不印，退出码 0（同 `stop`）。只拉核心。人停过的（`run/stopped`，施工 CS-5）照样拉。起不来的说原因，退出码 1。
2. **`miyu core status [--format <text|json>]`**：只看，不拉起。
   - 在跑：三行「核心正在运行」「版本：…」「数据根：…」，退出码 0。版本是核心握手报的那个：升级以后没重启的，和 `miyu --version` 不一样，看得出来。
   - 没在跑：三行「核心没有在运行」「数据根：…」「提示：运行 'miyu core start' 启动」，退出码 3。
   - 说不准（核心还在跑却连不上、握手被拒）：说在标准错误上，退出码 1。
   - `--format json`：一行 `{"running", "version", "data_root"}`，没在跑的 `version` 是 `null`，退出码照上面。
3. **帮助**：`core` 写进主程序帮助页的「系统」一组；`miyu core -h` 那一页改成三条命令，多一节「status 的退出状态」。中文、英文两页一起改。
4. **文档**：蓝图 `core.md`「对外的样子」、`cli/main.md`；手册 `cli.md` 的 `miyu core` 一节、`service.md` 里手动起核心的几句，中英一起。

### 定下来的

- **退出码 3 是「没在跑」**：核心主会话定的，同 `systemctl status`；网页包的 `miyu web status` 照同一套（0 在跑、3 没在跑、1 别的错），托盘、脚本两边一个判法。
- **`start` 成了不说话**：和 `stop` 一样。要看结果敲 `status`。
- **数据根写绝对路径**，不写成 `~`：文字和 JSON 是同一个值，脚本和人看到的一样。

### 不做（留给 CL-8 补）

手册里讲 `miyu web` 的句子（`--stop` 改 `stop`、`status`、不再闲置退出、`web.idle_seconds` 的默认值、托盘调的命令）这一步不动：那些说的是网页程序的行为和输出，等网页包做完、核心的 `heads.toml` 升到它的新提交的那一步一起改，不然合了以后手册和实际对不上。

### 验收

- 改前红：`miyu core start`、`miyu core status` 在没改的程序上是「不认识的子命令」。
- 假核心的测试、真核心的测试、门禁、三台 CI。
- 实测：沙箱里照着帮助页敲一遍。

### 做下来的

- 改前红：没改的程序上 `miyu core start`、`miyu core status` 是「'miyu core' 没有子命令」，新测试编不过（`core_start`、`core_status` 不存在）。
- 测试：假核心的三条（`crates/miyu-cli/src/core_status/tests.rs`）、真核心的一条（`crates/miyu/tests/core_start.rs`）；命令行 372 条单元测试里守帮助页的几条（每个子命令都印自己那一页、最宽 80 列、说明不带句号不列一串）跟着过。头一版帮助页的说明写了「启动、停止核心，查看状态」，被「说明不列一串」那条挡下，改成「管理核心」。
- 实测（2026-11-18，新的空沙箱，数据根和家目录都在沙箱里，不用模型）照帮助页敲一遍：
  1. `miyu core status` → 三行「核心没有在运行」「数据根：<沙箱的绝对路径>」「提示：运行 'miyu core start' 启动」，退出码 3。
  2. `miyu core start` → 什么都不印，退出码 0；再敲一次一样，运行日志里只有一条 `starting`。
  3. `miyu core status` → 「核心正在运行」「版本：2026.11.0」「数据根：…」，退出码 0；英文环境印 `The core is running`、`version: 2026.11.0`、`data root: …`。
  4. `miyu core status --format json` → `{"data_root":"…","running":true,"version":"2026.11.0"}`，退出码 0。
  5. `miyu core stop` → 什么都不印，退出码 0，`run/` 里留下 `stopped`；`status --format json` → `{"data_root":"…","running":false,"version":null}`，退出码 3。
  6. 人停过以后 `miyu core start` → 退出码 0，核心起来了，`run/stopped` 没了，运行日志第二条 `starting`。
  7. `miyu core bogus` → 「miyu: 'miyu core' 没有子命令 'bogus'」和一行提示。
