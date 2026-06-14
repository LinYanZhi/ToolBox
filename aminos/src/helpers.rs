use crate::software;

/// 检查注册表显示名称是否匹配软件定义。
pub fn name_matches(reg_name: &str, sd: &software::SoftwareDef) -> bool {
    let rn_lower = reg_name.to_lowercase();
    let dn = sd.display_name.to_lowercase();
    if !dn.is_empty() && dn == rn_lower {
        return true;
    }
    if !dn.is_empty() && word_match(&dn, reg_name) {
        return true;
    }
    // Aliases: exact case-insensitive match only (no substring)
    for alias in &sd.aliases {
        if alias.to_lowercase() == rn_lower {
            return true;
        }
    }
    word_match(&sd.name.to_lowercase(), reg_name)
}

fn word_match(keyword: &str, text: &str) -> bool {
    let lower_text = text.to_lowercase();
    let lower_kw = keyword.to_lowercase();
    lower_text.contains(&lower_kw)
}
