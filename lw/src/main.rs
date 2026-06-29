//! lw — 窗口位置和大小管理工具
//!
//! 三种模式：
//!   -b  后台模式：静默自动居中
//!   -i  交互模式：快捷键 + 鼠标操作
//!   -r  规则模式：根据配置自动调整窗口

mod config;

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use arg::*;
use color::Style;
use rdev::Key;
use windows::Win32::Foundation::HWND;

use config as cfg;

// ── 颜色常量 ──

const CLR_CENTER: u8 = 94;
const CLR_RULE: u8 = 92;
const CLR_INTERACT: u8 = 96;
const CLR_SKIP: u8 = 93;

const CLR_AUTO: u8 = 94;
const CLR_MATCH: u8 = 92;
const CLR_SCALE: u8 = 95;
const CLR_MOVE: u8 = 96;
const CLR_POSITION: u8 = 92;
const CLR_NUDGE: u8 = 93;
const CLR_EDGE: u8 = 95;
const CLR_RESIZE: u8 = 95;

const CLR_SUCCESS: u8 = 92;
const CLR_FAIL: u8 = 91;
const CLR_PAUSE: u8 = 93;
const CLR_RESUME: u8 = 92;

const CLR_WHITELIST: u8 = 92;
const CLR_RULES: u8 = 92;
const CLR_TIP: u8 = 93;
const CLR_GRAY: u8 = 90;

// ── 全局状态 ──

static HOOK_PAUSED: AtomicBool = AtomicBool::new(false);

// ── CLI ────────────────────────────────────────────

fn build_cmd() -> Cmd {
    Cmd::new("lw")
        .about("窗口位置和大小管理工具 — 自动居中、快捷键交互、规则匹配")
        .arg(flag("help", 'h', "显示帮助").global())
        .arg(flag("version", 'V', "显示版本号").global())
        .arg(flag("back", 'b', "后台模式：静默自动居中"))
        .arg(flag("input", 'i', "交互模式：启用快捷键和鼠标操作"))
        .arg(flag("rule", 'r', "规则模式：根据配置调整窗口"))
        .arg(flag("no-esc", 'n', "禁用 Esc 键退出功能"))
}

fn main() {
    init();

    let cmd = build_cmd();
    let argv: Vec<String> = std::env::args().collect();
    let args = match parse(&cmd, &argv) {
        Ok(a) => a,
        Err(e) => { print_error(&e); return; }
    };

    let exe_path = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "lw".into());

    if args.flag("help") {
        print_help(&cmd, &exe_path);
        return;
    }
    if args.flag("version") {
        print_version(&cmd, "0.1.0", "");
        return;
    }

    let has_back = args.flag("back");
    let has_input = args.flag("input");
    let has_rule = args.flag("rule");
    let no_esc = args.flag("no-esc");

    // 无参数默认 = 交互模式
    let (back, input, rule) = if !has_back && !has_input && !has_rule {
        (false, true, false)
    } else {
        (has_back, has_input, has_rule)
    };

    show_mode_info(back, input, rule);

    if back || rule {
        run_background(back, rule, input, no_esc);
    } else {
        run_interactive(no_esc);
    }
}

fn show_mode_info(back: bool, input: bool, rule: bool) {
    let bs = if back { c("后台", CLR_CENTER) } else { "后台".into() };
    let is = if input { c("交互", CLR_INTERACT) } else { "交互".into() };
    let rs = if rule { c("规则", CLR_RULE) } else { "规则".into() };
    println!("模式配置: {bs}={back}, {is}={input}, {rs}={rule}");
}

// ── 交互模式 ────────────────────────────────────────

fn run_interactive(no_esc: bool) {
    println!("{}", c("交互模式已启动", CLR_INTERACT));
    println!("快捷键列表：");
    println!("  Alt + 数字键：{}", c("快速定位", CLR_POSITION));
    println!("  Alt + 滚轮：{}", c("等比例缩放", CLR_SCALE));
    println!("  Shift + 滚轮：{}", c("垂直移动", CLR_MOVE));
    println!("  Ctrl + 滚轮：{}", c("水平移动", CLR_MOVE));
    println!("  Alt + 方向键：{}", c("微调位置", CLR_NUDGE));
    println!("  Alt + Ctrl + 方向键：{}", c("调整大小", CLR_RESIZE));
    println!("{} 输入 ` 可暂停/恢复快捷键和鼠标监听", c("提示：", CLR_TIP));
    if !no_esc {
        println!("按 Esc 键退出程序...");
    }

    let modifiers: Arc<Mutex<HashSet<Key>>> = Arc::new(Mutex::new(HashSet::new()));
    let m = modifiers.clone();

    thread::spawn(move || {
        if let Err(e) = rdev::listen(move |event| {
            handle_event(event, &m, no_esc);
        }) {
            eprintln!("监听失败: {e:?}");
        }
    });

    loop {
        thread::sleep(Duration::from_millis(200));
    }
}

