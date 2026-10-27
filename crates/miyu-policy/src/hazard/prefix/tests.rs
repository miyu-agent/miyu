//! 命令前缀怎么取（施工 SB-3）。

use super::prefix;

fn of(command: &str) -> Option<String> {
    prefix(command)
}

#[test]
fn the_first_word_or_a_program_with_its_subcommand() {
    assert_eq!(of("ls -la").as_deref(), Some("ls"));
    assert_eq!(of("docker ps -a").as_deref(), Some("docker ps"));
    assert_eq!(of("npm run build").as_deref(), Some("npm run"));
    assert_eq!(of("git status").as_deref(), Some("git status"));
    assert_eq!(of("cargo").as_deref(), Some("cargo"));
    assert_eq!(
        of("docker run \"$IMAGE\" ${TAG}").as_deref(),
        Some("docker run")
    );
    assert_eq!(of("make > out.txt 2>&1").as_deref(), Some("make"));
}

#[test]
fn every_part_of_a_chain_has_to_agree() {
    assert_eq!(
        of("npm run lint && npm run test").as_deref(),
        Some("npm run")
    );
    assert_eq!(
        of("docker ps | docker ps -q; docker ps &").as_deref(),
        Some("docker ps")
    );
    assert_eq!(of("cd sub && npm run build"), None);
    assert_eq!(of("docker ps | grep x"), None);
    assert_eq!(of("git status\nrm -rf x"), None);
}

#[test]
fn a_path_is_its_own_prefix() {
    assert_eq!(of("./docker ps").as_deref(), Some("./docker"));
    assert_eq!(of("/tmp/x/docker ps").as_deref(), Some("/tmp/x/docker"));
}

#[test]
fn assignments_in_front_give_no_prefix() {
    assert_eq!(of("PATH=/tmp/evil docker ps"), None);
    assert_eq!(of("LD_PRELOAD=/tmp/x.so npm run x"), None);
    assert_eq!(of("FOO=1; docker ps"), None);
}

#[test]
fn wrappers_give_no_prefix() {
    for command in [
        "env docker ps",
        "sudo docker ps",
        "timeout 5 docker ps",
        "nohup npm run x",
        "bash -c 'docker ps'",
        "sh script.sh",
        "xargs rm",
        ". ./env.sh",
        "source env.sh",
        "exec docker ps",
        "eval docker ps",
        "command docker ps",
    ] {
        assert_eq!(of(command), None, "{command}");
    }
}

#[test]
fn what_cannot_be_read_gives_no_prefix() {
    for command in [
        "docker $(echo ps)",
        "docker `echo ps`",
        "docker \"$(echo ps)\"",
        "(docker ps)",
        "{ docker ps; }",
        "diff <(ls a) <(ls b)",
        "$CMD ps",
        "docker $SUB",
        "git -C /tmp status",
        "docker --version",
        "",
        "   ",
        "> out.txt",
    ] {
        assert_eq!(of(command), None, "{command:?}");
    }
}

#[test]
fn quoted_text_does_not_count_as_structure() {
    assert_eq!(
        of("git commit -m 'fix (x) {y}'").as_deref(),
        Some("git commit")
    );
    assert_eq!(of("grep 'a|b' file").as_deref(), Some("grep"));
    assert_eq!(of("echo \"a; b\"").as_deref(), Some("echo"));
}
