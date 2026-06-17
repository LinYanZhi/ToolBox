use std::collections::HashMap;

use color::{self, DisplayWidth, pad_left as pad};

// ══════════════════════════════════════════════════════════
// 参数类型
// ══════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub enum ArgType {
    Flag,
    String,
    Strings,
}

#[derive(Debug, Clone)]
pub struct ArgDef {
    pub short: Option<char>,
    pub long: &'static str,
    pub arg_type: ArgType,
    pub help: &'static str,
    pub positional: bool,
    pub required: bool,
}

impl ArgDef {
    pub fn positional(name: &'static str, required: bool, help: &'static str) -> Self {
        Self { short: None, long: name, arg_type: ArgType::Strings, help, positional: true, required }
    }
    pub fn flag(short: Option<char>, long: &'static str, help: &'static str) -> Self {
        Self { short, long, arg_type: ArgType::Flag, help, positional: false, required: false }
    }
    pub fn string(short: Option<char>, long: &'static str, help: &'static str) -> Self {
        Self { short, long, arg_type: ArgType::String, help, positional: false, required: false }
    }

    fn label(&self) -> String {
        if self.positional {
            let n = self.long.to_uppercase();
            if self.required { format!("<{}>", n) } else { format!("[{}]", n) }
        } else {
            let mut parts = vec![];
            if let Some(s) = self.short { parts.push(format!("-{}", s)); }
            if self.is_long_only() {
                parts.push(format!("--{}", self.long));
            } else {
                let tag = match self.arg_type {
                    ArgType::Flag => String::new(),
                    ArgType::String | ArgType::Strings => format!(" <{}>", self.long.to_uppercase()),
                };
                parts.push(format!("--{}{}", self.long, tag));
            }
            parts.join(", ")
        }
    }

    fn label_short(&self) -> String {
        if self.positional {
            let n = self.long.to_uppercase();
            if self.required { format!("<{}>", n) } else { format!("[{}]", n) }
        } else if self.arg_type.is_flag() {
            format!("[--{}]", self.long)
        } else {
            format!("[--{} <{}>]", self.long, self.long.to_uppercase())
        }
    }

    fn is_long_only(&self) -> bool {
        self.short.is_none()
    }
}

impl ArgType {
    fn is_flag(&self) -> bool { matches!(self, ArgType::Flag) }
}

// ══════════════════════════════════════════════════════════
// 命令定义
// ══════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct CommandDef {
    pub name: &'static str,
    pub description: &'static str,
    pub args: Vec<ArgDef>,
    pub subcommands: Vec<CommandDef>,
    pub category: &'static str,
    /// 为 true 时，无参数也会执行（如 `as list`），不显示帮助
    pub run_on_empty: bool,
}

