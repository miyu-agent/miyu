## SD-3 作者文档「经系统源发」

状态：施工完，待 core1 审（2026-11-05，sub1）。设计 `60-系统源分包.md` 第四节、第六节第 3 条；SD-1（系统加装层）、SD-2（分包的布局）。

### 目的

第三方可以经系统源（AUR、Homebrew）发自己的扩展（设计 60 第七节第 4 条，项目主人 11-05 定）。作者文档原来只有一段话说「装到哪」，
写包的人照它还是不知道包文件夹长什么样、依赖写什么、装好以后怎么查、用户卸了会怎样。这一步把它扩成一节。

### 做什么

作者文档 `docs/authors/README.md`、`README.en.md` 的「发到软件源」后面另起一节「经系统源发」，原来那段并进来：

1. 装到哪：Linux 照 `XDG_DATA_DIRS`（系统包装进 `/usr/share/miyu/packages/<编号>/`），Linuxbrew 要用户把它的 `share` 写进去；
   macOS 照 Homebrew 前缀；Windows 没有。
2. 包文件夹同 `miyu pkg pack` 打进去的那一份，程序在 `bin/`、带执行位；不放进核心的出厂层（`/usr/lib/miyu/`、`libexec/`）。
3. 包名 `miyu-<编号>`；依赖 `miyu-core`，不依赖合集 `miyu-agent`；版本下限照清单的协议（现在只有 1，不写也行）；
   依赖别的 Miyu 包的，清单的 `depends` 和系统包的依赖都写上。
4. 装好以后核心起来时读，层是 `distro`；对不上的不加载，`miyu check` 报原因。
5. 和软件源一起发：用户装的盖过系统装的，系统那份更新时 `miyu check` 报 `shadowed`（SD-1补 改的名字）。
6. 卸：`miyu pkg remove` 只记一笔，文件归包管理器；`miyu pkg install <编号>` 装回（SD-1补）。
7. 自己先试：`MIYU_DISTRO_PACKAGES` 加临时 `MIYU_HOME`，看 `pkg list` 的层、`pkg check` 找不找得到程序（同 SD-2 的 `dist-check`）。

另附 `PKGBUILD`、Homebrew formula 各一小段示例。

### 怎么验

只改文档。说的每一条照代码核对过：
- 找法见 `miyu_store::packages::distro`；
- 装回见 SD-1补 的 `packages::underneath`；
- 警告代码见 SD-1补 的 `shadowed`；
- `MIYU_DISTRO_PACKAGES` 见 SD-2 的 `dist-check`，实测用过。

门禁的文档检查过了。

### 先后

接在 SD-2 后面。文中的 `shadowed`、照编号装回系统加装的包，要 SD-1补 合了才对得上，合的时候放在 SD-1补 之后。
