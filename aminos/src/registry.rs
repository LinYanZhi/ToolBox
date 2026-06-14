/// 重新导出 sys::registry，保持 aminos 内部接口不变。
pub use sys::registry::scan_all_installed;

use std::collections::HashMap;

/// 检测指定软件是否已安装，返回注册表键值对。
///
/// 直接委托给 `sys::registry::detect_installed_by`，避免重写注册表遍历逻辑。
pub fn detect_installed(detection: &crate::software::Detection) -> Option<HashMap<String, String>> {
    let dn = detection.display_name.as_deref()?;
    let publisher = detection.publisher.as_deref();
    sys::registry::detect_installed_by(dn, publisher)
}
