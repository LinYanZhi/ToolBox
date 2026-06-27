use std::collections::HashMap;
use std::sync::OnceLock;
use color::{DisplayWidth, strip_ansi, gray, cyan, bold_cyan, bright_cyan, bright_green, bright_red, bright_yellow, bright_magenta, yellow};
use crate::software;

/// as list — 列出已安装软件
///
/// 9 列：# 名称 版本 来源 类型 分类 状态 路径 所有版本
/// 默认过滤掉系统组件等非软件条目，-a 显示全部。
pub fn run(show_all: bool) -> anyhow::Result<()> {
    let entries = software::read_all_entries()?;
    let installed = software::read_installed()?;
    let (reg_matched, reg_unmatched) = software::scan_registry_installed(&entries);

    // ── 预计算名称集合 ─────────────────────────────
    let reg_matched_names: std::collections::HashSet<String> = reg_matched.keys().cloned().collect();

    // ── 非软件过滤 ─────────────────────────────────
    fn is_non_software(name: &str) -> bool {
        let n = name.to_lowercase();
        // VC++ 运行时
        if n.contains("microsoft visual c++") { return true; }
        // Windows SDK / 系统组件
        if n.contains("windows sdk") || n.contains("windows software development kit") { return true; }
        if n.contains("windows sdk addon") { return true; }
        if n.contains("update for windows") { return true; }
        if n.contains("microsoft update health") { return true; }
        if n.starts_with("vs_") { return true; }
        if n.contains("windows 10 for") && n.contains("based") { return true; }
        false
    }

    // ── 收集行数据 ──────────────────────────────────
    // cells: [名称, 版本, 来源, 类型, 分类, 状态, 路径, 所有版本]
    struct Row {
        cells: Vec<String>,
    }

    let mut rows: Vec<Row> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // 1) as 管理的安装记录
    for (name, rec) in &installed {
        seen.insert(name.clone());
        let entry = entries.get(name);
        let is_reg = reg_matched_names.contains(name);
        let src_label = if is_reg { "注册表+源" } else { "源" };
        let cat = entry.map(|e| e.category.clone().unwrap_or_else(|| "-".to_string())).unwrap_or_else(|| "-".to_string());

        // 从注册表获取该版本的信息
        let reg_info: Option<&software::RegistryInfo> = if is_reg {
            reg_matched.get(name)
                .and_then(|infos| infos.iter().find(|info| {
                    map_registry_version(entry, &info.version).as_deref() == Some(&rec.version)
                }))
        } else {
            None
        };

        let display_version = reg_info.map(|r| r.version.clone()).unwrap_or_else(|| rec.version.clone());
        let status = eval_entry(entry, Some(&rec.version)).1;
        let type_label = version_type_label(entry, &rec.version);
        let path = resolve_path(
            name,
            Some(&display_version),
            reg_info.and_then(|r| r.install_path.as_deref()),
            reg_info.and_then(|r| r.uninstall_string.as_deref()),
        );
        let version_display = colorize_by_status(&display_version, &status);
        let all_vers = all_versions_colored(entry, &[rec.version.clone()]);

        rows.push(Row {
            cells: vec![
                name.clone(),
                version_display,
                src_label.to_string(),
                type_label,
                cat.clone(),
                status,
                path,
                all_vers,
            ],
        });
    }

    // 2) 注册表匹配到的（未在 as 记录中，但 source 中有定义 → 注册表+源）
    for (name, infos) in &reg_matched {
        if seen.contains(name) {
            continue;
        }
        seen.insert(name.clone());
        let entry = entries.get(name);
        let src_label = "注册表+源";
        let cat = entry.map(|e| e.category.clone().unwrap_or_else(|| "-".to_string())).unwrap_or_else(|| "-".to_string());

        // 取最新版本
        let mut sorted_infos: Vec<&software::RegistryInfo> = infos.iter().collect();
        sorted_infos.sort_by(|a, b| parse_version(&b.version).cmp(&parse_version(&a.version)));
        let info = sorted_infos[0];

        let version_key = map_registry_version(entry, &info.version).unwrap_or_default();
        let status = eval_entry(entry, Some(&info.version)).1;
        let type_label = if version_key.is_empty() {
            get_type_label(entry.unwrap())
        } else {
            version_type_label(entry, &version_key)
        };
        let path = resolve_path(name, Some(&info.version), info.install_path.as_deref(), info.uninstall_string.as_deref());
        let version_display = colorize_by_status(&info.version, &status);
        let all_vers = all_versions_colored(entry, &[]);

        rows.push(Row {
            cells: vec![
                name.clone(),
                version_display,
                src_label.to_string(),
                type_label,
                cat.clone(),
                status,
                path,
                all_vers,
            ],
        });
    }

    // 3) 未匹配到 source 的注册表条目（只显示"注册表"）
    for info in &reg_unmatched {
        if !show_all {
            if is_non_software(&info.display_name) {
                continue;
            }
        }
        let path = resolve_path(&info.display_name, Some(&info.version), info.install_path.as_deref(), info.uninstall_string.as_deref());
        let all_vers = all_versions_colored(None, &[]);
        rows.push(Row {
            cells: vec![
                info.display_name.clone(),
                colorize_by_status(&info.version, "-"),
                "注册表".to_string(),
                "-".to_string(),
                "-".to_string(),
                "-".to_string(),
                path,
                all_vers,
            ],
        });
    }

    // 4) -a 模式下：未安装但有源的软件
    if show_all {
        // 预计算：收集所有注册表条目的 display_name（含未匹配的），用于检测重复
        let all_reg_display_names: Vec<String> = {
            let mut names: Vec<String> = reg_matched.values()
                .flat_map(|infos| infos.iter().map(|info| info.display_name.clone()))
                .collect();
            for info in &reg_unmatched {
                names.push(info.display_name.clone());
            }
            names
        };

        for (name, entry) in &entries {
            if seen.contains(name) {
                continue;
            }
            // 如果源条目有 detect 配置，且任意注册表条目的 display_name 与 detect.display_name 重叠，
            // 说明该软件已在注册表中存在（只是因 publisher 等原因未匹配上），不重复显示
            if let Some(ref detect) = entry.detect {
                let det_lower = detect.display_name.to_lowercase();
                if all_reg_display_names.iter().any(|dn| {
                    dn.to_lowercase().contains(&det_lower) || det_lower.contains(&dn.to_lowercase())
                }) {
                    continue;
                }
            }
            let latest = latest_version(entry);
            let ver = latest.as_deref().unwrap_or("-");
            let cat = entry.category.clone().unwrap_or_else(|| "-".to_string());
            let type_label = get_type_label(entry);
            let all_vers = all_versions_colored(Some(entry), &[]);
            rows.push(Row {
                cells: vec![
                    name.clone(),
                    colorize_by_status(ver, "未安装"),
                    "源".to_string(),
                    type_label,
                    cat,
                    "未安装".to_string(),
                    "-".to_string(),
                    all_vers,
                ],
            });
        }
    }

    if rows.is_empty() {
        println!("暂无已安装软件");
        return Ok(());
    }

    // 按名称排序（不区分大小写）
    rows.sort_by(|a, b| a.cells[0].to_lowercase().cmp(&b.cells[0].to_lowercase()));

    // ── 列定义（左→右优先级递减） ──────────────────
    let headers = ["#", "名称", "版本", "来源", "类型", "分类", "状态", "路径", "所有版本"];
    const NCOLS: usize = 9;
    let gap = 1usize;

    // 路径列索引
    const PATH_COL: usize = 7;

    // 计算每列最大显示宽度
    let mut col_widths = [0usize; NCOLS];
    for (i, h) in headers.iter().enumerate() {
        col_widths[i] = h.display_width();
    }
    // 序号列宽度 = 最大行数的位数
    let seq_w = rows.len().to_string().len();
    if seq_w > col_widths[0] {
        col_widths[0] = seq_w;
    }
    // 数据列宽度（基于纯文本，不含 ANSI）
    for row in &rows {
        for (i, val) in row.cells.iter().enumerate() {
            let w = val.display_width();
            if w > col_widths[i + 1] {
                col_widths[i + 1] = w;
            }
        }
    }

    // ── 终端宽度 + 动态隐藏右侧列 ──────────────────
    let tw = terminal_width();

    // 特判：如果最长路径本身比终端还宽，那就不显示路径列
    let mut has_path_col = true;
    if col_widths[PATH_COL] >= tw {
        has_path_col = false;
    }

    let mut effective_ncols = NCOLS;
    if !has_path_col {
        effective_ncols = NCOLS - 1;
    }

    let mut visible = effective_ncols;
    loop {
        let total: usize = col_widths[..visible].iter().sum::<usize>()
            + gap * (visible.saturating_sub(1));
        if total <= tw || visible <= 4 {
            break;
        }
        visible -= 1;
    }

    // ── 渲染 ─────────────────────────────────────────
    // 表头（全部灰色）
    let mut line = String::with_capacity(tw);
    for i in 0..visible {
        let h = gray(headers[i]);
        if i == 0 {
            line.push_str(&rpad(&h, col_widths[i]));
        } else {
            line.push_str(&pad(&h, col_widths[i]));
        }
        if i < visible - 1 {
            line.push(' ');
        }
    }
    println!("{}", line);

    // 分隔线
    line.clear();
    for i in 0..visible {
        line.push_str(&"-".repeat(col_widths[i]));
        if i < visible - 1 {
            line.push(' ');
        }
    }
    println!("{}", line);

    // 数据行
    let mut seq = 0;
    for row in rows.iter() {
        line.clear();

        seq += 1;
        let seq_str = format!("{:0>width$}", seq, width = seq_w);
        line.push_str(&rpad(&gray(&seq_str), col_widths[0]));
        if visible > 1 {
            line.push(' ');
        }

        for di in 1..visible {
            let ci = di - 1; // data column index (0..6)
            let value = &row.cells[ci];
            let colored = if ci == 0 {
                // 名称列：未安装 → 灰色
                let status = row.cells.get(5).map(|s| s.as_str()).unwrap_or("");
                if status == "未安装" {
                    gray(value)
                } else {
                    value.clone()
                }
            } else if ci == 1 || ci == 7 {
                // 版本列/所有版本列：已预着色（或为空），直接返回
                value.clone()
            } else {
                colorize_data(ci, value)
            };
            // 空值不填充额外空格（保持列宽对齐由空格补齐）
            if colored.is_empty() {
                line.push_str(&" ".repeat(col_widths[di]));
            } else {
                line.push_str(&pad(&colored, col_widths[di]));
            }
            if di < visible - 1 {
                line.push(' ');
            }
        }
        println!("{}", line);
    }

    // ── 宽度不足提示 ───────────────────────────────
    if visible < effective_ncols {
        let hidden_cols = effective_ncols - visible;
        let msg = format!(
            "警告：终端宽度仅 {} 列，不足以显示全部列，{} 列内容已隐藏（扩展终端宽度可查看）",
            tw, hidden_cols
        );
        println!("{}", yellow(&msg));
    }

    Ok(())
}

