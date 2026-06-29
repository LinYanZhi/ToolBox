//! 颜色主题 — 帮助输出的颜色可全局配置

/// ANSI 颜色码（与 color crate 的 Style 配合使用）
#[derive(Debug, Clone)]
pub struct Theme {
    /// 标题（命令名）
    pub title: &'static str,
    /// 小节标题（用法/子命令/选项）
    pub heading: &'static str,
    /// 命令名 / 选项名（cyan）
    pub name: &'static str,
    /// 描述文字（gray）
    pub desc: &'static str,
    /// 错误文字（red）
    pub error: &'static str,
    /// 提示文字（yellow）
    pub hint: &'static str,
    /// 版本号（green）
    pub version: &'static str,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            title: "cyan",
            heading: "yellow",
            name: "cyan",
            desc: "gray",
            error: "red",
            hint: "yellow",
            version: "green",
        }
    }
}

impl Theme {
    /// 把颜色名转为 ANSI 码
    fn code(name: &str) -> u8 {
        match name {
            "black" => 30,
            "red" => 31,
            "green" => 32,
            "yellow" => 33,
            "blue" => 34,
            "magenta" | "purple" => 35,
            "cyan" => 36,
            "white" => 37,
            "gray" => 90,
            "lightred" => 91,
            "lightgreen" => 92,
            "lightyellow" => 93,
            "lightblue" => 94,
            "lightmagenta" | "lightpurple" => 95,
            "lightcyan" => 96,
            "brightwhite" => 97,
            _ => 0,
        }
    }

    /// 用指定主题颜色给文本上色
    pub fn paint(&self, text: &str, color_key: &str) -> String {
        let name = match color_key {
            "title" => self.title,
            "heading" => self.heading,
            "name" => self.name,
            "desc" => self.desc,
            "error" => self.error,
            "hint" => self.hint,
            "version" => self.version,
            _ => color_key,
        };
        let code = Self::code(name);
        if code == 0 {
            text.to_string()
        } else {
            color::Style::new(code).paint(text)
        }
    }
}
