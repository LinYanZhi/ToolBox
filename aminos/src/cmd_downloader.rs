use crate::downloader;
use crate::cmd_names;

pub fn run_downloader(list: bool, set: Option<Vec<String>>, open: bool) -> anyhow::Result<()> {
    if open {
        return downloader::run_downloader_config(true);
    }

    if let Some(args) = set {
        if args.len() < 2 {
            anyhow::bail!("用法: {} set <名称> on|off", cmd_names::DOWNLOADER_SET);
        }
        let name = &args[0];
        let state = &args[1];
        let enable = match state.as_str() {
            "on" => true,
            "off" => false,
            _ => anyhow::bail!("无效状态: {}（使用 on/off）", state),
        };
        return downloader::run_downloader_set(name, enable);
    }

    if list {
        return downloader::run_downloader_list();
    }

    anyhow::bail!("请指定操作：{} 列出后端，set <名称> on|off 切换状态，{} 打开配置目录",
        cmd_names::DOWNLOADER_LIST, cmd_names::DOWNLOADER_OPEN);
}
