## SD-2 系统源分包用的几份压缩包

状态：施工完，待 core1 审（2026-11-05，sub1）。设计 `60-系统源分包.md` 第二节、第六节第 2 条；`59-发版.md` 第三、四节；施工 RL-1、RL-2。

### 目的

系统源分包（设计 60）：AUR、Homebrew 上 `miyu-core`、`miyu-tui`、`miyu-webui` 各是一个包，各装各的文件。包管理器只管把文件放到位，
不该自己拆一体的压缩包（拆法会随版本变，拆错了包之间抢文件）。这一步让 `cargo xtask dist` 另打出这三份，`release.yml` 一起查、一起传，
RL-3、RL-5 的 PKGBUILD、formula 照它们装。一体的压缩包、安装脚本、`miyu upgrade` 照旧。

### 定了的（core1 11-05）

1. **每种系统三份**：核心一份、每个头一份，版本各照各的（核心照工作区版本，头照自己清单的）。
2. **`dist-check` 加一条**：头那一份解开后照系统加装层的找法放好（`MIYU_DISTRO_PACKAGES` 指过去），用旁边核心那一份的 `miyu` 看它认不
   认得出是 `distro` 层、程序找得到。
3. **`release.yml` 一起改**，改完演练一次（rc.4，标签由项目主人推）。
4. **只打 Linux 两种和 macOS**：Windows 没有系统源（不做 scoop），打包时说一行「这台不出分包（没有系统源）」。

### 做什么

- **`xtask/src/dist/split.rs`**（新）：名字、搭目录。
  - `miyu-core-<版本>-<系统>.tar.gz`：一层同名目录，三个程序（`miyu`、`miyu-sandbox`、`miyu-extract`）、`resources/`（只有核心登记的，
    不带两个头的包文件夹：系统加装那一层会有一份，出厂这一层再有就重了）、`LICENSE`、`THIRD-PARTY-LICENSES.txt`（仓库根上那一份，只有核心）。
    AUR 整层放进 `/usr/lib/miyu/`，Homebrew 放进 `libexec/`。
  - `miyu-tui-<版本>-<系统>.tar.gz`、`miyu-webui-<版本>-<系统>.tar.gz`：里面直接是包文件夹（`miyu-tui/`、`miyu-webui/`），清单原样，
    程序在包文件夹的 `bin/`（系统加装那一层照这里找程序，SD-1），带各自的许可证、第三方清单；包管理器原样拷进
    `share/miyu/packages/<编号>/`。终端的包文件夹照一体包里那样搭（清单、许可证），再把程序放进 `bin/`；网页的就是 `pack-web.py --stage`
    搭的那一份（程序本来就在 `bin/`）。
- **`xtask/src/dist.rs`**：一体的打完，Windows 说一行就停；别的照上面搭三份、打包。`dist()` 回打出来的几份，`cargo xtask dist` 一份一行
  报路径和大小。
- **`xtask/src/dist/check.rs`、`check/parts.rs`**（新）：`dist-check` 照文件名分三种。
  - 核心那一份：一层目录名对；三个程序在、能跑，没有两个头的程序；`LICENSE`、第三方清单和仓库的一字不差；`resources/` 照登记的逐项比字节，
    多出来的报，`resources/packages/` 下不该有两个头；再跑包里的 `miyu --version`、隔离数据根里 `miyu check`（同一体的）。
  - 头那一份：一层包文件夹名对；清单的版本和压缩包名一样；许可证、第三方清单在；`bin/<程序>` 在、能跑；网页的还要 `pages/index.html`。
    再把它当系统加装的放好：解开**同一目录里**核心那一份，数据根放临时的，`MIYU_DISTRO_PACKAGES` 指到解开头的地方，跑
    `miyu pkg list --format json`：这个编号要在、`layer` 是 `distro`、`status` 不是 `program_missing`、没有 `code`；跑完 `miyu core stop`。
    旁边没有核心那一份的直接报。
  - 别的照旧当一体的查，名字不对的那句话把分包的三种名字也列上。
- **`.github/workflows/release.yml`**：查包一步照 `target/dist/` 里这一种系统的全部压缩包逐个查，Windows 要正好 1 个、别的 4 个；
  上传这一种的全部；`SHA256SUMS` 要 13 个（四个一体的，三种系统各三份）；草稿说明加一句哪个是装的、哪几个给系统源。签名、构建出处、
  上传原来就照 `miyu-*` 通配，不用动。
- **`miyu upgrade` 不用动**：它照 `miyu-<版本>-<系统>.tar.gz` 的全名找，`miyu-core-…` 这些不会被拿错。
- **文档**：`packaging/README.md` 加分包那一段，设计 60 第六节指到这份施工单。

### 怎么验

- xtask 单元（`dist/split/tests.rs`、`dist/check/parts/tests.rs`）：哪几种出分包；三种名字、版本照清单读（读不成的话）；核心那一份只有
  三个程序和核心的资源、许可证原样；头的包文件夹清单原样、程序进 `bin/` 带执行位；照名字认头（版本空的、别的系统、核心、`.zip` 都不认）；
  核心那一份六种毛病（少程序、带了头的程序、带了头的包文件夹、第三方清单不是核心那份、`LICENSE` 不一样、资源改过）各报一句；头那一份版本
  对不上、少程序、少许可证、网页少首页；旁边没有核心那一份。`dist::` 下的测试全过。
- 本机真打一次、四份逐个 `dist-check`（见「实测」）。
- 演练：合进去以后 core1 给项目主人写 rc.4 的步骤，CI 上四种机器打 13 份、逐个查、签名、建草稿。

### 变异

手写 15 个，都抓住：Windows 也出、头的程序不进 `bin/`、核心的程序不给执行位、版本空的也认、带了头的程序不报、
第三方清单不比、`LICENSE` 不比、资源不比字节、带了头的包文件夹不报、清单版本不比、`LICENSE` 不查、头的程序不查、网页首页不查、
旁边没有核心那一份不报、少了核心程序不报。

### 实测

本机（`linux-x86_64`）真打一次 `cargo xtask dist --out <临时目录>`，出四份：

| 压缩包 | 大小 |
|---|---|
| `miyu-0.0.0-linux-x86_64.tar.gz`（一体的） | 30.5 MB |
| `miyu-core-0.0.0-linux-x86_64.tar.gz` | 17.1 MB |
| `miyu-tui-0.1.0-linux-x86_64.tar.gz` | 7.8 MB |
| `miyu-webui-0.1.0-linux-x86_64.tar.gz` | 5.5 MB |

四份逐个 `cargo xtask dist-check`，都是「没有问题」。反过来：把终端那份解开、删掉 `bin/miyu-tui` 再打回去，和核心那一份放一起查，
报「少了程序 bin/miyu-tui」「核心找不到 miyu-tui 的程序」两处、退出码 1：系统加装那一步真是核心在认。

### 坑

- **头那一份查的时候要用旁边的核心那一份**：`dist-check` 一次只给一个压缩包，核心那一份照同一目录、同版本同系统的名字找。CI 上四份都在
  `target/dist/`，次序不要紧。
- **核心那一份不带两个头的包文件夹**：出厂、系统加装两层都有同一个编号时系统加装的盖住出厂的（SD-1），不出错，但包管理器卸了终端以后
  出厂那份还在，`miyu tui` 照样能用，和「卸了」对不上；所以核心那一份只放核心自己登记的资源。
