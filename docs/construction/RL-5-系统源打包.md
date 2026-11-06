## RL-5 系统源打包：AUR、Homebrew

状态：已合（715de271e）；托盘见末尾「RL-5补」（2026-11-05，sub1）。设计 `59-发版.md` 第三节第 3 条、第六节第 5 条；`60-系统源分包.md` 第二节、第五节第 1、2 条；
SD-1（系统加装层）、SD-2（分包的布局）、RL-4（`miyu upgrade` 认 `install.toml`）。

### 目的

AUR、Homebrew 上的 Miyu 照设计 60 分包：`miyu-core`、`miyu-tui`、`miyu-webui` 各一个包，`miyu-agent` 是合集，`miyu-agent-git` 从源码编全套。
真相源放在核心仓库的 `packaging/`，发版那天照这一次发布的 `SHA256SUMS` 填好版本、哈希，再推到 AUR、tap。

### 定了的（core1 11-05）

1. **做哪几份**：AUR 的 `miyu-core`、`miyu-tui`、`miyu-webui`、`miyu-agent`、`miyu-agent-git`；Homebrew 的四个 formula。托盘 `miyu-applet` 的
   PKGBUILD、cask 搬不搬、怎么和托盘仓库的流水线分工，core1 去问托盘那条线，另开一步。
2. **模板加 `cargo xtask packaging <标签> --sums <SHA256SUMS>`** 填版本、哈希。
3. **`miyu-core` 只写 `conflicts=('miyu')`、`provides=('miyu')`，不写 `replaces`**：旧的 `miyu` 是 0.7.x 那一代、另一套程序，装新的要人明白
   自己在换掉它，让 pacman 问一次。
4. **paru、yay 在 AUR 上那一问**：包得真在 AUR 上才测得到（对外发布），写进「发版那天」，到时照着测。
5. **实测**：rc.5 出来以后用它的分包真装、真卸；在那之前做静态检查。
6. **tap 仓库**：新的是 `miyu-agent/homebrew-miyu`，主人在发版前建。旧版 Miyu 的是 `SHORiN-KiWATA/homebrew-miyu`，名字一样、组织不同，
   别弄混。

### AUR 上的名字（2026-11-05 用 aur.archlinux.org 的 RPC 匿名查）

`miyu-core`、`miyu-tui`、`miyu-webui`、`miyu-agent`、`miyu-agent-git` 都没人占。以 `miyu` 开头的只有主人自己维护的 `miyu`（0.7.4-1）、
`miyu-git`（0.7.4.r1542）、`miyu-voice`。

### 做什么

- **`packaging/aur/`**：
  - `miyu-core/PKGBUILD.in`：下载 `miyu-core-<版本>-linux-{x86_64,aarch64}.tar.gz`。
    - 整层放进 `/usr/lib/miyu/`（程序照真实位置的旁边找 `resources/`），`/usr/bin/miyu` 链过去；
    - 写 `install.toml`（`channel = "aur"`），许可证进 `/usr/share/licenses/`；
    - `.miyu` 文件类型进 `/usr/share/mime/packages/`，pacman 的钩子会更新类型库；
    - `depends=('glibc' 'gcc-libs')`：三个程序只要 `libc`、`libm`、`libgcc_s`。`optdepends` 是 bubblewrap。
  - `miyu-tui/`、`miyu-webui/PKGBUILD.in`：
    - 版本照它们自己的（压缩包名里写的），下载地址照核心的发版标签；
    - 包文件夹原样放进 `/usr/share/miyu/packages/<编号>/`，核心照 `XDG_DATA_DIRS` 认它是系统加装那一层；
    - `depends` 有 `miyu-core`。
  - `miyu-agent/PKGBUILD.in`：`arch=('any')`，只依赖那三个。托盘进系统源以后加进去。
  - `miyu-agent-git/PKGBUILD`：不是模板。
    - `cargo xtask dist` 照 `heads.toml` 编全套（要 rustup、git、python3、网络），解开分包的三份，照上面三个包的位置装；
    - `provides`、`conflicts` 是 `miyu`、`miyu-core`、`miyu-tui`、`miyu-webui`、`miyu-agent`；
    - `pkgver` 是 `<工作区版本>.r<提交数>.g<短号>`。
