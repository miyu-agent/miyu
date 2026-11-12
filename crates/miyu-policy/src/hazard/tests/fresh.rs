//! 同一条命令里 `mktemp` 刚建的临时路径（施工 HZ-2，2026-11-12 项目主人定）：`x=$(mktemp …)` 以后不带后缀地删 `$x` 不问；
//! 带后缀的、赋值不止一条 `mktemp` 的、中间改过的、跨命令的、别的命令替换照旧判。

use super::kind;

/// 项目主人撞上的那一条：写测试最常见的写法，删的是自己刚建的。
#[test]
fn the_owners_probe_directory_is_deleted_without_asking() {
    assert_eq!(
        kind(
            "tmp=$(mktemp -d /tmp/probe.XXXXXX); TMPDIR=$tmp cargo test -p miyu-policy; rm -rf $tmp"
        ),
        None
    );
}

/// 认的写法：几种 `mktemp`、几种引用、反引号、`export` 这类单独成句的赋值。
#[test]
fn the_ways_of_writing_it_that_are_recognised() {
    for command in [
        "x=$(mktemp); rm -f $x",
        "x=$(mktemp -d); rm -rf \"$x\"",
        "x=$(mktemp -d); rm -rf ${x}",
        "x=$(mktemp -d); rm -rf \"${x}\"",
        "x=$(mktemp -d); rm -r $x",
        "x=`mktemp -d`; rm -rf $x",
        "x=\"$(mktemp -d)\"; rm -rf $x",
        "x=$(mktemp -p /var/tmp); rm -rf $x",
        "x=$(mktemp -d -t probe.XXXXXX); rm -rf $x",
        "x=$(mktemp -dt probe.XXXXXX); rm -rf $x",
        "x=$(mktemp --tmpdir probe.XXXXXX); rm -rf $x",
        "x=$(mktemp --tmpdir=/var/tmp -d); rm -rf $x",
        "x=$(mktemp -u); rm -rf $x",
        "x=$(mktemp -q -d) || exit 1; rm -rf $x",
        "export x=$(mktemp -d); rm -rf $x",
        "x=$(mktemp -d) && cd $x && make; rm -rf $x",
        "x=$(mktemp -d); if [ -d \"$x\" ]; then rm -rf \"$x\"; fi",
        "x=$(mktemp -d); rm -rf $x; x=/tmp/other",
        "x=$(mktemp -d)\ncargo test\nrm -rf \"$x\"",
        // 同一条命令里先写死、后 `mktemp` 的，照后来的。
        "x=/tmp/zt; x=$(mktemp -d); rm -rf $x",
        // 不是同一个 rm 的其他目标照原规矩：普通的照旧不管。
        "x=$(mktemp -d); rm -rf $x build",
    ] {
        assert_eq!(kind(command), None, "{command}");
    }
}

/// 规则 2：带后缀的照原规矩判（`mktemp` 失败时 `x` 是空的，`$x/*` 就是 `/*`）。
#[test]
fn a_suffix_after_the_name_is_judged_as_before() {
    for command in [
        "x=$(mktemp -d); rm -rf $x/*",
        "x=$(mktemp -d); rm -rf \"$x\"/*",
        "x=$(mktemp -d); rm -rf \"$x\"/",
        "x=$(mktemp -d); rm -rf $x/..",
        "x=$(mktemp -d); rm -rf ${x}foo",
        "x=$(mktemp -d); rm -rf ${x:-/tmp/a}",
        "x=$(mktemp -d); rm -rf $x$y",
    ] {
        assert_eq!(kind(command), Some("delete_unresolved"), "{command}");
    }
}

/// 规则 3：赋值正好是一条不带变量的 `mktemp` 才认。
#[test]
fn only_a_lone_literal_mktemp_counts() {
    for command in [
        "x=$(mktemp -d)/a; rm -rf $x",
        "x=pre$(mktemp); rm -rf $x",
        "x=$(mktemp -d)$(mktemp -d); rm -rf $x",
        "x=$(mktemp -d; echo /); rm -rf $x",
        "x=$(mktemp -d && cd /); rm -rf $x",
        "x=$(mktemp -d | head -1); rm -rf $x",
        "x=$(mktemp -d $(echo /tmp/a.XXXX)); rm -rf $x",
        "x=$(mktemp -p $D); rm -rf $x",
        "x=$(mktemp -d \"$T\"); rm -rf $x",
        "x=$(mktemp -d /tmp/*.XXXX); rm -rf $x",
        "x=$(mktemp -d 2>&1); rm -rf $x",
        "x=$(mktemp --suffix=.d); rm -rf $x",
        "x=$(./mktemp -d); rm -rf $x",
        "x=$(env mktemp -d); rm -rf $x",
        "x=<(mktemp -d); rm -rf $x",
        "declare -i x=$(mktemp -d); rm -rf $x",
        // 规则 7：别的命令替换照旧问。
        "x=$(pwd); rm -rf $x",
        "x=$(echo /tmp/a); rm -rf $x",
    ] {
        assert_eq!(kind(command), Some("delete_unresolved"), "{command}");
    }
}