impl CommandDef {
    pub fn new(name: &'static str, description: &'static str) -> Self {
        Self { name, description, args: vec![], subcommands: vec![], category: "", run_on_empty: false }
    }
    pub fn arg(mut self, arg: ArgDef) -> Self { self.args.push(arg); self }
    pub fn subcommand(mut self, cmd: CommandDef) -> Self { self.subcommands.push(cmd); self }
    pub fn category(mut self, cat: &'static str) -> Self { self.category = cat; self }
    pub fn run_on_empty(mut self) -> Self { self.run_on_empty = true; self }

    pub fn has_subcommands(&self) -> bool { !self.subcommands.is_empty() }
    pub fn has_positional(&self) -> bool { self.args.iter().any(|a| a.positional) }
    pub fn has_options(&self) -> bool { self.args.iter().any(|a| !a.positional) }

    fn usage_str(&self) -> String {
        let mut parts = vec!["as".to_string(), self.name.to_string()];
        if self.has_subcommands() { parts.push("<子命令>".to_string()); }
        for a in &self.args { if a.positional { parts.push(a.label_short()); } }
        if self.has_options() { parts.push("[选项]".to_string()); }
        parts.join(" ")
    }

    fn has_any_arg_config(&self) -> bool {
        !self.args.is_empty() || self.has_subcommands()
    }
}

// ══════════════════════════════════════════════════════════
// 解析结果
// ══════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub enum ArgValue {
    Flag(bool),
    String(String),
    Strings(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct ParsedArgs {
    pub command: String,
    pub subcommand_path: Vec<String>,
    pub values: HashMap<String, ArgValue>,
    pub positional: Vec<String>,
}

/// parse() 返回值：执行 / 显示帮助
pub enum ParseResult<'a> {
    /// 正常执行
    Executed(ParsedArgs, &'a CommandDef),
    /// 需要显示帮助（无参数时自动触发）
    ShowHelp(&'a CommandDef, Vec<String>),
}

impl ParsedArgs {
    pub fn flag(&self, name: &str) -> bool {
        self.values.get(name).map(|v| matches!(v, ArgValue::Flag(true))).unwrap_or(false)
    }
    pub fn get_string(&self, name: &str) -> Option<&str> {
        self.values.get(name).and_then(|v| if let ArgValue::String(s) = v { Some(s.as_str()) } else { None })
    }
    pub fn first(&self) -> Option<&str> { self.positional.first().map(|s| s.as_str()) }
    pub fn all(&self) -> &[String] { &self.positional }
}

// ══════════════════════════════════════════════════════════
// 主题配色
// ══════════════════════════════════════════════════════════

/// 统一主题配色，所有颜色集中在这里定义。
/// 调用 `App::with_theme()` 可全局替换。
pub struct Theme {
    /// 主标题色（应用名、命令全名）
    pub title: fn(&str) -> String,
    /// 描述文字色
    pub desc: fn(&str) -> String,
    /// 区块标题色（"用法:"、"命令:" 等）
    pub header: fn(&str) -> String,
    /// 代码语法色（命令名、选项名）
    pub code: fn(&str) -> String,
    /// 错误消息色
    pub error: fn(&str) -> String,
    /// 提示文本色
    pub hint: fn(&str) -> String,
    /// 辅助灰色文本色
    pub muted: fn(&str) -> String,
    /// 示例分组标题色
    pub example_group: fn(&str) -> String,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            title: |s| color::bold_cyan(s),
            desc: |s| color::green(s),
            header: |s| color::bold_yellow(s),
            code: |s| color::cyan(s),
            error: |s| color::bold_red(s),
            hint: |s| color::yellow(s),
            muted: |s| color::gray(s),
            example_group: |s| color::bold_green(s),
        }
    }
}

// ══════════════════════════════════════════════════════════
// App 主程序
// ══════════════════════════════════════════════════════════

pub struct App {
    pub name: &'static str,
    pub description: &'static str,
    pub version: &'static str,
    pub commands: Vec<CommandDef>,
    pub theme: Theme,
}

impl App {
    pub fn new(name: &'static str, description: &'static str, version: &'static str) -> Self {
        Self { name, description, version, commands: vec![], theme: Theme::default() }
    }
    pub fn command(mut self, cmd: CommandDef) -> Self { self.commands.push(cmd); self }
    /// 更换全局配色主题
    pub fn with_theme(mut self, theme: Theme) -> Self { self.theme = theme; self }

    // ── 查找 ──────────────────────────────────

    pub fn find_command(&self, name: &str) -> Option<&CommandDef> {
        self.commands.iter().find(|c| c.name == name)
    }

    pub fn find_subcommand<'a>(&'a self, parent: &'a CommandDef, name: &str) -> Option<&'a CommandDef> {
        parent.subcommands.iter().find(|c| c.name == name)
    }

    pub fn fuzzy_find(&self, input: &str, max: usize) -> Vec<(&CommandDef, usize)> {
        let mut r: Vec<_> = self.commands.iter()
            .map(|c| (c, levenshtein(input, c.name)))
            .filter(|(_, d)| *d <= max).collect();
        r.sort_by_key(|(_, d)| *d); r
    }

    pub fn fuzzy_find_sub<'a>(&'a self, parent: &'a CommandDef, input: &str, max: usize) -> Vec<(&'a CommandDef, usize)> {
        let mut r: Vec<_> = parent.subcommands.iter()
            .map(|c| (c, levenshtein(input, c.name)))
            .filter(|(_, d)| *d <= max).collect();
        r.sort_by_key(|(_, d)| *d); r
    }

