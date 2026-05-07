/// contentTracing - 性能追踪与导出
///
/// 支持 Chrome Trace Event Format 导出，可用于 Chrome DevTools 或 trace viewer 分析。
/// 参考：https://docs.google.com/document/d/1CvAClvFfyA5R-PhYUmn5OOQtYMH4h6I0nSsKchNAySU/preview
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Trace 事件
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    /// 事件名称
    pub name: String,
    /// 事件类别
    pub cat: String,
    /// 事件类型：B=begin, E=end, I=instant, C=counter, etc.
    pub ph: String,
    /// 进程 ID
    pub pid: u64,
    /// 线程 ID
    pub tid: u64,
    /// 时间戳（微秒）
    pub ts: u64,
    /// 持续时间（微秒，仅用于 complete 事件）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dur: Option<u64>,
    /// 额外参数
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<serde_json::Value>,
}

/// 追踪会话
#[derive(Debug)]
pub struct TraceSession {
    pub id: String,
    pub events: Vec<TraceEvent>,
    pub start_time: std::time::Instant,
    pub is_recording: bool,
}

/// 追踪状态
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceState {
    pub session_id: String,
    pub is_recording: bool,
    pub event_count: usize,
    pub duration_ms: u64,
}

/// 全局追踪存储
#[derive(Debug, Clone)]
pub struct TraceStore {
    sessions: Arc<Mutex<HashMap<String, TraceSession>>>,
}

impl TraceStore {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn start_session(&self, session_id: String) -> Result<(), String> {
        let mut sessions = self.sessions.lock().map_err(|e| e.to_string())?;

        if sessions.contains_key(&session_id) {
            return Err(format!("Session {} already exists", session_id));
        }

        sessions.insert(
            session_id.clone(),
            TraceSession {
                id: session_id,
                events: Vec::new(),
                start_time: std::time::Instant::now(),
                is_recording: true,
            },
        );

        Ok(())
    }

    pub fn add_event(&self, session_id: &str, event: TraceEvent) -> Result<(), String> {
        let mut sessions = self.sessions.lock().map_err(|e| e.to_string())?;

        if let Some(session) = sessions.get_mut(session_id) {
            if !session.is_recording {
                return Err(format!("Session {} is not recording", session_id));
            }
            session.events.push(event);
            Ok(())
        } else {
            Err(format!("Session {} not found", session_id))
        }
    }

    pub fn stop_session(&self, session_id: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock().map_err(|e| e.to_string())?;

        if let Some(session) = sessions.get_mut(session_id) {
            session.is_recording = false;
            Ok(())
        } else {
            Err(format!("Session {} not found", session_id))
        }
    }

    pub fn export_trace(&self, session_id: &str) -> Result<String, String> {
        let sessions = self.sessions.lock().map_err(|e| e.to_string())?;

        if let Some(session) = sessions.get(session_id) {
            // 导出为 Chrome Trace Format
            let trace_json = serde_json::to_string_pretty(&session.events)
                .map_err(|e| format!("Failed to serialize trace: {}", e))?;
            Ok(trace_json)
        } else {
            Err(format!("Session {} not found", session_id))
        }
    }

    pub fn get_trace_state(&self, session_id: &str) -> Result<TraceState, String> {
        let sessions = self.sessions.lock().map_err(|e| e.to_string())?;

        if let Some(session) = sessions.get(session_id) {
            Ok(TraceState {
                session_id: session.id.clone(),
                is_recording: session.is_recording,
                event_count: session.events.len(),
                duration_ms: session.start_time.elapsed().as_millis() as u64,
            })
        } else {
            Err(format!("Session {} not found", session_id))
        }
    }

    pub fn clear_session(&self, session_id: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock().map_err(|e| e.to_string())?;
        sessions.remove(session_id);
        Ok(())
    }
}

/// 创建新的追踪会话
#[tauri::command]
pub async fn tracing_start_session(
    state: tauri::State<'_, TraceStore>,
    session_id: String,
) -> Result<(), String> {
    state.start_session(session_id)
}

/// 添加追踪事件
#[tauri::command]
pub async fn tracing_add_event(
    state: tauri::State<'_, TraceStore>,
    session_id: String,
    name: String,
    category: String,
    phase: String,
    args: Option<serde_json::Value>,
) -> Result<(), String> {
    let event = TraceEvent {
        name,
        cat: category,
        ph: phase,
        pid: std::process::id() as u64,
        tid: get_thread_id(),
        ts: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64,
        dur: None,
        args,
    };

    state.add_event(&session_id, event)
}

/// 添加完整事件（包含开始和结束）
#[tauri::command]
pub async fn tracing_add_complete_event(
    state: tauri::State<'_, TraceStore>,
    session_id: String,
    name: String,
    category: String,
    duration_us: u64,
    args: Option<serde_json::Value>,
) -> Result<(), String> {
    let event = TraceEvent {
        name,
        cat: category,
        ph: 'X'.to_string(), // Complete event
        pid: std::process::id() as u64,
        tid: get_thread_id(),
        ts: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64
            - duration_us,
        dur: Some(duration_us),
        args,
    };

    state.add_event(&session_id, event)
}

/// 停止追踪会话
#[tauri::command]
pub async fn tracing_stop_session(
    state: tauri::State<'_, TraceStore>,
    session_id: String,
) -> Result<(), String> {
    state.stop_session(&session_id)
}

/// 导出追踪数据为 Chrome Trace Format
#[tauri::command]
pub async fn tracing_export_trace(
    state: tauri::State<'_, TraceStore>,
    session_id: String,
) -> Result<String, String> {
    state.export_trace(&session_id)
}

/// 获取追踪状态
#[tauri::command]
pub async fn tracing_get_state(
    state: tauri::State<'_, TraceStore>,
    session_id: String,
) -> Result<TraceState, String> {
    state.get_trace_state(&session_id)
}

/// 清除追踪会话
#[tauri::command]
pub async fn tracing_clear_session(
    state: tauri::State<'_, TraceStore>,
    session_id: String,
) -> Result<(), String> {
    state.clear_session(&session_id)
}

/// 获取当前线程 ID
fn get_thread_id() -> u64 {
    use std::thread;
    let thread = thread::current();
    // 使用线程名称的哈希作为简易线程 ID
    let name = thread.name().unwrap_or("unknown");
    let mut hash: u64 = 0;
    for byte in name.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u64);
    }
    hash
}