// ── 后台模式 ────────────────────────────────────────

fn run_background(center: bool, rule: bool, interactive: bool, no_esc: bool) {
    let mut modes = vec![];
    if center { modes.push(c("自动居中", CLR_CENTER)); }
    if rule { modes.push(c("规则匹配", CLR_RULE)); }
    if interactive { modes.push(c("快捷键", CLR_INTERACT)); }
    println!("后台模式已启动: {}", modes.join(", "));

    if !no_esc {
        println!("按 Ctrl+C 或 Esc 键退出程序...");
    }

    // 键盘/鼠标钩子（交互启用时）
    if interactive {
        let modifiers: Arc<Mutex<HashSet<Key>>> = Arc::new(Mutex::new(HashSet::new()));
        let m = modifiers.clone();
        thread::spawn(move || {
            if let Err(e) = rdev::listen(move |event| {
                handle_event(event, &m, no_esc);
            }) {
                eprintln!("监听失败: {e:?}");
            }
        });
        println!("{} 输入 ` 可暂停/恢复键盘和鼠标监听", c("提示：", CLR_TIP));
    }

    println!("{}: {} 标题, {} 尺寸过滤",
        c("白名单已加载", CLR_WHITELIST),
        cfg::WHITELIST_ENTRIES.len(),
        cfg::SIZE_FILTERS.len());

    if rule {
        println!("{}: {} 条", c("窗口规则已加载", CLR_RULES), cfg::WINDOW_RULES.len());
        println!("{}: {} 条", c("规则跳过列表已加载", CLR_RULES), cfg::RULE_SKIP_ENTRIES.len());
    }

    let mut last_hwnd = HWND(std::ptr::null_mut());

    loop {
        unsafe {
            let hwnd = windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
            if hwnd.0.is_null() {
                thread::sleep(Duration::from_millis(50));
                continue;
            }
            if hwnd == last_hwnd {
                thread::sleep(Duration::from_millis(50));
                continue;
            }
            last_hwnd = hwnd;

            let title = cfg::get_window_title(hwnd);
            if title.is_empty() { continue; }

            let rect = cfg::get_window_rect(hwnd);
            let process_name = cfg::get_process_name(hwnd);

            // 白名单
            if cfg::is_whitelisted(&title, &process_name, rect) {
                log_skip("白名单", &process_name, &title);
                continue;
            }

            if rule {
                // 规则跳过
                if cfg::is_rule_skipped(&title, &process_name) {
                    log_skip("规则跳过", &process_name, &title);
                } else if let Some(ri) = cfg::match_rule(&title, &process_name) {
                    let old_rect = rect;
                    let applied = cfg::apply_rule(hwnd, ri);
                    let new_rect = cfg::get_window_rect(hwnd);
                    if old_rect != new_rect {
                        log_change("规则", CLR_RULE, "匹配", CLR_MATCH, hwnd, &process_name, &title, old_rect, new_rect);
                    } else if !applied {
                        log_status("规则", CLR_RULE, "无权限", CLR_FAIL, hwnd, &process_name, &title);
                    } else {
                        log_status("规则", CLR_RULE, "已定位", CLR_SUCCESS, hwnd, &process_name, &title);
                    }
                    continue;
                }
            }

            if center {
                try_center(hwnd);
            }
        }
    }
}

fn try_center(hwnd: HWND) {
    let old = cfg::get_window_rect(hwnd);
    let process_name = cfg::get_process_name(hwnd);
    let title = cfg::get_window_title(hwnd);

    if cfg::center_window(hwnd) {
        let new = cfg::get_window_rect(hwnd);
        if old != new {
            log_change("居中", CLR_CENTER, "自动", CLR_AUTO, hwnd, &process_name, &title, old, new);
        }
    } else {
        log_status("居中", CLR_CENTER, "无权限", CLR_FAIL, hwnd, &process_name, &title);
    }
}

// ── rdev 事件分发 ───────────────────────────────────

