//! Lazy Common Item Dialog facade. No shell, child process or resident thread.
//! FFI is isolated here; callers retain an `Arc<Window>` throughout the operation.
use std::fmt;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DialogError {
    pub stage: &'static str,
    pub code: i32,
}
impl DialogError {
    pub fn category(&self) -> &'static str {
        match self.code as u32 {
            0x80070005 => "permission denied",
            0x80070002 | 0x80070003 => "path unavailable",
            0x80040154 => "Windows dialog provider unavailable",
            0x80010106 => "dialog worker apartment incompatible",
            0x80070057 => "invalid dialog options or path",
            _ => "Windows dialog failed",
        }
    }
}
impl fmt::Display for DialogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} at {} (HRESULT 0x{:08X})",
            self.category(),
            self.stage,
            self.code as u32
        )
    }
}
impl std::error::Error for DialogError {}
pub fn is_cancel(code: i32) -> bool {
    code as u32 == 0x800704c7
}
#[cfg(windows)]
mod native;
#[cfg(windows)]
pub use native::{Request, pick};
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cancellation_is_distinct_from_permission_and_provider_errors() {
        assert!(is_cancel(0x800704c7u32 as i32));
        for code in [0x80070005u32, 0x80040154, 0x80010106, 0x80070002] {
            assert!(!is_cancel(code as i32));
        }
        assert_eq!(
            DialogError {
                stage: "show",
                code: 0x80070005u32 as i32
            }
            .category(),
            "permission denied"
        );
    }
    #[test]
    fn error_text_contains_stage_code_without_private_path() {
        let text = DialogError {
            stage: "initial directory",
            code: 0x80070003u32 as i32,
        }
        .to_string();
        assert_eq!(
            text,
            "path unavailable at initial directory (HRESULT 0x80070003)"
        );
    }
}
