//! 硬编码配置 — 开发者直接编辑此文件修改规则，打包后无需外部配置文件

use std::path::Path;
use windows::Win32::Foundation::HWND;

// ── 交互设置 ──

pub const SCALE_STEP: f64 = 0.1;
pub const MOVE_STEP: i32 = 10;

// ── 白名单 ──

pub struct WhitelistEntry {
    /// 标题关键字（部分匹配，不区分大小写）
    pub title: Option<&'static str>,
    /// 进程名（不含 .exe，精确匹配）
    pub process: Option<&'static str>,
}

pub struct SizeFilter {
    pub min_width: i32,
    pub max_width: i32,
    pub min_height: i32,
    pub max_height: i32,
}

pub static WHITELIST_ENTRIES: &[WhitelistEntry] = &[
    WhitelistEntry { title: Some("Program Manager"), process: None },
    WhitelistEntry { title: Some("任务切换"), process: None },
    WhitelistEntry { title: Some("任务视图"), process: None },
    WhitelistEntry { title: Some("搜索"), process: Some("SearchApp") },
    WhitelistEntry { title: Some("音量控制"), process: Some("ShellExperienceHost") },
    WhitelistEntry { title: Some("电池信息"), process: Some("ShellExperienceHost") },
    WhitelistEntry { title: Some("日期和时间信息"), process: Some("ShellExperienceHost") },
    WhitelistEntry { title: Some("网络连接"), process: Some("ShellExperienceHost") },
    WhitelistEntry { title: Some("操作中心"), process: Some("ShellExperienceHost") },
    WhitelistEntry { title: Some("启动"), process: Some("StartMenuExperienceHost") },
    WhitelistEntry { title: Some("运行"), process: Some("explorer") },
    WhitelistEntry { title: Some("Cortana"), process: Some("SearchUI") },
    WhitelistEntry { title: Some("小娜"), process: Some("SearchUI") },
    WhitelistEntry { title: Some("EarTrumpet"), process: Some("EarTrumpet") },
    WhitelistEntry { title: Some("Menu"), process: Some("steamwebhelper") },
    WhitelistEntry { title: Some("好友列表"), process: Some("steamwebhelper") },
    WhitelistEntry { title: Some("WeiXin"), process: Some("WeChat") },
    WhitelistEntry { title: Some("TRAY_ICON_MENU_FORM"), process: Some("uu") },
    WhitelistEntry { title: Some("PixPin"), process: Some("PixPin") },
    WhitelistEntry { title: Some("企业微信"), process: Some("WXWork") },
    WhitelistEntry { title: Some("鱼书"), process: Some("fish-book") },
    WhitelistEntry { title: Some("Fish-book"), process: Some("fish-book") },
];

pub static SIZE_FILTERS: &[SizeFilter] = &[
    SizeFilter { min_width: 0, max_width: 400, min_height: 0, max_height: 400 },
];

// ── 规则跳过列表 ──

pub static RULE_SKIP_ENTRIES: &[WhitelistEntry] = &[
    WhitelistEntry { title: Some("复制文件"), process: Some("explorer") },
    WhitelistEntry { title: Some("移动文件"), process: Some("explorer") },
    WhitelistEntry { title: Some("推送提交"), process: Some("pycharm64") },
];

// ── 窗口规则 ──

#[allow(dead_code)]
pub enum SizeSpec {
    Pixels(i32),
    Percent(f64),
    Ratio(f64, f64),
    Unset,
}

pub struct WindowRule {
    pub title: &'static str,
    pub process: Option<&'static str>,
    pub width: SizeSpec,
    pub height: SizeSpec,
    pub left: Option<i32>,
    pub top: Option<i32>,
    pub right: Option<i32>,
    pub bottom: Option<i32>,
    pub center: bool,
}

pub static WINDOW_RULES: &[WindowRule] = &[
    WindowRule {
        title: "*欢迎访问 PyCharm*", process: Some("pycharm64"),
        width: SizeSpec::Unset, height: SizeSpec::Unset,
        left: None, top: None, right: None, bottom: None, center: false,
    },
    WindowRule {
        title: "*", process: Some("pycharm64"),
        width: SizeSpec::Percent(90.0), height: SizeSpec::Percent(90.0),
        left: None, top: None, right: None, bottom: None, center: true,
    },
];

// ── 匹配逻辑 ──

