## 施工单 LV-5：去掉 bubblewrap 一路和沙盒强度、受保护的路径两组设置

状态：已完成（2026-11-11）。项目主人 2026-11-11 定，设计 `11-权限与沙盒.md` A17。

### 目的

A16 以后本人的会话只在只读时进沙盒、能写的范围恒为空，外部身份的规格写死不用挂载。bubblewrap 只在「本人只读」里用得到，比 Landlock 只多三样边角；`sandbox.strength` 只管用不用它；`sandbox.readonly.*`（受保护的路径）在判法表里已经不产生差别。这一步把这一路和两组设置删掉，行为只变一处：有 bubblewrap 的机器上，只读时藏起来的目录从「连名字都看不到」变成「看得到名字、读不到里面」。

### 蓝图改哪几节

- `sandbox/linux.md`：「在哪」「怎么走」去掉 bubblewrap；「已知的限制」写清少掉的三样。
- `sandbox.md`：规格去掉 `mount`，探测的手段去掉 `bwrap`。
- `session/tools.md` 第 1a 条：只读的规格不再写 `mount`，不看沙盒强度。
- `session/guard.md`：去掉受保护的路径（第 7 条后半、问人说明里的 `read_only`、守着它的几行）。
- `fs.md`：边界表去掉只禁写名单。
- `config.md`：去掉 `sandbox.strength`、`sandbox.readonly.*` 四行和它们的报错；`sandbox.private.*` 的说明写清只在只读下起作用。
- `core.md`：探测不再设「能不能用挂载」。

### 删除清单

动手之前列的；清单外补的在最后一节标出。

**一、bubblewrap 一路（施工 5-16）**

| 文件 | 删什么 |
|---|---|
| `crates/miyu-sandbox/src/bin/miyu-sandbox/linux/bwrap.rs`、`bwrap/tests.rs` | 整个模块 |
| `crates/miyu-sandbox/src/bin/miyu-sandbox/linux.rs` | `mod bwrap`、`mechanisms()` 里报 `bwrap` 的、`run` 里写了 `mount` 换成 bubblewrap 的、`run_ruled` 里查 `mount` 的 |
| `crates/miyu-sandbox/src/spec.rs` | `Spec::mount` |
| `crates/miyu-sandbox/src/lib.rs` | `MOUNTS`、`set_mounts`、`mounts` |
| `crates/miyu-sandbox/src/linux/cache.rs` | `cacheable` 里的 `!spec.mount` |
| `crates/miyu-sandbox/tests/linux_mounts.rs` | 整份（只测挂载那一路） |
| `crates/miyu-sandbox/tests/support/linux.rs` | 夹具 `bwrap()` |
| `crates/miyu-sandbox/tests/linux_sockets.rs`、`lifeline.rs`、`linux_cache.rs`，`src/linux/cache/tests.rs`、`src/tests.rs` | 只在有 bwrap 时多跑的那几段、造挂载规格的那几项 |
| 各处造 `Spec` 时写的 `mount: false`（`miyu-sandbox`、`miyu-session`、`miyu-call` 的代码和测试） | 那一格 |
| `crates/miyu-core/src/sandbox.rs` | `availability` 里照 `bwrap` 设 `set_mounts` 的那一行 |
| `crates/miyu-session/src/sandbox.rs` | `weak`、`set_weak`、`mount()`、`hides_inside` 和 `hidden` 的 `mount` 参数 |
| `crates/miyu-session/src/tools/back.rs` | `set_weak` |
| `crates/miyu-session/src/sandbox/tests.rs` | 测弱沙盒、挂载的那几条 |

外部身份那一路的测试（`tests/linux.rs`、`linux_aliases.rs`、`linux_sockets.rs` 里不带挂载的、`miyu-session` 的 `sandbox_member.rs` 等）断言不动，只去掉造规格时的 `mount: false`。

规格 JSON 少一格 `mount`：助手和核心同版发，探测时构建 id 对不上的助手不用（`ProbeError::Stale`），没有跨版本的事。探测缓存里旧的 `bwrap` 只是手段名单里的一个字：构建 id 换了的那一份本来就不认，认的那一份里多一个不认识的手段名不碍事，加一条测试守着。

**二、`sandbox.strength`（施工 5-16 下）**

