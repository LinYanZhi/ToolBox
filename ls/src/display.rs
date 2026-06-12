use crate::config::ColorConfig;
use crate::scanner::ItemInfo;

/// 带颜色的文本
pub fn colored(text: &str, code: &str) -> String {
    format!("\x1b[{}m{}\x1b[0m", code, text)
}

/// 格式化时间戳（Unix 秒 → YYYY-MM-DD HH:mm:ss）
pub fn format_timestamp(secs: i64) -> String {
    const SECS_PER_DAY: i64 = 86400;
    const DAYS_PER_YEAR: [i64; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    fn is_leap(year: i64) -> bool {
        (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
    }

    let mut days = secs / SECS_PER_DAY;
    let time_secs = secs % SECS_PER_DAY;
    if time_secs < 0 {
        days -= 1;
    }

    let hour = ((time_secs / 3600 + 24) % 24) as u32;
    let min = ((time_secs / 60 + 60) % 60) as u32;
    let sec = ((time_secs + 60) % 60) as u32;

    let mut year = 1970i64;

    if days >= 0 {
        let mut d = days;
        loop {
            let ydays = if is_leap(year) { 366 } else { 365 };
            if d < ydays {
                break;
            }
            d -= ydays;
            year += 1;
        }
        let leap = is_leap(year);
        let mut month = 0u32;
        for (i, &mdays) in DAYS_PER_YEAR.iter().enumerate() {
            let m = mdays + if i == 1 && leap { 1 } else { 0 };
            if d < m {
                month = i as u32;
                break;
            }
            d -= m;
        }
        let day = (d + 1) as u32;
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            year, month + 1, day, hour, min, sec
        )
    } else {
        let mut d = -days - 1;
        loop {
            year -= 1;
            let ydays = if is_leap(year) { 366 } else { 365 };
            if d < ydays {
                break;
            }
            d -= ydays;
        }
        let leap = is_leap(year);
        let mut month = 11i32;
        for i in (0..12).rev() {
            let m = DAYS_PER_YEAR[i] + if i == 1 && leap { 1 } else { 0 };
            if d < m {
                month = i as i32;
                break;
            }
            d -= m;
        }
        let day = (DAYS_PER_YEAR[month as usize] - d) as u32;
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            year, month as u32 + 1, day, hour, min, sec
        )
    }
}

