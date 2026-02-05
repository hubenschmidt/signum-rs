/**
 * VST3 GUI Support for Hallucinator DAW
 *
 * This file provides a C-compatible interface for Rust to embed VST3 plugin GUIs
 * into native windows. VST3 plugins use Steinberg's IPlugView interface for their
 * graphical editors, and this code handles:
 *
 * 1. Loading VST3 plugin modules (.vst3 bundles)
 * 2. Creating the plugin's audio processor component (IComponent)
 * 3. Obtaining the edit controller (IEditController) for GUI and parameter access
 * 4. Creating and attaching the plugin's GUI view (IPlugView) to a native window
 * 5. Bidirectional parameter synchronization between host and plugin GUI
 *
 * Architecture Overview:
 * ----------------------
 * VST3 plugins have a split architecture:
 *   - IComponent: The audio processing component (runs in real-time thread)
 *   - IEditController: The GUI/parameter editing component (runs in UI thread)
 *
 * These may be the same object (single component) or separate objects that need
 * to be connected via IConnectionPoint for state synchronization.
 *
 * The plugin's GUI is obtained via IEditController::createView() and then
 * attached to a native window handle (X11 Window ID on Linux).
 */

#include "vst3_gui.h"
#include "public.sdk/source/vst/hosting/module.h"
#include "public.sdk/source/vst/hosting/plugprovider.h"
#include "public.sdk/source/vst/hosting/hostclasses.h"
#include "public.sdk/source/common/memorystream.h"
#include "pluginterfaces/gui/iplugview.h"
#include "pluginterfaces/vst/ivsteditcontroller.h"
#include "pluginterfaces/vst/ivstcomponent.h"
#include "pluginterfaces/vst/ivstmessage.h"

#include <string>
#include <cstring>
#include <vector>
#include <mutex>

using namespace VST3;
using namespace Steinberg;
using namespace Steinberg::Vst;

// Forward declarations
struct Vst3GuiHandle;
class GuiComponentHandler;

/**
 * Parse a VST3 UID string into the SDK's UID type.
 * UIDs are 128-bit identifiers in the format "XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX"
 * that uniquely identify plugin classes within a VST3 module.
 */
static bool string_to_uid(const char* str, VST3::UID& uid) {
    if (!str) return false;
    auto uid_opt = VST3::UID::fromString(std::string(str));
    if (!uid_opt) return false;
    uid = *uid_opt;
    return true;
}

// Debug logging macro - outputs to stderr for visibility from Rust
#define LOG_DEBUG(fmt, ...) do { fprintf(stderr, fmt "\n", ##__VA_ARGS__); fflush(stderr); } while(0)

/**
 * GuiComponentHandler - Receives callbacks from the plugin's GUI
 *
 * When the user adjusts a parameter in the plugin's GUI (e.g., turns a knob),
 * the plugin calls these methods to notify the host. This allows the host to:
 * - Update its internal parameter state
 * - Record automation
 * - Sync parameters to the audio processor
 *
 * The VST3 parameter editing flow:
 *   1. beginEdit() - User starts touching a control (mouse down)
 *   2. performEdit() - Value changes as user drags (called many times)
 *   3. endEdit() - User releases control (mouse up)
 */
class GuiComponentHandler : public IComponentHandler {
public:
    explicit GuiComponentHandler(Vst3GuiHandle* owner) : owner_(owner), refCount_(1) {}
    virtual ~GuiComponentHandler() = default;

    // IUnknown interface - COM-style reference counting
    // All VST3 interfaces inherit from FUnknown which requires these methods
    tresult PLUGIN_API queryInterface(const TUID iid, void** obj) override {
        if (FUnknownPrivate::iidEqual(iid, IComponentHandler::iid) ||
            FUnknownPrivate::iidEqual(iid, FUnknown::iid)) {
            *obj = this;
            addRef();
            return kResultOk;
        }
        *obj = nullptr;
        return kNoInterface;
    }

    uint32 PLUGIN_API addRef() override { return ++refCount_; }

    uint32 PLUGIN_API release() override {
        auto count = --refCount_;
        if (count == 0) delete this;
        return count;
    }

    // IComponentHandler interface - Parameter change notifications

    /** Called when user starts editing a parameter (mouse down on knob/slider) */
    tresult PLUGIN_API beginEdit(ParamID) override { return kResultOk; }

