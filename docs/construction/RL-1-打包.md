## RL-1 命令行版的压缩包：`cargo xtask dist`

状态：施工完，待 core1 审（2026-11-05，sub1）。设计 `59-发版.md` 第三节第 1 条、第六节第 1 条；`12-进程形态与分发.md` 第六、八节。

### 目的

发版要的压缩包现在没有能打它的东西（「测试包 1」是手工打的）。做成以后，`cargo xtask dist` 在一台机器上打出这一种系统的
`miyu-<版本>-<系统>.tar.gz`（Windows 是 `.zip`）：照 `heads.toml` 取两个头、编、搭目录、合许可证清单、打包。RL-2 的
`release.yml` 在四种机器上各跑一次它，再查包、签名、建草稿 Release。

### 做什么（`xtask/src/dist.rs`；取和编在 `dist/fetch.rs`，搭目录在 `dist/layout.rs`，打包在 `dist/archive.rs`）

1. **版本和系统**：版本照 `Cargo.toml` 的 `[workspace.package] version`（发版那天改，设计 59 第五节第 2 条；现在是 `0.0.0`）。
   系统照本机：`linux-x86_64`、`linux-aarch64`、`macos-aarch64`、`windows-x86_64`；写了 `--target` 的要和本机一样，不交叉编（四种
   各在自己的机器上打，RL-2 的矩阵）。`--out <目录>` 换放的地方，默认 cargo 的 target 目录下的 `dist/`。
2. **取头**：`heads::pins` 读 `heads.toml`（写法有问题的照 `heads::check` 报），克隆进 `<dist>/heads/<编号>/`，有了的 `fetch`；检出钉的
   那一份、`git clean` 掉多余的（`target/` 留着，下次编得快），再核对 `HEAD` 就是 `<rev>^{commit}`。
3. **编**，都是 release、`--locked`：
   - 核心 `-p miyu -p miyu-sandbox -p miyu-extract`；
   - 终端在它的检出里 `cargo build`，网页 `-p miyu-web`，同它们各自的发版模板；
   - 头照 PATH 上的 `cargo` 编（rustup 认它们自己的 `rust-toolchain.toml`），外面这个 cargo 给的 `RUSTUP_TOOLCHAIN`、`CARGO`、
     `CARGO_TARGET_DIR`、`CARGO_PKG_*` 这些去掉，不然会照核心的工具链、编进核心的 target 目录。
4. **搭目录** `<dist>/miyu-<版本>-<系统>/`：
   - 五个程序放一起：`miyu`、`miyu-sandbox`、`miyu-extract`、`miyu-tui`、`miyu-web`（头的程序放在 `miyu` 旁边，`heads.toml`
     开头那段；`miyu_store::packages::locate` 先找包文件夹的 `bin/`、再找 `miyu` 旁边）。Unix 上带执行位。
   - `resources/`：仓库里登记了的资源（`git ls-files resources`，没登记的不带），加两个头的包文件夹：
     - `packages/miyu-tui/`：终端仓库 `miyu-tui/` 里登记的文件，加它的 `LICENSE`、`THIRD-PARTY-LICENSES.txt`（同它的发版模板）；
     - `packages/miyu-webui/`：网页仓库 `tools/pack-web.py --stage` 搭的（清单、`web.json`、`matugen/`、`loading.html`、`pages/`、
       许可证），去掉 `bin/`。用它自己的脚本搭：页面带哪几样由网页仓库定，核心这边不另抄一份。
     - 核心的 `resources/packages/` 里已经有这两个编号的，报错，不盖。
   - `LICENSE`（AGPL-3.0 全文）。
   - `THIRD-PARTY-LICENSES.txt`：合三份，核心的（仓库里 `cargo xtask notices` 生成、门禁查过最新）、终端的、网页的（各自仓库里同法
     生成的），开头写清每段是谁的、取的哪一份（`packaging/README.md`「带了头的，头的那一份也放进去」，设计 59「核心和两个头的」）。
   - `README.txt`：中英两段，怎么用（`miyu`、`miyu web`、`miyu --help`）、程序和 `resources/` 要放在一起、数据在 `.miyu`、
     `miyu service enable`、Linux 上的 bubblewrap、怎么卸。
5. **打包**：`miyu-<版本>-<系统>.tar.gz`（gzip 最高压缩）或 `.zip`（deflate），里面一层同名目录；照名字排，属主 0、时刻 0，能跑的
   `0755`、别的 `0644`；同一个目录打两次一字不差。搭出来的目录里有链接、别的种类的文件报错。印路径、SHA-256、大小。