// ── 版本列表组装 ──────────────────────────────────

/// 收集软件的所有版本（源 + 注册表），降序排列。
/// 已安装版本用亮色标注（最新安装版=bold_cyan，其余已安装版=bright_cyan），未安装版=gray。
fn all_versions_colored(
    entry: Option<&software::SoftwareEntry>,
    installed_versions: &[String],
) -> String {
    let mut all_vers: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();

    // 构建 registry_version → source_version_key 的反向映射
    let mut registry_to_source: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();

    if let Some(e) = entry {
        for (sk, ve) in &e.versions {
            if seen.insert(sk.clone()) {
                all_vers.push(sk.clone());
            }
            if let Some(ref rv) = ve.registry_version {
                registry_to_source.insert(rv.as_str(), sk.as_str());
            }
        }
    }

    // 添加注册表版本（不在 source 中的）
    for v in installed_versions {
        if seen.insert(v.clone()) {
            all_vers.push(v.clone());
        }
    }

    if all_vers.is_empty() {
        return "-".to_string();
    }

    // 降序排列
    all_vers.sort_by(|a, b| {
        let a_ver = parse_version(a);
        let b_ver = parse_version(b);
        b_ver.cmp(&a_ver)
    });

    // 将 installed_versions 映射到 source key 集合（用于判断某版本是否已安装）
    let installed_source_keys: std::collections::HashSet<String> = installed_versions.iter().map(|v| {
        registry_to_source.get(v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| v.clone())
    }).collect();

    // 找最新已安装版本
    let latest_installed = installed_versions.iter()
        .max_by(|a, b| parse_version(a).cmp(&parse_version(b)))
        .map(|v| registry_to_source.get(v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| v.clone()));

    let parts: Vec<String> = all_vers.iter().map(|v| {
        let is_latest = latest_installed.as_deref() == Some(v.as_str());
        let is_installed = installed_source_keys.contains(v.as_str());

        if is_latest {
            bold_cyan(v)
        } else if is_installed {
            bright_cyan(v)
        } else {
            gray(v)
        }
    }).collect();

    parts.join(", ")
}

