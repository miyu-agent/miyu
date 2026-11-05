## RL-2 发版流程：`release.yml`

状态：施工中（2026-11-05，sub1）。代码和工作流写完、本地验过；预发布标签的演练要 core1 问过主人再推。设计 `59-发版.md` 第三、四、五节，
第六节第 2 条；`12-进程形态与分发.md` 第八节 R14。

### 目的

RL-1 有了 `cargo xtask dist`，这一步把它放进 CI：推 `v<版本>` 标签时四种系统各打一个压缩包、查包，四个都过了再写校验和、
签名、加构建出处证明，建一个**草稿** Release。发不发由主人看过草稿自己点。

### 要什么（主人自己配，不经任何会话）

| 在哪 | 名字 | 是什么 |
|---|---|---|
| 仓库的 Actions secret | `MIYU_RELEASE_KEY` | 发版钥匙的私钥：`miyu-index keygen <文件>` 生成的 PKCS#8 base64（和软件源那把分开，设计 59 第七节第 5、6 条） |
| 仓库的 Actions variable | `MIYU_RELEASE_PUBLIC` | 同一把的公钥（`keygen` 印的 `public` 那一行），签完照它验一遍；以后安装脚本、`miyu upgrade` 也照它（RL-3、RL-4） |

生成：`cargo run -p miyu-source --bin miyu-index -- keygen <文件>`，私钥文件 0600，印出公钥编号和公钥。私钥贴进 secret，公钥贴进
variable，私钥文件自己保管。两样少了哪一样，签名那一步报错停下，不建 Release（不跳过签名，core1 11-05 定）。

### 做什么

1. **`release.yml`**（`.github/workflows/`），推 `v*` 标签时跑：
   - `dist`，四种系统的矩阵：`ubuntu-24.04`（linux-x86_64）、`ubuntu-24.04-arm`（linux-aarch64，同网页仓库）、`macos-latest`
     （macos-aarch64）、`windows-latest`（windows-x86_64）。每台：
     - 核对标签：要是 `v<版本>` 或者 `v<版本>-rc.N`，版本是 `Cargo.toml` 的 `[workspace.package] version`；对不上的停下。
     - `rustup toolchain install`，`cargo xtask dist --target <系统>`（放在默认的 `target/dist/`：相对路径在 Windows 的 bash 里也照常
       通配，`$RUNNER_TEMP` 在那边带反斜杠）。
     - `cargo xtask dist-check <压缩包>` 查包，不过的这一趟不发。
     - 压缩包传成 `dist-<系统>` 这份产物（留 7 天）。
   - `release`，四台都过了才跑（`ubuntu-24.04`）：
     - 取四份产物，正好四个压缩包，`sha256sum` 写 `SHA256SUMS`（`sha256sum -c` 认的写法，照名字排）。
     - 签名：没有 secret 或 variable 的报错停下；`miyu-index sign-sums` 照 `MIYU_INDEX_KEY`（填的是 `MIYU_RELEASE_KEY`）签出
       `SHA256SUMS.sig`，再 `verify-sums --public <MIYU_RELEASE_PUBLIC>` 验一遍。
     - `actions/attest-build-provenance` 给四个压缩包和 `SHA256SUMS` 加构建出处证明（仓库是公开的，`gh attestation verify` 查得到）。
     - 草稿 Release：标题 `Miyu <标签>`，说明里写打包的提交、怎么核对、`SHA256SUMS` 原文；`-rc.N` 的标成预发布。同一个标签已经有
       草稿的（重跑）照传、覆盖；已经发布了的报错，不动。只建草稿，不发布。