    /** Called repeatedly while user is adjusting a parameter value */
    tresult PLUGIN_API performEdit(ParamID id, ParamValue valueNormalized) override;

    /** Called when user finishes editing a parameter (mouse up) */
    tresult PLUGIN_API endEdit(ParamID) override { return kResultOk; }

    /**
     * Called when plugin needs the host to restart/reconfigure something.
     * Flags indicate what changed (latency, tail size, bus config, etc.)
     */
    tresult PLUGIN_API restartComponent(int32) override { return kResultOk; }

private:
    Vst3GuiHandle* owner_;           // Back-pointer to access param change queue
    std::atomic<uint32> refCount_;   // COM-style reference count
};

/**
 * Vst3GuiHandle - Main state container for a loaded VST3 plugin GUI
 *
 * This opaque handle is returned to Rust and holds all resources needed
 * to manage the plugin's lifecycle:
 */
struct Vst3GuiHandle {
    /**
     * The loaded VST3 module (.vst3 bundle).
     * This must outlive all objects created from its factory.
     */
    Hosting::Module::Ptr module;

    /**
     * The plugin's audio processor component.
     * Handles audio/MIDI processing and holds the "processor state".
     */
    IPtr<IComponent> component;

    /**
     * The plugin's edit controller.
     * Manages parameters, GUI, and holds the "controller state".
     * May be the same object as component, or a separate instance.
     */
    IPtr<IEditController> controller;

    /**
     * The plugin's GUI view.
     * Can be attached to a native window for display.
     */
    IPtr<IPlugView> view;

    /**
     * Our handler that receives parameter change callbacks from the plugin GUI.
     */
    IPtr<GuiComponentHandler> componentHandler;

    /** Whether the view is currently attached to a window */
    bool attached = false;

    /** Whether component and controller are separate objects */
    bool separateController = false;

    /**
     * Queue of parameter changes from the GUI.
     * Protected by mutex since GUI callbacks come from UI thread
     * but Rust may poll from another thread.
     */
    std::mutex paramChangesMutex;
    std::vector<std::pair<ParamID, ParamValue>> paramChanges;

    /** Clean up all plugin resources in proper order */
    void cleanup() {
        view = nullptr;  // Release view first
        if (controller && separateController) {
            controller->terminate();  // Only terminate if separate
        }
        controller = nullptr;
        if (component) {
            component->terminate();
        }
        component = nullptr;
        module = nullptr;  // Module must be released last
    }
};

/**
 * Handle parameter changes from the plugin GUI.
 * Queues the change for later retrieval by the host (Rust side).
 * Thread-safe as GUI callbacks come from the UI thread.
 */
tresult PLUGIN_API GuiComponentHandler::performEdit(ParamID id, ParamValue valueNormalized) {
    if (!owner_) return kResultOk;

    std::lock_guard<std::mutex> lock(owner_->paramChangesMutex);
    owner_->paramChanges.push_back({id, valueNormalized});
    LOG_DEBUG("performEdit: param %u = %f", id, valueNormalized);
    return kResultOk;
}

// ============================================================================
// Helper functions for plugin initialization steps
// ============================================================================

/**
 * Step 1: Load the VST3 module from disk.
 *
 * VST3 plugins are bundles (directories with .vst3 extension) containing
 * the plugin binary and resources. The Module class handles platform-specific
 * loading of the shared library.
 */
static bool load_module(Vst3GuiHandle* handle, const char* path) {
    std::string error_desc;
    handle->module = Hosting::Module::create(path, error_desc);
    if (!handle->module) {
        LOG_DEBUG("vst3_gui_create: failed to load module: %s", error_desc.c_str());
        return false;
    }
    LOG_DEBUG("vst3_gui_create: module loaded");
    return true;
}

/**
 * Step 2: Create and initialize the audio processor component.
 *
 * The component is created from the module's factory using the plugin's UID.
 * We pass a minimal HostApplication to satisfy the plugin's initialization.
 */