/// 根据状态着色版本号
fn colorize_by_status(version: &str, status: &str) -> String {
    match status {
        "可更新" => bright_green(version),
        "未安装" => bright_red(version),
        _ => gray(version),
    }
}

/// 将注册表原始版本号映射回 source key。
/// 例如 registry_version: "3.14.5150.0" → source key: "3.14.5"
/// 如果映射不到则返回 None。
fn map_registry_version(entry: Option<&software::SoftwareEntry>, reg_version: &str) -> Option<String> {
    entry.and_then(|e| {
        // 1) 精确匹配 registry_version
        if let Some(found) = e.versions.iter().find_map(|(sk, ve)| {
            if ve.registry_version.as_deref() == Some(reg_version) {
                Some(sk.clone())
            } else {
                None
            }
        }) {
            return Some(found);
        }
        // 2) 前缀回退：注册表版本以 source key 开头（如 "3.13.14150.0" 以 "3.13.14" 开头）
        e.versions.keys().find_map(|sk| {
            if reg_version.starts_with(sk.as_str()) {
                Some(sk.clone())
            } else {
                None
            }
        })
    })
}

fn parse_version(v: &str) -> Vec<u32> {
    v.split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse::<u32>().ok())
        .collect()
}

// ── 颜色化 ──────────────────────────────────────────