fn simple_match(pattern: &str, text: &str) -> bool {
    let p = pattern.to_lowercase();
    let t = text.to_lowercase();
    let parts: Vec<&str> = p.split('*').collect();
    if parts.is_empty() || (parts.len() == 1 && parts[0].is_empty()) { return true; }
    if !parts[0].is_empty() && !t.starts_with(parts[0]) { return false; }
    let last = parts.last().unwrap();
    if !last.is_empty() && !t.ends_with(last) { return false; }
    let mut pos = parts[0].len();
    for i in 1..parts.len() - 1 {
        let part = parts[i];
        if part.is_empty() { continue; }
        match t[pos..].find(part) {
            Some(idx) => pos += idx + part.len(),
            None => return false,
        }
    }
    true
}

// ── Win32 辅助 ──

/// 获取窗口进程名（不含 .exe）
pub fn get_process_name(hwnd: HWND) -> String {
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    };
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

    unsafe {
        let mut pid: u32 = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 { return String::new(); }

        let handle = OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid);
        if let Ok(handle) = handle {
            let mut buf = vec![0u16; 260];
            let mut len = buf.len() as u32;
            let result = QueryFullProcessImageNameW(
                handle, PROCESS_NAME_FORMAT(0),
                windows_core::PWSTR(buf.as_mut_ptr()),
                &mut len,
            );
            let _ = CloseHandle(handle);
            if result.is_ok() {
                let name = String::from_utf16_lossy(&buf[..len as usize]);
                let name = Path::new(&name)
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                return name;
            }
        }
    }
    String::new()
}

/// 获取窗口标题
pub fn get_window_title(hwnd: HWND) -> String {
    use windows::Win32::UI::WindowsAndMessaging::GetWindowTextW;
    unsafe {
        let mut buf = vec![0u16; 512];
        let len = GetWindowTextW(hwnd, &mut buf);
        String::from_utf16_lossy(&buf[..len as usize])
    }
}

/// 获取窗口矩形 (left, top, right, bottom)
pub fn get_window_rect(hwnd: HWND) -> (i32, i32, i32, i32) {
    use windows::Win32::UI::WindowsAndMessaging::GetWindowRect;
    unsafe {
        let mut rect = std::mem::zeroed();
        GetWindowRect(hwnd, &mut rect).ok();
        (rect.left, rect.top, rect.right, rect.bottom)
    }
}

/// 屏幕尺寸
pub fn screen_size() -> (i32, i32) {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    unsafe {
        (GetSystemMetrics(SM_CXSCREEN), GetSystemMetrics(SM_CYSCREEN))
    }
}

/// 居中窗口
pub fn center_window(hwnd: HWND) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::SetWindowPos;
    let (left, _top, right, bottom) = get_window_rect(hwnd);
    let w = right - left;
    let h = bottom - _top;
    let (sw, sh) = screen_size();
    let x = (sw - w) / 2;
    let y = (sh - h) / 2;
    unsafe {
        SetWindowPos(hwnd, None, x, y, 0, 0,
            windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE | windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER,
        ).is_ok()
    }
}

/// 移动窗口
pub fn move_window_to(hwnd: HWND, x: i32, y: i32) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::SetWindowPos;
    unsafe {
        SetWindowPos(hwnd, None, x, y, 0, 0,
            windows::Win32::UI::WindowsAndMessaging::SWP_NOSIZE | windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER,
        ).is_ok()
    }
}

/// 设置窗口位置和大小
pub fn set_window_pos(hwnd: HWND, x: i32, y: i32, w: i32, h: i32) -> bool {
    use windows::Win32::UI::WindowsAndMessaging::SetWindowPos;
    unsafe {
        SetWindowPos(hwnd, None, x, y, w, h,
            windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER,
        ).is_ok()
    }
}

/// 等比例缩放窗口
pub fn scale_window(hwnd: HWND, factor: f64) -> bool {
    let (left, _top, right, bottom) = get_window_rect(hwnd);
    let w = right - left;
    let h = bottom - _top;
    let nw = ((w as f64) * (1.0 + factor)).round() as i32;
    let nh = ((h as f64) * (1.0 + factor)).round() as i32;
    let (sw, sh) = screen_size();
    let nw = nw.min(sw);
    let nh = nh.min(sh);
    set_window_pos(hwnd, (sw - nw) / 2, (sh - nh) / 2, nw, nh)
}

/// 缩放宽度
pub fn scale_width(hwnd: HWND, factor: f64) {
    use windows::Win32::UI::WindowsAndMessaging::SetWindowPos;
    let (left, top, right, bottom) = get_window_rect(hwnd);
    let w = right - left;
    let h = bottom - top;
    let nw = ((w as f64) * (1.0 + factor)).round() as i32;
    let (sw, _) = screen_size();
    let nw = nw.min(sw);
    unsafe {
        SetWindowPos(hwnd, None, (sw - nw) / 2, top, nw, h,
            windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER,
        ).ok();
    }
}

