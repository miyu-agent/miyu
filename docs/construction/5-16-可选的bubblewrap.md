## 施工单 5-16：系统里有 bubblewrap 就用它

状态：上已合（2026-10-20，4c9636f2）；再补合了（2026-10-22，8b4da48b）；下拆成两半（2026-10-21 主会话定）：下上半（沙盒强度）合了（5d7f92d8，先过了三台）；下下半（起不来当场重探、改用 Landlock、发系统通知）待开工：要动命令跑完看退出码的几处（`shell/` 下前台、后台、脚本、通道），通道、进程那几个文件 M-3 下正在改，等它合了再和 shorin-dd 对边界；系统通知是协议上新的推送，形状那时定。2026-10-20 开工时拆成两半：上是探测、挂载、执行器；下是沙盒强度设置、起不来发系统通知、当场重探。2026-10-20 项目主人定（有就用、不自带，沙盒强度自动、弱沙盒两档）；图纸 `docs/designs/11-权限与沙盒.md` 第四节「有 bubblewrap 就用它」「设置页「隐私与安全」」、A7 的 2026-10-20 补。

### 做什么

1. 探测：只在 `/usr/bin`、`/bin`、`/usr/local/bin` 找、要属于 root 且别人写不了；照正式的挂载真起一次，在里面验藏起来的是空的、写进去报只读、能写的写得进，都对 `mechanisms` 多一个 `bwrap`（记下的探测结果照 V-5 上带上 bwrap 的路径、大小、改动时间）。
1½. 配置项沙盒强度（只在 Linux 上登记）：`auto`（默认）、`weak`（只用 Landlock）；设置页「隐私与安全」。
1¾. 用着起不来：那一条退出 125，当场重探，探不过的改用 Landlock；发系统通知（同一个原因一次运行一次），别的界面不动。
2. 有 bwrap 时，助手照规格写挂载：`/` 只读，能写的可写挂回，藏起来的盖空的只读 tmpfs（连名字都看不到、写进去报只读），放开只读的再只读挂回；系统服务的套接字照旧由 Landlock 拦（bwrap 里再收紧一次）。
3. 执行器在有 bwrap 时把落在能写范围里的私密目录也写进 `hidden`。
4. 进程：打断、超时整组杀、退出码、信号和现在一样（中间多一层 bwrap 父进程，`--die-with-parent`）。
5. 量：每条命令的时间、多出的常驻内存（本机 1.7 ms、2.3 MB），记进 `sandbox/linux.md`。

### 验收

- 有 bwrap 的机器上：放开 `~` 以后 `mkdir ~/x`、`sed -i ~/.bashrc` 都成；`ls ~/.ssh` 是空的；数据根看不到；技能文件夹读得到、写不了。
- 没有、起不来的（假一个失败的 bwrap）照 Landlock，行为同 5-15。
- 打断、超时、后台命令照旧。CI 的 Ubuntu 上两条路都跑。

### 上：做了什么

1. 助手 `probe`：Landlock 能用、bubblewrap 也能用的多报 `bwrap`（找法、真起一次验，照第 1 条）。
2. 规格多 `mount`（没有的不写）；助手照它换成 bubblewrap：挂载的先后照 `sandbox/linux.md`「怎么走」第 3 条，挂好以后在里面再跑一次自己，里面那一次照旧用 Landlock（系统服务的套接字照旧拦）。规格要挂载、没有能用的 bwrap 的不跑。
3. 核心起来时照探测设 `miyu_sandbox::set_mounts`；执行器照它写 `mount`，落在能写的里面的私密目录照样藏。
4. 本机量：放开 `~` 时挂载约 2.4 ms、只用 Landlock 约 1.2 ms。

### 上：验收