/// 根据列索引和数据行内容返回着色后的文本
fn colorize_data(col_idx: usize, value: &str) -> String {
    match col_idx {
        1 => { // 版本列 — 已在行构建时预着色
            value.to_string()
        }
        2 => { // 来源列：源=深青色(cyan)，注册表=浅青色(bright_cyan)
            match value {
                "注册表" => bright_cyan(value),
                "注册表+源" => {
                    format!("{}{}{}", bright_cyan("注册表"), gray("+"), cyan("源"))
                }
                "源" => cyan(value),
                _ => value.to_string(),
            }
        }
        3 => { // 类型列
            if value == "-" {
                return gray(value);
            }
            if value == "安装/便携" {
                return format!("{}/{}", bright_yellow("安装"), bright_magenta("便携"));
            }
            match value {
                "安装版" => bright_yellow(value),
                "便携版" => bright_magenta(value),
                _ => value.to_string(),
            }
        }
        4 => { // 分类列
            if value == "-" { gray(value) } else { value.to_string() }
        }
        5 => { // 状态列
            match value {
                "可更新" => bright_green(value),
                "未安装" => bright_red(value),
                _ => gray(value),
            }
        }
        6 => { // 路径列
            if value == "-" || value.is_empty() {
                return gray("-");
            }
            let raw = value.trim().trim_matches('"');
            if raw.starts_with("C:\\Program Files")
                || raw.starts_with("c:\\program files")
                || raw.starts_with("C:\\ProgramData")
                || raw.starts_with("c:\\programdata")
            {
                gray(value)
            } else {
                value.to_string()
            }
        }
        7 => { // 所有版本列（已预着色，直接返回）
            value.to_string()
        }
        _ => value.to_string(),
    }
}

/// 带颜色的左对齐填充（自动处理 ANSI 宽度）
fn pad(s: &str, w: usize) -> String {
    let plain = strip_ansi(s);
    let cw = plain.display_width();
    if cw >= w {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(w - cw))
    }
}

/// 带颜色的右对齐填充
fn rpad(s: &str, w: usize) -> String {
    let plain = strip_ansi(s);
    let cw = plain.display_width();
    if cw >= w {
        s.to_string()
    } else {
        format!("{}{}", " ".repeat(w - cw), s)
    }
}

// ── 辅助函数 ──────────────────────────────────────

/// 解析路径：优先用 InstallLocation，失败则尝试
/// 开始菜单快捷方式，最后尝试从 UninstallString 提取目录。
///
/// 环境变量（如 %ProgramFiles% 等）会被展开。
fn resolve_path(name: &str, version: Option<&str>, install_path: Option<&str>, uninstall_string: Option<&str>) -> String {
    // 1) 尝试 InstallLocation（注册表中的安装目录）
    if let Some(p) = install_path {
        let p = p.trim().trim_matches('"');
        if !p.is_empty() && looks_like_path(p)
            && !p.to_lowercase().contains("package cache")
        {
            let expanded = expand_env(p);
            let cleaned = format_path(&expanded);
            if is_sensible_install_path(&cleaned) {
                return cleaned;
            }
        }
    }
    // 2) 开始菜单快捷方式（比 UninstallString 更准确）
    let sm = resolve_via_start_menu(name, version);
    if sm != "-" {
        return sm;
    }
    // 3) 尝试从 UninstallString 提取目录
    if let Some(us) = uninstall_string {
        let us = us.trim();
        if us.is_empty() { return "-".to_string(); }

        // 3a) Steam 游戏：解析 appid → 读 manifest → 得游戏目录
        if let Some(steam_path) = try_resolve_steam_game(us) {
            let cleaned = format_path(&steam_path);
            if is_sensible_install_path(&cleaned) {
                return cleaned;
            }
        }

        // 3b) 非 msiexec 的常规卸载 → 提取 exe 目录
        if !contains_msiexec(us) {
            // 提取可执行文件路径
            let exe_path = if us.starts_with('"') {
                // 双引号括起来的 → 取引号内
                if let Some(end) = us[1..].find('"') {
                    &us[1..1 + end]
                } else {
                    us
                }
            } else if let Some(exe_idx) = us.to_lowercase().find(".exe") {
                // 无引号 → 用 .exe 定位路径结尾
                &us[..exe_idx + 4]
            } else {
                us.split(' ').next().unwrap_or(us)
            };
            if let Some(parent) = std::path::Path::new(exe_path).parent() {
                let s = format_path(&expand_env(&parent.to_string_lossy()));
                if is_sensible_install_path(&s) {
                    return s;
                }
            }
        }
    }
    "-".to_string()
}


