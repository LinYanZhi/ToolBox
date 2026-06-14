# `as` 命令参考手册

## 全局选项

| 参数 | 说明 |
|------|------|
| `-e`, `--example` | 显示所有命令的示例用法 |

---

## 1. `as install` — 安装指定软件

```
as install <名称>... [选项]
```

| 参数 | 说明 |
|------|------|
| `<名称>` | 软件名称（可同时指定多个，必需） |
| `-v`, `--version` `<版本>` | 指定版本号 |
| `-g`, `--gui` | 使用图形界面安装（不静默） |
| `-r`, `--renew` | 强制重新下载 |
| `-d`, `--download-only` | 仅下载，不安装 |
| `--type` `<portable\|installer>` | 安装类型：便携版或安装版 |

**示例：**
```
as install 7zip
as install vscode python git
as install 7zip -v 1.0.0
as install 7zip --gui
as install 7zip --renew
as install 7zip --download-only
as install 7zip --type portable
```

---

## 2. `as list` — 列出可用软件及安装状态

```
as list [选项]
```

| 参数 | 说明 |
|------|------|
| `-f`, `--filter` `<分类>` | 按分类过滤 |
| `-i`, `--installed` | 仅显示已安装（与 `-m` 互斥） |
| `-m`, `--not-installed` | 仅显示未安装（与 `-i` 互斥） |
| `-s`, `--search` `<关键字>` | 搜索软件名、别名或描述 |
| `-d`, `--downloaded` | 仅显示已下载（与 `--downloading`、`--no-download` 互斥） |
| `--downloading` | 仅显示下载中（与 `-d`、`--no-download` 互斥） |
| `--no-download` | 仅显示未下载（与 `-d`、`--downloading` 互斥） |
| `-g`, `--group` | 按分类分组显示 |
| `--categories` | 显示所有分类概览 |

**示例：**
```
as list
as list -g
as list --categories
as list -i
as list -m
as list -f 开发工具
as list -s 压缩
as list -s python
as list -d
as list --downloading
as list --no-download
```

---

## 3. `as info` — 查看软件详细信息

```
as info <名称> [选项]
```

| 参数 | 说明 |
|------|------|
| `<名称>` | 软件名称（必需） |
| `-u`, `--urls` | 显示所有下载地址 |

**示例：**
```
as info 7zip
as info 7zip --urls
```

---

## 4. `as uninstall` — 卸载指定软件

```
as uninstall <名称>... [选项]
```

| 参数 | 说明 |
|------|------|
| `<名称>` | 软件名称（可同时指定多个，必需） |
| `-g`, `--gui` | 使用图形界面卸载 |
| `-f`, `--force` | 强制删除（跳过卸载器） |

**示例：**
```
as uninstall 7zip
as uninstall vscode python
as uninstall 7zip --gui
as uninstall 7zip --force
```

---

## 5. `as cache` — 查看已下载的缓存文件

```
as cache [选项]
```

| 参数 | 说明 |
|------|------|
| `-c`, `--clear` | 清除所有缓存文件（与 `-o` 互斥） |
| `-o`, `--open` | 在资源管理器中打开缓存目录（与 `-c` 互斥） |

**示例：**
```
as cache
as cache --clear
as cache --open
```

---

## 6. `as upgrade` — 升级所有已安装的软件

```
as upgrade [名称]... [选项]
```

| 参数 | 说明 |
|------|------|
| `<名称>` | 可选：仅升级指定软件（不指定则全部升级） |
| `-c`, `--check` | 仅检查更新，不下也不装（与 `--renew` 互斥） |
| `--renew` | 强制重新下载（即使版本相同，与 `-c` 互斥） |

**示例：**
```
as upgrade
as upgrade 7zip
as upgrade --check
as upgrade --renew
```

---

## 7. `as source` — 管理软件源定义

```
as source <子命令> [选项]
```

### 7.1 `as source update`

从远程仓库下载最新源定义。

```
as source update
```

无参数。

### 7.2 `as source path`

显示当前源目录路径。

```
as source path [选项]
```

| 参数 | 说明 |
|------|------|
| `-o`, `--open` | 在资源管理器中打开 |

### 7.3 `as source dirs`

显示所有数据目录位置。

```
as source dirs [选项]
```

| 参数 | 说明 |
|------|------|
| `-o`, `--open` | 在资源管理器中打开 |

### 7.4 `as source speedtest`

测速所有下载源。

```
as source speedtest [名称]... [选项]
```

| 参数 | 说明 |
|------|------|
| `<名称>` | 可选：仅测速指定软件 |
| `-S`, `--software` | 以软件为单位统计（任一源可用即为通） |

---

## 8. `as init` — 初始化 as 环境

```
as init
```

创建 `tools/bin` 目录并注册到用户 PATH。

无参数。

---

## 9. `as self-update` — 更新 as 自身

```
as self-update
```

下载最新版 as 并热替换。

无参数。

---

## 10. `as tool` — 管理自研工具

```
as tool <子命令> [选项]
```

### 10.1 `as tool list`

列出已安装的自研工具。

```
as tool list
```

无参数。

### 10.2 `as tool remove`

移除一个自研工具（同 `as uninstall`）。

```
as tool remove <名称>
```

| 参数 | 说明 |
|------|------|
| `<名称>` | 工具名称（必需） |

---

## 11. `as downloader` — 管理下载引擎后端

```
as downloader <子命令> [选项]
```

### 11.1 `as downloader list`

列出所有下载后端及其启用状态。

```
as downloader list
```

无参数。

### 11.2 `as downloader set`

启用或禁用一个后端。

```
as downloader set <名称> <状态>
```

| 参数 | 说明 |
|------|------|
| `<名称>` | 后端名称（如 curl, RustRange, Aria2c，必需） |
| `<状态>` | `on` 或 `off`（必需） |

### 11.3 `as downloader config`

显示或打开配置文件。

```
as downloader config [选项]
```

| 参数 | 说明 |
|------|------|
| `-o`, `--open` | 在资源管理器中打开配置目录 |

---

## 汇总：命令层级结构

```
as
├── -e, --example                              # 显示示例
├── install       <名称>...  [选项]             # 安装软件
├── list                    [选项]             # 列出软件
├── info          <名称>     [选项]             # 查看详情
├── uninstall     <名称>...  [选项]             # 卸载软件
├── cache                   [选项]             # 查看缓存
├── upgrade       [名称]... [选项]             # 升级软件
├── source                                     # 管理源
│   ├── update                                 # 更新源定义
│   ├── path                [选项]             # 源目录路径
│   ├── dirs                [选项]             # 数据目录
│   └── speedtest  [名称]... [选项]            # 测速
├── init                                       # 初始化环境
├── self-update                                # 更新自身
├── tool                                       # 自研工具
│   ├── list                                   # 列出工具
│   └── remove      <名称>                     # 移除工具
└── downloader                                 # 下载后端
    ├── list                                   # 列出后端
    ├── set          <名称> <状态>              # 启用/禁用
    └── config                [选项]            # 配置文件
```
