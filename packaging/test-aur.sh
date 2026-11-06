#!/bin/sh
# 系统源 AUR 那几份的实测（施工 RL-5）：在 Arch 的容器里跑（root），`cargo xtask packaging` 填好的放在 $1。
#
#   sh packaging/test-aur.sh <填好的目录> [static|install]
#
# static：每份 PKGBUILD 语法对、makepkg --printsrcinfo 出得来、namcap 只报已知的那一种（source_x86_64 里写 x86_64 的误报）。
# install：照真的发布装 miyu-core、miyu-tui、miyu-webui；填了托盘的（RL-5补）另装 miyu-applet、miyu-agent（合集依赖托盘），查：
#   - /usr/bin/miyu --version 报这一版，install.toml 写 aur，miyu upgrade 说用包管理器升级；
#   - 隔离的数据根里 miyu pkg list：miyu-tui、miyu-webui 是系统加装那一层（distro）、程序找得到；
#   - 托盘：/usr/bin/miyu-applet 能跑、桌面文件和图标在；
#   - pacman -R 卸掉以后 /usr/lib/miyu、/usr/share/miyu、/usr/bin/miyu-applet 都没了。
set -eu

OUT="$1"
MODE="${2:-static}"

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

pacman -Sy --noconfirm --needed base-devel namcap python >/dev/null
id builder >/dev/null 2>&1 || useradd -m builder
rm -rf /home/builder/aur
cp -r "${OUT}/aur" /home/builder/aur
chown -R builder /home/builder/aur

for dir in /home/builder/aur/*; do
  name=$(basename "${dir}")
  bash -n "${dir}/PKGBUILD" || fail "${name}：PKGBUILD 语法不对"
  su builder -c "cd '${dir}' && makepkg --printsrcinfo > .SRCINFO" || fail "${name}：出不了 .SRCINFO"
  # 照字面滤掉那一句（里面的 $CARCH 是原文，不是变量）。
  # shellcheck disable=SC2016
  warnings=$(namcap "${dir}/PKGBUILD" | grep -v -F 'Reference to x86_64 should be changed to $CARCH' || true)
  [ -z "${warnings}" ] || fail "${name}：namcap 报了 ${warnings}"
  echo "${name}: static checks passed"
done
# 版本号是 年.月.序号（施工 VR-1，设计 59 第十节）：pacman 照数字比，候选版写成紧跟的 rc<号>，排在同号的正式版前面。
older() {
  [ "$(vercmp "$1" "$2")" = -1 ] || fail "vercmp 该说 $1 比 $2 旧"
}
older 2026.11.0rc1 2026.11.0rc2
older 2026.11.0rc2 2026.11.0rc10
older 2026.11.0rc10 2026.11.0
older 2026.11.0 2026.11.1
older 2026.11.1 2026.11.10
older 2026.9.0 2026.10.0
older 2026.11.10 2026.12.0rc1
older 2026.12.0 2027.1.0
older 0.7.4 2026.11.0rc1
echo "vercmp: year.month versions order by number"

[ "${MODE}" = install ] || exit 0

names="miyu-core miyu-tui miyu-webui"
if [ -d /home/builder/aur/miyu-applet ]; then
  names="${names} miyu-applet miyu-agent"
fi
for name in ${names}; do
  su builder -c "cd /home/builder/aur/${name} && makepkg --noconfirm -f" || fail "${name}：makepkg 没成"
  pacman -U --noconfirm /home/builder/aur/"${name}"/*.pkg.tar.zst || fail "${name}：装不上"
done

version=$(sed -n 's/^_version=//p' /home/builder/aur/miyu-core/PKGBUILD)
said=$(/usr/bin/miyu --version)
[ "${said}" = "miyu ${version}" ] || fail "miyu --version 报的是 ${said}"
grep -q '^channel = "aur"' /usr/lib/miyu/install.toml || fail "install.toml 不对"
up=$(LANG=C /usr/bin/miyu upgrade 2>&1) || fail "miyu upgrade 没成：${up}"
printf '%s\n' "${up}" | grep -q 'installed with AUR' || fail "miyu upgrade 该说用包管理器：${up}"

home=$(mktemp -d)
listed=$(MIYU_HOME="${home}" /usr/bin/miyu pkg list --format json) || fail "miyu pkg list 没成"
MIYU_HOME="${home}" /usr/bin/miyu core stop || true
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

if [ -d /home/builder/aur/miyu-applet ]; then
  [ -x /usr/bin/miyu-applet ] || fail "没有 /usr/bin/miyu-applet"
  [ -f /usr/share/applications/miyu-applet.desktop ] || fail "没有桌面文件"
  [ -f /usr/share/icons/hicolor/128x128/apps/miyu-applet.png ] || fail "没有图标"
  pacman -R --noconfirm miyu-agent miyu-applet
fi
pacman -R --noconfirm miyu-webui miyu-tui miyu-core
[ ! -e /usr/lib/miyu ] || fail "/usr/lib/miyu 还在"
[ ! -e /usr/share/miyu ] || fail "/usr/share/miyu 还在"
[ ! -e /usr/bin/miyu-applet ] || fail "/usr/bin/miyu-applet 还在"
echo "aur: all install checks passed"