6. **`heads.toml`**：钉终端 `46f2699c6ea326489e819292da64426ea90c858f`（v0.1.0，标签还没打，打了换成 `v0.1.0`）、网页 `v0.1.0`
   （`85fcd43c`）。

### 怎么验

- 单元（`dist/tests.rs`、`heads/tests.rs`）：
  - 命令行：不写照本机，写了别的系统拒，少值、不认识的参数拒；
  - 版本读法；压缩包的名字；
  - 合许可证的先后和每段的说明；README 两种语言、照系统换说法；
  - 搭目录：程序和执行位、只带登记了的资源、两个头的包文件夹、网页不带 `bin/`、撞名拒、不盖上一次的；
  - tar.gz：一层目录、照名字排、权限、属主和时刻是 0、打两次一字不差；zip 同样的布局；链接拒；
  - SHA-256；头的构建去掉外面 cargo 的环境变量；`heads::pins` 的先后、和 `check` 一样拒；
  - 取头：拿本地 git 仓库当上游，克隆、检出钉的提交；上游再提交以后再取（要先拉才检得出）、清掉多余的、`target/` 留着；标签也认，
    不在的报错；`tracked` 只列登记了的。
- 手写变异 14 个都抓住：不核对本机、权限写死、不排序、网页带 `bin/`、`CARGO_PKG_*` 不去、撞名不拒、不写属主、不清、清掉
  `target/`、程序不带执行位、不拉、盖上一次的、合许可证不收空行、README 的 Linux 说法反了。「不拉」第一轮没抓住（测试里第二个
  提交在克隆之前就有了），改成克隆以后上游再提交。
- 本机真打一次（下面「实测」）。

### 实测

2026-11-05 本机（linux-x86_64）`cargo xtask dist`，target 目录是共用的那一份：

- 取两个头（克隆）；编核心 4 分 04 秒、终端 4 分 07 秒、网页 1 分 09 秒（都是第一次 release）；搭网页的包文件夹、搭目录、打包。
- `miyu-0.0.0-linux-x86_64.tar.gz`，30246222 字节。`tar -tvzf`：一层 `miyu-0.0.0-linux-x86_64/`，属主 0/0、时刻 1970-01-01，五个程序
  `rwxr-xr-x`、别的 `rw-r--r--`。
- 解开：`resources/` 下 core、models、packages、presets、software；`packages/` 下出厂的 28 个加 `miyu-tui`（清单、LICENSE、
  第三方清单）、`miyu-webui`（清单、`web.json`、`matugen/`、`loading.html`、`pages/` 七样、许可证，没有 `bin/`）。
  `THIRD-PARTY-LICENSES.txt` 1.9 MB，三段：核心第 9 行起、终端第 12788 行起、网页第 25430 行起，开头写着终端 `46f2699c…`、
  网页 `v0.1.0`。
- 不设 `MIYU_RESOURCES`、隔离的 `MIYU_HOME`：`miyu --version` → `miyu 0.0.0`；`miyu check` → 「没有问题」（另报语义模型没装，照常）；
  `miyu pkg list` 有 `miyu-tui`、`miyu-webui`；`miyu pkg info miyu-webui` 版本 0.1.0；`miyu tui --help` 转交到旁边的 `miyu-tui`
  印出用法。没开界面、没起网页。

### 坑

- **`tar::Header::new_gnu` 不填属主**：属主、属组那两格是空的，`tar` 读出来报「不是数字」。要显式 `set_uid(0)`、`set_gid(0)`。
- **网页的清单 `targets` 没写 `linux-aarch64`**：`pack-web.py --stage` 照它核对，在 `linux-aarch64` 上会拒，四种里那一种打不出来。
  要网页那边补上（终端写全了四种）。
- **头的构建要去掉外面 cargo 的环境变量**：`cargo xtask` 跑起来时 rustup 设了 `RUSTUP_TOOLCHAIN`，带进头的构建就不认头自己的
  工具链；`CARGO_TARGET_DIR` 带进去，头会编进核心的 target 目录。

- **一个文件写下来 766 行**：门禁的行数上限是 500，照取、搭、打拆成三个子模块，`dist.rs` 只留命令行、版本、名字和走一趟的先后。

### 留给后面的

- RL-2：`release.yml` 四种机器各跑一次 `cargo xtask dist`，查包（资源齐不齐、`miyu --version`、临时数据根里 `miyu check`）、
  `SHA256SUMS`、签名、构建出处证明、草稿 Release。
- 版本号从 `0.0.0` 改成 `1.0.0` 是发版那天的事（设计 59 第五节第 2 条），这一步不改。
