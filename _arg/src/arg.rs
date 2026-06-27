//! 参数定义 — Flag（布尔开关）和 Arg（带值参数）

/// 参数类型
#[derive(Debug, Clone, PartialEq)]
pub enum ArgKind {
    /// 布尔开关：-n / --no-color，不取值
    Flag,
    /// 带值参数：-s name / --sort name
    Value,
}

/// 参数定义
#[derive(Debug, Clone)]
pub struct ArgDef {
    /// 长名（"help" → --help）
    pub long: String,
    /// 短名（'h' → -h），None 表示只有长名
    pub short: Option<char>,
    /// 中文描述
    pub desc: String,
    /// 参数类型
    pub kind: ArgKind,
    /// 是否允许多个值（--exclude .txt .md）
    pub multi: bool,
    /// 值可选（-t [depth]），仅 Value 类型有效
    pub optional: bool,
    /// 默认值
    pub default: Option<String>,
    /// 全局参数（子命令中也可识别）
    pub global: bool,
    /// 位置参数（不带 - 前缀的参数）
    pub positional: bool,
    /// 可选值列表（如 sort 只能是 name/size）
    pub choices: Vec<String>,
}

impl ArgDef {
    /// 创建 Flag
    pub fn flag(long: &str, short: Option<char>, desc: &str) -> Self {
        Self {
            long: long.to_string(),
            short,
            desc: desc.to_string(),
            kind: ArgKind::Flag,
            multi: false,
            optional: false,
            default: None,
            global: false,
            positional: false,
            choices: vec![],
        }
    }

    /// 创建带值参数
    pub fn value(long: &str, short: Option<char>, desc: &str) -> Self {
        Self {
            long: long.to_string(),
            short,
            desc: desc.to_string(),
            kind: ArgKind::Value,
            multi: false,
            optional: false,
            default: None,
            global: false,
            positional: false,
            choices: vec![],
        }
    }

    /// 标记为全局参数
    pub fn global(mut self) -> Self {
        self.global = true;
        self
    }

    /// 允许多个值
    pub fn multi(mut self) -> Self {
        self.multi = true;
        self
    }

    /// 值可选（不必须）
    pub fn optional(mut self) -> Self {
        self.optional = true;
        self
    }

    /// 设置默认值
    pub fn default(mut self, val: &str) -> Self {
        self.default = Some(val.to_string());
        self
    }

    /// 标记为位置参数
    pub fn positional(mut self) -> Self {
        self.positional = true;
        self
    }

    /// 设置可选值列表
    pub fn choices(mut self, vals: &[&str]) -> Self {
        self.choices = vals.iter().map(|s| s.to_string()).collect();
        self
    }
}