fn handle_event(event: rdev::Event, modifiers: &Arc<Mutex<HashSet<Key>>>, no_esc: bool) {
    match event.event_type {
        rdev::EventType::KeyPress(key) => {
            let mut m = modifiers.lock().unwrap();
            m.insert(key);

            // Esc 退出
            if key == Key::Escape && !no_esc {
                println!("{}", c("程序已退出", CLR_FAIL));
                std::process::exit(0);
            }

            // 反引号（`）切换暂停
            if key == Key::BackQuote {
                let was_paused = HOOK_PAUSED.swap(!HOOK_PAUSED.load(Ordering::Relaxed), Ordering::Relaxed);
                if was_paused {
                    println!("\n{} 按键监听 {} - 输入 ` 再次切换",
                        c("[控制]", CLR_INTERACT), c("已恢复", CLR_RESUME));
                    println!("{} 快捷键和鼠标监听已恢复", c("提示：", CLR_TIP));
                } else {
                    println!("\n{} 按键监听 {} - 输入 ` 再次切换",
                        c("[控制]", CLR_INTERACT), c("已暂停", CLR_PAUSE));
                    println!("{} 现在可以用 Ctrl+滚轮 调整控制台字体大小了！", c("提示：", CLR_TIP));
                }
                drop(m);
                return;
            }

            // 贴边快捷键
            let alt = m.contains(&Key::Alt) || m.contains(&Key::AltGr);
            if alt {
                match key {
                    Key::LeftArrow => { drop(m); edge_left(); return; }
                    Key::RightArrow => { drop(m); edge_right(); return; }
                    Key::UpArrow => { drop(m); edge_up(); return; }
                    Key::DownArrow => { drop(m); edge_down(); return; }
                    _ => {}
                }
            }

            drop(m);

            if HOOK_PAUSED.load(Ordering::Relaxed) { return; }
            on_keypress(key, modifiers);
        }
        rdev::EventType::KeyRelease(key) => {
            modifiers.lock().unwrap().remove(&key);
        }
        rdev::EventType::Wheel { delta_y, .. } => {
            if HOOK_PAUSED.load(Ordering::Relaxed) { return; }
            let m = modifiers.lock().unwrap();
            let alt = m.contains(&Key::Alt) || m.contains(&Key::AltGr);
            let shift = m.contains(&Key::ShiftLeft) || m.contains(&Key::ShiftRight);
            let ctrl = m.contains(&Key::ControlLeft) || m.contains(&Key::ControlRight);
            drop(m);
            if !alt && !shift && !ctrl { return; }
            on_scroll(alt, shift, ctrl, delta_y);
        }
        _ => {}
    }
}

fn on_keypress(key: Key, modifiers: &Arc<Mutex<HashSet<Key>>>) {
    let m = modifiers.lock().unwrap();
    let alt = m.contains(&Key::Alt) || m.contains(&Key::AltGr);
    let ctrl = m.contains(&Key::ControlLeft) || m.contains(&Key::ControlRight);
    drop(m);

    unsafe {
        let hwnd = windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        if hwnd.0.is_null() { return; }

        if alt && !ctrl {
            // Alt + 数字键 9 宫格
            if let Some(num) = key_to_num(key) {
                if let Some((x, y)) = cfg::pos_by_key(num, hwnd) {
                    let old = cfg::get_window_rect(hwnd);
                    cfg::move_window_to(hwnd, x, y);
                    let new = cfg::get_window_rect(hwnd);
                    if old != new {
                        let pn = cfg::get_process_name(hwnd);
                        let title = cfg::get_window_title(hwnd);
                        log_change("交互", CLR_INTERACT, &format!("定位-{num}"), CLR_POSITION, hwnd, &pn, &title, old, new);
                    }
                }
                return;
            }

            // Alt + 方向键微调
            match key {
                Key::LeftArrow => nudge(hwnd, -cfg::MOVE_STEP, 0, "微调-左"),
                Key::RightArrow => nudge(hwnd, cfg::MOVE_STEP, 0, "微调-右"),
                Key::UpArrow => nudge(hwnd, 0, -cfg::MOVE_STEP, "微调-上"),
                Key::DownArrow => nudge(hwnd, 0, cfg::MOVE_STEP, "微调-下"),
                _ => {}
            }
        }

        if alt && ctrl {
            match key {
                Key::LeftArrow => scale_width_log(hwnd, -cfg::SCALE_STEP),
                Key::RightArrow => scale_width_log(hwnd, cfg::SCALE_STEP),
                Key::UpArrow => scale_height_log(hwnd, cfg::SCALE_STEP),
                Key::DownArrow => scale_height_log(hwnd, -cfg::SCALE_STEP),
                _ => {}
            }
        }
    }
}

