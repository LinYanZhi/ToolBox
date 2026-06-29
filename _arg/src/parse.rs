//! 命令行参数解析引擎

use std::collections::HashMap;

use crate::arg::{ArgDef, ArgKind};
use crate::cmd::{Cmd, ParsedArgs};

/// 解析命令行参数（args 应包含 argv[0] 即程序名，会自动跳过）
pub fn parse(cmd: &Cmd, args: &[String]) -> Result<ParsedArgs, String> {
    let mut pa = ParsedArgs::new();
    // 跳过 argv[0]（程序名）
    let mut i = 1;

    // 收集所有可识别的参数（本命令 + 全局参数）
    let mut all_args: HashMap<String, &ArgDef> = HashMap::new();
    for a in &cmd.args {
        all_args.insert(format!("--{}", a.long), a);
        if let Some(short) = a.short {
            all_args.insert(format!("-{}", short), a);
        }
    }

    while i < args.len() {
        let arg = &args[i];

        if arg == "--" {
            // 剩余参数全部作为位置参数
            i += 1;
            while i < args.len() {
                pa.positional.push(args[i].clone());
                i += 1;
            }
            break;
        }

        if let Some(def) = all_args.get(arg.as_str()) {
            let def = (*def).clone();
            if def.kind == ArgKind::Flag {
                pa.set_flag(&def.long);
                i += 1;
            } else {
                // Value 类型
                i += 1;
                if def.multi {
                    // 多值：收集后续非选项的 token
                    while i < args.len() && !args[i].starts_with('-') {
                        pa.push_value(&def.long, args[i].clone());
                        i += 1;
                    }
                } else {
                    // 单值
                    if i < args.len() && !args[i].starts_with('-') {
                        pa.push_value(&def.long, args[i].clone());
                        i += 1;
                    } else if def.optional {
                        // 可选值，不提供就用默认值，没默认值就给空串
                        if let Some(ref d) = def.default {
                            pa.push_value(&def.long, d.clone());
                        } else {
                            pa.push_value(&def.long, String::new());
                        }
                    } else {
                        return Err(format!("选项 --{} 需要一个值", def.long));
                    }
                }
            }
        } else if arg.starts_with('-') {
            return Err(format!("未知的选项 '{}'", arg));
        } else {
            // 可能是子命令或位置参数
            if let Some(sub) = find_sub(&cmd.subs, arg) {
                // 子命令
                pa.sub = Some(sub.cmd.name.clone());
                let mut sub_args: Vec<String> = args[i + 1..].to_vec();
                // 递归解析需要 argv[0]，补一个占位
                sub_args.insert(0, sub.cmd.name.clone());
                // 合并全局参数
                let mut sub_cmd = sub.cmd.clone();
                for a in &cmd.args {
                    if a.global {
                        sub_cmd.args.push(a.clone());
                    }
                }
                pa.sub_args = Some(Box::new(parse(&sub_cmd, &sub_args)?));
                break;
            } else {
                // 优先匹配 positional arg
                if let Some(pos_arg) = cmd.args.iter().find(|a| a.positional) {
                    pa.positional.push(arg.clone());
                    pa.push_value(&pos_arg.long, arg.clone());
                    i += 1;
                    // 如果是 multi 的 positional，收集后续非选项 token
                    if pos_arg.multi {
                        while i < args.len() && !args[i].starts_with('-') {
                            // 检查是否是子命令
                            if find_sub(&cmd.subs, &args[i]).is_some() { break; }
                            pa.positional.push(args[i].clone());
                            pa.push_value(&pos_arg.long, args[i].clone());
                            i += 1;
                        }
                    }
                } else {
                    pa.positional.push(arg.clone());
                    i += 1;
                }
            }
        }
    }

    // 设置默认值
    for def in &cmd.args {
        if def.kind == ArgKind::Value {
            if !pa.val_map.contains_key(&def.long) {
                if let Some(ref d) = def.default {
                    pa.push_value(&def.long, d.clone());
                }
            }
        }
    }

    Ok(pa)
}

fn find_sub<'a>(subs: &'a [crate::cmd::SubCmd], name: &str) -> Option<&'a crate::cmd::SubCmd> {
    subs.iter().find(|s| {
        s.cmd.name == name || s.aliases.iter().any(|a| a == name)
    })
}