2. **`cargo xtask dist-check <压缩包>`**（`xtask/src/dist/check.rs`，设计 59 第四节第 4 条）：
   - 名字要是 `miyu-<工作区版本>-<本机>.tar.gz`（Windows `.zip`），解开（两种都不许写到目录外面）以后只有一层同名目录；
   - 五个程序都在、Unix 上能跑；`LICENSE` 和仓库的一字不差；`THIRD-PARTY-LICENSES.txt` 里有核心那一份；`README.txt` 开头是这一版、
     这一种；
   - `resources/` 照 `git ls-files resources` 逐项比字节，多出来的报（两个头的包文件夹除外）；终端的清单和两份许可证、网页的清单、
     两份许可证、`pages/index.html` 都在，网页没有 `bin/`；
   - 跑包里的 `miyu --version`，要是 `miyu <版本>`；去掉全部 `MIYU_` 环境变量、`MIYU_HOME` 指到临时目录跑 `miyu check`，要过；跑完
     `miyu core stop`。
   - 有一处不对退出码 1，逐条印；解开的目录查完删掉。
3. **签校验和清单**（`miyu-index sign-sums`、`verify-sums`，`crates/miyu-source/src/bin/miyu-index.rs`）：签法、签名文件的写法同索引
   （`miyu_source::sign`，Ed25519 签原始字节，`{"key","signature"}`）。签之前照 `miyu_source::sums::check` 查清单：不是空的、以换行
   结尾、每行「64 位小写十六进制、两个空格、文件名」、文件名不带路径分隔符、不是 `.` `..`、不重名。`verify-sums` 一定要写
   `--public`：编进来的是软件源的钥匙，不算数。私钥照旧从 `MIYU_INDEX_KEY` 读（或 `--key`）。
4. 设计 59 第六节、`packaging/README.md` 指过来。

### 怎么验

