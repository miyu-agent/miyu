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
#   - pacman -R 卸掉以后 /usr/lib/miyu、/usr/share/miyu、/usr/bin/miyu-applet 都没了；
#   - 旧版升级（RL-5补2，填了托盘的才做：合集依赖托盘）：先装一个假的旧 miyu 0.7.4（自己有 /usr/bin/miyu），再把这几份放进一个
#     本机的 file:// 软件源，照用户平常那样 pacman -Syu：旧的 miyu 升成过渡包、带进一整套，没有冲突要问、/usr/bin/miyu 换成新的，
#     还印了怎么搬旧数据。
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

if [ -d /home/builder/aur/miyu-applet ]; then
  # 假的旧版：AUR 上 0.7.x 那一代的样子，自己装一个 /usr/bin/miyu。
  old=/home/builder/old-miyu
  rm -rf "${old}"
  mkdir -p "${old}"
  cat > "${old}/PKGBUILD" <<'PKG'
pkgname=miyu
pkgver=0.7.4
pkgrel=1
pkgdesc='Old Miyu, for the upgrade test'
arch=('any')
license=('MIT')
package() {
  install -Dm755 /dev/stdin "${pkgdir}/usr/bin/miyu" <<'SH'
#!/bin/sh
echo "miyu 0.7.4"
SH
}
PKG
  chown -R builder "${old}"
  su builder -c "cd '${old}' && makepkg --noconfirm -f" || fail "假的旧 miyu：makepkg 没成"
  pacman -U --noconfirm "${old}"/miyu-0.7.4-1-any.pkg.tar.zst || fail "假的旧 miyu 装不上"
  [ "$(/usr/bin/miyu)" = "miyu 0.7.4" ] || fail "旧 miyu 没装好"

  # 它依赖的合集刚卸掉，编的时候不查依赖（-d）：装的时候由 pacman 从下面的软件源带进来。
  su builder -c "cd /home/builder/aur/miyu && makepkg --noconfirm -d -f" || fail "过渡包 miyu：makepkg 没成"
  repo=/home/builder/repo
  rm -rf "${repo}"
  mkdir -p "${repo}"
  for name in miyu miyu-core miyu-tui miyu-webui miyu-applet miyu-agent; do
    cp /home/builder/aur/"${name}"/*.pkg.tar.zst "${repo}/"
  done
  repo-add -q "${repo}/miyutest.db.tar.gz" "${repo}"/*.pkg.tar.zst
  cp /etc/pacman.conf /etc/pacman.conf.orig
  printf '\n[miyutest]\nSigLevel = Optional TrustAll\nServer = file://%s\n' "${repo}" >> /etc/pacman.conf
  # 照用户平常的升级：--noconfirm 遇到冲突要问时答「不」，升级就失败，所以这一步成了就是没有冲突要问。只认真的冲突那几句：
  # pacman 平常就印「looking for conflicting packages」「checking for file conflicts」。
  upgraded=$(pacman -Syu --noconfirm 2>&1) || fail "pacman -Syu 没成：${upgraded}"
  printf '%s\n' "${upgraded}" | grep -qE 'are in conflict|conflicting files|unresolvable' && fail "升级时报了冲突：${upgraded}"
  printf '%s\n' "${upgraded}" | grep -q 'miyu migrate' || fail "升级时没说怎么搬旧数据：${upgraded}"
  wanted=$(sed -n 's/^pkgver=//p' /home/builder/aur/miyu/PKGBUILD)
  [ "$(pacman -Q miyu)" = "miyu ${wanted}-1" ] || fail "miyu 没升成过渡包：$(pacman -Q miyu)"
  pacman -Q miyu-core miyu-tui miyu-webui miyu-applet miyu-agent >/dev/null || fail "一整套没带进来"
  [ "$(/usr/bin/miyu --version)" = "miyu ${version}" ] || fail "/usr/bin/miyu 不是新的：$(/usr/bin/miyu --version)"
  [ "$(pacman -Qqo /usr/bin/miyu)" = miyu-core ] || fail "/usr/bin/miyu 该归 miyu-core"

  pacman -R --noconfirm miyu miyu-agent miyu-applet miyu-webui miyu-tui miyu-core
  mv /etc/pacman.conf.orig /etc/pacman.conf
  pacman -Sy >/dev/null
  echo "aur: the old miyu upgrades to the transitional package and the whole set"
fi
echo "aur: all install checks passed"