/// 缩放高度
pub fn scale_height(hwnd: HWND, factor: f64) {
    use windows::Win32::UI::WindowsAndMessaging::SetWindowPos;
    let (left, top, right, bottom) = get_window_rect(hwnd);
    let w = right - left;
    let h = bottom - top;
    let nh = ((h as f64) * (1.0 + factor)).round() as i32;
    let (_sw, sh) = screen_size();
    let nh = nh.min(sh);
    unsafe {
        SetWindowPos(hwnd, None, left, (sh - nh) / 2, w, nh,
            windows::Win32::UI::WindowsAndMessaging::SWP_NOZORDER,
        ).ok();
    }
}

/// 检查白名单
pub fn is_whitelisted(title: &str, process_name: &str, rect: (i32, i32, i32, i32)) -> bool {
    let t = title.to_lowercase();
    for entry in WHITELIST_ENTRIES.iter() {
        if let Some(et) = entry.title {
            if t.contains(&et.to_lowercase()) {
                if let Some(ep) = entry.process {
                    if process_name.to_lowercase() == ep.to_lowercase() { return true; }
                } else {
                    return true;
                }
            }
        }
    }
    let (left, top, right, bottom) = rect;
    let w = right - left;
    let h = bottom - top;
    for f in SIZE_FILTERS.iter() {
        if w >= f.min_width && w <= f.max_width && h >= f.min_height && h <= f.max_height {
            return true;
        }
    }
    false
}

/// 规则跳过列表检查
pub fn is_rule_skipped(title: &str, process_name: &str) -> bool {
    for entry in RULE_SKIP_ENTRIES.iter() {
        if let (Some(et), Some(ep)) = (entry.title, entry.process) {
            if title.to_lowercase().contains(&et.to_lowercase())
                && process_name.to_lowercase() == ep.to_lowercase()
            {
                return true;
            }
        }
    }
    false
}

/// 匹配窗口规则
pub fn match_rule(title: &str, process_name: &str) -> Option<usize> {
    for (i, rule) in WINDOW_RULES.iter().enumerate() {
        if simple_match(rule.title, title) {
            let pm = match rule.process {
                Some(rp) => process_name.to_lowercase() == rp.to_lowercase(),
                None => true,
            };
            if pm { return Some(i); }
        }
    }
    None
}

/// 应用窗口规则
pub fn apply_rule(hwnd: HWND, rule_idx: usize) -> bool {
    let rule = &WINDOW_RULES[rule_idx];
    let (sw, sh) = screen_size();
    let fw = resolve_size(&rule.width, sw);
    let fh = resolve_size(&rule.height, sh);
    let (mut fx, mut fy) = if rule.center {
        ((sw - fw) / 2, (sh - fh) / 2)
    } else {
        (rule.left.unwrap_or(0), rule.top.unwrap_or(0))
    };
    if let Some(l) = rule.left { fx = l; }
    if let Some(t) = rule.top { fy = t; }
    if let Some(r) = rule.right { fx = sw - fw - r; }
    if let Some(b) = rule.bottom { fy = sh - fh - b; }
    set_window_pos(hwnd, fx, fy, fw, fh)
}

fn resolve_size(spec: &SizeSpec, screen: i32) -> i32 {
    match spec {
        SizeSpec::Pixels(px) => (*px).min(screen),
        SizeSpec::Percent(pct) => ((screen as f64) * pct / 100.0).round() as i32,
        SizeSpec::Ratio(n, d) => ((screen as f64) * n / d).round() as i32,
        SizeSpec::Unset => screen,
    }
}

/// 9 宫格定位
pub fn pos_by_key(key: char, hwnd: HWND) -> Option<(i32, i32)> {
    let (left, _top, right, bottom) = get_window_rect(hwnd);
    let w = right - left;
    let h = bottom - _top;
    let (sw, sh) = screen_size();
    match key {
        '1' => Some((0, sh - h)),
        '2' => Some(((sw - w) / 2, sh - h)),
        '3' => Some((sw - w, sh - h)),
        '4' => Some((0, (sh - h) / 2)),
        '5' => Some(((sw - w) / 2, (sh - h) / 2)),
        '6' => Some((sw - w, (sh - h) / 2)),
        '7' => Some((0, 0)),
        '8' => Some(((sw - w) / 2, 0)),
        '9' => Some((sw - w, 0)),
        _ => None,
    }
}
