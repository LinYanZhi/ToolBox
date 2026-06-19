use std::fs;
use std::path::Path;

fn main() {
    let root = Path::new("C:\\Users\\LinYanZhi\\Code\\ToolBox\\aminos-source");
    format_dir(root);
    println!("done");
}

fn format_dir(dir: &Path) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                format_dir(&path);
            } else if path.extension().map_or(false, |e| e == "json") {
                format_file(&path);
            }
        }
    }
}

fn format_file(path: &Path) {
    let content = match fs::read_to_string(path) {
        Ok(c) => c.trim_start_matches('\u{FEFF}').to_string(),
        Err(e) => {
            eprintln!("  ERR read {}: {}", path.display(), e);
            return;
        }
    };
    // Use serde_json for reliable parsing, then convert 2-space to 4-space
    let value: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("  ERR parse {}: {}", path.display(), e);
            return;
        }
    };
    let pretty2 = serde_json::to_string_pretty(&value).unwrap();
    let result = indent_2to4(&pretty2);
    if let Err(e) = fs::write(path, &result) {
        eprintln!("  ERR write {}: {}", path.display(), e);
    } else {
        println!("  OK {}", path.display());
    }
}

fn indent_2to4(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + s.len() / 2);
    for line in s.lines() {
        if !out.is_empty() {
            out.push('\n');
        }
        let trimmed = line.trim_start();
        let leading = line.len() - trimmed.len();
        for _ in 0..leading / 2 {
            out.push_str("    ");
        }
        out.push_str(trimmed);
    }
    out.push('\n');
    out
}