    // ── 解析 ──────────────────────────────────

    /// 解析命令行参数。
    pub fn parse(&self, args: &[String]) -> Result<ParseResult<'_>, CliError> {
        if args.is_empty() { return Err(CliError::NoCommand); }

        let cmd_name = &args[0];
        let cmd = self.find_command(cmd_name).ok_or_else(|| {
            let s = self.fuzzy_find(cmd_name, 3);
            CliError::UnknownCommand {
                input: cmd_name.clone(),
                suggestions: s.into_iter().map(|(c, _)| c.name.to_string()).collect(),
            }
        })?;

        let remaining = &args[1..];

        if cmd.has_subcommands() {
            if remaining.is_empty() {
                return Ok(ParseResult::ShowHelp(cmd, vec![cmd.name.to_string()]));
            }
            // as tool -h / --help
            if remaining[0] == "-h" || remaining[0] == "--help" {
                return Ok(ParseResult::ShowHelp(cmd, vec![cmd.name.to_string()]));
            }
            let sub_name = &remaining[0];
            let sub = self.find_subcommand(cmd, sub_name).ok_or_else(|| {
                let s = self.fuzzy_find_sub(cmd, sub_name, 3);
                CliError::UnknownCommand {
                    input: format!("{} {}", cmd.name, sub_name),
                    suggestions: s.into_iter().map(|(c, _)| format!("{} {}", cmd.name, c.name)).collect(),
                }
            })?;

            let mut parsed = ParsedArgs {
                command: sub.name.to_string(),
                subcommand_path: vec![cmd.name.to_string()],
                values: HashMap::new(),
                positional: vec![],
            };

            let sub_remaining = &remaining[1..];

            // -h/--help → ShowHelp
            if sub_remaining.iter().any(|a| a == "-h" || a == "--help") {
                return Ok(ParseResult::ShowHelp(sub, vec![cmd.name.to_string(), sub.name.to_string()]));
            }

            if sub_remaining.is_empty() && sub.has_any_arg_config() && !sub.run_on_empty {
                return Ok(ParseResult::ShowHelp(sub, vec![cmd.name.to_string(), sub.name.to_string()]));
            }

            parse_args(sub, sub_remaining, &mut parsed)?;
            return Ok(ParseResult::Executed(parsed, sub));
        }

        // 根命令：-h/--help → ShowHelp
        if remaining.iter().any(|a| a == "-h" || a == "--help") {
            return Ok(ParseResult::ShowHelp(cmd, vec![cmd.name.to_string()]));
        }

        if remaining.is_empty() && cmd.has_any_arg_config() && !cmd.run_on_empty {
            return Ok(ParseResult::ShowHelp(cmd, vec![cmd.name.to_string()]));
        }