- **`packaging/homebrew/`**：`miyu-core.rb.in` 等四个。
  - 核心整层进 `libexec/`，`bin/miyu` 链过去，写 `install.toml`（`channel = "brew"`）。
  - 终端、网页进 `share/miyu/packages/<编号>/`。链进前缀以后，核心照 `miyu` 所在目录的上一级认它（设计 60 第三节）。
  - 合集 `miyu-agent`：Homebrew 的 formula 一定要有下载，拿这一版的 `SHA256SUMS` 当下载（哈希照本地那份算），装进文档目录。
  - 都写 `depends_on arch: :arm64`、`:macos`，各有 `test do`。
- **`xtask/src/packaging.rs`**：`cargo xtask packaging <标签> --sums <SHA256SUMS> [--out <目录>]`。
  - 占位：
    - `@VERSION@` 核心版本；
    - `@PKG_VERSION@` 这个包自己的版本；
    - `@PKGVER@` 同上、去掉 `-`：pacman 的 pkgver 不许有 `-`，`0.0.0-rc.5` 写成 `0.0.0rc.5`，照样排在 `0.0.0` 前面（施工 VR-1 起照 AUR 的惯例写成紧跟的 `rc<号>`：`2026.11.0-rc.1` 写成 `2026.11.0rc1`）；
    - `@SHA256_<系统>@` 这个包那一种系统的压缩包的哈希；
    - `@SHA256_SUMS@` 这份 `SHA256SUMS` 自己的哈希。
  - 出错的：缺哪一份、终端网页三种系统写的版本不一样、哈希写错、填完还有占位。先全填好再写，哪一份不成都不写。
  - 不写 `--out` 的放 cargo 的 target 目录下的 `packaging/<标签>/`。
- **实测**：
  - `packaging/test-aur.sh`：在 Arch 容器里跑，root。
    - `static`：每份语法对、`makepkg --printsrcinfo` 出得来、namcap 只报已知的那一种。
    - `install`：`makepkg` 真下载、核哈希，装四个包，查下面这几样，最后 `pacman -R` 卸干净：
      - `miyu --version` 报这一版，`install.toml` 写 `aur`；
      - `miyu upgrade` 说用包管理器升级；
      - 隔离数据根里 `miyu pkg list`：终端、网页是 `distro` 层、程序找得到。
  - `packaging/test-brew.sh`：在 macOS 上跑。
    - `static`：`brew style`。
    - `install`：建本地 tap `local/miyu-test`，`brew install` 合集，查同样几样，`brew test` 四个，`brew uninstall`、`untap`。
  - `.github/workflows/packaging-test.yml`：
    - 改了模板、实测脚本、填的程序时自动跑，照造出来的 `SHA256SUMS` 只做静态检查；
    - 手动跑写了标签的，照那一次发布真装。
- **守着的**（`xtask/src/packaging/tests.rs`）：
  - 照造的 `SHA256SUMS` 填仓库里真的模板，填得全，版本、哈希放对地方，合集照 `SHA256SUMS` 自己的哈希，`-git` 照原样；
  - 缺一份、版本对不上、核心那份不是这一版、哈希写错、一行读不懂、标签没有 `v`、没写 `--sums` 都报错，缺一份时什么都不写；
  - 剩下没填的占位报出来，邮箱里的 `@` 不算；
  - 没有一份写 `replaces`，`miyu-core` 写 `conflicts`、`provides` 和 `install.toml`；
  - `miyu-core` 写的文件类型同 `packaging/mime/miyu.xml`。
