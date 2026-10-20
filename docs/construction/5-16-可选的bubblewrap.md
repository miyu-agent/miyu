## 施工单 5-16：系统里有 bubblewrap 就用它

状态：上已合（2026-10-20，4c9636f2）；下待开工。2026-10-20 开工时拆成两半：上是探测、挂载、执行器；下是沙盒强度设置、起不来发系统通知、当场重探。2026-10-20 项目主人定（有就用、不自带，沙盒强度自动、弱沙盒两档）；图纸 `docs/designs/11-权限与沙盒.md` 第四节「有 bubblewrap 就用它」「设置页「隐私与安全」」、A7 的 2026-10-20 补。

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
