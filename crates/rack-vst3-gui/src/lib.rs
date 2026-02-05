//! VST3 Native GUI support for hallucinator
//!
//! This crate provides native plugin GUI embedding via IPlugView.

use std::ffi::CString;
use std::ptr;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Vst3GuiError {
    #[error("Failed to load plugin")]
    LoadFailed,
    #[error("Plugin has no GUI view")]
    NoView,
    #[error("Failed to attach view to window")]
    AttachFailed,
    #[error("Invalid parameter")]
    InvalidParam,
    #[error("Unknown error: {0}")]
    Unknown(i32),
}

impl From<i32> for Vst3GuiError {
    fn from(code: i32) -> Self {
        match code {
            -1 => Self::LoadFailed,
            -2 => Self::NoView,
            -3 => Self::AttachFailed,
            -4 => Self::InvalidParam,
            other => Self::Unknown(other),
        }
    }
}

/// Convert FFI result to Result type
fn check_result(result: i32) -> Result<(), Vst3GuiError> {
    if result == 0 { Ok(()) } else { Err(result.into()) }
}

// FFI declarations
mod ffi {
    use std::os::raw::c_char;

    #[repr(C)]
    pub struct Vst3GuiHandle {
        _private: [u8; 0],
    }

    extern "C" {
        pub fn vst3_gui_create(path: *const c_char, uid: *const c_char) -> *mut Vst3GuiHandle;
        pub fn vst3_gui_get_size(handle: *mut Vst3GuiHandle, width: *mut i32, height: *mut i32) -> i32;
        pub fn vst3_gui_attach_x11(handle: *mut Vst3GuiHandle, window_id: u32) -> i32;
        pub fn vst3_gui_detach(handle: *mut Vst3GuiHandle);
        pub fn vst3_gui_destroy(handle: *mut Vst3GuiHandle);
        pub fn vst3_gui_get_parameter_count(handle: *mut Vst3GuiHandle) -> i32;
        pub fn vst3_gui_get_parameter(handle: *mut Vst3GuiHandle, index: i32, value: *mut f64) -> i32;
        pub fn vst3_gui_set_parameter(handle: *mut Vst3GuiHandle, index: i32, value: f64) -> i32;
        pub fn vst3_gui_get_component_state(handle: *mut Vst3GuiHandle, state_out: *mut u8, state_size: i32) -> i32;
    }
}

/// Handle to a VST3 plugin's native GUI
pub struct Vst3Gui {
    handle: *mut ffi::Vst3GuiHandle,
}

// Safety: The handle is thread-safe when properly synchronized
unsafe impl Send for Vst3Gui {}

impl Vst3Gui {
    /// Create a new GUI handle for a VST3 plugin
    ///
    /// # Arguments
    /// * `path` - Path to the .vst3 bundle
    /// * `uid` - Plugin unique ID (hex string from scan)
    pub fn new(path: &str, uid: &str) -> Result<Self, Vst3GuiError> {
        let path_c = CString::new(path).map_err(|_| Vst3GuiError::InvalidParam)?;
        let uid_c = CString::new(uid).map_err(|_| Vst3GuiError::InvalidParam)?;

        let handle = unsafe { ffi::vst3_gui_create(path_c.as_ptr(), uid_c.as_ptr()) };

        if handle.is_null() {
            return Err(Vst3GuiError::LoadFailed);
        }

        Ok(Self { handle })
    }

    /// Get the preferred size of the plugin view
    pub fn get_size(&self) -> Result<(i32, i32), Vst3GuiError> {
        let mut width = 0i32;
        let mut height = 0i32;
        let result = unsafe { ffi::vst3_gui_get_size(self.handle, &mut width, &mut height) };
        check_result(result)?;
        Ok((width, height))
    }

    /// Attach the plugin view to an X11 window
    #[cfg(target_os = "linux")]
    pub fn attach_x11(&self, window_id: u32) -> Result<(), Vst3GuiError> {
        let result = unsafe { ffi::vst3_gui_attach_x11(self.handle, window_id) };
        check_result(result)
    }

    /// Detach the plugin view from its window
    pub fn detach(&self) {
        unsafe { ffi::vst3_gui_detach(self.handle) };
    }

    /// Get the number of parameters
    pub fn parameter_count(&self) -> usize {
        let count = unsafe { ffi::vst3_gui_get_parameter_count(self.handle) };
        count.max(0) as usize
    }

    /// Get a parameter value (normalized 0-1)
    pub fn get_parameter(&self, index: usize) -> Result<f64, Vst3GuiError> {
        let mut value = 0.0f64;
        let result = unsafe { ffi::vst3_gui_get_parameter(self.handle, index as i32, &mut value) };
        check_result(result)?;
        Ok(value)
    }

    /// Set a parameter value (normalized 0-1)
    pub fn set_parameter(&self, index: usize, value: f64) -> Result<(), Vst3GuiError> {
        let result = unsafe { ffi::vst3_gui_set_parameter(self.handle, index as i32, value) };
        check_result(result)
    }

    /// Get all parameter values
    pub fn get_all_parameters(&self) -> Vec<f64> {
        (0..self.parameter_count())
            .map(|i| self.get_parameter(i).unwrap_or(0.0))
            .collect()
    }

    /// Get the component state as a byte array
    /// This captures the full plugin state including presets, samples, etc.
    pub fn get_component_state(&self) -> Result<Vec<u8>, Vst3GuiError> {
        // First call to get size
        let size = unsafe { ffi::vst3_gui_get_component_state(self.handle, ptr::null_mut(), 0) };
        if size < 0 {
            return Err(size.into());
        }
        if size == 0 {
            return Ok(Vec::new());
        }

        // Allocate buffer and get state
        let mut buffer = vec![0u8; size as usize];
        let result = unsafe { ffi::vst3_gui_get_component_state(self.handle, buffer.as_mut_ptr(), size) };
        if result < 0 {
            return Err(result.into());
        }

        buffer.truncate(result as usize);
        Ok(buffer)
    }
}

impl Drop for Vst3Gui {
    fn drop(&mut self) {
        if !self.handle.is_null() {
            unsafe { ffi::vst3_gui_destroy(self.handle) };
            self.handle = ptr::null_mut();
        }
    }
}