- **文档**：`packaging/README.md` 加系统源一段，设计 59、60 指过来。

### 怎么验

- xtask 的 `packaging` 测试 6 条过；clippy 两种目标都没有警告。
- 本机照造的 `SHA256SUMS` 填，Arch 容器里 `test-aur.sh static` 五份都过。
- CI 的 `packaging-test` 静态那一趟见交付。
- 真装要 rc.5：草稿或预发布出来以后手动跑 `packaging-test`，写 `tag: v0.0.0-rc.5`，Arch、macOS 两台都要过。结果补进这一节。
  - 要是没出来：只靠静态检查交，真装记成待办。

### 变异

手写 13 个，都抓住：
- pkgver 不去 `-`；
- 核心的版本也照文件名找；
- 三种系统版本不一样也认；
- 合集的哈希不照 `SHA256SUMS` 自己算；
- 剩下的占位不报；
- 邮箱也当占位；
- 哈希不查写法；
- 不去二进制模式的 `*`；
- 两种 Linux 的哈希对调；
- `-git` 也当模板填；
- `miyu-core` 不写 `conflicts`；
- `install.toml` 写成 `script`；
- 文件类型的权重改了。

「`install.toml` 写成 `script`」第一次没抓住：测试只查模板里有没有 `channel = "aur"`，模板开头的注释里也有这几个字。改成只查写 `install.toml` 的那一行以后抓住了。
「剩下的占位不报」第一次的写法编不过，换了写法补跑，抓住了。

### 发版那天

在设计 59 第五节「发了以后」那一步做，Release 已经由主人点了发布：

1. **填**：
   ```sh
   gh release download v<版本> -R miyu-agent/miyu -p SHA256SUMS -p SHA256SUMS.sig -D /tmp/rel
   target/debug/miyu-index verify-sums /tmp/rel/SHA256SUMS --public "<发版公钥>"   # 核过签名再用
   cargo xtask packaging v<版本> --sums /tmp/rel/SHA256SUMS --out /tmp/rel/out
   ```
2. **先真装一遍**：手动跑 `packaging-test`，写这个标签，Arch、macOS 都过了再发。
3. **AUR**：每个包一个 AUR 仓库（`ssh://aur@aur.archlinux.org/<包>.git`）。
   - 把填好的 `PKGBUILD` 放进去，在 Arch 上 `makepkg --printsrcinfo > .SRCINFO`，一起提交、推送。第一次推送就是在 AUR 上建这个包。
   - 推的顺序：`miyu-core`、`miyu-tui`、`miyu-webui`，再 `miyu-agent`、`miyu-agent-git`。
   - 这是对外发布，由主人推，或者主人当面同意以后推。
4. **paru、yay 那一问**（设计 59 第三节第 3 条）：在干净的 Arch 容器里各装一次 paru、yay。照下面三步做，照实写进发行说明：问了的写怎么答，
   不问的写手动换的命令（`paru -S miyu-agent`，问冲突时答 y 换掉 `miyu`）。
   - 先装旧的 `miyu`（AUR 上 0.7.4）；
   - 再 `paru -Syu`、`yay -Syu`，看会不会提到 `miyu-agent`：`miyu-core` 不写 `replaces`，预计不会；
   - 再 `paru -S miyu-agent`、`yay -S miyu-agent`，看冲突那一问长什么样，答 y 以后 `/usr/bin/miyu` 是不是新的、旧的数据在不在。
5. **tap**：主人先建 `miyu-agent/homebrew-miyu`（不是旧版的 `SHORiN-KiWATA/homebrew-miyu`）。把 `homebrew/Formula/` 下四个推进它的 `Formula/`。
   装法写进发行说明：`brew install miyu-agent/miyu/miyu-agent`。
6. 照设计 59 第五节，主人装机脚本的分支合进去以后，再从 AUR、旧 tap 删旧的 `miyu`。

### 坑