| 文件 | 删什么 |
|---|---|
| `crates/miyu-session/src/settings.rs` | `SandboxSettings`、`WEAK` |
| `crates/miyu-session/src/actor/model.rs` | 回合开始照它设 `set_weak` 的两行 |
| `crates/miyu-core/src/settings.rs` | 登记 `SandboxSettings::ITEMS` 的那一行 |
| `crates/miyu-endpoint/src/config/private.rs`、`config/methods.rs` | `hidden_here` 和调它的地方 |
| `resources/core/human/{zh,en,ja}.json` | `sandbox.strength` 的名字、说明、选项；设置页组 `sandbox` 的名字 |

**三、`sandbox.readonly.enabled`、`.add`、`.remove`（施工 5-17 下）**

| 文件 | 删什么 |
|---|---|
| `crates/miyu-tool/src/readonly.rs`、`readonly/tests.rs`、`lib.rs` 里的 `pub mod readonly` | 整个模块 |
| `crates/miyu-fs/src/readonly.rs`、`readonly/shipped.rs`、`readonly/tests.rs`，`lib.rs` 里的导出（`Readonly`、`ReadonlyEntry`、`SHIPPED_READONLY`、`readonly_entry`） | 整个模块，连出厂名单 |
| `crates/miyu-fs/src/boundary.rs` | `Places::readonly`、`Boundary` 的 `inside`、`readonly` 两格、`protected`、`writable`、`listed_inside`、`zone_with` 的 `listed` 参数 |
| `crates/miyu-fs/tests/boundary.rs`、`tests/support/mod.rs` | 测只禁写名单的几条、夹具里那一格 |
| `crates/miyu-session/src/guard.rs`、`guard/bounds.rs` | `Guard::readonly`、`readonly_of`、`set_readonly` |
| `crates/miyu-session/src/guard/asking.rs` | `Asked::alone` 和 `Asked::new` 里查受保护的、`scope` 里那一支、`granted` 里那一个条件、问人说明里的区名 `read_only`（只读时要问的只有私密目录） |
| `crates/miyu-session/src/actor/model.rs` | 回合开始换名单的那一行 |
| `crates/miyu-core/src/settings.rs` | 登记 `readonly::ITEMS` 的那一行 |
| `crates/miyu-endpoint/src/config/private.rs`、`private/tests.rs` | `readonly_added`、`problem`/`ticks`/`options` 里认这组键的 |
| `crates/miyu-config/src/problem.rs`、`problem/tell.rs` | `Code::ReadonlyTooBroad`、`ReadonlyPath`、`ReadonlyNotShipped` |
| `resources/core/human/{zh,en,ja}.json` | 三个键的名字、说明；设置页组 `readonly` 的名字；三句报错 `config/readonly-*` |
| `crates/miyu-endpoint/tests/config_set_private.rs`、`crates/miyu-session/tests/support/routing.rs`、`crates/miyu-core/tests/settings.rs` | 直接测这组设置的 |

先证明是摆设：判法表 `guard/levels.rs` 的 `mark` 里，名单只把「能写」换成「只能读」，这两片在完全放开、只读两列下判的一样；问人只剩只读时读私密目录，私密目录不会是受保护的。删之前全量测试记下结果，删之后同一批测试除了直接测这组设置的以外不许有行为断言变化。

**四、不删、只改说明**

`sandbox.private.enabled`、`.add`、`.remove`（私密目录）行为不变。三种语言的说明写清只在只读模式下起作用。

**五、打包**

| 文件 | 删什么 |
|---|---|
| `packaging/aur/miyu-core/PKGBUILD.in`、`packaging/aur/miyu-agent-git/PKGBUILD` | `optdepends` 的 bubblewrap |
| `packaging/README.md`、`packaging/systemd/system/miyu.service` | 「不要写 `RestrictNamespaces=`」的提醒：Landlock 不用命名空间 |
| `crates/miyu-core/tests/packaging.rs` | 守样例里没有 `RestrictNamespaces=` 的那一句断言 |
| `xtask/src/dist/layout.rs`、`dist/tests.rs` | 发行包 README 里讲 bubblewrap 的那一句 |

**六、文档**

设计 11 加 A17，开头那一段写明 5-16 的 bubblewrap、5-17 的只禁写名单以 A17 为准；蓝图见上；`docs/reference/config.md`、`config.en.md`；设计样本 `docs/designs/samples/config/`（照 `MIYU_SAMPLE_WRITE=1` 重新生成）；`docs/designs/samples/sandbox/spec.json` 没有 `mount`，不动；施工单 `5-16-可选的bubblewrap.md`、`5-17下-只禁写.md` 开头标已撤；`README.md` 加 LV-5 一行。