fn edge_left() {
    unsafe {
        let hwnd = windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        if hwnd.0.is_null() { return; }
        let old = cfg::get_window_rect(hwnd);
        let (_l, t, _r, _b) = old;
        cfg::move_window_to(hwnd, 0, t);
        let new = cfg::get_window_rect(hwnd);
        if old != new {
            log_iaction("贴边-左", CLR_EDGE, hwnd, old, new);
        }
    }
}

fn edge_right() {
    unsafe {
        let hwnd = windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        if hwnd.0.is_null() { return; }
        let old = cfg::get_window_rect(hwnd);
        let (ol, ot, orr, _ob) = old;
        let (sw, _) = cfg::screen_size();
        cfg::move_window_to(hwnd, sw - (orr - ol), ot);
        let new = cfg::get_window_rect(hwnd);
        if old != new {
            log_iaction("贴边-右", CLR_EDGE, hwnd, old, new);
        }
    }
}

fn edge_up() {
    unsafe {
        let hwnd = windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        if hwnd.0.is_null() { return; }
        let old = cfg::get_window_rect(hwnd);
        let (l, _t, _r, _b) = old;
        cfg::move_window_to(hwnd, l, 0);
        let new = cfg::get_window_rect(hwnd);
        if old != new {
            log_iaction("贴边-上", CLR_EDGE, hwnd, old, new);
        }
    }
}

fn edge_down() {
    unsafe {
        let hwnd = windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        if hwnd.0.is_null() { return; }
        let old = cfg::get_window_rect(hwnd);
        let (l, t, _r, b) = old;
        let (_sw, sh) = cfg::screen_size();
        cfg::move_window_to(hwnd, l, sh - (b - t));
        let new = cfg::get_window_rect(hwnd);
        if old != new {
            log_iaction("贴边-下", CLR_EDGE, hwnd, old, new);
        }
    }
}

fn nudge(hwnd: HWND, dx: i32, dy: i32, label: &str) {
    let old = cfg::get_window_rect(hwnd);
    let (l, t, _r, _b) = old;
    cfg::move_window_to(hwnd, l + dx, t + dy);
    let new = cfg::get_window_rect(hwnd);
    if old != new {
        log_iaction(label, CLR_NUDGE, hwnd, old, new);
    }
}

fn scale_width_log(hwnd: HWND, f: f64) {
    let old = cfg::get_window_rect(hwnd);
    cfg::scale_width(hwnd, f);
    let new = cfg::get_window_rect(hwnd);
    if old != new {
        log_iaction(if f > 0.0 { "缩宽-右" } else { "缩宽-左" }, CLR_RESIZE, hwnd, old, new);
    }
}

fn scale_height_log(hwnd: HWND, f: f64) {
    let old = cfg::get_window_rect(hwnd);
    cfg::scale_height(hwnd, f);
    let new = cfg::get_window_rect(hwnd);
    if old != new {
        log_iaction(if f > 0.0 { "缩高-上" } else { "缩高-下" }, CLR_RESIZE, hwnd, old, new);
    }
}

fn on_scroll(alt: bool, shift: bool, ctrl: bool, dy: i64) {
    unsafe {
        let hwnd = windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
        if hwnd.0.is_null() { return; }

        if alt {
            let old = cfg::get_window_rect(hwnd);
            let scale_factor = if dy > 0 { cfg::SCALE_STEP } else { -cfg::SCALE_STEP };
            cfg::scale_window(hwnd, scale_factor);
            let new = cfg::get_window_rect(hwnd);
            if old != new {
                let action = if dy > 0 { c("放大", CLR_SUCCESS) } else { c("缩小", CLR_PAUSE) };
                log_iaction_detail("缩放", CLR_SCALE, &action, hwnd, old, new);
            }
        } else if shift {
            let old = cfg::get_window_rect(hwnd);
            let (l, t, _r, _b) = old;
            let step = cfg::MOVE_STEP as i64;
            cfg::move_window_to(hwnd, l, t - (dy * step) as i32);
            let new = cfg::get_window_rect(hwnd);
            if old != new {
                log_iaction("垂直移动", CLR_MOVE, hwnd, old, new);
            }
        } else if ctrl {
            let old = cfg::get_window_rect(hwnd);
            let (l, t, _r, _b) = old;
            let step = cfg::MOVE_STEP as i64;
            cfg::move_window_to(hwnd, l - (dy * step) as i32, t);
            let new = cfg::get_window_rect(hwnd);
            if old != new {
                log_iaction("水平移动", CLR_MOVE, hwnd, old, new);
            }
        }
    }
}

