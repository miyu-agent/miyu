//! 喂给 shell 跑的脚本（施工 FX-22）：heredoc、here-string、管道前面 `echo`、`printf`、`cat` 印的字，`env -S`、`su -c` 的值，里面的
//! 照样另认。修以前几种都判成安全：里面的字不拆。

use super::kind;

#[test]
fn scripts_fed_to_a_shell_are_looked_into() {
    for command in [
        "bash <<'EOF'\nrm -rf ~\nEOF",
        "sh <<EOF\nrm -rf ~\nEOF\n",
        "bash -s <<'EOF'\nrm -rf ~\nEOF",
        "sudo bash <<-EOF\n\trm -rf ~\n\tEOF",
        "bash <<< 'rm -rf ~'",
        "sh <<<\"rm -rf ~\"",
        "echo 'rm -rf ~' | sh",
        "echo -n 'rm -rf ~' | bash -s",
        "printf 'cd /tmp\\nrm -rf ~\\n' | bash",
        "cat <<'EOF' | bash\nrm -rf ~\nEOF",
        "env -S 'rm -rf ~'",
        "env --split-string='rm -rf ~'",
        "env -i -S 'rm -rf ~'",
        "su -c 'rm -rf ~'",
        "su root -c 'rm -rf ~'",
        "su --command='rm -rf ~' root",
    ] {
        assert_eq!(kind(command), Some("delete_home"), "{command:?}");
    }
}

/// 不报的：shell 跑的是脚本文件、标准输入给的是数据；不是 shell 读的（`cat`、`grep`、`python3`）。
#[test]
fn input_that_is_not_a_script_for_a_shell_is_left_alone() {
    for command in [
        "cat <<'EOF'\nrm -rf ~\nEOF",
        "bash script.sh <<< 'rm -rf ~'",
        "bash ./install.sh <<'EOF'\nrm -rf ~\nEOF",
        "echo 'rm -rf ~' | grep rm",
        "echo 'rm -rf ~' | bash script.sh",
        "python3 <<'EOF'\nprint('rm -rf ~')\nEOF",
        "echo 'rm -rf ~' > notes.txt",
        "env -S 'echo hi'",
        "su -c 'ls ~'",
        "bash -c 'echo hi' <<< 'rm -rf ~'",
    ] {
        assert_eq!(kind(command), None, "{command:?}");
    }
}
