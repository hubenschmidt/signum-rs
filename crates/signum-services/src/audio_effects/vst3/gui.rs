//! VST3 Native GUI support
//!
//! This module provides native plugin GUI windows for VST3 plugins.
//! On Linux, it creates X11 windows and embeds the plugin view using XEmbed.

use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use thiserror::Error;
use tracing::info;

#[cfg(target_os = "linux")]
use rack_vst3_gui::Vst3Gui;

#[derive(Debug, Error)]
pub enum Vst3GuiError {
    #[error("X11 connection failed: {0}")]
    X11Connection(String),
    #[error("Window creation failed: {0}")]
    WindowCreation(String),
    #[error("Plugin view creation failed: {0}")]
    ViewCreation(String),
    #[error("Plugin not found")]
    PluginNotFound,
    #[error("Plugin GUI error: {0}")]
    PluginGui(String),
}

/// Native window handle for plugin GUI embedding
#[derive(Debug, Clone, Copy)]
pub struct NativeWindowHandle {
    #[cfg(target_os = "linux")]
    pub x11_window: u32,
    #[cfg(target_os = "linux")]
    pub x11_display: *mut std::ffi::c_void,
}

unsafe impl Send for NativeWindowHandle {}
unsafe impl Sync for NativeWindowHandle {}

/// Plugin GUI window state
pub struct PluginGuiWindow {
    pub plugin_id: u64,
    pub plugin_path: String,
    pub plugin_uid: String,
    pub title: String,
    pub width: u32,
    pub height: u32,
    pub visible: bool,
    pub native_handle: Option<NativeWindowHandle>,
    #[cfg(target_os = "linux")]
    pub vst3_gui: Option<Vst3Gui>,
    /// Last known parameter values for change detection
    #[cfg(target_os = "linux")]
    last_params: Vec<f64>,
    /// Last known component state hash for preset change detection
    #[cfg(target_os = "linux")]
    last_state_hash: u64,
    /// Frame counter for state sync debouncing
    #[cfg(target_os = "linux")]
    state_check_counter: u32,
}

/// Manager for plugin GUI windows
pub struct PluginGuiManager {
    windows: HashMap<u64, PluginGuiWindow>,
    #[cfg(target_os = "linux")]
    x11_connection: Option<Arc<x11rb::rust_connection::RustConnection>>,
}

