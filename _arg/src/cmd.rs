//! 命令定义 + 解析结果

use crate::arg::ArgDef;
use std::collections::HashMap;

/// 子命令定义
#[derive(Debug, Clone)]
pub struct SubCmd {
    pub cmd: Cmd,
    /// 别名（如 "i" → install）
    pub aliases: Vec<String>,
}

/// 命令定义
#[derive(Debug, Clone)]
pub struct Cmd {
    /// 命令名称
    pub name: String,
    /// 简短描述
    pub about: String,
    /// 参数列表
    pub args: Vec<ArgDef>,
    /// 子命令
    pub subs: Vec<SubCmd>,
}

impl Cmd {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            about: String::new(),
            args: vec![],
            subs: vec![],
        }
    }

    pub fn about(mut self, text: &str) -> Self {
        self.about = text.to_string();
        self
    }

    pub fn arg(mut self, arg: ArgDef) -> Self {
        self.args.push(arg);
        self
    }

    pub fn sub(mut self, cmd: Cmd) -> Self {
        self.subs.push(SubCmd {
            cmd,
            aliases: vec![],
        });
        self
    }

    /// 带别名的子命令
    pub fn sub_alias(mut self, cmd: Cmd, aliases: &[&str]) -> Self {
        self.subs.push(SubCmd {
            cmd,
            aliases: aliases.iter().map(|s| s.to_string()).collect(),
        });
        self
    }
}

/// 解析结果
#[derive(Debug, Clone)]
pub struct ParsedArgs {
    /// 标志值: flag名 → true/false
    flags: HashMap<String, bool>,
    /// 选项值: 选项名 → Vec<值>
    pub(crate) val_map: HashMap<String, Vec<String>>,
    /// 位置参数值
    pub positional: Vec<String>,
    /// 匹配的子命令名（None = 无子命令）
    pub sub: Option<String>,
    /// 子命令的解析结果
    pub sub_args: Option<Box<ParsedArgs>>,
}

impl ParsedArgs {
    pub fn new() -> Self {
        Self {
            flags: HashMap::new(),
            val_map: HashMap::new(),
            positional: vec![],
            sub: None,
            sub_args: None,
        }
    }

    /// 检查 flag 是否设置
    pub fn flag(&self, name: &str) -> bool {
        *self.flags.get(name).unwrap_or(&false)
    }

    /// 获取单值选项（取第一个值）
    pub fn value(&self, name: &str) -> Option<&str> {
        self.val_map.get(name).and_then(|v| v.first().map(|s| s.as_str()))
    }

    /// 获取多值选项
    pub fn values(&self, name: &str) -> Vec<&str> {
        self.val_map.get(name).map(|v| v.iter().map(|s| s.as_str()).collect()).unwrap_or_default()
    }

    pub(crate) fn set_flag(&mut self, name: &str) {
        self.flags.insert(name.to_string(), true);
    }

    pub(crate) fn push_value(&mut self, name: &str, val: String) {
        self.val_map.entry(name.to_string()).or_default().push(val);
    }
}
