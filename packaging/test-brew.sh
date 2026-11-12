#!/bin/sh
# 系统源 Homebrew 那几份的实测（施工 RL-5）：在 macOS 上跑，`cargo xtask packaging` 填好的放在 $1。
#
#   sh packaging/test-brew.sh <填好的目录> [static|install]
#
# static：brew style 四个 formula，填了托盘的加上 cask（RL-5补）。
# install：照真的发布建一个本地 tap（local/miyu-test，不是 miyu-agent/homebrew-miyu），brew install miyu-agent，查：
#   - miyu --version 报这一版，install.toml 写 brew，miyu upgrade 说用 brew 升级；
#   - 隔离的数据根里 miyu pkg list：包 tui、webui 是系统加装那一层、程序找得到；brew test 四个都过；
#   - 填了托盘的：brew install --cask 托盘，/Applications/Miyu.app 在，卸掉以后没了；
#   - brew uninstall 以后 share/miyu 没了，tap 删掉。
set -eu

OUT="$1"
MODE="${2:-static}"
TAP="local/miyu-test"

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

casks=""
if [ -d "${OUT}/homebrew/Casks" ]; then
  casks=$(ls "${OUT}"/homebrew/Casks/*.rb)
fi
# shellcheck disable=SC2086 # casks 是空的或者几个路径，要拆开
brew style "${OUT}"/homebrew/Formula/*.rb ${casks} || fail "brew style 没过"
echo "brew: static checks passed"
[ "${MODE}" = install ] || exit 0

brew tap-new --no-git "${TAP}" >/dev/null
# 新的 Homebrew 不从没信任过的 tap 载 formula：装的那一个照命令行信任，它依赖的 miyu-core 那几个不算（rc.6 真装红在这里）。
# 整个本地 tap 信任上；旧的 Homebrew 没有这个命令，也不查。
# 日志里记下这台机器的 Homebrew、python3 版本，和 trust 调没调（RL-5补3）：没调的就没验到「依赖的 formula 被拒」。
echo "brew: $(brew --version | head -n 1); python3: $(python3 --version 2>&1)"
if brew trust --help >/dev/null 2>&1; then
  brew trust "${TAP}" || fail "brew trust 没成"
  echo "brew: trusted ${TAP}"
else
  echo "brew: no trust command, skipped"
fi
tap_dir="$(brew --repository "${TAP}")"
cp "${OUT}"/homebrew/Formula/*.rb "${tap_dir}/Formula/"
if [ -n "${casks}" ]; then
  mkdir -p "${tap_dir}/Casks"
  cp "${OUT}"/homebrew/Casks/*.rb "${tap_dir}/Casks/"
fi
brew install "${TAP}/miyu-agent" || fail "brew install 没成"

prefix=$(brew --prefix)
version=$(sed -n 's/^  version "\(.*\)"$/\1/p' "${OUT}/homebrew/Formula/miyu-core.rb")
said=$("${prefix}/bin/miyu" --version)
[ "${said}" = "miyu ${version}" ] || fail "miyu --version 报的是 ${said}"
grep -q '^channel = "brew"' "$(brew --prefix miyu-core)/libexec/install.toml" || fail "install.toml 不对"
up=$(LANG=C "${prefix}/bin/miyu" upgrade 2>&1) || fail "miyu upgrade 没成：${up}"
printf '%s\n' "${up}" | grep -q 'installed with Homebrew' || fail "miyu upgrade 该说用 brew：${up}"

home=$(mktemp -d)
listed=$(MIYU_HOME="${home}" "${prefix}/bin/miyu" pkg list --format json) || fail "miyu pkg list 没成"
MIYU_HOME="${home}" "${prefix}/bin/miyu" core stop || true
printf '%s' "${listed}" | python3 -c '
import json, sys
items = {item["package"]: item for item in json.load(sys.stdin)}
for id in ("tui", "webui"):
    item = items.get(id)
    assert item, f"{id} not listed"
    # f 串的花括号里不写引号：外面是 sh 的单引号，里面写 \" 是 Python 看到的反斜杠，语法错（同 test-aur.sh，RL-5补3）。
    layer, code = item.get("layer"), item.get("code")
    assert layer == "distro", f"{id} layer {layer}"
    assert item.get("status") != "program_missing", f"{id} program missing"
    assert "code" not in item, f"{id} code {code}"
print("distro layer ok")
' || fail "系统加装那一层不对"
rm -rf "${home}"

for name in miyu-core miyu-tui miyu-webui miyu-agent; do
  brew test "${TAP}/${name}" || fail "brew test ${name} 没过"
done

if [ -n "${casks}" ]; then
  brew install --cask "${TAP}/miyu-applet" || fail "brew install --cask 托盘没成"
  [ -d /Applications/Miyu.app ] || fail "没有 /Applications/Miyu.app"
  brew uninstall --cask miyu-applet
  [ ! -e /Applications/Miyu.app ] || fail "Miyu.app 还在"
fi
brew uninstall miyu-agent miyu-webui miyu-tui miyu-core
[ ! -e "${prefix}/share/miyu/packages/tui" ] || fail "share/miyu 还在"
brew untap "${TAP}"
echo "brew: all install checks passed"
