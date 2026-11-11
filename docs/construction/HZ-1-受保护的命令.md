## 施工单 HZ-1：受保护的命令按类给开关

状态：已完成（2026-11-11）。项目主人 2026-11-11 定，设计 `11-权限与沙盒.md` A18。

### 目的

A16 以后本人的会话默认完全放开：命令不进沙盒、什么都不拦，只有高危命令照旧，灭顶的直接拒，其余的先问人（没有确认界面的场所直接拒）。问人的那几种以前没有开关，嫌烦也关不掉。这一步在设置页「隐私与安全」最上面加一节「受保护的命令」，把完全放开下也先问的几类列出来，一类一个开关；直接拒的几种只列在说明里，不给开关。

### 蓝图改哪几节

- `session/guard.md` 第四条的表里「工具报高危」那一行、第四条末「高危命令」那一条（改成现在的样子：完全放开照问，按类给开关，谁不看开关）、「在哪」「守着它的」。
- `config.md`：键表、键的说明多 `permission.confirm.*` 五项；`config.schema` 的组多一格可选的 `description`；「给人看的字」多五项的名字和说明、组 `protected` 和它的说明；参考文件的样本块；查字的规矩、「守着它的」。
- `store/resources.md`：`human/<语言>.json` 的 `config` 多一格 `group_descriptions`。
- `tools/shell.md`「高危」第 3、6 条和「在哪」：看不透的单列一种；问人的全交回、带归的类；这一段挪到 `shell/hazard.rs`。
- `policy.md`：`hazard.rs` 那一行。
- 参考 `docs/reference/config.md`、`config.en.md`：组的 `description`。
- 设计 `11-权限与沙盒.md`：加 A18；A16 里把 `mkfs`、`dd` 写设备列成问人的那一句改正。

### 定了的

**键**。五项都是开关，出厂 `true`（照旧问），只有系统一层，下一轮生效；在 `crates/miyu-session/src/settings.rs` 的 `ConfirmSettings` 声明。

| 键 | 管哪几种（`miyu_policy::hazard::Kind`） |
|---|---|
| `permission.confirm.git` | `GitForcePush`、`GitResetHard`、`GitClean`、`GitDiscard`、`GitStashDrop` |
| `permission.confirm.delete` | `DeleteWorkingDir`、`DeleteUnresolved` |
| `permission.confirm.power` | `Power` |
| `permission.confirm.pipe_to_shell` | `PipeToShell` |
| `permission.confirm.container` | `ContainerShare` |

问人的十种正好被这五类覆盖。直接拒的七种（`DeleteRoot`、`DeleteHome`、`DeleteSystem`、`DeleteDataRoot`、`DiskFormat`、`DiskWrite`、`ContainerHost`）不归类。

**怎么判**。

1. 策略层（`miyu-policy` 的 `hazard`）只说哪一种归哪一类（`Group`、`Kind::group`），并把一条命令里认出的每一种问人的全交回（`Assessed::Asks`，照先后、不重复）；有拒的只交回拒的那一种（`Assessed::Refused`）。以前只交回一种，判不了「命中几类的有一类开着就照旧问」。
2. `shell` 报给权限策略的高危改成两种样子（`miyu_tool::Hazard`）：`Refused` 带拒的那一句；`Asks` 带每一种的写法和归的类（`HazardAsk`）。
3. 权限策略（`guard.rs`、`guard/asking.rs`）去掉归关掉的那几类的：还有剩的照剩下的最先认出的那一种问；一种都不剩的照没认出高危接着判，完全放开就是放行。
4. 关掉的那几类照回合开始时的配置（`actor/model.rs` 的 `turn_start` 换进 `Guard::set_waived`，同私密目录），判的时候不读配置。
5. 外部身份的会话（`confined`）不看开关，照旧都问。只读不看高危，不受影响。报的路径已经拒了的照拒。
6. 套得太深、看不透的命令（命令替换、`eval` 套了 8 层以上）单列一种 `Kind::TooDeep`，不归哪一类，五个开关全关也照旧问；对外的写法照旧是 `delete_unresolved`（原来就记在那一种名下），没有新增对外的种类，头的说明不用改。项目主人 2026-11-11 定：它说的是「看不懂这条命令」，不是删除。

**设置页**。组 `protected`「受保护的命令」，在「隐私与安全」那一页最上面（核心登记时排在 `PermissionSettings` 前面，页照旧排在「用户」下面）。组带说明：`config.schema` 的组多一格可选的 `description`，资源里写在 `config.group_descriptions`，哪一组都能写，没写的组不带这一格、输出和原来逐字节一样。五个开关的名字、说明和组的说明中文、英文、日文都有，见 `config.md`「给人看的字」。

