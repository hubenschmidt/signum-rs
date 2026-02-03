#ifndef VST3_GUI_H
#define VST3_GUI_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>
#include <stddef.h>

// Opaque handle to plugin GUI
typedef struct Vst3GuiHandle Vst3GuiHandle;

// Error codes
#define VST3_GUI_OK 0
#define VST3_GUI_ERROR_LOAD_FAILED -1
#define VST3_GUI_ERROR_NO_VIEW -2
#define VST3_GUI_ERROR_ATTACH_FAILED -3
#define VST3_GUI_ERROR_INVALID_PARAM -4
#define VST3_GUI_ERROR_GENERIC -5

// Create a GUI handle for a VST3 plugin
// path: path to the .vst3 bundle
// uid: plugin unique ID (hex string)
// Returns handle or NULL on failure
Vst3GuiHandle* vst3_gui_create(const char* path, const char* uid);

// Get the preferred size of the plugin view
// Returns 0 on success, negative on error
int vst3_gui_get_size(Vst3GuiHandle* handle, int* width, int* height);

// Attach the plugin view to an X11 window
// window_id: X11 window ID (XID)
// Returns 0 on success, negative on error
int vst3_gui_attach_x11(Vst3GuiHandle* handle, uint32_t window_id);

// Detach the plugin view
void vst3_gui_detach(Vst3GuiHandle* handle);

// Destroy the GUI handle and free resources
void vst3_gui_destroy(Vst3GuiHandle* handle);

// Get the number of parameters
int vst3_gui_get_parameter_count(Vst3GuiHandle* handle);

// Get a parameter value (normalized 0-1)
// Returns 0 on success, negative on error
int vst3_gui_get_parameter(Vst3GuiHandle* handle, int index, double* value);

// Set a parameter value (normalized 0-1)
// Returns 0 on success, negative on error
int vst3_gui_set_parameter(Vst3GuiHandle* handle, int index, double value);

// Get the component state as a byte array
// state_out: output buffer (caller allocated), or NULL to query size
// state_size: size of output buffer
// Returns: size of state on success, negative on error
// Usage: call with NULL to get size, allocate buffer, call again to fill
int vst3_gui_get_component_state(Vst3GuiHandle* handle, uint8_t* state_out, int state_size);

#ifdef __cplusplus
}
#endif

#endif // VST3_GUI_H