/// 尝试解析 Steam 游戏的真实安装路径。
///
/// UninstallString 格式: `"C:\Program Files (x86)\Steam\steam.exe" uninstall_app <appid>`
/// 对应 manifest: `{SteamPath}\steamapps\appmanifest_{appid}.acf`
/// 从中读取 `"installdir"` → 路径为 `{SteamPath}\steamapps\common\{installdir}`
fn try_resolve_steam_game(us: &str) -> Option<String> {
    let lower = us.to_lowercase();
    if !lower.contains("steam.exe") {
        return None;
    }

    // 提取 Steam 基础路径
    let steam_base = if us.starts_with('"') {
        // "C:\Program Files (x86)\Steam\steam.exe" ...
        if let Some(end) = us[1..].find('"') {
            let p = std::path::Path::new(&us[1..1 + end]);
            // steam.exe 在 Steam 根目录，parent() 就是 Steam 根
            p.parent()?.to_string_lossy().to_string()
        } else {
            return None;
        }
    } else if let Some(exe_idx) = lower.find(".exe") {
        // C:\Program Files (x86)\Steam\steam.exe ...
        let p = std::path::Path::new(&us[..exe_idx + 4]);
        p.parent()?.to_string_lossy().to_string()
    } else {
        return None;
    };

    // 提取 appid（支持两种格式）
    // 1) "C:\...\steam.exe" uninstall_app 322330
    // 2) "C:\...\steam.exe" steam://uninstall/322330
    let appid = us.split_whitespace()
        .filter_map(|t| {
            // 先直接尝试解析为 u64
            if let Ok(id) = t.parse::<u64>() { return Some(id); }
            // 再试 steam://uninstall/{appid} 格式
            let t = t.trim_matches('"');
            if let Some(pos) = t.find("uninstall/") {
                let rest = &t[pos + 10..];
                // 直到非数字字符
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if !digits.is_empty() {
                    return digits.parse::<u64>().ok();
                }
            }
            None
        })
        .next()?;

    // 构造 manifest 路径
    let manifest_path = format!("{}\\steamapps\\appmanifest_{}.acf", steam_base, appid);
    let content = std::fs::read_to_string(&manifest_path).ok()?;

    // 解析 "installdir" 字段
    // ACF 格式: "installdir"  "Don't Starve Together"
    let installdir = content.lines()
        .find(|line| line.trim().to_lowercase().starts_with("\"installdir\""))
        .and_then(|line| {
            let line = line.trim();
            // 双引号分隔: ""..." "...""..."value"
            let parts: Vec<&str> = line.splitn(4, '"').collect();
            if parts.len() >= 4 {
                Some(parts[3].to_string())
            } else {
                None
            }
        })?;

    let game_path = format!("{}\\steamapps\\common\\{}", steam_base, installdir);
    if looks_like_path(&game_path) {
        Some(game_path)
    } else {
        None
    }
}

/// 检查字符串中是否包含 msiexec
fn contains_msiexec(s: &str) -> bool {
    let lower = s.to_lowercase();
    lower.contains("msiexec")
}