/// 规则 4：赋值以后、删以前又改过的不算；认不准中间有没有改过的（分支、循环、子 shell、`eval`、`source`、`trap`、算术、
/// `${x:=…}`、动过 `IFS`）照旧问。
#[test]
fn a_change_in_between_or_a_doubt_about_it_asks() {
    for (command, expected) in [
        ("x=$(mktemp -d); x=/; rm -rf $x", "delete_root"),
        ("x=$(mktemp -d); x=~; rm -rf $x", "delete_home"),
        ("x=$(mktemp -d); read x; rm -rf $x", "delete_unresolved"),
        (
            "x=$(mktemp -d); for x in a b; do rm -rf $x; done",
            "delete_unresolved",
        ),
        (
            "x=$(mktemp -d); export x=$Y; rm -rf $x",
            "delete_unresolved",
        ),
        ("x=$(mktemp -d); x+=/a; rm -rf $x", "delete_unresolved"),
        (
            "x=$(mktemp -d); printf -v x %s /; rm -rf $x",
            "delete_unresolved",
        ),
        ("x=$(mktemp -d); let x=1; rm -rf $x", "delete_unresolved"),
        (
            "x=$(mktemp -d); declare -n r=x; r=/; rm -rf $x",
            "delete_unresolved",
        ),
        ("x=$(mktemp -d); x=/ true; rm -rf $x", "delete_unresolved"),
        ("x=$(mktemp -d); : ${x:=/}; rm -rf $x", "delete_unresolved"),
        (
            "x=$(mktemp -d); (( x = 0 )); rm -rf $x",
            "delete_unresolved",
        ),
        (
            "x=$(mktemp -d); echo $((x=0)); rm -rf $x",
            "delete_unresolved",
        ),
        ("x=$(mktemp -d); IFS=/; rm -rf $x", "delete_unresolved"),
        (
            "x=$(mktemp -d); eval \"$CMD\"; rm -rf $x",
            "delete_unresolved",
        ),
        (
            "x=$(mktemp -d); source ./env.sh; rm -rf $x",
            "delete_unresolved",
        ),
        (
            "trap 'x=/' DEBUG; x=$(mktemp -d); rm -rf $x",
            "delete_unresolved",
        ),
        // 循环里删、循环里后面又改的：第二圈删的是改过的。
        (
            "x=$(mktemp -d); while true; do rm -rf $x; x=/; done",
            "delete_unresolved",
        ),
        // 函数里改、后面调的。
        (
            "f() { x=/; }; x=$(mktemp -d); f; rm -rf $x",
            "delete_unresolved",
        ),
        // 赋值不一定跑到的：分支里、`&&`/`||` 后面、子 shell 里、放到后台的。
        (
            "if true; then x=$(mktemp -d); fi; rm -rf $x",
            "delete_unresolved",
        ),
        ("cd /a && x=$(mktemp -d); rm -rf $x", "delete_unresolved"),
        ("false || x=$(mktemp -d); rm -rf $x", "delete_unresolved"),
        ("(x=$(mktemp -d)); rm -rf $x", "delete_unresolved"),
        ("{ x=$(mktemp -d); }; rm -rf $x", "delete_unresolved"),
        ("x=$(mktemp -d) & rm -rf $x", "delete_unresolved"),
        (
            "case $a in b) x=$(mktemp -d);; esac; rm -rf $x",
            "delete_unresolved",
        ),
        // 管道里的、只给一个命令的不算赋值。
        ("x=$(mktemp -d) | cat; rm -rf $x", "delete_unresolved"),
        ("x=$(mktemp -d) rm -rf $x", "delete_unresolved"),
        // 删在赋值之前的。
        ("rm -rf $x; x=$(mktemp -d)", "delete_unresolved"),
    ] {
        assert_eq!(kind(command), Some(expected), "{command}");
    }
}

/// 规则 5、6、7：同一个 rm 里别的目标照原规矩，最严者胜；跨命令的、替换里删外面的、`find` 照旧。
#[test]
fn other_targets_other_commands_and_other_places_are_unchanged() {
    for (command, expected) in [
        ("x=$(mktemp -d); rm -rf $x ~", "delete_home"),
        ("x=$(mktemp -d); rm -rf $x $y", "delete_unresolved"),
        ("x=$(mktemp -d); rm -rf $x .", "delete_working_dir"),
        ("rm -rf $x", "delete_unresolved"),
        ("x=$(mktemp -d); sh -c 'rm -rf $x'", "delete_unresolved"),
        ("x=$(mktemp -d); echo $(rm -rf $x)", "delete_unresolved"),
        ("x=$(mktemp -d); find $x -delete", "delete_unresolved"),
        ("x=$(mktemp -d); y=$x; rm -rf $y", "delete_unresolved"),
        // 别的变量借着命令替换拼出同一个名字（`$(echo /)x` 拆出来也是 `$x`），不当成 `$x`。
        ("x=$(mktemp -d); rm -rf $(echo /)x", "delete_unresolved"),
        ("T=/tmp/zt; rm -rf $(echo /)T", "delete_unresolved"),
    ] {
        assert_eq!(kind(command), Some(expected), "{command}");
    }
}