- 单元：
  - `sums`：`sha256sum` 写的样子收，空的、没换行、不是两个空格、哈希短一位、大写、带 `/` `\`、`..`、空名字、名字前多空格、
    控制字符、重名都拒，说第几行。
  - `dist-check`：照仓库搭的包过；少程序、`LICENSE` 不同、第三方清单里没有核心的、README 版本不对、少资源、资源字节不同、多出
    资源、少终端清单、少网页首页、网页带 `bin/` 各自报出来；没有执行位的程序报；tar.gz、zip 解开都是一层目录、程序能跑；解不开的
    报错。
- 手写变异 16 个都抓住（查包十个、清单六个）。
- 本机：
  - `miyu-index`：`keygen`；没钥匙的 `sign-sums` 拒；签了用对的公钥验过；不写 `--public` 的退出码 2；别的钥匙、改过一个字节的
    清单、写坏的清单都拒。openssl 照原始公钥（前面接 Ed25519 的 DER 头）`pkeyutl -verify -rawin` 验得过同一份签名：RL-3 的安装
    脚本能用系统的 openssl 验，不用另带程序。
  - `cargo xtask dist-check` 查 RL-1 打的 `miyu-0.0.0-linux-x86_64.tar.gz`：「没有问题」，15 秒。
  - `release.yml` 读得成 YAML；里面的 shell 拿出来本地跑：标签 `v0.0.0`、`v0.0.0-rc.1` 过，`v0.0.0-rc.x`、`v1.0.0`、`v0.0.0-beta`
    拒；没有 secret、没有 variable 的各自报错停下；有了的签、验都过；公钥不对的验不过。
- 演练（下一节）。

### 演练（预发布标签）

照设计 59 第六节第 2 条，用 `v0.0.0-rc.N` 演练（core1 11-05 定）：

- 标签打在 main 上 RL-2 合进去的那个提交上，不打在分支上：分支合了就删，标签不能指向一个以后没有了的提交。
- 打标签是对外的动作：推哪个由 core1 问主人，主人点头以后**主人自己推**。签名钥匙也由主人照上面的生成法自己弄。
- 第一趟 `v0.0.0-rc.1` 可以在主人配钥匙之前推：四台打包、查包走一遍，签名那一步该报「没有密钥 MIYU_RELEASE_KEY」停下、不建
  Release，正好验「不跳过签名」。
- 主人配好 secret 和 variable 以后再推一趟：签名、出处证明、草稿 Release 都该出来。
- 演练完删草稿、删预发布标签，也要主人点头，会话不自己删。

（演练的结果补在这里。）

### 补2：演练标签用标签里的版本（2026-11-05）

rc.2 演练以后拿 `miyu upgrade --prerelease` 实测（RL-4）没换成：rc.2 是在工作区版本 `0.0.0` 上打的标签，`dist` 照工作区版本给压缩包起名
（`miyu-0.0.0-<系统>`）、包里的程序报 `miyu 0.0.0`；`upgrade` 照标签认版本，找 `miyu-0.0.0-rc.2-<系统>`、核对 `miyu 0.0.0-rc.2`，
两头对不上。正式标签就是 `v<版本>`，没有这个问题。core1 照推荐定：

- `dist` 那一台上加一步「演练标签用标签里的版本」：标签是 `v<版本>` 的什么都不动（打一行「正式标签」）；是 `v<版本>-rc.N` 的，只在这台
  机器上把 `[workspace.package] version` 改成标签里的版本（不提交），打一行「演练标签：工作区版本临时改成 …」，再读一遍核对改成了，
  跑一次 `cargo metadata` 把 `Cargo.lock` 里工作区自己那几个 crate 的版本跟着刷新（不刷新的话后面 `--locked` 的编译拒）。
- 之后打包、查包都照新的版本：压缩包叫 `miyu-0.0.0-rc.N-<系统>`，`miyu --version` 报 `miyu 0.0.0-rc.N`。
- shell 里变量后面接中文的一律 `${var}`，「标签和版本对得上」那一步也改成这样。

验：把这一步拿出来，在仓库的一份拷贝上照两种标签跑：`v0.0.0` 照旧、lock 一行不动；`v0.0.0-rc.3` 改成了、lock 改了 37 行（工作区的
crate 都是 `version = "0.0.0-rc.3"`），`cargo metadata --locked` 认；不刷新 lock 的对照 `--locked` 拒。四台上的打包、查包等 rc.3。

### 没验到

- **去掉 `MIYU_*` 跑 `miyu check`（`dist-check` 的最后一步）在 macOS、Windows 上没跑过**：只在本机 Linux 上查过 RL-1 打的包。
  这两种系统上 `miyu check` 在隔离的数据根、没有供应商时过不过，要第一趟演练的 CI 才看得到；不过的照实补。
- linux-aarch64（`ubuntu-24.04-arm`）上的打包、查包同样只有演练才跑得到。
- 签名、出处证明、草稿 Release 那一段在 CI 上要主人配好钥匙以后的那一趟演练；本地只跑过工作流里的 shell。

### 坑

- **Windows 上包里走出来的路径带 `\`**（分支 CI 41949460958 红在 Windows：报出来的是 `resources/core\stray.md`）：`git ls-files` 写的是
  `/`。`Path` 比较时 Windows 上两种分隔符算一样，所以那一趟是报的字不对；但比、报都靠 `Path` 的这一点不稳，改成一律换成 `/` 分开的
  字再比、再报（`check::relative`：先把 `\` 换成 `/`，再去掉开头的 `resources/`）。测试加两条：几种写法都换成同一个；清单写成
  Windows 的样子也对得上包里的文件。变异「不换 `\`」「不去开头」都抓住。

- **签名的私钥变量名叫 `MIYU_INDEX_KEY`**：`miyu-index` 签什么都从它读，发版的工作流把 `MIYU_RELEASE_KEY` 填进去，两把钥匙不混：
  软件源的 Actions 在 packages 仓库，填的是软件源那把。
- **Windows 机器的 bash 里 `$RUNNER_TEMP` 带反斜杠**，拿它拼通配不稳，压缩包放在仓库下的 `target/dist/`。
- `miyu check` 在三种 CI 机器上第一次跑（隔离的数据根、没有供应商）：演练时看它在 macOS、Windows 上过不过，不过的照实补。