- **namcap 对按架构写的下载报「x86_64 应该改成 $CARCH」**：`source_x86_64` 本来就要写死架构，是已知的误报。`test-aur.sh` 只滤掉这一句，
  别的照报。
- **pkgver 不许有 `-`**：版本号去掉 `-`，pacman 的比较照样把 `rc` 排在正式版前面。下载地址照原来带 `-` 的版本。
- **Homebrew 的 formula 一定要有下载**：合集没有文件，拿 `SHA256SUMS` 当下载。
- **aarch64 的 Linux 没有实测**：CI 的 Arch 容器只有 x86_64，`source_aarch64` 只靠和 x86_64 同一个写法、`.SRCINFO` 出得来。
- **`brew style` 管说明的写法**：CI 第一趟红在这里。
  - 说明不超过 80 个字符；
  - 写 `command-line`，不写 `command line`；
  - 取别的 formula 的程序用 `formula_opt_bin`；
  - 一行不超过 118 个字符。
  - 只给 macOS 的 cask 要写 `depends_on :macos`（RL-5补 第一趟红在这里）；几行 `depends_on` 照 `arch`、`formula`、`macos` 的先后排（第二趟）。
- **`miyu-agent-git` 没有在 CI 上真编**：编一整套要几十分钟、要网络取头，只做静态检查。发版前在 Arch 上手动 `makepkg` 一次。

### RL-5补：托盘 miyu-applet 的 PKGBUILD 和 cask

托盘仓库做好了只装托盘的产物（托盘 main fa92da492，AP-11），版本照托盘自己的 `Cargo.toml`（现在 0.1.0），地址照
`https://github.com/miyu-agent/miyu-applet/releases/download/v<版本>/<产物名>`，附件带 `SHA256SUMS`、没签名（core1 11-05 转告）。

- **`packaging/aur/miyu-applet/PKGBUILD.in`**：下载 `miyu-applet-<版本>-linux-{x86_64,aarch64}.tar.gz`。
  - 里面一层 `miyu-applet-<版本>-linux-<架构>/` 照 `/usr` 摆好，整层拷进 `/usr`。
  - `depends` 有 `miyu-core`、`miyu-webui`，加上直接链接的 gtk3、cairo、gdk-pixbuf2、glib2，以及托盘仓库蓝图写的 ayatana 托盘库（运行时才加载）。
  - 托盘的完整包（deb、rpm 带核心）和 `miyu-core` 冲突，AUR 上不发那种，不写 `conflicts`。
- **`packaging/homebrew/miyu-applet.cask.rb.in`**：下载 `miyu-applet-<版本>-macos-aarch64.zip`，`app "Miyu.app"`，`depends_on formula:` `miyu-core`、`miyu-webui`。
  填出来放 `homebrew/Casks/`。
- **合集**：AUR 的 `miyu-agent` 依赖加上 `miyu-applet`。Homebrew 的 formula 依赖不了 cask，`miyu-agent` 在 caveats 里说
  `brew install --cask miyu-agent/miyu/miyu-applet`。
- **`cargo xtask packaging`** 多 `--applet <托盘的标签> --applet-sums <托盘的 SHA256SUMS>`：
  - 两个要一起写；托盘那两份照托盘的发布填，版本照托盘的标签，macOS 的产物是 `.zip`；
  - 没写的，这两份不填，说一句（核心发了、托盘还没发的时候也能填核心的）。
- **实测**：
  - `test-aur.sh` 的 install 填了托盘的另装 `miyu-applet`、`miyu-agent`，查程序、桌面文件、图标，卸干净；
  - `test-brew.sh` 的 static 也查 cask，install 填了托盘的 `brew install --cask`，查 `/Applications/Miyu.app`，卸掉；
  - `packaging-test` 手动跑多一个 `applet_tag`，静态那一趟照造的托盘 `SHA256SUMS` 填。