static bool create_component(Vst3GuiHandle* handle, const VST3::UID& plugin_uid) {
    const auto& factory = handle->module->getFactory();
    handle->component = factory.createInstance<IComponent>(plugin_uid);
    if (!handle->component) {
        LOG_DEBUG("vst3_gui_create: failed to create component");
        return false;
    }
    LOG_DEBUG("vst3_gui_create: component created");

    // Initialize with a host application context
    if (handle->component->initialize(FUnknownPtr<IHostApplication>(new HostApplication())) != kResultOk) {
        LOG_DEBUG("vst3_gui_create: failed to initialize component");
        return false;
    }
    LOG_DEBUG("vst3_gui_create: component initialized");
    return true;
}

/**
 * Step 3: Obtain the edit controller for GUI and parameter access.
 *
 * VST3 has two patterns for the controller:
 * A) Single-component: Component implements both IComponent and IEditController
 * B) Separate-controller: Component provides a separate controller class ID
 *
 * We try the separate controller first, then fall back to casting the component.
 */
static bool get_controller(Vst3GuiHandle* handle) {
    const auto& factory = handle->module->getFactory();

    // Try to get a separate edit controller class
    TUID controllerCID;
    if (handle->component->getControllerClassId(controllerCID) == kResultTrue) {
        VST3::UID controllerUID = VST3::UID::fromTUID(controllerCID);
        handle->controller = factory.createInstance<IEditController>(controllerUID);
        if (handle->controller) {
            handle->controller->initialize(FUnknownPtr<IHostApplication>(new HostApplication()));
            handle->separateController = true;
            LOG_DEBUG("vst3_gui_create: edit controller created from separate class");
        }
    }

    // Fall back: try casting component to IEditController (single-component pattern)
    if (!handle->controller) {
        handle->controller = U::cast<IEditController>(handle->component);
        if (handle->controller) {
            LOG_DEBUG("vst3_gui_create: edit controller from component cast");
        }
    }

    if (!handle->controller) {
        LOG_DEBUG("vst3_gui_create: failed to get edit controller");
        return false;
    }
    return true;
}

/**
 * Step 4: Connect component and controller, synchronize state.
 *
 * For separate-controller plugins, we need to:
 * 1. Connect them via IConnectionPoint so they can exchange messages
 * 2. Transfer the component's state to the controller so it knows current values
 *
 * This ensures the GUI displays the correct initial parameter values.
 */
static void connect_and_sync(Vst3GuiHandle* handle) {
    if (!handle->separateController) return;

    // Get connection points from both sides
    FUnknownPtr<IConnectionPoint> componentCP(handle->component);
    FUnknownPtr<IConnectionPoint> controllerCP(handle->controller);

    // Establish bidirectional connection for message passing
    if (componentCP && controllerCP) {
        componentCP->connect(controllerCP);
        controllerCP->connect(componentCP);
        LOG_DEBUG("vst3_gui_create: component and controller connected");
    }

    // Sync component state to controller
    // This ensures the GUI shows the correct initial parameter values
    MemoryStream stream;
    if (handle->component->getState(&stream) == kResultTrue) {
        stream.seek(0, IBStream::kIBSeekSet, nullptr);
        handle->controller->setComponentState(&stream);
        LOG_DEBUG("vst3_gui_create: state synchronized");
    }
}

/**
 * Step 5: Register our component handler with the controller.
 *
 * This allows us to receive callbacks when the user changes parameters
 * through the plugin's GUI.
 */
static void register_handler(Vst3GuiHandle* handle) {
    handle->componentHandler = owned(new GuiComponentHandler(handle));
    auto result = handle->controller->setComponentHandler(handle->componentHandler);
    LOG_DEBUG("vst3_gui_create: component handler %s",
              result == kResultOk ? "registered" : "registration FAILED");
}

/**
 * Step 6: Create the plugin's GUI view.
 *
 * The view is created but not yet attached to any window.
 * ViewType::kEditor requests the main editor view (vs. an aux view).
 * Some plugins don't have GUIs - they return nullptr here.
 */
static bool create_view(Vst3GuiHandle* handle) {
    handle->view = handle->controller->createView(ViewType::kEditor);
    if (!handle->view) {
        LOG_DEBUG("vst3_gui_create: failed to create view (no GUI support?)");
        return false;
    }
    LOG_DEBUG("vst3_gui_create: view created successfully!");
    return true;
}

// ============================================================================
// C API - Exported functions callable from Rust via FFI
// ============================================================================