### 不做什么

- 不动外部身份的沙盒、macOS 的 Seatbelt、Windows 的 MXC。
- 不动私密目录的行为。
- 不动挂载别名（施工 SEC-3，`linux/aliases.rs`）：Landlock 那一路照旧要它。
- 不补只读下「改时间戳」这类系统调用过滤。

### 验收

- 不变量，改前改后都过：本人只读下数据根和私密目录的内容读不到、哪儿都写不进（`miyu-session` 的 `sandbox/tests.rs`、`tests/sandbox.rs`、`tests/guard_private.rs`）；外部身份那一路的测试断言一个不改、全过；规则集缓存照旧能用（`tests/linux_cache.rs`、`src/linux/cache/tests.rs`）。
- 这台机器装着 bubblewrap：助手的 `probe` 不再报 `bwrap`，只读会话的规格里没有 `mount`。
- 老配置里留着 `sandbox.strength`、`sandbox.readonly.*` 的：同 LV-2 的 `sandbox.tools`，照不在清单里的键报 `unknown_key`（警告，原样留在文件里，不进最终值），核心照常起。加一条测试守着。
- `cargo test --workspace --no-fail-fast`、两个目标的 clippy、`cargo fmt --check`、`cargo doc` 全过。

### 风险

- 有 bubblewrap 的 Linux 上只读时少了三样（藏起来的目录看得到名字；改得了时间戳、权限、属主；内核 6.2 以前截得了文件）。项目主人 2026-11-11 接受，写进蓝图 `sandbox/linux.md`「已知的限制」。
- 以前靠 `sandbox.readonly.add` 保护的文件：A16 起它已经不起作用，这一步只是把设置拿掉。

### 清单外补的

- `crates/miyu/tests/config.rs`：`miyu config get` 列全部最终值的那一条，去掉 `sandbox.readonly.enabled`、`sandbox.strength` 两行。
- `crates/miyu-core/src/settings.rs`：登记的模块数 42 → 40。
- `crates/miyu-config/src/test_support.rs`：测试用的三句 `config/readonly-*`。
- `crates/miyu-core/tests/packaging.rs`：另一条测试里守 `packaging/README.md` 写着 `RestrictNamespaces` 的那一句。
- 助手对「`--ruleset` 配了写着 `read` 的规格」说的那一句从 `bad spec: --ruleset with mount or read` 改成 `bad spec: --ruleset with read`（核心不会这样交，测试没有断言它）。
- 注释里提 bubblewrap 的几处：`crates/miyu-fs/src/wide.rs`、`crates/miyu-sandbox/src/linux/aliases.rs`、`crates/miyu-perf/src/cpu.rs`、`crates/miyu-sandbox/tests/lifeline.rs`。
- `crates/miyu-sandbox/src/tests.rs`：套接字管不管得住的那一条，输入的手段名单里去掉 `bwrap`（断言不变）。
- `crates/miyu-session/src/sandbox/tests.rs` 的 `a_command_runs_when_a_private_dir_holds_the_data_root`：原来只在有 bubblewrap 的机器上跑，改成照 Landlock 跑，断言不变（命令起得来、上级里面的读不到）。
- 问人说明里的区名：`read_only` 去掉以后，不是私密目录的写 `outside`（要问人的只有只读时读私密目录，走不到）。
- 设计 `12-进程形态与分发.md`（安装包依赖 bubblewrap 那一条）、`62-扩展沙盒.md`（靠 bubblewrap 一路和受保护的路径的几处）各加一句以 A17 为准；蓝图 `perf.md`、`skills.md`、`core.md` 里提到的一句。
- 新加的测试：`crates/miyu-session/tests/guard_protected.rs`（以前名单上的和平常的文件判的一样）、`tests/sandbox.rs` 的 `a_read_only_command_cannot_read_inside_a_private_dir`、`crates/miyu-core/src/sandbox/tests.rs` 的 `a_remembered_probe_that_still_names_bwrap_is_used`；`crates/miyu-endpoint/tests/check.rs` 那一条多查四个键。

没删、留着的：`miyu_sandbox::linux::with_aliases` 的导出（原来给 bubblewrap 那一路用，现在只有测试调；规则集那一路用的是带挂载表的那一个）；`Sandbox::hidden` 里照「能写的范围」判藏不藏的那几支（本人的会话能写的范围恒为空，只有 macOS 的那一支和测试走得到）。
