//! Cursor handling is specific to Win32. Keep the tool selection API available
//! on other platforms while letting the native window system manage cursors.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Tool {
    Select = 0,
    Draw = 1,
    Stretch = 2,
    Slice = 3,
    Erase = 4,
    Glue = 5,
    MarkerPosition = 6,
    MarkerRegion = 7,
    MarkerLoop = 8,
    MarkerCdTrack = 9,
}

impl Tool {
    #[allow(dead_code)]
    pub fn from_index(index: usize) -> Option<Self> {
        Some(match index {
            0 => Self::Select,
            1 => Self::Draw,
            2 => Self::Stretch,
            3 => Self::Slice,
            4 => Self::Erase,
            5 => Self::Glue,
            6 => Self::MarkerPosition,
            7 => Self::MarkerRegion,
            8 => Self::MarkerLoop,
            9 => Self::MarkerCdTrack,
            _ => return None,
        })
    }
}

static TOOL: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

pub fn init_tool_manager() -> Result<(), String> {
    Ok(())
}
pub fn set_tool(tool: Tool) {
    TOOL.store(tool as usize, std::sync::atomic::Ordering::Relaxed);
}
pub fn current_tool() -> Tool {
    Tool::from_index(TOOL.load(std::sync::atomic::Ordering::Relaxed)).unwrap_or(Tool::Select)
}
pub fn set_dark_mode(_: bool) {}
pub fn update_cursor() {}
pub fn set_hwnd<T>(_: T) {}
pub fn set_cursor_override(_: Tool) {}
pub fn set_gain_override(_: bool) {}
pub fn set_hand_override(_: bool) {}
pub fn set_vresize_override(_: bool) {}
