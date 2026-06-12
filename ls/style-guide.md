# ls — 颜色与样式指南

> 通过修改本文档后告知我，我会同步更新代码实现。

## 目录

1. [运行时颜色函数](#1-运行时颜色函数-color)
2. [Config 驱动颜色](#2-config-驱动颜色-configrs)
3. [文件扩展名颜色映射](#3-文件扩展名颜色映射)
4. [文件大小颜色规则](#4-文件大小颜色规则)

---

## 1. 运行时颜色函数 (`color::*`)

这些在 `src/main.rs` 中直接调用，涵盖帮助信息、版本信息、错误提示。

### 1.1 帮助信息 (`print_help()`)

| 元素 | 样式 | 当前颜色 | 代码位置 |
|------|------|----------|----------|
| `"用法:"` 标题 | **加粗黄色** | `bold_yellow()` | `main.rs` `print_help()` |
| `"列出目录内容"` 标题 | **加粗黄色** | `bold_yellow()` | `main.rs` `print_help()` |
| `"位置参数:"` 标题 | **加粗黄色** | `bold_yellow()` | `main.rs` `print_help()` |
| `"选项:"` 标题 | **加粗黄色** | `bold_yellow()` | `main.rs` `print_help()` |
| 用法行前导空格着色 | 青色 | `cyan("")` | `main.rs` `print_help()` |
| 占位符 `<EXCLUDE>` `<INCLUDE>` `<PATH>` `directory` | 下划线 | `Style::new(4).paint(...)` | `main.rs` `print_help()` |
| 占位符 `<排序>` `<后缀>` `[深度]` 在选项标签中 | 下划线 | `Style::new(4).paint(...)` | `main.rs` `print_help()` |
| `"TREE"` | 灰色 | `gray("TREE")` | `main.rs` `print_help()` |
| `"default,name,suffix,create,update"` | 灰色 | `gray(...)` | `main.rs` `print_help()` |
| 所有选项的右侧说明文本 | 灰色 | `gray(desc)` | `main.rs` `print_help()` |

### 1.2 版本信息 (`version_info()`)

| 元素 | 样式 | 当前颜色 | 代码位置 |
|------|------|----------|----------|
| `"ls"` | **加粗青色** | `bold_cyan("ls")` | `main.rs` `version_info()` |
| 版本号 | **绿色** | `green(env!("CARGO_PKG_VERSION"))` | `main.rs` `version_info()` |
| `"Rust 版"` | 灰色 | `gray("Rust 版")` | `main.rs` `version_info()` |
| 描述文字 | 黄色 | `yellow(...)` | `main.rs` `version_info()` |
| `"GitHub:"` 标签 | 蓝色 | `blue("GitHub")` | `main.rs` `version_info()` |
| GitHub 链接 | 下划线 | `Style::new(4).paint(...)` | `main.rs` `version_info()` |

### 1.3 错误提示 (`print_clap_error()`)

| 元素 | 样式 | 当前颜色 | 代码位置 |
|------|------|----------|----------|
| `"错误:"` 标签 | 红色 | `red("错误:")` | `main.rs` `print_clap_error()` |
| `"提示:"` / `"用法:"` 标签 | 灰色 / **加粗黄色** | `gray(...)` / `bold_yellow(...)` | `main.rs` `print_clap_error()` |

---

## 2. Config 驱动颜色 (`config.rs`)

在 `src/config.rs` 的 `ColorConfig::default()` 中定义，通过 ANSI 数字码指定颜色。

### 2.1 目录颜色

| 用途 | ANSI 码 | 颜色名 | 字段名 | 建议修改值 |
|------|---------|--------|--------|-----------|
| 普通目录 | `96` | 亮青色 (bright cyan) | `dir_color` | 36=cyan, 94=light blue, 93=yellow |
| 链接目录 | `36` | 青色 (cyan) | `dir_link_color` | 96=bright cyan, 33=yellow |
| 目录链接箭头 `=>` | `90` | 灰色 (gray) | `dir_link_arrow_color` | 37=white, 2=dim |
| 目录链接路径 | `90` | 灰色 (gray) | `dir_link_path_color` | 37=white, 90=gray |
| 目录链接目标目录名 | `96` | 亮青色 (bright cyan) | `dir_link_path_basename_color` | 36=cyan, 94=blue |
| 文件链接指向目录时的目录名 | `96` | 亮青色 (bright cyan) | `file_link_dir_color` | 36=cyan, 94=blue |

### 2.2 文件链接颜色

| 用途 | ANSI 码 | 颜色名 | 字段名 | 建议修改值 |
|------|---------|--------|--------|-----------|
| 文件链接箭头 `->` | `90` | 灰色 (gray) | `file_link_arrow_color` | 37=white, 2=dim |

### 2.3 通用占位符颜色

| 用途 | ANSI 码 | 颜色名 | 对应 `color::*` | 代码位置 |
|------|---------|--------|-----------------|----------|
| 文件名默认色 | `97` | 亮白色 (bright white) | 无（直接 97） | `display.rs` `print_file_name()` / `get_item_color()` |
| 文件名本体（非扩展名部分） | `97` | 亮白色 (bright white) | `paint_by_code(..., "97")` | `display.rs` `print_file_name()` |

### 2.4 类型标记颜色

| 用途 | 函数 | 当前颜色 | 代码位置 |
|------|------|----------|----------|
| `<dir>` / `<file>` 标记 | `print_type_marker()` | `color::gray(...)` | `display.rs` |
| 时间戳 | `print_timestamp()` | `color::gray(...)` | `display.rs` |
| 环境版本号（`.venv` 等） | `format_text_colored()` | `color::gray(...)` | `main.rs` |

---

## 3. 文件扩展名颜色映射

在 `config.rs` `file_extensions` 中定义。ANSI 前景色码参考：

| 码 | 颜色名 | 示例 |
|----|--------|------|
| `31` | 红色 (red) | 压缩包 `.7z .zip .rar .tar .gz .bz2 .xz` |
| `32` | 绿色 (green) | 可执行 `.exe .msi .bat .cmd` |
| `33` | 黄色 (yellow) | 代码 `.rs .js .ts` |
| `35` | 紫色 (magenta) | 网页 `.html .css` |
| `37` | 白色 (white) | 数据 `.json .toml .yaml .yml .md .txt` |
| `90` | 灰色 (gray) | 系统文件 `.dll .pdb .dat .ini .lock .log` |
| `93` | 亮黄色 (bright yellow) | Python `.py` |
| `94` | 亮蓝色 (light blue) | 快捷方式 `.lnk` |

### 修改示例

```rust
file_extensions: vec![
    (".rs".into(), "33".into()),   // ← 当前黄色
    (".py".into(), "93".into()),   // ← 当前亮黄色
    // 要改为绿色：
    // (".rs".into(), "32".into()),
    // (".py".into(), "32".into()),
],
```

---

## 4. 文件大小颜色规则

在 `config.rs` `size_rules` 中定义，按文件大小范围着色。

| 范围 | mode | ANSI 码 | 颜色 | 含义 |
|------|------|---------|------|------|
| `< 1 KB` | `full` | `90` | 灰色 | 整体灰色，如 `900 B` |
| `< 1 MB` | `unit` | `90` | 灰色 | 仅单位灰色，如 `500` `KB` |
| `< 100 MB` | `unit` | `93` | 亮黄色 | 仅单位亮黄，如 `50` `MB` |
| `< 1 GB` | `full` | `93` | 亮黄色 | 整体亮黄，如 `500 MB` |
| `< 2 GB` | `unit` | `91` | 亮红色 | 仅单位亮红，如 `1.5` `GB` |
| `>= 2 GB` | `full` | `91` | 亮红色 | 整体亮红 |

- `mode = "full"`：整个大小字符串同色
- `mode = "unit"`：仅单位后缀着色（数字部分保持默认色）