extern "C" {

/**
 * Create a VST3 GUI handle for the specified plugin.
 *
 * @param path Path to the .vst3 bundle
 * @param uid  Plugin class UID string (e.g., "XXXXXXXX-XXXX-XXXX-XXXX-XXXXXXXXXXXX")
 * @return Opaque handle, or nullptr on failure
 *
 * This performs the full initialization sequence:
 * 1. Load the VST3 module
 * 2. Create the audio processor component
 * 3. Get the edit controller
 * 4. Connect and sync component/controller if separate
 * 5. Register our parameter change handler
 * 6. Create the GUI view (not yet attached)
 */
Vst3GuiHandle* vst3_gui_create(const char* path, const char* uid) {
    if (!path || !uid) {
        LOG_DEBUG("vst3_gui_create: null path or uid");
        return nullptr;
    }
    LOG_DEBUG("vst3_gui_create: path=%s uid=%s", path, uid);

    // Parse the UID string
    VST3::UID plugin_uid;
    if (!string_to_uid(uid, plugin_uid)) {
        LOG_DEBUG("vst3_gui_create: failed to parse UID: %s", uid);
        return nullptr;
    }
    LOG_DEBUG("vst3_gui_create: UID parsed successfully");

    // Allocate handle
    auto handle = new(std::nothrow) Vst3GuiHandle();
    if (!handle) {
        LOG_DEBUG("vst3_gui_create: failed to allocate handle");
        return nullptr;
    }

    // Run initialization sequence, cleaning up on any failure
    if (!load_module(handle, path)) {
        delete handle;
        return nullptr;
    }

    if (!create_component(handle, plugin_uid)) {
        handle->cleanup();
        delete handle;
        return nullptr;
    }

    if (!get_controller(handle)) {
        handle->cleanup();
        delete handle;
        return nullptr;
    }

    connect_and_sync(handle);
    register_handler(handle);

    if (!create_view(handle)) {
        handle->cleanup();
        delete handle;
        return nullptr;
    }

    return handle;
}

/**
 * Get the plugin GUI's preferred size.
 *
 * @param handle Plugin handle
 * @param width  Output: preferred width in pixels
 * @param height Output: preferred height in pixels
 * @return VST3_GUI_OK on success
 *
 * If the plugin doesn't report a size, defaults to 800x600.
 */
int vst3_gui_get_size(Vst3GuiHandle* handle, int* width, int* height) {
    if (!handle || !handle->view || !width || !height) {
        return VST3_GUI_ERROR_INVALID_PARAM;
    }

    ViewRect rect;
    if (handle->view->getSize(&rect) != kResultOk) {
        // Fallback size if plugin doesn't report
        *width = 800;
        *height = 600;
        return VST3_GUI_OK;
    }

    *width = rect.getWidth();
    *height = rect.getHeight();
    return VST3_GUI_OK;
}

/**
 * Attach the plugin GUI to an X11 window.
 *
 * @param handle    Plugin handle
 * @param window_id X11 Window ID to embed the plugin GUI into
 * @return VST3_GUI_OK on success, error code on failure
 *
 * The window should be created by the host (Rust side) with appropriate
 * size before calling this. The plugin will render into this window.
 */
int vst3_gui_attach_x11(Vst3GuiHandle* handle, uint32_t window_id) {
    if (!handle || !handle->view) return VST3_GUI_ERROR_INVALID_PARAM;
    if (handle->attached) return VST3_GUI_OK;  // Already attached

    // Check if plugin supports X11 embedding
    if (handle->view->isPlatformTypeSupported(kPlatformTypeX11EmbedWindowID) != kResultTrue) {
        return VST3_GUI_ERROR_ATTACH_FAILED;
    }

    // Cast window ID to void* as expected by VST3 API
    void* parent = reinterpret_cast<void*>(static_cast<uintptr_t>(window_id));
    if (handle->view->attached(parent, kPlatformTypeX11EmbedWindowID) != kResultOk) {
        return VST3_GUI_ERROR_ATTACH_FAILED;
    }

    handle->attached = true;
    return VST3_GUI_OK;
}

/**
 * Detach the plugin GUI from its window.
 *
 * Should be called before destroying the window, or when hiding the plugin GUI.
 */
void vst3_gui_detach(Vst3GuiHandle* handle) {
    if (!handle || !handle->view || !handle->attached) return;
    handle->view->removed();
    handle->attached = false;
}

/**
 * Destroy the plugin handle and release all resources.
 *
 * Automatically detaches the GUI if still attached.
 * After this call, the handle is invalid.
 */
void vst3_gui_destroy(Vst3GuiHandle* handle) {
    if (!handle) return;

    // Detach view if still attached
    if (handle->attached && handle->view) {
        handle->view->removed();
    }
    handle->cleanup();
    delete handle;
}

/**
 * Get the number of parameters exposed by the plugin.
 *
 * @return Parameter count, or 0 if invalid handle
 */
int vst3_gui_get_parameter_count(Vst3GuiHandle* handle) {
    if (!handle || !handle->controller) return 0;
    return handle->controller->getParameterCount();
}

/**
 * Get a parameter's current normalized value (0.0 to 1.0).
 *
 * @param handle Plugin handle
 * @param index  Parameter index (0 to count-1)
 * @param value  Output: normalized value
 * @return VST3_GUI_OK on success
 */
int vst3_gui_get_parameter(Vst3GuiHandle* handle, int index, double* value) {
    if (!handle || !handle->controller || !value) return VST3_GUI_ERROR_INVALID_PARAM;

    int param_count = handle->controller->getParameterCount();
    if (index < 0 || index >= param_count) return VST3_GUI_ERROR_INVALID_PARAM;

    // Get parameter info to find the ParamID
    ParameterInfo info;
    if (handle->controller->getParameterInfo(index, info) != kResultOk) {
        return VST3_GUI_ERROR_INVALID_PARAM;
    }

    // Get normalized value (0.0 to 1.0)
    *value = handle->controller->getParamNormalized(info.id);
    return VST3_GUI_OK;
}

/**
 * Set a parameter's normalized value (0.0 to 1.0).
 *
 * @param handle Plugin handle
 * @param index  Parameter index (0 to count-1)
 * @param value  Normalized value to set
 * @return VST3_GUI_OK on success
 *
 * This updates the controller's value, which should update the GUI.
 * Note: This doesn't automatically update the audio processor - that
 * requires sending parameter changes through the proper channel.
 */
int vst3_gui_set_parameter(Vst3GuiHandle* handle, int index, double value) {
    if (!handle || !handle->controller) return VST3_GUI_ERROR_INVALID_PARAM;

    int param_count = handle->controller->getParameterCount();
    if (index < 0 || index >= param_count) return VST3_GUI_ERROR_INVALID_PARAM;

    // Get parameter info to find the ParamID
    ParameterInfo info;
    if (handle->controller->getParameterInfo(index, info) != kResultOk) {
        return VST3_GUI_ERROR_INVALID_PARAM;
    }

    // Set normalized value
    if (handle->controller->setParamNormalized(info.id, value) != kResultOk) {
        return VST3_GUI_ERROR_INVALID_PARAM;
    }

    return VST3_GUI_OK;
}

/**
 * Get the component's state (for preset saving).
 *
 * @param handle     Plugin handle
 * @param state_out  Buffer to write state, or nullptr to query size
 * @param state_size Size of state_out buffer
 * @return Bytes written on success, required size if state_out is nullptr,
 *         or negative error code
 *
 * Usage:
 *   int size = vst3_gui_get_component_state(handle, nullptr, 0);  // Get size
 *   uint8_t* buf = malloc(size);
 *   vst3_gui_get_component_state(handle, buf, size);              // Get state
 */
int vst3_gui_get_component_state(Vst3GuiHandle* handle, uint8_t* state_out, int state_size) {
    if (!handle || !handle->component) return VST3_GUI_ERROR_INVALID_PARAM;

    // Serialize state to memory stream
    MemoryStream stream;
    if (handle->component->getState(&stream) != kResultOk) {
        return VST3_GUI_ERROR_GENERIC;
    }

    // Get stream size
    int64 size = 0;
    stream.seek(0, IBStream::kIBSeekEnd, &size);
    stream.seek(0, IBStream::kIBSeekSet, nullptr);

    // If no output buffer, just return required size
    if (!state_out) return static_cast<int>(size);
    if (state_size < size) return VST3_GUI_ERROR_INVALID_PARAM;

    // Copy state to output buffer
    int32 bytesRead = 0;
    if (stream.read(state_out, static_cast<int32>(size), &bytesRead) != kResultOk) {
        return VST3_GUI_ERROR_GENERIC;
    }

    return static_cast<int>(bytesRead);
}

} // extern "C"
