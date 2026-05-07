/// 键盘布局检测 - 获取系统当前键盘布局和物理键位映射
use serde::{Deserialize, Serialize};

/// 键盘布局信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyboardLayoutInfo {
    /// 布局名称（如 "US", "Japanese", "German"）
    pub name: String,
    /// 布局 ID（如 "com.apple.keylayout.US"）
    pub id: String,
    /// 是否为物理键盘
    pub is_physical: bool,
    /// 平台特定信息
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_info: Option<PlatformKeyboardInfo>,
}

/// 平台特定键盘信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlatformKeyboardInfo {
    /// 扫描码到键名的映射
    pub scan_code_map: Option<serde_json::Value>,
    /// 布局变体
    pub variant: Option<String>,
}

/// 获取当前键盘布局
pub fn get_keyboard_layout() -> Result<KeyboardLayoutInfo, String> {
    #[cfg(target_os = "macos")]
    {
        get_keyboard_layout_macos()
    }

    #[cfg(target_os = "windows")]
    {
        get_keyboard_layout_windows()
    }

    #[cfg(target_os = "linux")]
    {
        get_keyboard_layout_linux()
    }
}

/// 获取物理键位映射（扫描码到键名）
pub fn get_physical_mapping() -> Result<serde_json::Value, String> {
    #[cfg(target_os = "macos")]
    {
        get_physical_mapping_macos()
    }

    #[cfg(target_os = "windows")]
    {
        get_physical_mapping_windows()
    }

    #[cfg(target_os = "linux")]
    {
        get_physical_mapping_linux()
    }
}

// ==================== macOS 实现 ====================

#[cfg(target_os = "macos")]
fn get_keyboard_layout_macos() -> Result<KeyboardLayoutInfo, String> {
    use std::process::Command;

    // 获取当前输入源
    let output = Command::new("defaults")
        .args(["read", "com.apple.HIToolbox", "AppleCurrentKeyboardLayoutInputSourceID"])
        .output()
        .map_err(|e| format!("Failed to get keyboard layout: {}", e))?;

    let layout_id = String::from_utf8_lossy(&output.stdout)
        .trim()
        .trim_matches('"')
        .to_string();

    // 获取布局名称
    let name = layout_id
        .split('.')
        .last()
        .unwrap_or("Unknown")
        .to_string();

    Ok(KeyboardLayoutInfo {
        name,
        id: layout_id,
        is_physical: true,
        platform_info: Some(PlatformKeyboardInfo {
            scan_code_map: None,
            variant: None,
        }),
    })
}

#[cfg(target_os = "macos")]
fn get_physical_mapping_macos() -> Result<serde_json::Value, String> {
    // macOS 使用 Carbon API 获取键盘布局
    // 这里返回一个简化的映射
    let mut map = serde_json::Map::new();

    // 常用键位映射（简化版）
    map.insert("0".to_string(), serde_json::json!("KeyA"));
    map.insert("1".to_string(), serde_json::json!("KeyS"));
    map.insert("2".to_string(), serde_json::json!("KeyD"));
    map.insert("3".to_string(), serde_json::json!("KeyF"));
    map.insert("4".to_string(), serde_json::json!("KeyH"));
    map.insert("5".to_string(), serde_json::json!("KeyG"));
    map.insert("6".to_string(), serde_json::json!("KeyZ"));
    map.insert("7".to_string(), serde_json::json!("KeyX"));
    map.insert("8".to_string(), serde_json::json!("KeyC"));
    map.insert("9".to_string(), serde_json::json!("KeyV"));

    Ok(serde_json::Value::Object(map))
}

// ==================== Windows 实现 ====================

#[cfg(target_os = "windows")]
fn get_keyboard_layout_windows() -> Result<KeyboardLayoutInfo, String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyboardLayout;
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    unsafe {
        let foreground_window = GetForegroundWindow();
        let thread_id = windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(
            foreground_window,
            None,
        );

        let hkl = GetKeyboardLayout(thread_id);
        let layout_id = format!("{:08x}", hkl.0 as u32);

        // 解析布局 ID 获取语言 ID
        let lang_id = hkl.0 as u32 & 0xFFFF;
        let name = match lang_id {
            0x0409 => "US",
            0x0411 => "Japanese",
            0x0407 => "German",
            0x040C => "French",
            0x0804 => "Chinese (Simplified)",
            0x0404 => "Chinese (Traditional)",
            _ => "Unknown",
        };

        Ok(KeyboardLayoutInfo {
            name: name.to_string(),
            id: layout_id,
            is_physical: true,
            platform_info: Some(PlatformKeyboardInfo {
                scan_code_map: None,
                variant: None,
            }),
        })
    }
}