### 不做什么

- 不写旧版本兼容（项目主人 2026-11-11：没发版）。
- 不给装包、开扩展那一类（访问类别 `install`，完全放开每次问）加开关：它不走高危这条路，在 `guard.rs` 的 `verdict` 末尾另判。
- 不改给她看的字：问人时的说明、被拒时回给她的几句都不动，`resources` 里只动 `core/human/*.json`。
- 头怎么画组的说明归 UI 的会话。
- 施工图不画。

### 规则里保守归类的几处

照「看不懂的不归开关管」逐条看过 `hazard/rules.rs`、`paths.rs`、`container.rs`：

| 在哪 | 什么情况 | 现在归 | 这一步怎么办 |
|---|---|---|---|
| `rules.rs` 的 `look` | 命令替换、`sh -c`、`eval` 套了 8 层以上，不再往里读 | 原来 `DeleteUnresolved` | 改成 `TooDeep`，不归类，永远问 |
| `paths.rs` 的 `target` | `rm -r` 的目标换不成绝对路径：带变量、命令替换，`~` 而家目录不知道，`cd` 到说不清的地方（`cd $DIR`、`cd -`、`popd`）以后删相对路径 | `DeleteUnresolved` | 没动：确实在递归删除，只是目标运行时才知道，正是这一种的本义，归「递归删除」 |
| `rules.rs` 的 `one` | `… \| xargs rm -r`：目标从管道里来 | `DeleteUnresolved` | 没动，同上 |
| `rules.rs` 的 `one` | `systemctl suspend`、`hibernate`、`kexec` | `Power` | 没动：不是「认不出」，是关机重启这一类收得宽，随 `permission.confirm.power` |

容器那一份（`container.rs`）认不出的选项不报，没有保守归类的。

### 验收

- 策略层：问人的十种正好分进五类，直接拒的和看不透的不归类；一条命令里认出几种的全交回。
- 权限策略（真的 `shell`，直接调 `Guard::judge`）：没关的每一种都问；关一类，这一类的放行、别的类照旧问；直接拒的七种五类全关也照拒；命中两类的只关一类照旧问；九层命令替换里的删除、十层 `eval` 五类全关也照旧问；外部身份的会话五类全关和没关时判的一样、没有一条放行；只读照旧。
- 会话层：`permission.confirm.git` 关着的那一轮 `git reset --hard` 不问就跑，改回开着下一轮又问。
- 端点：`config.schema` 里五个开关和那一组的名字、说明照连接的语言，组带 `description`，没写说明的组不带；`config.set` 写得进，`config.check` 不报。核心登记的清单里这一组在「隐私与安全」那一页最上面。
- `cargo test`（`miyu-policy`、`miyu-session`、`miyu-endpoint`、`miyu-core`、`miyu-config`、`miyu-tool`、`miyu-basesystem`、`miyu-store`、`miyu`）、两个目标的 clippy、`cargo fmt --check`、`cargo doc`、`cargo xtask prompts`（没有差异）全过。

人工走一遍：设置页「隐私与安全」最上面看得到「受保护的命令」和五个开关；关掉「破坏性 git 操作」，新开一轮让她在一个临时仓库里跑 `git reset --hard`，不弹确认；让她跑 `sudo reboot`，照旧弹；让她跑 `rm -rf ~`，照旧直接拒；把开关打开，下一轮 `git reset --hard` 又弹。

### 风险

- 关掉一类以后，她幻觉出的这一类命令直接跑，没有人看一眼：这是开关的本意，出厂都开。
- 「递归删除」关掉以后，目标运行时才知道的 `rm -rf "$X"` 也不问了（`X` 是空的就是删当前目录下的全部）；删根、家目录、系统目录、数据根照旧直接拒，认得出目标的话。
- 头没跟上以前，组的说明不显示，「哪几类永远拒绝」那句话人看不到；开关本身照常能用。

### 清单外补的

- `crates/miyu-basesystem/src/shell/hazard.rs`：`shell` 认高危的那一段从 `shell.rs` 挪出来（那边到了行数上限），连同拒的那一句怎么拼。
- `crates/miyu/tests/config.rs`：`miyu config get` 列全部最终值的那一条多五行。
- `crates/miyu-core/src/settings.rs`：登记的模块数 40 → 41。
- `crates/miyu-store/tests/human_languages.rs`：三种语言写了说明的组要一样、说明不空。
- `docs/designs/samples/config/` 四份样本照 `MIYU_SAMPLE_WRITE=1` 重新生成。