/// 通过"开始菜单"快捷方式解析安装路径（回退机制）。
///
/// 单次 PowerShell 调用构建完整开始菜单索引（进程级缓存），
/// 后续查询 O(1) HashMap，零额外 PowerShell 开销。
fn resolve_via_start_menu(name: &str, version: Option<&str>) -> String {
    static INDEX: OnceLock<HashMap<String, Vec<String>>> = OnceLock::new();
    let index = INDEX.get_or_init(build_start_menu_index);

    let lower_name = name.to_lowercase();
    let lower_name_with_space = format!("{} ", lower_name);

    // 拆分名称中字母部分和数字部分：如 "python3.13" → root="python", ver_suffix="3.13"
    // 用于匹配开始菜单中 "Python 3.13 (64-bit)" 这类条目
    let split_idx = lower_name.find(|c: char| c.is_ascii_digit());
    let root_name = split_idx.and_then(|i| {
        let root = &lower_name[..i];
        let suffix = &lower_name[i..];
        if !root.is_empty() && !suffix.is_empty() { Some((root, suffix)) } else { None }
    });

    // 构建版本前缀候选（小写，不含空格）
    // 例如 version="3.14.5150.0" → ["python 3.14", "python 3.14.5150"]
    let mut version_prefixes: Vec<String> = Vec::new();
    if let Some(v) = version {
        let parts: Vec<&str> = v.splitn(3, '.').collect();
        if parts.len() >= 2 {
            version_prefixes.push(format!("{} {}", lower_name, parts[0..2].join(".")));
            // 也尝试 root_name + 版本，如 "python 3.14"
            if let Some((root, _)) = root_name {
                version_prefixes.push(format!("{} {}", root, parts[0..2].join(".")));
            }
        }
        version_prefixes.push(format!("{} {}", lower_name, v));
        if let Some((root, _)) = root_name {
            version_prefixes.push(format!("{} {}", root, v));
        }
    }

    // 命中缓存中的第一个合理路径
    let hit = |targets: &Vec<String>| -> Option<String> {
        for t in targets {
            let cleaned = format_path(t);
            if is_sensible_install_path(&cleaned) { return Some(cleaned); }
        }
        None
    };

    // 1) 精确匹配名称
    if let Some(targets) = index.get(&lower_name) {
        if let Some(p) = hit(targets) { return p; }
    }

    // 2) 版本前缀匹配: 遍历索引键，查找以 "python 3.14" 开头的键
    //    （解决 index key 如 "python 3.14 (64-bit)" 的情况）
    //    多匹配时选最短 key（最接近纯版本名，避免 "manual"、"documentation" 等干扰）
    let matched_by_prefix = version_prefixes.iter().find_map(|prefix| {
        let mut candidates: Vec<(&String, &Vec<String>)> = index.iter()
            .filter(|(key, _)| key.starts_with(prefix.as_str()))
            .collect();
        candidates.sort_by(|a, b| a.0.len().cmp(&b.0.len()));
        for (_, targets) in &candidates {
            if let Some(p) = hit(targets) {
                return Some(p);
            }
        }
        None
    });
    if let Some(p) = matched_by_prefix {
        return p;
    }

    // 3) 以 "name " 开头匹配（含版本二次过滤）
    //    先收集所有匹配的路径，再用 version 做更精确的匹配
    if let Some(ref ver) = version {
        let ver_lower = ver.to_lowercase();
        for (key, targets) in index.iter() {
            if key.starts_with(&lower_name_with_space) && key.contains(&ver_lower) {
                if let Some(p) = hit(targets) { return p; }
            }
        }
        // 也尝试以 root_name 匹配
        if let Some((root, _)) = root_name {
            let root_with_space = format!("{} ", root);
            for (key, targets) in index.iter() {
                if key.starts_with(&root_with_space) && key.contains(&ver_lower) {
                    if let Some(p) = hit(targets) { return p; }
                }
            }
        }
    }
    for (key, targets) in index.iter() {
        if key.starts_with(&lower_name_with_space) {
            if let Some(p) = hit(targets) { return p; }
        }
    }
    // 也尝试以 root_name 开头匹配
    if let Some((root, suffix)) = root_name {
        let root_with_space = format!("{} ", root);
        for (key, targets) in index.iter() {
            if key.starts_with(&root_with_space) {
                // 含版本后缀过滤：如 "python3.13" → 只匹配含 ".13" 的条目
                let version_match = if suffix.len() >= 3 {
                    key.contains(suffix)
                } else {
                    true
                };
                if version_match {
                    if let Some(p) = hit(targets) { return p; }
                }
            }
        }
    }

    // 4) 以 "name" 开头（不含空格，如 pygame → ）
    for (key, targets) in index.iter() {
        if key != &lower_name && key.starts_with(&lower_name) {
            if let Some(p) = hit(targets) { return p; }
        }
    }

    "-".to_string()
}