#[cfg(target_os = "windows")]
fn get_physical_mapping_windows() -> Result<serde_json::Value, String> {
    // Windows 使用 GetKeyboardState 获取键位状态
    let mut map = serde_json::Map::new();

    // 常用键位映射（简化版）
    map.insert("0".to_string(), serde_json::json!("KeyA"));
    map.insert("1".to_string(), serde_json::json!("KeyS"));
    map.insert("2".to_string(), serde_json::json!("KeyD"));
    map.insert("3".to_string(), serde_json::json!("KeyF"));
    map.insert("4".to_string(), serde_json::json!("KeyH"));
    map.insert("5".to_string(), serde_json::json!("KeyG"));
    map.insert("6".to_string(), serde_json::json!("KeyZ"));
    map.insert("7".to_string(), serde_json::json!("KeyX"));
    map.insert("8".to_string(), serde_json::json!("KeyC"));
    map.insert("9".to_string(), serde_json::json!("KeyV"));

    Ok(serde_json::Value::Object(map))
}

// ==================== Linux 实现 ====================

#[cfg(target_os = "linux")]
fn get_keyboard_layout_linux() -> Result<KeyboardLayoutInfo, String> {
    use std::process::Command;

    // 尝试使用 setxkbmap 获取布局
    let output = Command::new("setxkbmap")
        .arg("-query")
        .output();

    if let Ok(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut layout = "us".to_string();
        let mut variant = String::new();

        for line in stdout.lines() {
            if line.starts_with("layout:") {
                layout = line.split(':').nth(1).unwrap_or("us").trim().to_string();
            }
            if line.starts_with("variant:") {
                variant = line.split(':').nth(1).unwrap_or("").trim().to_string();
            }
        }

        let name = layout.clone();
        let id = format!("xkb_{}", layout);

        return Ok(KeyboardLayoutInfo {
            name,
            id,
            is_physical: true,
            platform_info: Some(PlatformKeyboardInfo {
                scan_code_map: None,
                variant: if variant.is_empty() { None } else { Some(variant) },
            }),
        });
    }

    // 如果 setxkbmap 不可用，尝试读取 XKB 配置
    if let Ok(content) = std::fs::read_to_string("/etc/default/keyboard") {
        for line in content.lines() {
            if line.starts_with("XKBLAYOUT=") {
                let layout = line.trim_start_matches("XKBLAYOUT=")
                    .trim_matches('"')
                    .to_string();
                return Ok(KeyboardLayoutInfo {
                    name: layout.clone(),
                    id: format!("xkb_{}", layout),
                    is_physical: true,
                    platform_info: None,
                });
            }
        }
    }

    // 默认返回 US
    Ok(KeyboardLayoutInfo {
        name: "US".to_string(),
        id: "xkb_us".to_string(),
        is_physical: true,
        platform_info: None,
    })
}

#[cfg(target_os = "linux")]
fn get_physical_mapping_linux() -> Result<serde_json::Value, String> {
    // Linux 使用 evdev 扫描码
    let mut map = serde_json::Map::new();

    // 常用键位映射（简化版）
    map.insert("0".to_string(), serde_json::json!("KeyA"));
    map.insert("1".to_string(), serde_json::json!("KeyS"));
    map.insert("2".to_string(), serde_json::json!("KeyD"));
    map.insert("3".to_string(), serde_json::json!("KeyF"));
    map.insert("4".to_string(), serde_json::json!("KeyH"));
    map.insert("5".to_string(), serde_json::json!("KeyG"));
    map.insert("6".to_string(), serde_json::json!("KeyZ"));
    map.insert("7".to_string(), serde_json::json!("KeyX"));
    map.insert("8".to_string(), serde_json::json!("KeyC"));
    map.insert("9".to_string(), serde_json::json!("KeyV"));

    Ok(serde_json::Value::Object(map))
}