        let mut parsed = ParsedArgs {
            command: cmd.name.to_string(),
            subcommand_path: vec![],
            values: HashMap::new(),
            positional: vec![],
        };
        parse_args(cmd, remaining, &mut parsed)?;
        Ok(ParseResult::Executed(parsed, cmd))
    }

    // ── 帮助打印 ──────────────────────────────

    /// 打印根级帮助
    pub fn print_root_help(&self) {
        let t = &self.theme;
        let max_w = self.commands.iter().map(|c| c.name.display_width()).max().unwrap_or(10);
        println!();
        println!("  {} — {}", (t.title)(self.name), (t.desc)(self.description));
        println!();
        println!("  {}", (t.header)("用法:"));
        println!("    {} {} {}", (t.code)(self.name), (t.desc)("<命令>"), (t.muted)("[参数]"));
        println!();
        println!("  {}", (t.header)("命令:"));
        for cmd in &self.commands {
            let label = pad(cmd.name, max_w);
            println!("    {}    {}", (t.code)(&label), cmd.description);
        }
        println!();
        println!("  {}", (t.header)("选项:"));
        let example = pad("-e, --example", 18);
        let help = pad("-h, --help", 18);
        let version = pad("-V, --version", 18);
        println!("    {}  {}", (t.code)(&example), "显示所有命令的示例用法");
        println!("    {}  {}", (t.code)(&help), "显示帮助信息");
        println!("    {}  {}", (t.code)(&version), "显示版本信息");
        println!();
        println!("  {}", (t.header)("提示:"));
        println!("    {} 了解更多请使用 {}as <命令>{}", (t.muted)("•"), (t.code)(""), (t.muted)(""));
    }

    /// 打印任意命令/子命令的帮助（统一格式）
    pub fn print_command_help(&self, cmd: &CommandDef, path: &[String]) {
        let t = &self.theme;
        let full = if path.is_empty() {
            cmd.name.to_string()
        } else {
            path.join(" ")
        };
        let heading = format!("as {}", full);
        let usage = cmd.usage_str();

        println!();
        println!("  {} — {}", (t.title)(&heading), (t.desc)(cmd.description));
        println!();
        println!("  {}", (t.header)("用法:"));
        println!("    {}", (t.code)(&usage));

        if cmd.has_subcommands() {
            println!();
            println!("  {}", (t.header)("子命令:"));
            let mw = cmd.subcommands.iter().map(|c| c.name.display_width()).max().unwrap_or(10);
            for sub in &cmd.subcommands {
                let label = pad(sub.name, mw);
                println!("    {}  {}", (t.code)(&label), sub.description);
            }
        }

        let pos: Vec<&ArgDef> = cmd.args.iter().filter(|a| a.positional).collect();
        if !pos.is_empty() {
            println!();
            println!("  {}", (t.header)("参数:"));
            let mw = pos.iter().map(|a| a.long.len() + 4).max().unwrap_or(10);
            for a in &pos {
                let raw = if a.required { format!("<{}>", a.long.to_uppercase()) } else { format!("[{}]", a.long.to_uppercase()) };
                let label = pad(&raw, mw as usize);
                println!("    {}  {}", (t.code)(&label), a.help);
            }
        }

        let opts: Vec<&ArgDef> = cmd.args.iter().filter(|a| !a.positional).collect();
        if !opts.is_empty() {
            println!();
            println!("  {}", (t.header)("选项:"));
            let mw = opts.iter().map(|a| a.label().display_width()).max().unwrap_or(18);
            for a in &opts {
                let label = pad(&a.label(), mw);
                println!("    {}  {}", (t.code)(&label), a.help);
            }
        }
        println!();
    }

    // ── 错误处理 ──────────────────────────────

    pub fn print_error(&self, err: &CliError) {
        let t = &self.theme;
        match err {
            CliError::NoCommand => self.print_root_help(),
            CliError::UnknownCommand { input, suggestions } => {
                eprintln!();
                eprintln!("  {} 无法识别的命令 '{}'", (t.error)("错误:"), input);
                if !suggestions.is_empty() {
                    eprintln!("  {} 您是不是想输入:", (t.hint)("提示:"));
                    for s in suggestions { eprintln!("    {}", (t.code)(s)); }
                }
                eprintln!();
            }
            CliError::MissingArg { arg } => {
                let msg = format!("缺少必需参数: <{}>", arg.to_uppercase());
                eprintln!();
                eprintln!("  {} {}", (t.error)("错误:"), &msg);
                eprintln!();
            }
            CliError::UnknownOption { option } => {
                let msg = format!("无法识别的选项: {}", option);
                eprintln!();
                eprintln!("  {} {}", (t.error)("错误:"), &msg);
                eprintln!();
            }
            CliError::MissingOptionValue { option } => {
                let msg = format!("选项 {} 需要一个值", option);
                eprintln!();
                eprintln!("  {} {}", (t.error)("错误:"), &msg);
                eprintln!();
            }
            CliError::Custom(msg) => {
                eprintln!();
                eprintln!("  {} {}", (t.error)("错误:"), msg);
                eprintln!();
            }
        }
    }

    /// 打印示例用法
    pub fn print_examples(&self, examples: &[ExampleGroup]) {
        let t = &self.theme;
        let title = format!("{} 命令参考手册", self.name);
        println!();
        println!("  {}", (t.title)(&title));
        println!();
        let max_w = examples.iter().flat_map(|g| g.entries.iter()).map(|(u, _)| u.display_width()).max().unwrap_or(44);
        for g in examples {
            let group_label = format!("{:<12}", g.command);
            println!("  {}  {}", (t.example_group)(&group_label), (t.muted)(g.description));
            println!();
            for (usage, explanation) in &g.entries {
                let padded = pad(usage, max_w);
                println!("    {}  {}", (t.code)(&padded), explanation);
            }
            println!();
        }
    }
}