/// 单次 PowerShell 调用扫描开始菜单全部 .lnk，构建 name→[target_dir] 索引。
///
/// 结果缓存到磁盘文件 %TEMP%\.aspkg_start_menu_cache，1 小时内复用。
fn build_start_menu_index() -> HashMap<String, Vec<String>> {
    let cache_path = std::env::temp_dir().join(".aspkg_start_menu_cache");
    let mut map: HashMap<String, Vec<String>> = HashMap::new();

    // 尝试加载磁盘缓存（1 小时内有效）
    if let Ok(meta) = std::fs::metadata(&cache_path) {
        if let Ok(modified) = meta.modified() {
            if let Ok(dur) = modified.elapsed() {
                if dur.as_secs() < 3600 {
                    // 缓存未过期 → 读取
                    if let Ok(content) = std::fs::read_to_string(&cache_path) {
                        for line in content.lines() {
                            let line = line.trim();
                            if line.is_empty() { continue; }
                            if let Some(pipe) = line.find('|') {
                                let lnk_name = line[..pipe].to_string();
                                let target_dir = line[pipe + 1..].trim().to_string();
                                if lnk_name.is_empty() || target_dir.is_empty() || !looks_like_path(&target_dir) {
                                    continue;
                                }
                                map.entry(lnk_name).or_default().push(target_dir);
                            }
                        }
                        // 只要成功解析出数据就返回
                        if !map.is_empty() { return map; }
                    }
                }
            }
        }
    }

    // 缓存缺失/过期/损坏 → 重新运行 PowerShell
    let script = r#"
$ws = New-Object -ComObject WScript.Shell
$output = @()
$dirs = @(
    'C:\ProgramData\Microsoft\Windows\Start Menu\Programs',
    [Environment]::GetFolderPath('Programs')
)
foreach ($dir in $dirs) {
    if (-not (Test-Path $dir)) { continue }
    Get-ChildItem -Path $dir -Recurse -Filter '*.lnk' -ErrorAction SilentlyContinue | ForEach-Object {
        try {
            $sc = $ws.CreateShortcut($_.FullName)
            $base = $_.BaseName.ToLower()
            $target = $sc.TargetPath
            if ($target -and (Test-Path $target)) {
                $parent = [System.IO.Path]::GetDirectoryName($target)
                if ($parent) { $output += "$base|$parent" }
            }
            [System.Runtime.InteropServices.Marshal]::ReleaseComObject($sc) | Out-Null
        } catch {}
    }
}
[System.Runtime.InteropServices.Marshal]::ReleaseComObject($ws) | Out-Null
$output -join "`n"
"#;

    map.clear();
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output()
        .ok()
        .and_then(|o| if o.status.success() { Some(o.stdout) } else { None });

    let Some(out) = output else { return map };
    let text = String::from_utf8_lossy(&out);

    // 写入磁盘缓存
    let _ = std::fs::write(&cache_path, text.as_ref());

    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() { continue; }
        if let Some(pipe) = line.find('|') {
            let lnk_name = line[..pipe].to_string();
            let target_dir = line[pipe + 1..].trim().to_string();
            if lnk_name.is_empty() || target_dir.is_empty() || !looks_like_path(&target_dir) {
                continue;
            }
            map.entry(lnk_name).or_default().push(target_dir);
        }
    }

    map
}

/// 检查路径是否看起来像一个合法的 Windows 路径
fn looks_like_path(s: &str) -> bool {
    if s.is_empty() { return false; }
    // 含中括号、花括号等非路径字符 → 无效
    if s.contains('[') || s.contains(']') || s.contains('{') || s.contains('}') { return false; }
    // 至少包含一个反斜杠或正斜杠
    s.contains('\\') || s.contains('/')
}

/// 检查路径是否是一个"合理的"安装路径（拒绝驱动器根目录、系统目录等）
fn is_sensible_install_path(s: &str) -> bool {
    if s.is_empty() { return false; }
    // 去掉尾随斜杠后再判断
    let s = s.trim_end_matches('\\').trim_end_matches('/');

    // 拒绝驱动器根目录: C:  或  C:\
    if s.len() <= 3 && s.ends_with(':') { return false; }
    if s.len() == 3 && s.as_bytes().get(1) == Some(&b':') && s.as_bytes().get(2) == Some(&b'\\') { return false; }

    // 拒绝纯驱动器字母: C:\ → 变成 C: 已经上面拦截了

    // 拒绝 Windows 系统目录
    if s.eq_ignore_ascii_case("C:\\Windows") { return false; }
    if s.eq_ignore_ascii_case("C:\\Windows\\System32") { return false; }
    if s.eq_ignore_ascii_case("C:\\Windows\\SysWOW64") { return false; }
    if s.eq_ignore_ascii_case("C:\\Windows\\System") { return false; }

    // 拒绝 MSI 包缓存目录（不是实际安装位置）
    if s.to_lowercase().contains("package cache") { return false; }

    true
}

/// 展开常见的 Windows 环境变量
fn expand_env(s: &str) -> String {
    let s = s
        .replace("%ProgramFiles%", "C:\\Program Files")
        .replace("%ProgramFiles(x86)%", "C:\\Program Files (x86)")
        .replace("%CommonProgramFiles%", "C:\\Program Files\\Common Files")
        .replace("%CommonProgramFiles(x86)%", "C:\\Program Files (x86)\\Common Files")
        .replace("%SystemRoot%", "C:\\Windows")
        .replace("%SystemDrive%", "C:");
    // 运行时动态展开
    let s = resolve_runtime_env(s, "APPDATA");
    let s = resolve_runtime_env(s, "LOCALAPPDATA");
    let s = resolve_runtime_env(s, "USERPROFILE");
    let s = resolve_runtime_env(s, "ProgramFiles");
    let s = resolve_runtime_env(s, "ProgramFiles(x86)");
    resolve_runtime_env(s, "ProgramData")
}

