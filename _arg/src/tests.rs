#[cfg(test)]
mod tests {
    use crate::*;

    fn args(s: &str) -> Vec<String> {
        std::iter::once("prog".to_string())
            .chain(s.split_whitespace().map(|s| s.to_string()))
            .collect()
    }

    #[test]
    fn test_simple_flag() {
        let cmd = Cmd::new("test")
            .arg(flag("verbose", 'v', "详细输出"));

        let r = parse(&cmd, &args("-v")).unwrap();
        assert!(r.flag("verbose"));

        let r = parse(&cmd, &args("")).unwrap();
        assert!(!r.flag("verbose"));
    }

    #[test]
    fn test_long_flag() {
        let cmd = Cmd::new("test")
            .arg(flag("verbose", 'v', "详细输出"));

        let r = parse(&cmd, &args("--verbose")).unwrap();
        assert!(r.flag("verbose"));
    }

    #[test]
    fn test_value_arg() {
        let cmd = Cmd::new("test")
            .arg(arg("sort", 's', "排序方式"));

        let r = parse(&cmd, &args("-s name")).unwrap();
        assert_eq!(r.value("sort"), Some("name"));
    }

    #[test]
    fn test_long_value() {
        let cmd = Cmd::new("test")
            .arg(arg("sort", 's', "排序方式"));

        let r = parse(&cmd, &args("--sort name")).unwrap();
        assert_eq!(r.value("sort"), Some("name"));
    }

    #[test]
    fn test_multi_value() {
        let cmd = Cmd::new("test")
            .arg(arg_long("exclude", "排除").multi());

        let r = parse(&cmd, &args("--exclude .txt .md .rs")).unwrap();
        assert_eq!(r.values("exclude"), vec![".txt", ".md", ".rs"]);
    }

    #[test]
    fn test_subcommand() {
        let cmd = Cmd::new("test")
            .sub(Cmd::new("list").about("列出"))
            .sub(Cmd::new("install").about("安装"));

        let r = parse(&cmd, &args("list")).unwrap();
        assert_eq!(r.sub.as_deref(), Some("list"));

        let r = parse(&cmd, &args("install")).unwrap();
        assert_eq!(r.sub.as_deref(), Some("install"));
    }

    #[test]
    fn test_subcommand_alias() {
        let cmd = Cmd::new("test")
            .sub_alias(Cmd::new("install").about("安装"), &["i"]);

        let r = parse(&cmd, &args("i")).unwrap();
        assert_eq!(r.sub.as_deref(), Some("install"));
    }

    #[test]
    fn test_positional() {
        let cmd = Cmd::new("test")
            .arg(pos("directory", "路径").default("."));

        let r = parse(&cmd, &args("")).unwrap();
        assert_eq!(r.value("directory"), Some("."));

        let r = parse(&cmd, &args("mydir")).unwrap();
        assert_eq!(r.positional, vec!["mydir"]);
    }

    #[test]
    fn test_default_value() {
        let cmd = Cmd::new("test")
            .arg(arg("sort", 's', "排序").default("name"));

        let r = parse(&cmd, &args("")).unwrap();
        assert_eq!(r.value("sort"), Some("name"));
    }

    #[test]
    fn test_global_flag() {
        let cmd = Cmd::new("test")
            .arg(flag("help", 'h', "帮助").global())
            .sub(Cmd::new("list").about("列出"));

        let r = parse(&cmd, &args("list -h")).unwrap();
        assert_eq!(r.sub.as_deref(), Some("list"));
        let sub = r.sub_args.as_ref().unwrap();
        assert!(sub.flag("help"));
    }

    #[test]
    fn test_unknown_flag_error() {
        let cmd = Cmd::new("test");
        let r = parse(&cmd, &args("--unknown"));
        assert!(r.is_err());
    }

    #[test]
    fn test_help_output() {
        let cmd = Cmd::new("as")
            .about("极简 Windows 软件下载器")
            .arg(flag("help", 'h', "显示帮助"))
            .arg(arg("sort", 's', "排序方式").default("name"))
            .sub(Cmd::new("install").about("下载软件"))
            .sub_alias(Cmd::new("list").about("列出所有支持的软件"), &["l"]);

        // 不 panic 就算通过
        print_help(&cmd, r"C:\Tools\as.exe");
        println!();
        print_version(&cmd, "0.1.0", "github.com/xxx");
    }
}