// ── 日志辅助 ──

fn now() -> String {
    use std::time::SystemTime;
    let t = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = t.as_secs() as i64;
    let total = secs + 8 * 3600; // UTC+8
    let days = total / 86400;
    let time_secs = total % 86400;
    let h = time_secs / 3600;
    let m = (time_secs % 3600) / 60;
    let s = time_secs % 60;

    let mut y = 1970i64;
    let mut d = days;
    loop {
        let yd = if is_leap(y) { 366 } else { 365 };
        if d < yd { break; }
        d -= yd;
        y += 1;
    }
    let leap = is_leap(y);
    let mdays: [i64; 12] = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut mo = 1;
    for &md in &mdays {
        if d < md { break; }
        d -= md;
        mo += 1;
    }
    format!("{y:04}-{mo:02}-{:02} {h:02}:{m:02}:{s:02}", d + 1)
}

fn is_leap(y: i64) -> bool { (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 }

fn c(text: &str, clr: u8) -> String { Style::new(clr).paint(text) }

fn win_info(hwnd: HWND, process: &str, title: &str) -> String {
    let h = hwnd.0 as usize;
    let colors: [u8; 12] = [94, 91, 93, 92, 96, 95, 31, 32, 33, 34, 35, 36];
    let clr = colors[h.wrapping_mul(3) % colors.len()];
    format!("{} {}",
        Style::new(clr).paint(format!("[{process}]")), title)
}

fn format_change(old: (i32, i32, i32, i32), new: (i32, i32, i32, i32)) -> String {
    let (ol, ot, orr, ob) = old;
    let (nl, nt, nr, nb) = new;
    let ow = orr - ol;
    let oh = ob - ot;
    let nw = nr - nl;
    let nh = nb - nt;
    format!("({ol},{ot},{ow}x{oh}) -> ({nl},{nt},{nw}x{nh})")
}

fn log_skip(reason: &str, process: &str, title: &str) {
    println!("[{}] {} {} {} {}",
        now(), c("[跳过]", CLR_SKIP), c(&format!("[{reason}]"), CLR_GRAY), process, title);
}

fn log_status(mode: &str, mode_clr: u8, action: &str, action_clr: u8, hwnd: HWND, pn: &str, title: &str) {
    let info = win_info(hwnd, pn, title);
    println!("[{}] {} {} {}",
        now(), c(&format!("[{mode}]"), mode_clr), c(&format!("[{action}]"), action_clr), info);
}

fn log_change(mode: &str, mode_clr: u8, action: &str, action_clr: u8, hwnd: HWND, pn: &str, title: &str, old: (i32, i32, i32, i32), new: (i32, i32, i32, i32)) {
    let info = win_info(hwnd, pn, title);
    let change = format_change(old, new);
    println!("[{}] {} {} {} {}",
        now(), c(&format!("[{mode}]"), mode_clr), c(&format!("[{action}]"), action_clr), info, change);
}

fn log_iaction(label: &str, action_clr: u8, hwnd: HWND, old: (i32, i32, i32, i32), new: (i32, i32, i32, i32)) {
    let pn = cfg::get_process_name(hwnd);
    let title = cfg::get_window_title(hwnd);
    log_change("交互", CLR_INTERACT, label, action_clr, hwnd, &pn, &title, old, new);
}

fn log_iaction_detail(action: &str, action_clr: u8, detail: &str, hwnd: HWND, old: (i32, i32, i32, i32), new: (i32, i32, i32, i32)) {
    let pn = cfg::get_process_name(hwnd);
    let title = cfg::get_window_title(hwnd);
    let info = win_info(hwnd, &pn, &title);
    let change = format_change(old, new);
    println!("[{}] {} {} {} {} {}",
        now(), c("[交互]", CLR_INTERACT), c(&format!("[{action}]"), action_clr), detail, info, change);
}

// ── 键盘码 → 数字 ──

fn key_to_num(key: Key) -> Option<char> {
    match key {
        Key::Num1 => Some('1'),
        Key::Num2 => Some('2'),
        Key::Num3 => Some('3'),
        Key::Num4 => Some('4'),
        Key::Num5 => Some('5'),
        Key::Num6 => Some('6'),
        Key::Num7 => Some('7'),
        Key::Num8 => Some('8'),
        Key::Num9 => Some('9'),
        _ => None,
    }
}
