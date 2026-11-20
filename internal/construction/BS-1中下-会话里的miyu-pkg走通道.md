## BS-1 中下：会话里的 `miyu pkg` 走通道

状态：主会话做。设计 44（基础系统拆包）第三节第 4 条：装卸包不再是她的工具（BS-1 中上），会话里她跑 `miyu pkg`，经 `miyu call` 的通道带着会话身份，照原来的权限表问人。这一步做命令行那一头和写包、写技能的说明。

### 一、做了什么

1. **认出会话**：`miyu pkg` 看见 `MIYU_CALL_SOCK`（`miyu_cli::call::SOCK`）的，一律经通道发 `op: "pkg"`，不找数据根、不读本机令牌、不连核心的协议；`pack` 照旧先在本机打。通道连不上的说 `pkg-off.txt`、退出码 2，不退回去走协议（同 MC11）。
2. **子命令换成调用**（`pkg/session.rs`）：`list`、`info`、`search`（`-Q`、`-Qi`、`-Ss`）调 `packages`，`info` 写路径的看那个包文件夹；`install`、`remove`、`upgrade`（`-U`、`-R`、`-S <编号…>`、`-Su`、`-Syu`）调 `package`，路径照现在的目录换成绝对的、`.`、`..` 照字面去掉，写到 `package.toml` 的取它所在的文件夹；`files`、`owns`、`check`、`update`、`clean`、只有 `-y` 或 `-c` 的 `-S` 说 `pkg-unavailable.txt`、退出码 2。几次挨着做，有一次不成的照它退出、后面的不做。
3. **不认的**：`--yes`（问不问由权限策略判，她跳不过）、`--system`（装进哪一层由核心照会话判）、`--format`（印的就是工具的输出）。
4. **复用 `miyu call` 的通道**：`call.rs` 拆出 `exchange`（发请求、收回答，没收到的照 `Lost` 说为什么），`miyu call` 照旧写成自己的退出码和那几句。
5. **字**：新 `software/basesystem/shell/call/pkg-unavailable.txt`；出厂技能 `package-creator`、`skill-creator` 的「检查、装」两步改成在命令里跑 `miyu pkg info <文件夹>`、`miyu pkg install <文件夹>`；登记簿（设计 26 第十节）三行；设计 44 第三节第 4 条更正（命令行没有 `enable`、`disable`）；蓝图 `cli/pkg.md`「会话里」。

### 二、不做什么

- `packages` 包关着、通道没开的会话里没有 `MIYU_CALL_SOCK`，`miyu pkg` 照旧走协议、借本人的身份：同设计 39 MC10、MC11 记着的那一条（人批过在沙盒外跑的命令读得到本机令牌），设计 44 不加新口子。
- 会话外的 `miyu pkg info <路径>`（看一个包文件夹）还是只认编号，要的话另排。
- `package.list` 带组、按组装卸、斜杠命令的拥有者是 BS-1 下。

### 三、怎么验

1. 命令行：`pkg/session/tests.rs`（读的调 `packages`、写路径的看文件夹；改的调 `package`、路径换成绝对的文件夹、`.`、`..`、包文件、`-S` 几个编号、`--yes` 不认；只在本机做的子命令说用不了）。
2. 端到端：`miyu/tests/pkg_session.rs`，真核心、假模型调一次 `shell`，命令里的 `miyu pkg list`、`miyu pkg info shell` 经通道调成、记 `tool.nested`，`miyu pkg files shell` 退出码 2、不交给核心；`call.rs` 照旧过。
3. 手写变异 9 个全抓到：不认会话、`op` 发成 `call`、`info` 路径不看文件夹、`package.toml` 不取文件夹、`..` 不去掉、`-Su` 不升级、`.` 不当路径、`miyu call` 不在会话里的退出码、只在本机做的交给核心。
4. 整个工作区测试、本机门禁。