- `crates/miyu-sandbox/tests/linux_mounts.rs` 的 `with_mounts_a_hidden_path_inside_a_writable_one_vanishes_and_nothing_is_frozen`：有 bwrap 的机器上那一层新建、删除都行，藏起来的列出来是空的、读不到、写不进、挪不走；没有的拒绝执行。`tests/linux.rs` 的 `system_service_sockets_outside_the_spec_cannot_be_connected` 挂载时照样拦。共用的几样挪进 `tests/support/linux.rs`。
- `linux/bwrap/tests.rs`：挂载的先后、文件拿 `/dev/null` 盖、不在的跳过、里面的规格只留 `write`。
- `crates/miyu-session/src/sandbox/tests.rs`：能写的里面的私密目录只在有挂载时藏。
- `cargo xtask check` 八项。

### 下上半：做了什么（2026-10-21）

1. 配置项 `sandbox.strength`（`miyu-session` 的 `SandboxSettings`）：`auto`（默认）、`weak`，系统、个人两层，下一轮生效，「隐私与安全」页的「沙盒」组，下拉。
2. 三个系统都登记：只在 Linux 上登记的话生成的 Schema、参考文件照系统不同（5-15 下撞过一回）。别的系统上 `config.schema` 照这台机器标 `hidden`（端点 `config/private.rs` 的 `hidden_here`）。
3. 回合开始照这一轮的配置换（`Tools::set_weak` → `Sandbox::set_weak`）：`weak` 的写规格时不写 `mount`（`Sandbox::mount`），能写范围里的私密目录照 5-13 不藏。

### 下上半：验收

- `miyu-session` 的 `sandbox/tests.rs` 的 `weak_strength_turns_mounts_off`、`a_weak_turn_writes_no_mount_into_the_spec`（机器能用挂载时弱沙盒的规格不写 `mount`）；变异 3 个全抓住；端点 `config/private/tests.rs` 的 `the_strength_is_drawn_only_on_linux`。
- 配置清单、三份字表、生成的样本跟着改。`cargo xtask check` 八项；先推 step 分支过三台。

### 下补（2026-10-21，网页实测报的）

沙盒强度在「隐私与安全」页上没地方挂：X-36 的放行表（`sandbox.tools.<名字>`，不画）也用组 `sandbox`，却挂在「通用」页、登记得更早，`config.schema` 的组照先出现的那一项定页，被挤到了「通用」。放行表挪到「隐私与安全」、在清单里排到沙盒强度后面；两边各往三份字表的组名里加了一回 `sandbox`，删掉一份。核心的设置测试多一条 `a_group_belongs_to_one_page`：同一个组名挂在两页上当场报。

### 再补（2026-10-22，网页的 demo 树报的）

核心和沙盒助手版本对不上：网页的 demo 树只编了 `-p miyu`，助手停在旧版，新规格里的 `sockets`（施工 M-3）它认不出，所有沙盒命令报 bad spec 退出 125，看不出为什么。

1. 构建 id（`miyu-sandbox` 的 `build.rs`）：照这个包自己的源码（`src/` 下的文件、`Cargo.toml`、`build.rs`）算 64 位 FNV-1a 摘要，十六位十六进制，代码里是 `miyu_sandbox::BUILD`。核心链进来的这个包和助手是同一份源码编的才一样；别的包改了不算。
2. 助手的 `probe` 那一行多一格 `build`；核心探的时候对（`probe.rs`）：对不上的（旧版的不报，读成空的）是 `ProbeError::Stale`，记一行 `WARN sandbox unavailable reason="helper <路径> is from another build (helper <它的>, core <核心的>); rebuild it: cargo build -p miyu-sandbox"`，沙盒照「助手跑不起来」用不了（协议上照旧 `helper_failed`，头不用改）。
3. 记下的探测（施工 V-5 上）照构建 id 认：核心换了、助手没换的当没记过，照常探，探出来对不上就照上一条说。
4. 验收：`miyu-sandbox` 的 `probe/tests.rs`（那一行带 `build`、构建 id 是十六位十六进制、对不上的那一句）、`tests/probe.rs`（真的助手报的和核心一样；假的助手不报的、报了别的，都说出是哪个文件和 `cargo build -p miyu-sandbox`）；核心的 `sandbox/tests.rs`（旧版的助手用不了、记的那一行说清；记下的构建 id 不对当没记过）。变异全抓住。`cargo xtask check` 八项；先推 step 分支过三台。