impl PluginGuiManager {
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            #[cfg(target_os = "linux")]
            x11_connection: None,
        }
    }

    /// Initialize the window manager (connect to display server)
    #[cfg(target_os = "linux")]
    pub fn initialize(&mut self) -> Result<(), Vst3GuiError> {

        let (conn, _screen_num) = x11rb::connect(None)
            .map_err(|e| Vst3GuiError::X11Connection(e.to_string()))?;

        self.x11_connection = Some(Arc::new(conn));
        info!("X11 connection established for plugin GUIs");
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn initialize(&mut self) -> Result<(), Vst3GuiError> {
        warn!("Native plugin GUI not yet implemented for this platform");
        Ok(())
    }

    /// Create a new plugin GUI window with native VST3 view
    #[cfg(target_os = "linux")]
    pub fn create_window(
        &mut self,
        plugin_id: u64,
        plugin_path: &str,
        plugin_uid: &str,
        title: &str,
        default_width: u32,
        default_height: u32,
    ) -> Result<(), Vst3GuiError> {
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::{
            ConnectionExt as _, CreateWindowAux, EventMask, WindowClass,
        };
        use x11rb::wrapper::ConnectionExt as WrapperConnectionExt;

        // First, create the VST3 GUI handle to get the actual plugin view size
        let vst3_gui = Vst3Gui::new(plugin_path, plugin_uid)
            .map_err(|e| Vst3GuiError::PluginGui(format!("Failed to create VST3 GUI: {}", e)))?;

        // Get the preferred size from the plugin
        let (width, height) = vst3_gui.get_size()
            .map(|(w, h)| (w as u32, h as u32))
            .unwrap_or((default_width, default_height));

        info!(plugin_id, width, height, "Plugin requested size");

        let conn = self.x11_connection.as_ref()
            .ok_or_else(|| Vst3GuiError::X11Connection("Not initialized".to_string()))?;

        let screen = &conn.setup().roots[0];
        let window_id = conn.generate_id()
            .map_err(|e| Vst3GuiError::WindowCreation(e.to_string()))?;

        let values = CreateWindowAux::new()
            .background_pixel(screen.black_pixel)
            .event_mask(
                EventMask::EXPOSURE
                    | EventMask::KEY_PRESS
                    | EventMask::KEY_RELEASE
                    | EventMask::BUTTON_PRESS
                    | EventMask::BUTTON_RELEASE
                    | EventMask::POINTER_MOTION
                    | EventMask::STRUCTURE_NOTIFY
            );

        conn.create_window(
            screen.root_depth,
            window_id,
            screen.root,
            0, 0,
            width as u16,
            height as u16,
            0,
            WindowClass::INPUT_OUTPUT,
            screen.root_visual,
            &values,
        ).map_err(|e: x11rb::errors::ConnectionError| Vst3GuiError::WindowCreation(e.to_string()))?;

        // Set window title
        WrapperConnectionExt::change_property8(
            conn.as_ref(),
            x11rb::protocol::xproto::PropMode::REPLACE,
            window_id,
            x11rb::protocol::xproto::AtomEnum::WM_NAME,
            x11rb::protocol::xproto::AtomEnum::STRING,
            title.as_bytes(),
        ).map_err(|e: x11rb::errors::ConnectionError| Vst3GuiError::WindowCreation(e.to_string()))?;

        conn.flush().map_err(|e: x11rb::errors::ConnectionError| Vst3GuiError::WindowCreation(e.to_string()))?;

        // Attach the VST3 plugin view to the X11 window
        vst3_gui.attach_x11(window_id)
            .map_err(|e| Vst3GuiError::PluginGui(format!("Failed to attach plugin view: {}", e)))?;

        info!(plugin_id, window_id, "Attached VST3 plugin view to X11 window");

        let native_handle = NativeWindowHandle {
            x11_window: window_id,
            x11_display: std::ptr::null_mut(),
        };

        // Get initial parameter values for change detection
        let param_count = vst3_gui.parameter_count();
        info!(plugin_id, param_count, "GUI instance has parameters");
        let last_params = vst3_gui.get_all_parameters();

        // Get initial state hash for preset change detection
        let last_state_hash = vst3_gui.get_component_state()
            .map(|state| {
                let mut hasher = DefaultHasher::new();
                state.hash(&mut hasher);
                hasher.finish()
            })
            .unwrap_or(0);

        let window = PluginGuiWindow {
            plugin_id,
            plugin_path: plugin_path.to_string(),
            plugin_uid: plugin_uid.to_string(),
            title: title.to_string(),
            width,
            height,
            visible: false,
            native_handle: Some(native_handle),
            vst3_gui: Some(vst3_gui),
            last_params,
            last_state_hash,
            state_check_counter: 0,
        };

        self.windows.insert(plugin_id, window);
        info!(plugin_id, title, "Created plugin GUI window with native view");

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn create_window(
        &mut self,
        plugin_id: u64,
        plugin_path: &str,
        plugin_uid: &str,
        title: &str,
        width: u32,
        height: u32,
    ) -> Result<(), Vst3GuiError> {
        let window = PluginGuiWindow {
            plugin_id,
            plugin_path: plugin_path.to_string(),
            plugin_uid: plugin_uid.to_string(),
            title: title.to_string(),
            width,
            height,
            visible: false,
            native_handle: None,
        };
        self.windows.insert(plugin_id, window);
        tracing::warn!("Native plugin GUI not available on this platform");
        Ok(())
    }

    /// Show a plugin window
    #[cfg(target_os = "linux")]
    pub fn show_window(&mut self, plugin_id: u64) -> Result<(), Vst3GuiError> {
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::ConnectionExt;

        let window = self.windows.get_mut(&plugin_id)
            .ok_or(Vst3GuiError::PluginNotFound)?;

        let conn = self.x11_connection.as_ref()
            .ok_or_else(|| Vst3GuiError::X11Connection("Not initialized".to_string()))?;

        if let Some(handle) = &window.native_handle {
            conn.map_window(handle.x11_window)
                .map_err(|e| Vst3GuiError::WindowCreation(e.to_string()))?;
            conn.flush().map_err(|e| Vst3GuiError::WindowCreation(e.to_string()))?;
            window.visible = true;
            info!(plugin_id, "Showing plugin GUI window");
        }

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn show_window(&mut self, plugin_id: u64) -> Result<(), Vst3GuiError> {
        if let Some(window) = self.windows.get_mut(&plugin_id) {
            window.visible = true;
        }
        Ok(())
    }

    /// Hide a plugin window
    #[cfg(target_os = "linux")]
    pub fn hide_window(&mut self, plugin_id: u64) -> Result<(), Vst3GuiError> {
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::ConnectionExt;

        let window = self.windows.get_mut(&plugin_id)
            .ok_or(Vst3GuiError::PluginNotFound)?;

        let conn = self.x11_connection.as_ref()
            .ok_or_else(|| Vst3GuiError::X11Connection("Not initialized".to_string()))?;

        if let Some(handle) = &window.native_handle {
            conn.unmap_window(handle.x11_window)
                .map_err(|e| Vst3GuiError::WindowCreation(e.to_string()))?;
            conn.flush().map_err(|e| Vst3GuiError::WindowCreation(e.to_string()))?;
            window.visible = false;
        }

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn hide_window(&mut self, plugin_id: u64) -> Result<(), Vst3GuiError> {
        if let Some(window) = self.windows.get_mut(&plugin_id) {
            window.visible = false;
        }
        Ok(())
    }

    /// Destroy a plugin window
    #[cfg(target_os = "linux")]
    pub fn destroy_window(&mut self, plugin_id: u64) -> Result<(), Vst3GuiError> {
        use x11rb::connection::Connection;
        use x11rb::protocol::xproto::ConnectionExt;

        if let Some(mut window) = self.windows.remove(&plugin_id) {
            // Detach VST3 plugin view first
            if let Some(vst3_gui) = window.vst3_gui.take() {
                vst3_gui.detach();
                // vst3_gui will be dropped here, which calls vst3_gui_destroy
            }

            // Then destroy the X11 window
            if let Some(handle) = window.native_handle {
                if let Some(conn) = &self.x11_connection {
                    let _ = conn.destroy_window(handle.x11_window);
                    let _ = conn.flush();
                }
            }
            info!(plugin_id, "Destroyed plugin GUI window");
        }

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn destroy_window(&mut self, plugin_id: u64) -> Result<(), Vst3GuiError> {
        self.windows.remove(&plugin_id);
        Ok(())
    }

    /// Get window handle for a plugin (for attaching plugin view)
    pub fn get_window_handle(&self, plugin_id: u64) -> Option<NativeWindowHandle> {
        self.windows.get(&plugin_id).and_then(|w| w.native_handle)
    }

    /// Check if a window exists for a plugin
    pub fn has_window(&self, plugin_id: u64) -> bool {
        self.windows.contains_key(&plugin_id)
    }

    /// Process pending window events (call periodically from main thread)
    #[cfg(target_os = "linux")]
    pub fn process_events(&self) -> Result<(), Vst3GuiError> {
        use x11rb::connection::Connection;

        let conn = match &self.x11_connection {
            Some(c) => c,
            None => return Ok(()),
        };

        // Process all pending events
        while let Ok(Some(_event)) = conn.poll_for_event() {
            // Events are handled by the plugin's embedded view
            // We just need to pump the event queue
        }

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn process_events(&self) -> Result<(), Vst3GuiError> {
        Ok(())
    }

    /// Get parameter changes from all visible GUI windows
    /// Returns: Vec<(plugin_id, param_index, new_value)>
    #[cfg(target_os = "linux")]
    pub fn get_parameter_changes(&mut self) -> Vec<(u64, usize, f64)> {
        let mut changes = Vec::new();

        for window in self.windows.values_mut() {
            let Some(ref gui) = window.vst3_gui else { continue };
            if !window.visible { continue; }

            let current_params = gui.get_all_parameters();

            // Only log occasionally to avoid spam
            static mut POLL_COUNTER: u32 = 0;
            unsafe {
                POLL_COUNTER += 1;
                if POLL_COUNTER % 300 == 1 {
                    // Log first 5 params every ~5 seconds at 60fps
                    let preview: Vec<_> = current_params.iter().take(5).collect();
                    tracing::debug!(
                        "Polling params for plugin_id={}: first 5 = {:?}",
                        window.plugin_id, preview
                    );
                }
            }

            // Compare with last known values
            for (i, &new_val) in current_params.iter().enumerate() {
                let old_val = window.last_params.get(i).copied().unwrap_or(0.0);
                // Use a small threshold for floating point comparison
                if (new_val - old_val).abs() > 0.0001 {
                    tracing::info!(
                        "GUI param change detected: plugin_id={} param[{}] {} -> {}",
                        window.plugin_id, i, old_val, new_val
                    );
                    changes.push((window.plugin_id, i, new_val));
                }
            }

            // Update last known values
            window.last_params = current_params;
        }

        changes
    }

    #[cfg(not(target_os = "linux"))]
    pub fn get_parameter_changes(&mut self) -> Vec<(u64, usize, f64)> {
        Vec::new()
    }

    /// Get component state changes (for preset/patch sync)
    /// Returns: Vec<(plugin_id, state_bytes)>
    /// Only checks every 30 frames (~0.5 sec at 60fps) to avoid overhead
    #[cfg(target_os = "linux")]
    pub fn get_state_changes(&mut self) -> Vec<(u64, Vec<u8>)> {
        let mut changes = Vec::new();

        for window in self.windows.values_mut() {
            let Some(ref gui) = window.vst3_gui else { continue };
            if !window.visible { continue; }

            // Only check state periodically to reduce overhead
            window.state_check_counter += 1;
            if window.state_check_counter < 30 {
                continue;
            }
            window.state_check_counter = 0;

            // Get current state and compute hash
            let Ok(current_state) = gui.get_component_state() else { continue };

            let mut hasher = DefaultHasher::new();
            current_state.hash(&mut hasher);
            let current_hash = hasher.finish();

            // Check if state changed
            if current_hash != window.last_state_hash {
                info!(plugin_id = window.plugin_id, "Component state changed (preset loaded)");
                changes.push((window.plugin_id, current_state));
                window.last_state_hash = current_hash;
            }
        }

        changes
    }

    #[cfg(not(target_os = "linux"))]
    pub fn get_state_changes(&mut self) -> Vec<(u64, Vec<u8>)> {
        Vec::new()
    }
}

impl Default for PluginGuiManager {
    fn default() -> Self {
        Self::new()
    }
}
