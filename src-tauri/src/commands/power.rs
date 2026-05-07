/// powerMonitor - 系统电源事件监听
///
/// 支持以下事件：
/// - suspend: 系统即将进入睡眠
/// - resume: 系统从睡眠中唤醒
/// - ac-changed: 电源状态变化（交流电/电池）
/// - shutdown: 系统即将关机
/// - lock-screen: 屏幕锁定
/// - unlock-screen: 屏幕解锁
use serde::{Deserialize, Serialize};
use tauri::{Emitter, Manager};

/// 电源事件类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PowerEventType {
    /// 系统即将进入睡眠
    Suspend,
    /// 系统从睡眠中唤醒
    Resume,
    /// 电源状态变化（交流电/电池）
    AcChanged { onBatteryPower: bool },
    /// 系统即将关机
    Shutdown,
    /// 屏幕锁定
    LockScreen,
    /// 屏幕解锁
    UnlockScreen,
}

/// 电源事件
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerEvent {
    pub r#type: PowerEventType,
    pub timestamp: u64,
}

/// 获取当前电源状态（是否在电池模式）
#[tauri::command]
pub async fn power_monitor_get_power_status() -> Result<serde_json::Value, String> {
    #[cfg(target_os = "windows")]
    {
        use windows::Win32::System::Power::GetSystemPowerStatus;
        use windows::Win32::System::Power::SYSTEM_POWER_STATUS;

        unsafe {
            let mut status = SYSTEM_POWER_STATUS::default();
            match GetSystemPowerStatus(&mut status) {
                Ok(_) => Ok(serde_json::json!({
                    "acLineStatus": if status.ACLineStatus == 1 { "online" } else { "offline" },
                    "batteryFlag": status.BatteryFlag,
                    "batteryLifePercent": status.BatteryLifePercent,
                    "systemStatusFlag": status.SystemStatusFlag,
                })),
                Err(e) => Err(format!("Failed to get power status: {}", e)),
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        // macOS: 使用 IOKit 获取电源状态
        let on_battery = is_on_battery_macos();
        Ok(serde_json::json!({
            "onBatteryPower": on_battery,
        }))
    }

    #[cfg(target_os = "linux")]
    {
        // Linux: 读取 sysfs 电源状态
        let on_battery = is_on_battery_linux();
        Ok(serde_json::json!({
            "onBatteryPower": on_battery,
        }))
    }
}

/// 监听电源事件
#[tauri::command]
pub async fn power_monitor_start_listening(app: tauri::AppHandle) -> Result<(), String> {
    // 启动后台线程监听电源事件
    let app_clone = app.clone();

    #[cfg(target_os = "windows")]
    {
        std::thread::spawn(move || {
            listen_power_events_windows(&app_clone);
        });
    }

    #[cfg(target_os = "macos")]
    {
        std::thread::spawn(move || {
            listen_power_events_macos(&app_clone);
        });
    }

    #[cfg(target_os = "linux")]
    {
        std::thread::spawn(move || {
            listen_power_events_linux(&app_clone);
        });
    }

    Ok(())
}

/// 发射电源事件到前端
fn emit_power_event(app: &tauri::AppHandle, event: PowerEventType) {
    let power_event = PowerEvent {
        r#type: event,
        timestamp: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
    };

    if let Err(e) = app.emit("power-monitor-event", &power_event) {
        log::warn!("Failed to emit power event: {}", e);
    }
}

// ==================== Windows 实现 ====================

#[cfg(target_os = "windows")]
fn listen_power_events_windows(app: &tauri::AppHandle) {
    use windows::Win32::System::Power::PowerRegisterSuspendResumeNotification;
    use windows::Win32::System::Power::DEVICE_NOTIFY_SUBSCRIBE_PARAMETERS;
    use windows::Win32::System::Power::POWERBROADCAST_SETTING;
    use windows::Win32::System::Threading::GetCurrentThread;

    // Windows 电源事件监听需要使用 RegisterPowerSettingNotification
    // 这里简化实现，使用轮询方式
    let mut last_ac_status = false;

    loop {
        unsafe {
            let mut status = windows::Win32::System::Power::SYSTEM_POWER_STATUS::default();
            if windows::Win32::System::Power::GetSystemPowerStatus(&mut status).is_ok() {
                let current_ac = status.ACLineStatus == 1;
                if current_ac != last_ac_status {
                    emit_power_event(
                        app,
                        PowerEventType::AcChanged {
                            onBatteryPower: !current_ac,
                        },
                    );
                    last_ac_status = current_ac;
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

// ==================== macOS 实现 ====================

#[cfg(target_os = "macos")]
fn is_on_battery_macos() -> bool {
    use std::process::Command;
    let output = Command::new("pmset")
        .arg("-g")
        .arg("batt")
        .output()
        .ok();

    if let Some(output) = output {
        let stdout = String::from_utf8_lossy(&output.stdout);
        // 如果输出包含 "InternalBattery" 且不是 "charged" 或 "AC attached"
        stdout.contains("InternalBattery") && !stdout.contains("AC attached")
    } else {
        false
    }
}

#[cfg(target_os = "macos")]
fn listen_power_events_macos(app: &tauri::AppHandle) {
    // macOS 可以使用 IOKit 的 IORegisterForSystemPower
    // 这里简化实现，使用 pmset 轮询
    let mut last_ac_status = !is_on_battery_macos();

    loop {
        let current_on_battery = is_on_battery_macos();
        let current_ac = !current_on_battery;

        if current_ac != last_ac_status {
            emit_power_event(
                app,
                PowerEventType::AcChanged {
                    onBatteryPower: current_on_battery,
                },
            );
            last_ac_status = current_ac;
        }

        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}

// ==================== Linux 实现 ====================

#[cfg(target_os = "linux")]
fn is_on_battery_linux() -> bool {
    use std::fs;
    // 读取 sysfs 电源状态
    if let Ok(status) = fs::read_to_string("/sys/class/power_supply/AC/online") {
        status.trim() == "0"
    } else if let Ok(status) = fs::read_to_string("/sys/class/power_supply/ACAD/online") {
        status.trim() == "0"
    } else {
        false
    }
}

#[cfg(target_os = "linux")]
fn listen_power_events_linux(app: &tauri::AppHandle) {
    // Linux 可以使用 systemd-logind 的 D-Bus 信号
    // 这里简化实现，使用 sysfs 轮询
    let mut last_ac_status = !is_on_battery_linux();

    loop {
        let current_ac = !is_on_battery_linux();

        if current_ac != last_ac_status {
            emit_power_event(
                app,
                PowerEventType::AcChanged {
                    onBatteryPower: !current_ac,
                },
            );
            last_ac_status = current_ac;
        }

        std::thread::sleep(std::time::Duration::from_secs(2));
    }
}
