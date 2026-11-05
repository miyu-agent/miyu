#!/bin/sh
# 系统源 Homebrew 那几份的实测（施工 RL-5）：在 macOS 上跑，`cargo xtask packaging` 填好的放在 $1。
#
#   sh packaging/test-brew.sh <填好的目录> [static|install]
#
# static：brew style 四个 formula。
# install：照真的发布建一个本地 tap（local/miyu-test，不是 miyu-agent/homebrew-miyu），brew install miyu-agent，查：
#   - miyu --version 报这一版，install.toml 写 brew，miyu upgrade 说用 brew 升级；
#   - 隔离的数据根里 miyu pkg list：miyu-tui、miyu-webui 是系统加装那一层、程序找得到；brew test 四个都过；
#   - brew uninstall 以后 share/miyu 没了，tap 删掉。
set -eu

OUT="$1"
MODE="${2:-static}"
TAP="local/miyu-test"

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

brew style "${OUT}"/homebrew/Formula/*.rb || fail "brew style 没过"
echo "brew: static checks passed"
[ "${MODE}" = install ] || exit 0

brew tap-new --no-git "${TAP}" >/dev/null
tap_dir="$(brew --repository "${TAP}")"
cp "${OUT}"/homebrew/Formula/*.rb "${tap_dir}/Formula/"
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
for id in ("miyu-tui", "miyu-webui"):
    item = items.get(id)
    assert item, f"{id} not listed"
    assert item["layer"] == "distro", f"{id} layer {item[\"layer\"]}"
    assert item.get("status") != "program_missing", f"{id} program missing"
    assert "code" not in item, f"{id} code {item.get(\"code\")}"
print("distro layer ok")
' || fail "系统加装那一层不对"
rm -rf "${home}"

for name in miyu-core miyu-tui miyu-webui miyu-agent; do
  brew test "${TAP}/${name}" || fail "brew test ${name} 没过"
done

brew uninstall miyu-agent miyu-webui miyu-tui miyu-core
[ ! -e "${prefix}/share/miyu/packages/miyu-tui" ] || fail "share/miyu 还在"
brew untap "${TAP}"
echo "brew: all install checks passed"
