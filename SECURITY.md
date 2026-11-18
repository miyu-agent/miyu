# 报告安全问题

发现 Miyu 的安全问题，请私下报告：不要开公开的 issue、PR，也不要在公开的地方贴怎么利用的细节。English: [below](#reporting-a-security-issue).

在这个仓库的 **Security** 页点 **Report a vulnerability**（[直接打开](https://github.com/miyu-agent/miyu/security/advisories/new)，要登录 GitHub）。报告只有你和维护者看得到。终端、网页、托盘、通讯平台和各个软件包的问题也报到这里，写明是哪个仓库。

报告里请写：

- 版本（`miyu --version`）和系统；
- 复现的步骤，越短越好；
- 造成的后果：读到、改到、发出去了什么。

Miyu 挡住什么、没挡住什么，见手册的 [安全](docs/guide/security.md)。

## Reporting a security issue

If you find a security issue in Miyu, report it privately: do not open a public issue or PR, and do not post exploit details anywhere public.

On this repository's **Security** tab, click **Report a vulnerability** ([open it directly](https://github.com/miyu-agent/miyu/security/advisories/new); you need to be signed in to GitHub). Only you and the maintainers can see the report. Issues in the terminal UI, the web UI, the tray app, the chat platform bridge and the packages are reported here too; say which repository.

Please include:

- the version (`miyu --version`) and the operating system;
- steps to reproduce, as short as possible;
- the impact: what could be read, changed or sent out.

What Miyu guards against and what it does not is described in the user guide: [Security](docs/guide/security.en.md).
