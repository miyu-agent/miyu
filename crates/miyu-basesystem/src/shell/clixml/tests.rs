use super::plain;

/// 桌面端在虚拟机上看到的那一段（截短）：错误原文取出来，换行、`&lt;` 还原，进度的记录丢掉，前面照常的输出留着。
#[test]
fn an_error_record_comes_back_as_plain_text() {
    let text = "Caption\r\n#< CLIXML\r\n<Objs Version=\"1.1.0.1\" xmlns=\"http://schemas.microsoft.com/powershell/2004/04\"><Obj S=\"progress\" RefId=\"0\"><TN RefId=\"0\"><T>System.Management.Automation.PSCustomObject</T></TN><MS><I64 N=\"SourceId\">1</I64></MS></Obj><S S=\"Error\">Get-CimInstance : Access to a CIM resource was not available to the client._x000D__x000A_</S><S S=\"Error\">At line:1 char:1 &lt;here&gt;_x000D__x000A_</S></Objs>";
    assert_eq!(
        plain(text),
        "Caption\r\nGet-CimInstance : Access to a CIM resource was not available to the client.\nAt line:1 char:1 <here>\n"
    );
}

#[test]
fn text_without_clixml_is_left_alone() {
    let text = "no xml here _x000A_ &lt;";
    assert!(matches!(plain(text), std::borrow::Cow::Borrowed(same) if same == text));
}

#[test]
fn a_block_cut_short_is_kept_as_it_is() {
    let text = "ok\n#< CLIXML\r\n<Objs><S S=\"Error\">half";
    assert_eq!(plain(text), text);
}

#[test]
fn chinese_messages_and_several_blocks_both_come_through() {
    let text = "#< CLIXML\n<Objs><S S=\"Error\">找不到路径_x000D__x000A_</S></Objs>\nmiddle\n#< CLIXML\n<Objs><S S=\"Warning\">小心_x000A_</S></Objs>";
    assert_eq!(plain(text), "找不到路径\nmiddle\n小心\n");
}

/// 修以前：`_x` 后面第 5 个字节正好是一个多字节字的结尾（`_x12中`），先切出 5 个字节成了，再在第 4 个字节切就落在字中间，
/// panic（审计 #010）。命令自己印的字就能写出这一段。现在不是四位十六进制的照原样留着。
#[test]
fn an_escape_cut_inside_a_wide_character_is_left_alone() {
    for body in ["_x12中文", "_x1中文", "_x中文", "_x123中", "_x"] {
        let text = format!("#< CLIXML\n<Objs><S S=\"Error\">{body}</S></Objs>");
        assert_eq!(plain(&text), format!("{body}\n"), "{body}");
    }
}