**怎么验**：
- xtask 的 packaging 8 条过，新加两条：
  - 照托盘的发布填对版本、哈希（macOS 的是 zip 那一行），合集带托盘；
  - 缺一份什么都不写，`--applet` 没配 `--applet-sums` 拒。
- Arch 容器 static 六份都过。
- 拿托盘流水线 42025906874 的 x86_64 真产物填哈希、在 Arch 容器里 `makepkg` 真打一次托盘的包：
  - 文件是 `/usr/bin/miyu-applet`、桌面文件、三种大小的图标、两份许可证，版本 0.1.0；
  - 核心、网页的包还没发，打包时跳过依赖检查。
- 真装、cask 要托盘正式发版以后，手动跑 `packaging-test` 写 `applet_tag`。

**变异**：手写 7 个，都抓住：
- macOS 的不当 zip；
- 托盘的版本照文件名找；
- 托盘照核心的发布填；
- `--applet` 不配 `--applet-sums` 也收；
- 没给托盘也报「填了」；
- 合集不带托盘；
- cask 的版本写错。

「没给托盘也报填了」第一次没抓住：这一格只管最后那一句提示，补了一条测试查它，抓住了。

### RL-5补2：旧版的过渡包，`miyu-core` 只挡旧版

项目主人 2026-11-06 定旧版过渡的做法，core1 转来（2026-11-06，sub1，和 VR-1 同一条分支）。原来是 `miyu-core` 无条件
`conflicts=('miyu')`、发了以后从 AUR 删旧的 `miyu`（设计 59 第三节第 3 条、第五节）：旧用户 `-Syu` 什么都不会发生，要自己
`paru -S miyu-agent`、答一次冲突。改成 AUR 上的 `miyu` 升级成一个过渡包，照常 `-Syu` 就换成新版的一整套。

- **`packaging/aur/miyu/PKGBUILD.in`**：`pkgname=miyu`，版本跟核心走（`@PKGVER@`，`2026.11.0` 起，比 0.7.x 新），`arch=('any')`，
  不带文件、没有 `package()`（同合集），`depends=('miyu-agent')`，`install=miyu.install`。
- **`packaging/aur/miyu/miyu.install`**：不是模板，照原样拷。`post_install`、`post_upgrade` 印「运行 miyu migrate 把旧版数据搬过来」
  中英两句。
- **`miyu-core`**：`conflicts=('miyu<2026')`，只挡 0.7.x 那一代；`provides=('miyu')` 去掉（core1 定：真有一个叫 `miyu` 的包了，
  不冒充它）。**`miyu-agent-git`** 同样：`conflicts` 里 `miyu` 换成 `miyu<2026`，`provides` 去掉 `miyu`，留着 `miyu-agent`（过渡包
  依赖它，装 `-git` 的也满足得了）。
- **`cargo xtask packaging`** 一起填过渡包（`FILES` 多一份，版本照核心的发布），`miyu.install` 进 `COPIED`。
- **`test-aur.sh`** 的 install 多一段（填了托盘的才做：合集依赖托盘）：
  - 照 AUR 上 0.7.x 的样子造一个假的旧 `miyu` 0.7.4（自己有 `/usr/bin/miyu`），装上；
  - 过渡包和那五份放进本机的 `file://` 软件源（`repo-add`），照用户平常那样 `pacman -Syu --noconfirm`；
  - 查：升级成了（`--noconfirm` 遇到要问的冲突答「不」，成了就是没有冲突）、没有「are in conflict」「conflicting files」
    「unresolvable」、印了 `miyu migrate`、`miyu` 是过渡包这一版、五个包都在、`/usr/bin/miyu` 报新版、归 `miyu-core`；
  - 卸干净，`pacman.conf` 换回去。
- **Homebrew 不在这一步**：见下面「发版那天」第 5 条。

