## 装包照 pacman

`miyu pkg info` 装着的、没装的都查得到，写成 pacman 的「键 : 值」；`miyu pkg install` 照 pacman 分阶段印，终端里带进度条；推荐的包装之前列出来、单独问一句。

状态：已定（2026-10-30 项目主人照样子选了三条推荐：`info` 照 pacman、装包分阶段带进度条、推荐的包装前列出单独问）。项目主人 10-29 在虚拟机里以新用户身份装包，说 `miyu pkg info` 查不了没装的包、装包时什么都看不出来、推荐的包从不提。第三节、第四节的拆法是技术细节，主会话定。

### 一、`miyu pkg info`

1. 装着的照本地（`-Qi`），没装的照软件源（`-Si`）。`miyu pkg info <编号>` 先看装没装，没装的去软件源找；`-Si` 只看软件源，`-Qi` 只看装着的。哪儿都没有的报「miyu: 未找到软件包 '<编号>'」，退出码 1。
2. 一格一行，写成「键 : 值」，键照最宽的那个按显示宽度补空格（中文一个字算两格），冒号前后各一个空格。没有值的行不印，「依赖」「推荐」没有的写「无」。
3. 没装的照软件源，行的先后：

```text
$ miyu pkg info miyu-im
软件源      : official
编号        : miyu-im
名称        : 接入 QQ
版本        : 0.2.1
描述        : 让她在 QQ 群和私聊里说话
依赖        : 无
推荐        : miyu-im-sponsor  miyu-memes
提供        : 命令 miyu im、后台页、接入 qq
需要的能力  : 联网、替主人说话
来源        : github.com/miyu-agent/miyu-im（官方）
作者        : miyu-agent
上游        : https://github.com/miyu-agent/miyu-im
下载大小    : 2.2 MiB
安装后大小  : 6.8 MiB
```

4. 装着的少「软件源」「下载大小」，多「被依赖」（装着的包里依赖它的）和最后一行「安装时间」（出厂的写「出厂自带」）；「来源」写「出厂」「软件源」「本地」之一，从软件源装的接来源仓库：本地库只记来源仓库，分不出装自官方的源还是别人的源，不说官方（IN-2 定）。
5. 英文的键照 pacman：Repository、Name、Title、Version、Description、Depends On、Recommends、Provides、Capabilities、Origin、Author、Upstream、Download Size、Installed Size、Required By、Install Date。
6. `--format json` 原样印核心的回应。

### 二、`miyu pkg install` 分阶段

```text
$ miyu pkg install miyu-im
正在解析依赖…
正在检查冲突…

软件包 (2)  miyu-embed-0.3.0  miyu-im-0.2.1
推荐 (1)    miyu-im-sponsor-0.1.0

下载大小    : 12.3 MiB
安装后大小  : 30.1 MiB
来源        : 官方，签名已验证
需要的能力  : 联网、替主人说话

安装 2 个软件包？[Y/n]
同时安装推荐的 1 个软件包？[Y/n]
正在下载…
 miyu-embed-0.3.0      10.1 MiB  [####################] 100%
 miyu-im-0.2.1          2.2 MiB  [####################] 100%
 miyu-im-sponsor-0.1.0  0.1 MiB  [####################] 100%
(3/3) 正在核对完整性
(1/3) 正在安装 miyu-embed
(2/3) 正在安装 miyu-im
(3/3) 正在安装 miyu-im-sponsor
```

1. 阶段：解析依赖、检查冲突（看一眼那一次）；列要装的；问；下载；核对完整性（SHA-256 和签名）；一个一个装。从包目录、包文件装的只有一个包，没有下载那一段。
2. 「软件包 (N)」照装的先后（依赖在前），`<编号>-<版本>`，空两格隔开；「推荐 (N)」只列没装的。下面四行照第一节的写法对齐；「来源」是官方的写「官方，签名已验证」，别的源写「<源的名字>，签名已验证」，社区的另接「，没人审过」。
3. 问：一个包的照设计 47 问「安装 '<编号>'？[Y/n]」，几个的问「安装 N 个软件包？[Y/n]」；有推荐的再问「同时安装推荐的 N 个软件包？[Y/n]」。`--yes` 两句都当是主包的答「是」、推荐的答「否」：脚本里不顺手多装东西。
4. 下载：终端里每个包一行进度条，照核心推的 `package.progress`；不在终端里（管道、脚本）不画进度条，每个包下完印一行「已下载 <编号>-<版本> <大小>」。
5. 装好了照旧问「现在启用 '<编号>'？[Y/n]」（`cli/pkg.md` 第 2½ 步），推荐的包各自照它问。
6. 失败的照设计 47 报错；前面装好的留着（同现在）。

### 三、要补的数据

1. **索引一个包多四格**：`recommends`（推荐的包编号）、`author`、`upstream`、`installed`（解开以后多少字节）。`miyu-index` 收录时照清单、包文件算；以前生成的索引没有这几格，命令行不印那几行。
2. **`package.info` 能查软件源**：参数多 `source`（`true` 只查软件源；不写的先查装着的，没装的查软件源），回应多 `from: "source"`、`source`（源的名字）、`download`、`installed_size`、`recommends`、`required_by`（装着的）。
3. **看一眼多两样**：`recommends`（没装的推荐包，带版本、下载大小）、`installed`（总的安装后大小）。
4. **进度带阶段**：`package.progress` 多 `stage`：`download`（照旧带 `done`、`total`）、`verify`、`install`；没写 `stage` 的当 `download`（老的头照旧认）。

### 四、怎么拆

| 步 | 做什么 |
|---|---|
| IN-1 | 索引多四格（`miyu-index` 收录、索引读写、老索引照旧读） |
| IN-2 | `package.info` 查软件源、装着的多「被依赖」；`miyu pkg info` 写成「键 : 值」，`-Si` |
| IN-3 | 看一眼多推荐和安装后大小、进度带阶段；`miyu pkg install` 分阶段印、进度条、问推荐 |