/// 格式化文件大小
pub fn format_size(size: u64) -> String {
    if size < 1024 {
        format!("{}B", size)
    } else if size < 1024 * 1024 {
        format!("{:.1}KB", size as f64 / 1024.0)
    } else if size < 1024 * 1024 * 1024 {
        format!("{:.1}MB", size as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1}GB", size as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// 计算显示宽度（中文字符占2）
pub fn display_width(s: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(s)
}

/// 文件大小着色结果
struct SizeStyle {
    color: String,
    mode: String,
}

/// 查找大小对应的颜色规则
fn get_size_style(size: u64, rules: &[crate::config::SizeRule]) -> SizeStyle {
    for rule in rules {
        if rule.max == -1 || (size as i64) < rule.max {
            return SizeStyle {
                color: rule.color.clone(),
                mode: rule.mode.clone(),
            };
        }
    }
    SizeStyle {
        color: "37".into(),
        mode: "full".into(),
    }
}

/// 显示格式化器
pub struct Formatter {
    pub config: ColorConfig,
    pub no_color: bool,
}

impl Formatter {
    pub fn new(config: ColorConfig, no_color: bool) -> Self {
        if !no_color {
            ColorConfig::init();
        }
        Self { config, no_color }
    }

    /// 打印类型标记 <dir> / <file>
    pub fn print_type_marker(&self, marker: &str) -> String {
        let text = format!("{:>7} ", marker);
        if self.no_color {
            text
        } else {
            colored(&text, "90")
        }
    }

    /// 打印时间戳
    pub fn print_timestamp(&self, secs: i64) -> String {
        let text = format!("{} ", format_timestamp(secs));
        if self.no_color {
            text
        } else {
            colored(&text, "90")
        }
    }

    /// 打印文件名（带颜色）
    pub fn print_file_name(&self, item: &ItemInfo, right_align: bool, max_width: usize) -> String {
        let name = &item.name;
        let width = display_width(name);
        let padding = if right_align && width < max_width {
            max_width - width
        } else {
            0
        };
        let pad_str = " ".repeat(padding);

        if self.no_color {
            return format!("{}{} ", pad_str, name);
        }

        let color = self.get_item_color(item);
        match color {
            Some(code) => {
                if item.is_dir || item.link_type != crate::links::LinkType::File {
                    format!("{}{} ", pad_str, colored(name, code))
                } else {
                    // 仅后缀着色：文件名用白色，后缀用扩展名颜色
                    let ext = std::path::Path::new(name)
                        .extension()
                        .map(|e| format!(".{}", e.to_string_lossy()))
                        .unwrap_or_default();
                    let name_part = name.strip_suffix(&ext).unwrap_or(name);
                    if ext.is_empty() {
                        format!("{}{} ", pad_str, colored(name, "97"))
                    } else {
                        let ext_color = self.config.ext_color(&ext);
                        match ext_color {
                            Some(ec) if ec != code => {
                                format!("{}{}{} ", pad_str, colored(name_part, "97"), colored(&ext, ec))
                            }
                            _ => format!("{}{} ", pad_str, colored(name, code)),
                        }
                    }
                }
            }
            None => format!("{}{} ", pad_str, name),
        }
    }

    /// 获取项目颜色
    pub fn get_item_color(&self, item: &ItemInfo) -> Option<&str> {
        if item.is_dir {
            match item.link_type {
                crate::links::LinkType::Symlink | crate::links::LinkType::Junction => {
                    Some(self.config.dir_link_color.as_str())
                }
                _ => Some(self.config.dir_color.as_str()),
            }
        } else {
            let ext = std::path::Path::new(&item.name)
                .extension()
                .map(|e| format!(".{}", e.to_string_lossy()))
                .unwrap_or_default();
            self.config.ext_color(&ext).or(Some("97"))
        }
    }

    /// 打印链接箭头
    pub fn print_link_arrow(&self, is_directory: bool) -> String {
        let (arrow, color) = if is_directory {
            (&self.config.dir_link_arrow, &self.config.dir_link_arrow_color)
        } else {
            (&self.config.file_link_arrow, &self.config.file_link_arrow_color)
        };
        let text = format!(" {} ", arrow);
        if self.no_color {
            text
        } else {
            colored(&text, &color)
        }
    }

    /// 打印链接目标
    pub fn print_link_target(&self, target: &str, is_file_link: bool) -> String {
        let target = target.replace("\\\\?\\", "").replace("\\??\\", "");

        if self.no_color {
            return target;
        }

        let path = std::path::Path::new(&target);
        let parent = path.parent().and_then(|p| p.to_str());
        let file_name = path.file_name().and_then(|n| n.to_str());

        let mut result = String::new();

        if let Some(parent) = parent {
            if !parent.is_empty() && parent != "." {
                result.push_str(&colored(parent, &self.config.dir_link_path_color));
                result.push('\\');
            }
        }

        if let Some(name) = file_name {
            let is_target_dir = target.ends_with('\\') || target.ends_with('/')
                || std::path::Path::new(&target).is_dir();

            let color = if is_target_dir {
                if is_file_link {
                    &self.config.file_link_dir_color
                } else {
                    &self.config.dir_link_path_basename_color
                }
            } else {
                "97"
            };

            result.push_str(&colored(name, color));
        } else {
            result.push_str(&target);
        }

        result
    }

    /// 打印文件大小
    pub fn print_size(&self, item: &ItemInfo, max_width: usize) -> String {
        let size_str = format_size(item.size);
        let padding = if max_width > size_str.len() {
            max_width - size_str.len()
        } else {
            0
        };
        let pad_str = " ".repeat(padding);

        if self.no_color {
            return format!("{}{} ", pad_str, size_str);
        }

        let style = get_size_style(item.size, &self.config.size_rules);
        let colored_size = if style.mode == "full" {
            colored(&size_str, &style.color)
        } else {
            let split_idx = size_str.len()
                - size_str.chars().rev().position(|c| c.is_alphabetic()).unwrap_or(0);
            let (num_part, unit_part) = size_str.split_at(split_idx);
            format!("{}{}", num_part, colored(unit_part, &style.color))
        };
        format!("{}{} ", pad_str, colored_size)
    }
}