**怎么验**：
- xtask 的 packaging 10 条过：过渡包照核心的版本填、只依赖合集、不带下载，`miyu.install` 照原样拷；`miyu-core` 只写
  `conflicts=('miyu<2026')`、不写 `provides`；`-git` 的 `conflicts` 有 `miyu<2026`、没有 `miyu`，`provides` 没有 `miyu`、有 `miyu-agent`；
  `miyu.install` 两个钩子都在、都说 `miyu migrate`。
- 本机 Arch 容器（`archlinux:latest`，不加特权）照 `test-aur.sh` 新那一段的写法，用真的过渡包模板加假的 `miyu-core`、`miyu-agent`
  跑了一遍：旧的 0.7.4 升成 `2026.11.0rc1-1`，带进 `miyu-core`、`miyu-agent`，`/usr/bin/miyu` 从旧包挪到 `miyu-core`，没有冲突，印了两句
  `miyu migrate`。对照：把假的 `miyu-core` 换回 `conflicts=('miyu')`，同一个升级报「miyu-core and miyu are in conflict」失败。
- 模拟里踩到两处，照改进了 `test-aur.sh`：
  - 过渡包编的时候合集刚卸掉，`makepkg` 报缺依赖：编它用 `-d`，依赖由 `pacman -Syu` 从软件源带；
  - pacman 平常就印「looking for conflicting packages」「checking for file conflicts」：查冲突只认真的那几句。
- 整段 `test-aur.sh install` 要真的发布（rc.6 以后，托盘也发了），到时手动跑 `packaging-test` 写 `tag`、`applet_tag`。
- **rc.6 第一次真装红在测具两处**（packaging-test run 42218108759，RL-5补3）：
  - `test-aur.sh` 查系统加装那一层的那段 Python：f 串花括号里写了 `item[\"layer\"]`，外面是 sh 的单引号，Python 看到反斜杠在字符串外面，
    语法错。install 那一段以前没真跑过，一直没露出来。改成先取到变量里再放进 f 串，断言一样。真装、版本、`install.toml`、`upgrade`
    那几步在它前面，都过了。
  - `test-brew.sh`：新的 Homebrew 不从没信任过的 tap 载 formula，`brew install` 照命令行信任了 `miyu-agent`，它依赖的 `miyu-core` 不算。
    建完本地 tap 以后整个 `brew trust` 上（旧的 Homebrew 没有这个命令，不调）。
- **namcap 不许说明里带包名**（`packaging-test` 第一趟红在这里）：它照子串比，过渡包叫 `miyu`，说明里的 `Miyu`、`miyu-agent`、
  `miyu migrate` 都算，改成一个 `miyu` 都不写（`the new agent packages`、`the migrate command`），怎么搬旧数据由装上时那两句说。
  本机照 `packaging-test` 的造法填好、在 Arch 容器里跑 `test-aur.sh static`，七份都过。
- 手写变异 7 个全抓住：`miyu-core` 写回 `conflicts=('miyu')`、加回 `provides=('miyu')`；`-git` 加回 `provides` 的 `miyu`、`conflicts` 写回 `miyu`；
  `miyu.install` 拷错地方；过渡包照终端的版本填；`post_upgrade` 写错名字。

**发版那天**跟着改（替换上面「发版那天」的第 3、4、6 条里说旧 `miyu` 的部分）：
- AUR 的 `miyu` 不删：主人维护着它，填好的 `aur/miyu/PKGBUILD`、`miyu.install` 推进去（在 `miyu-agent` 那几个都推上去以后，
  不然 `-Syu` 找不到依赖），旧用户 `-Syu` 就换过来了；`miyu-git` 照旧删。
- paru、yay 那一问换成：装旧的 `miyu` 0.7.4，`paru -Syu`、`yay -Syu`，看是不是不问冲突就换成一整套、印了 `miyu migrate`。
- Homebrew：旧 tap（`SHORiN-KiWATA/homebrew-miyu`）的 `miyu` formula 改成提示换新 tap（`brew install miyu-agent/miyu/miyu-agent`），
  等新 tap 建好再做，不在这一步。