fn resolve_runtime_env(s: String, var: &str) -> String {
    let pattern = format!("%{}%", var);
    if s.contains(&pattern) {
        if let Ok(val) = std::env::var(var) {
            return s.replace(&pattern, &val);
        }
    }
    s
}

/// 清理路径：去掉引号、去掉尾随斜杠/反斜杠、去掉分号
fn format_path(path: &str) -> String {
    let s = path.trim().trim_matches('"').trim_end_matches('\\').trim_end_matches('/').trim_end_matches(';').trim().to_string();
    if s.is_empty() { String::new() } else { s }
}

/// 评估分类和状态
fn eval_entry(
    entry: Option<&software::SoftwareEntry>,
    version: Option<&str>,
) -> (String, String) {
    match entry {
        Some(e) => {
            let cat = e.category.clone().unwrap_or_else(|| "-".to_string());
            // 如果 version 是某个 registry_version，映射回 source key 做比较
            let normalized = version.and_then(|v| {
                e.versions.iter().find_map(|(sk, ve)| {
                    ve.registry_version.as_deref().and_then(|rv| {
                        if rv == v { Some(sk.as_str()) } else { None }
                    })
                })
                .or(Some(v))
            });
            let status = match (normalized, latest_version(e)) {
                (Some(v), Some(latest)) if v == latest => "最新".to_string(),
                (Some(_), Some(_)) => "可更新".to_string(),
                _ => "-".to_string(),
            };
            (cat, status)
        }
        None => ("-".to_string(), "-".to_string()),
    }
}

/// 获取 source entry 中的"最新"版本（取排序后的最后一个）
fn latest_version(entry: &software::SoftwareEntry) -> Option<String> {
    if entry.versions.is_empty() {
        return None;
    }
    let mut vers: Vec<&String> = entry.versions.keys().collect();
    vers.sort();
    vers.last().map(|s| (*s).clone())
}

/// 根据 source entry 显示可用的安装类型
fn get_type_label(entry: &software::SoftwareEntry) -> String {
    let mut has_i = false;
    let mut has_p = false;
    for v in entry.versions.values() {
        for k in v.urls.keys() {
            match k.as_str() {
                "installer" => has_i = true,
                "portable" => has_p = true,
                _ => {}
            }
        }
    }
    match (has_i, has_p) {
        (true, true) => "安装/便携",
        (true, false) => "安装版",
        (false, true) => "便携版",
        (false, false) => "-",
    }
    .to_string()
}

/// 获取单个版本的安装类型标签（仅看该版本的 urls）
fn version_type_label(entry: Option<&software::SoftwareEntry>, version_key: &str) -> String {
    let ve = entry.and_then(|e| e.versions.get(version_key));
    match ve {
        Some(v) => {
            let has_i = v.urls.contains_key("installer");
            let has_p = v.urls.contains_key("portable");
            match (has_i, has_p) {
                (true, true) => "安装/便携",
                (true, false) => "安装版",
                (false, true) => "便携版",
                (false, false) => "-",
            }
        }
        None => "-",
    }
    .to_string()
}

// ── 终端宽度检测 ──────────────────────────────────

fn terminal_width() -> usize {
    #[cfg(windows)]
    {
        #[repr(C)]
        struct COORD { x: i16, y: i16 }
        #[repr(C)]
        struct SMALL_RECT { left: i16, top: i16, right: i16, bottom: i16 }
        #[repr(C)]
        struct CONSOLE_SCREEN_BUFFER_INFO {
            dw_size: COORD,
            dw_cursor_position: COORD,
            w_attributes: u16,
            sr_window: SMALL_RECT,
            dw_maximum_window_size: COORD,
        }
        unsafe extern "system" {
            fn GetStdHandle(nStdHandle: u32) -> isize;
            fn GetConsoleScreenBufferInfo(
                hConsoleOutput: isize,
                lpConsoleScreenBufferInfo: *mut CONSOLE_SCREEN_BUFFER_INFO,
            ) -> i32;
        }
        unsafe {
            const STD_OUTPUT_HANDLE: u32 = 0xFFFFFFF5u32;
            let handle = GetStdHandle(STD_OUTPUT_HANDLE);
            if handle == -1 || handle == 0 {
                return 80;
            }
            let mut info: CONSOLE_SCREEN_BUFFER_INFO = std::mem::zeroed();
            if GetConsoleScreenBufferInfo(handle, &mut info) != 0 {
                (info.sr_window.right - info.sr_window.left + 1) as usize
            } else {
                80
            }
        }
    }
    #[cfg(not(windows))]
    {
        std::env::var("COLUMNS").ok()
            .and_then(|v| v.parse().ok())
            .or_else(|| term_size::terminal_size().map(|(w, _)| w.0 as usize))
            .unwrap_or(80)
    }
}