pub struct ExampleGroup {
    pub command: &'static str,
    pub description: &'static str,
    pub entries: Vec<(String, &'static str)>,
}

// ══════════════════════════════════════════════════════════
// 参数解析
// ══════════════════════════════════════════════════════════

fn parse_args(cmd: &CommandDef, args: &[String], parsed: &mut ParsedArgs) -> Result<(), CliError> {
    let pos: Vec<&ArgDef> = cmd.args.iter().filter(|a| a.positional).collect();
    let opts: Vec<&ArgDef> = cmd.args.iter().filter(|a| !a.positional).collect();
    let mut pi = 0;
    let mut i = 0;

    while i < args.len() {
        let a = &args[i];
        if a.starts_with("--") {
            let name = a.trim_start_matches("--");
            let opt = opts.iter().find(|o| o.long == name).ok_or_else(|| CliError::UnknownOption { option: a.clone() })?;
            match opt.arg_type {
                ArgType::Flag => { parsed.values.insert(opt.long.to_string(), ArgValue::Flag(true)); i += 1; }
                ArgType::String => {
                    if i + 1 >= args.len() { return Err(CliError::MissingOptionValue { option: a.clone() }); }
                    i += 1; parsed.values.insert(opt.long.to_string(), ArgValue::String(args[i].clone())); i += 1;
                }
                ArgType::Strings => {
                    if i + 1 >= args.len() { return Err(CliError::MissingOptionValue { option: a.clone() }); }
                    i += 1; parsed.values.insert(opt.long.to_string(), ArgValue::Strings(vec![args[i].clone()])); i += 1;
                }
            }
        } else if a.starts_with('-') && a.len() > 1 {
            let s = a.chars().nth(1).unwrap();
            let opt = opts.iter().find(|o| o.short == Some(s)).ok_or_else(|| CliError::UnknownOption { option: a.clone() })?;
            match opt.arg_type {
                ArgType::Flag => { parsed.values.insert(opt.long.to_string(), ArgValue::Flag(true)); i += 1; }
                ArgType::String | ArgType::Strings => {
                    if i + 1 >= args.len() { return Err(CliError::MissingOptionValue { option: a.clone() }); }
                    i += 1; parsed.values.insert(opt.long.to_string(), ArgValue::String(args[i].clone())); i += 1;
                }
            }
        } else {
            parsed.positional.push(a.clone()); pi += 1; i += 1;
        }
    }

    for (idx, p) in pos.iter().enumerate() {
        if p.required && pi <= idx { return Err(CliError::MissingArg { arg: p.long }); }
    }
    Ok(())
}

// ══════════════════════════════════════════════════════════
// 错误类型 & 编辑距离
// ══════════════════════════════════════════════════════════

#[derive(Debug)]
pub enum CliError {
    NoCommand,
    UnknownCommand { input: String, suggestions: Vec<String> },
    MissingArg { arg: &'static str },
    UnknownOption { option: String },
    MissingOptionValue { option: String },
    Custom(String),
}

impl From<String> for CliError { fn from(s: String) -> Self { CliError::Custom(s) } }

fn levenshtein(a: &str, b: &str) -> usize {
    let ac: Vec<char> = a.chars().collect();
    let bc: Vec<char> = b.chars().collect();
    let (n, m) = (ac.len(), bc.len());
    if n == 0 { return m; }
    if m == 0 { return n; }
    let mut p: Vec<usize> = (0..=m).collect();
    let mut c: Vec<usize> = vec![0; m + 1];
    for i in 1..=n {
        c[0] = i;
        for j in 1..=m {
            let cost = if ac[i - 1] == bc[j - 1] { 0 } else { 1 };
            c[j] = std::cmp::min(std::cmp::min(c[j - 1] + 1, p[j] + 1), p[j - 1] + cost);
        }
        std::mem::swap(&mut p, &mut c);
    }
    p[m]
}
