# as (aminos) — 颜色与样式指南

> 通过修改本文档后告知我，我会同步更新代码实现。

## 目录

1. [HELP_TEMPLATE 常量](#1-help_template-常量)
2. [HELP_STYLES 常量（clap 全局配色）](#2-help_styles-常量clap-全局配色)
3. [运行时颜色函数](#3-运行时颜色函数-color)
4. [ansi 常量（表格数据着色）](#4-ansi-常量coloransicolor)
5. [installer 颜色](#5-installer-颜色)
6. [speedtest 颜色](#6-speedtest-颜色)
7. [run_example 颜色](#7-run_example-颜色)

---

## 1. HELP_TEMPLATE 常量

定义在 `src/main.rs` 顶部，是 `const &str` 编译时常量（clap 要求），不能使用运行时函数。

> 如果要在 const 中改变颜色，直接修改下面表格中对应的 `\x1b[...m` 字符串即可。

### 主帮助模板 `HELP_TEMPLATE`

| 元素 | 原始 ANSI 码 | 等价常量 | 颜色 | 行号 |
|------|-------------|---------|------|------|
| `aminos` 软件名 | `\x1b[1;36m` … `\x1b[0m` | `ansi::BOLD_CYAN` | **加粗青色** | 14 |
| `— 轻量级 Windows 软件包管理器` | `\x1b[32m` … `\x1b[0m` | `ansi::GREEN` | 绿色 | 14 |
| `用法:` `命令:` `选项:` `示例:` `提示:` 标题 | `\x1b[1;33m` … `\x1b[0m` | `ansi::BOLD_YELLOW` | **加粗黄色** | 16,19,21,24,34 |
| `as` 命令名 | `\x1b[36m` … `\x1b[0m` | `ansi::CYAN` | 青色 | 17,25-27,30 |
| `<命令>` 占位符 | `\x1b[32m` … `\x1b[0m` | `ansi::GREEN` | 绿色 | 17 |

### 子模板 `HELP_TEMPLATE_OPTIONS`

| 元素 | 原始 ANSI 码 | 等价常量 | 颜色 | 行号 |
|------|-------------|---------|------|------|
| `用法:` `选项:` 标题 | `\x1b[1;33m` … `\x1b[0m` | `ansi::BOLD_YELLOW` | **加粗黄色** | 42,45 |

### 子模板 `HELP_TEMPLATE_SUBCMDS`（含子命令）

| 元素 | 原始 ANSI 码 | 等价常量 | 颜色 | 行号 |
|------|-------------|---------|------|------|
| `用法:` `命令:` `选项:` 标题 | `\x1b[1;33m` … `\x1b[0m` | `ansi::BOLD_YELLOW` | **加粗黄色** | 52,55,57 |

---

## 2. HELP_STYLES 常量（clap 全局配色）

定义在 `src/main.rs` 顶部，使用 clap 的 `Styles` API 控制所有子命令帮助输出中 `{usage}`、`{options}`、`{subcommands}` 的自动着色。定义了 root `Cli` 的 `#[command(styles = HELP_STYLES)]`，所有子命令自动继承。

| 类别 | 方法 | 当前样式 | 效果 |
|------|------|----------|------|
| 用法行 | `.usage(...)` | **加粗黄色** | `as list [OPTIONS]` 整行 |
| 命令名/选项标志 | `.literal(...)` | 青色 | `-f, --filter` `install` `list` |
| 占位符 | `.placeholder(...)` | 绿色 | `<FILTER>` `<SEARCH>` |
| 错误标题 | `.error(...)` | 默认（红色加粗） | clap 默认 |
| 段标题 | `.header(...)` | 默认（加粗下划线） | 不使用（自定义模板已有颜色） |

> ⚠ `HELP_STYLES` 仅影响 `{usage}` `{options}` `{subcommands}` 等 clap 自动填充内容。模板中的硬编码 ANSI（如 `\x1b[1;33m用法:\x1b[0m`）不受影响，二者互补。

---

## 3. 运行时颜色函数 (`color::*`)

### 3.1 错误与提示

| 上下文 | 函数 | 当前颜色 | 代码位置 |
|--------|------|----------|----------|
| 错误标签 `"错误:"` | `color::red(...)` | 红色 | `main.rs:428` `print_clap_error()`, `main.rs:437` `run()` |
| 用法标签 `"用法:"` | `color::bold_yellow(...)` | **加粗黄色** | `main.rs:430` `print_clap_error()` |
| 灰色辅助提示 | `color::gray(...)` | 灰色 | `main.rs:432` `print_clap_error()` |
| 跳过提示 `"跳过"` | `color::yellow(...)` | 黄色 | `main.rs:448,476,545` `run_install`, `run_upgrade`, `run_uninstall` |

### 3.2 升级 (`run_upgrade`)

| 元素 | 函数 | 当前颜色 | 代码位置 |
|------|------|----------|----------|
| `"XXX 已是最新"` | `color::gray(...)` | 灰色 | `main.rs:503` |
| 当前版本（有更新时） | `color::yellow(...)` | 黄色 | `main.rs:510` |
| 源版本（有更新时） | `color::green(...)` | 绿色 | `main.rs:511` |
| `"升级 XXX 失败"` | `color::yellow(...)` | 黄色 | `main.rs:522` |
| 统计行（检查/升级结果） | `color::gray(...)` | 灰色 | `main.rs:531,535` |

### 3.3 列表 (`run_list`)

| 元素 | 函数/常量 | 当前颜色 | 代码位置 |
|------|----------|----------|----------|
| `"未找到源定义"` 提示 | `color::yellow(...)` | 黄色 | `main.rs:620` |
| 底部统计 `"共 N 项"` | `color::gray(...)` | 灰色 | `main.rs:758` |

### 3.4 信息 (`run_info`)

| 元素 | 函数 | 当前颜色 | 代码位置 |
|------|------|----------|----------|
| 软件显示名 | `color::green(...)` | 绿色 | `main.rs:771,802` |
| 版本号（带 `← 默认` 标记） | `color::green(...)` | 绿色 | `main.rs:870` |
| 已安装状态 `"已安装 (版本 X)"` | `color::green(...)` | 绿色 | `main.rs:822,830` |
| 未安装状态 `"未安装"` | `color::gray(...)` | 灰色 | `main.rs:848` |
| 标签 `"标识符:"` `"分类:"` `"官网:"` | `color::gray(...)` | 灰色 | `main.rs:811,816,817` |
| `"可用版本:"` | `color::gray(...)` | 灰色 | `main.rs:853` |
| `"类型:"` `"下载:"` | `color::gray(...)` | 灰色 | `main.rs:875,876` |
| 版本号（信息中） | `color::cyan(...)` | 青色 | `main.rs:789` |

### 3.5 source 命令 (`run_source` / `run_dirs` / `run_cache`)

#### `run_dirs`

| 元素 | 函数 | 当前颜色 | 代码位置 |
|------|------|----------|----------|
| `"aminos 数据目录一览"` | `color::bold_cyan(...)` | **加粗青色** | `main.rs:927` |
| 节标题（可执行/源定义/缓存/记录/快捷方式） | `color::bold_yellow(...)` | **加粗黄色** | `main.rs:929,933,937,941,945` |
| `"数据根目录"` | `color::bold_yellow(...)` | **加粗黄色** | `main.rs:949` |

#### `run_cache`

| 元素 | 函数 | 当前颜色 | 代码位置 |
|------|------|----------|----------|
| 清除成功 `"已清除 N 个缓存文件"` | `color::green(...)` | 绿色 | `main.rs:989` |
| 一致性标记 `" ⚠"`（不一致） | `color::yellow(...)` | 黄色 | `main.rs:1028` |
| 一致性标记 `" ✓"`（一致） | `color::green(...)` | 绿色 | `main.rs:1031` |
| `"下载缓存"` 节标题 | `color::bold_yellow(...)` | **加粗黄色** | `main.rs:1048` |
| 缓存路径 | `color::gray(...)` | 灰色 | `main.rs:1048` |
| 图例 `"✓ 版本与源定义一致 ⚠ 与源定义不一致"` | `color::green(...)` / `color::yellow(...)` | 绿/黄 | `main.rs:1066` |
| 页脚统计 `"共 N 个文件"` | `color::gray(...)` | 灰色 | `main.rs:1070,1071,1072` |

---

## 4. ansi 常量 (`color::ansi::*`)

用于表格行中的状态/颜色数据，存储在 `Vec` 中作为数据传递。

### 4.1 下载缓存扫描

| 状态 | 当前颜色 | 常量 | 代码位置 |
|------|----------|------|----------|
| `"下载中"` (黄色) | `\x1b[33m` | `color::ansi::YELLOW` | `main.rs:600` |
| `"已下载"` (青色) | `\x1b[36m` | `color::ansi::CYAN` | `main.rs:602` |

### 4.2 run_list 表格行

| 用途 | 当前颜色 | 常量 | 代码位置 |
|------|----------|------|----------|
| 有源定义 | 绿色 | `color::ansi::GREEN` | `main.rs:642,684,688` |
| 无源定义 / 未安装 / 未下载 | 灰色 | `color::ansi::GRAY` | `main.rs:642,646,648,651,681,688` |
| 行末复位 | 复位 | `color::ansi::RESET` | `main.rs:747,751` |

### 理想配色参考

| 用途 | 推荐颜色 | ANSI 码 | 常量 |
|------|---------|---------|------|
| 已安装 | 绿色 | `32` | `color::ansi::GREEN` |
| 未安装 | 灰色 | `90` | `color::ansi::GRAY` |
| 有源 | 绿色 | `32` | `color::ansi::GREEN` |
| 无源 | 灰色 | `90` | `color::ansi::GRAY` |
| 已下载 | 青色 | `36` | `color::ansi::CYAN` |
| 下载中 | 黄色 | `33` | `color::ansi::YELLOW` |
| 未下载 | 灰色 | `90` | `color::ansi::GRAY` |
| 复位码 | — | `0` | `color::ansi::RESET` |

---

## 5. installer 颜色

在 `src/installer.rs` 中。

| 元素 | 函数 | 当前颜色 | 代码位置 |
|------|------|----------|----------|
| 版本不一致提示 `"(源声明 vX, PE 真实 vY)"` | `color::gray(...)` | 灰色 | `installer.rs:171` |

---

## 6. speedtest 颜色（`as source speedtest`）

在 `src/speedtest.rs` 中，现在通过 `as source speedtest` 访问。

### 6.1 测速开始

| 元素 | 函数 | 当前颜色 | 代码位置 |
|------|------|----------|----------|
| `"共 N 个下载源，正在并发测速..."` | `color::gray(...)` | 灰色 | `speedtest.rs:64` |

### 6.2 每个 URL 的实时结果

| 条件 | Style 常量 | 当前颜色 | 代码位置 |
|------|-----------|----------|----------|
| 可用（有速度） | `color::GREEN` | 绿色 (32) | `speedtest.rs:92` |
| 不可用 | `color::YELLOW` | 黄色 (33) | `speedtest.rs:93` |

### 6.3 分隔线

| 元素 | 函数 | 当前颜色 | 代码位置 |
|------|------|----------|----------|
| `"═"` 分隔线 | `color::green(...)` | 绿色 | `speedtest.rs:113` |

### 6.4 per_software 汇总表

| 条件 | 函数 | 当前颜色 | 代码位置 |
|------|------|----------|----------|
| 可用速度（有值） | `color::green(...)` | 绿色 | `speedtest.rs:163` |
| `"可用"` 状态 | `color::green(...)` | 绿色 | `speedtest.rs:168` |
| `"不可用"` 状态 | `color::yellow(...)` | 黄色 | `speedtest.rs:170` |
| `"总计: N 个软件 \| "` | `color::gray(...)` | 灰色 | `speedtest.rs:182` |
| `"N 可用"` | `color::green(...)` | 绿色 | `speedtest.rs:183` |
| `"N 不可用"` | `color::yellow(...)` | 黄色 | `speedtest.rs:185` |
| `"⚠ 以下软件所有源均不可用:"` | `color::yellow(...)` | 黄色 | `speedtest.rs:188` |
| 不可用软件名标签 | `color::gray(...)` | 灰色 | `speedtest.rs:194` |

### 6.5 按源统计表

| 条件 | 函数 | 当前颜色 | 代码位置 |
|------|------|----------|----------|
| 可用速度 | `color::green(...)` | 绿色 | `speedtest.rs:233` |
| URL 文本 | `color::gray(...)` | 灰色 | `speedtest.rs:241` |
| `"总计: N 个源 \| "` | `color::gray(...)` | 灰色 | `speedtest.rs:246` |
| `"N 可用"` | `color::green(...)` | 绿色 | `speedtest.rs:247` |
| `"N 不可用"` | `color::yellow(...)` | 黄色 | `speedtest.rs:249` |
| `"⚠ 以下源不可用"` | `color::yellow(...)` | 黄色 | `speedtest.rs:252` |
| 不可用源名标签 | `color::gray(...)` | 灰色 | `speedtest.rs:255` |

---

## 7. run_example 颜色

在 `src/main.rs` `run_example()` 中。

| 元素 | 函数 | 当前颜色 | 代码位置 |
|------|------|----------|----------|
| 标题 `"aminos 命令参考手册"` | `color::bold_cyan(...)` | **加粗青色** | `main.rs` `run_example()` |
| 命令名（左侧列） | `color::bold_green(...)` | **加粗绿色** | `main.rs` `run_example()` |
| 命令描述 | `color::gray(...)` | 灰色 | `main.rs` `run_example()` |
| 示例命令文本（`as xxx ...`） | `color::cyan(pad(usage, max_usage_w))` | 青色（使用 `pad` + `display_width` 对齐） | `main.rs` `run_example()` |
| 示例说明 | 无（默认色） | 默认 | `main.rs` `run_example()` |

> **对齐策略**：使用 `pad(usage, max_usage_w)` 替代 `format!("{:<44}", usage)`，通过 `DisplayWidth` trait 正确计算 CJK 字符宽度（每个中文字=2列），确保 `as list -f 办公` 等含中文的示例命令对齐。`max_usage_w` 取所有示例命令文本的 `display_width` 最大值。
