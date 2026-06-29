use color::*;
use terminal_size::{Width, terminal_size};
use crate::software;

pub fn run(all: bool) -> anyhow::Result<()> {
    let entries = software::all_entries()?;

    let term_width = terminal_size().map(|(Width(w), _)| w as usize).unwrap_or(80);

    let mut sorted: Vec<(&String, &software::SoftwareEntry)> = entries.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(b.0));

    let mut max_name_w = "名称".display_width();
    let mut max_desc_w = "说明".display_width();
    let mut max_ver_w = "版本".display_width();
    let mut max_status_w = "状态".display_width();

    struct Line {
        name: String,
        desc: String,
        version: String,
        status: String,
        install_path: String,
    }

    let mut lines: Vec<Line> = Vec::new();

    for (name, entry) in &sorted {
        let nw = name.display_width();
        let dw = entry.desc.display_width();
        if nw > max_name_w { max_name_w = nw; }
        if dw > max_desc_w { max_desc_w = dw; }

        let mut versions: Vec<&str> = entry.versions.keys().map(|s| s.as_str()).collect();
        versions.sort_by(|a, b| cmp_versions(b, a));
        let version = versions.first().unwrap_or(&"").to_string();
        let vw = version.display_width();
        if vw > max_ver_w { max_ver_w = vw; }

        let (status, install_path) = get_install_status(entry);
        let sw = status.display_width();
        if sw > max_status_w { max_status_w = sw; }

        lines.push(Line {
            name: name.to_string(),
            desc: truncate_display(&entry.desc, 20),
            version,
            status,
            install_path,
        });
    }

    let gap = 2;

    println!("{}{}{}{}{}{}{}",
        pad_left("名称", max_name_w),
        " ".repeat(gap),
        pad_left("说明", max_desc_w),
        " ".repeat(gap),
        pad_left("版本", max_ver_w),
        " ".repeat(gap),
        "状态",
    );

    println!("{}{}{}{}{}{}{}",
        "-".repeat(max_name_w),
        " ".repeat(gap),
        "-".repeat(max_desc_w),
        " ".repeat(gap),
        "-".repeat(max_ver_w),
        " ".repeat(gap),
        "-".repeat(max_status_w),
    );

    for line in &lines {
        let name_display = truncate_display(&line.name, max_name_w);
        println!("{}{}{}{}{}{}{}",
            pad_left(name_display, max_name_w),
            " ".repeat(gap),
            pad_left(&line.desc, max_desc_w),
            " ".repeat(gap),
            pad_left(&line.version, max_ver_w),
            " ".repeat(gap),
            &line.status,
        );

        if all && !line.install_path.is_empty() {
            println!("{}{}{}",
                " ".repeat(max_name_w + gap + max_desc_w + gap + max_ver_w + gap),
                gray("  → "),
                gray(&line.install_path),
            );
        }
    }

    println!();
    println!("共 {} 个", sorted.len());
    println!();

    Ok(())
}

fn get_install_status(entry: &software::SoftwareEntry) -> (String, String) {
    if let Some(detect) = &entry.detect {
        if let Some(info) = software::detect_from_registry(detect) {
            (bold_green("已安装"), info.install_path.unwrap_or_default())
        } else {
            (gray("未安装"), String::new())
        }
    } else {
        (gray("未安装"), String::new())
    }
}

fn cmp_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let va: Vec<i64> = a.split('.').filter_map(|s| s.parse().ok()).collect();
    let vb: Vec<i64> = b.split('.').filter_map(|s| s.parse().ok()).collect();
    let max_len = va.len().max(vb.len());
    for i in 0..max_len {
        let na = va.get(i).copied().unwrap_or(0);
        let nb = vb.get(i).copied().unwrap_or(0);
        if na != nb {
            return na.cmp(&nb);
        }
    }
    if a == "latest" && b != "latest" { return std::cmp::Ordering::Greater; }
    if b == "latest" && a != "latest" { return std::cmp::Ordering::Less; }
    std::cmp::Ordering::Equal
}

fn truncate_display(s: &str, max: usize) -> String {
    let w = s.display_width();
    if w <= max {
        s.to_string()
    } else {
        let mut result = String::new();
        let mut cur = 0usize;
        for c in s.chars() {
            let cw = c.to_string().display_width();
            if cur + cw > max.saturating_sub(1) {
                result.push('…');
                break;
            }
            result.push(c);
            cur += cw;
        }
        result
    }
}

fn pad_left(s: &str, w: usize) -> String {
    let cw = s.display_width();
    if cw >= w { s.to_string() } else { format!("{}{}", " ".repeat(w - cw), s) }
}