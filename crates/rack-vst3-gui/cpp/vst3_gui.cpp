// VST3 GUI support for signum-rs
// This provides native plugin GUI embedding via IPlugView

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

// Helper to parse UID from string
static bool string_to_uid(const char* str, VST3::UID& uid) {
    if (!str) return false;
    auto uid_opt = VST3::UID::fromString(std::string(str));
    if (!uid_opt) return false;
    uid = *uid_opt;
    return true;
}

// Forward declaration
struct Vst3GuiHandle;

// ComponentHandler to receive parameter change notifications from the plugin GUI
class GuiComponentHandler : public IComponentHandler {
public:
    GuiComponentHandler(Vst3GuiHandle* owner) : owner_(owner), refCount_(1) {}

    // IUnknown
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
        if (--refCount_ == 0) {
            delete this;
            return 0;
        }
        return refCount_;
    }

    // IComponentHandler
    tresult PLUGIN_API beginEdit(ParamID id) override {
        return kResultOk;
    }

    tresult PLUGIN_API performEdit(ParamID id, ParamValue valueNormalized) override;

    tresult PLUGIN_API endEdit(ParamID id) override {
        return kResultOk;
    }

    tresult PLUGIN_API restartComponent(int32 flags) override {
        return kResultOk;
    }

private:
    Vst3GuiHandle* owner_;
    std::atomic<uint32> refCount_;
};

// Internal handle structure
struct Vst3GuiHandle {
    Hosting::Module::Ptr module;
    IPtr<IComponent> component;
    IPtr<IEditController> controller;
    IPtr<IPlugView> view;
    IPtr<GuiComponentHandler> componentHandler;
    bool attached;

    // Parameter change tracking
    std::mutex paramChangesMutex;
    std::vector<std::pair<ParamID, ParamValue>> paramChanges;

    Vst3GuiHandle() : attached(false) {}
};

// Implement performEdit after Vst3GuiHandle is defined
tresult PLUGIN_API GuiComponentHandler::performEdit(ParamID id, ParamValue valueNormalized) {
    if (owner_) {
        std::lock_guard<std::mutex> lock(owner_->paramChangesMutex);
        owner_->paramChanges.push_back({id, valueNormalized});
        fprintf(stderr, "performEdit: param %u = %f\n", id, valueNormalized);
        fflush(stderr);
    }
    return kResultOk;
}

extern "C" {

Vst3GuiHandle* vst3_gui_create(const char* path, const char* uid) {
    if (!path || !uid) {
        fprintf(stderr, "vst3_gui_create: null path or uid\n");
        fflush(stderr);
        return nullptr;
    }

    fprintf(stderr, "vst3_gui_create: path=%s uid=%s\n", path, uid);
    fflush(stderr);

    auto handle = new(std::nothrow) Vst3GuiHandle();
    if (!handle) {
        fprintf(stderr, "vst3_gui_create: failed to allocate handle\n");
        return nullptr;
    }

    VST3::UID plugin_uid;
    if (!string_to_uid(uid, plugin_uid)) {
        fprintf(stderr, "vst3_gui_create: failed to parse UID: %s\n", uid);
        fflush(stderr);
        delete handle;
        return nullptr;
    }

    fprintf(stderr, "vst3_gui_create: UID parsed successfully\n");
    fflush(stderr);

    // Load the module
    std::string error_desc;
    handle->module = Hosting::Module::create(path, error_desc);
    if (!handle->module) {
        fprintf(stderr, "vst3_gui_create: failed to load module: %s\n", error_desc.c_str());
        fflush(stderr);
        delete handle;
        return nullptr;
    }

    fprintf(stderr, "vst3_gui_create: module loaded\n");
    fflush(stderr);

    // Create the component
    const auto& factory = handle->module->getFactory();
    handle->component = factory.createInstance<IComponent>(plugin_uid);
    if (!handle->component) {
        fprintf(stderr, "vst3_gui_create: failed to create component\n");
        fflush(stderr);
        delete handle;
        return nullptr;
    }

    fprintf(stderr, "vst3_gui_create: component created\n");
    fflush(stderr);

    // Initialize component
    if (handle->component->initialize(FUnknownPtr<IHostApplication>(new HostApplication())) != kResultOk) {
        fprintf(stderr, "vst3_gui_create: failed to initialize component\n");
        fflush(stderr);
        delete handle;
        return nullptr;
    }

    fprintf(stderr, "vst3_gui_create: component initialized\n");
    fflush(stderr);

    // Get edit controller
    TUID controllerCID;
    bool separateController = false;
    if (handle->component->getControllerClassId(controllerCID) == kResultTrue) {
        VST3::UID controllerUID = VST3::UID::fromTUID(controllerCID);
        handle->controller = factory.createInstance<IEditController>(controllerUID);
        if (handle->controller) {
            handle->controller->initialize(FUnknownPtr<IHostApplication>(new HostApplication()));
            separateController = true;
            fprintf(stderr, "vst3_gui_create: edit controller created from separate class\n");
            fflush(stderr);
        }
    }

    if (!handle->controller) {
        // Try casting component to controller (single-component plugins)
        handle->controller = U::cast<IEditController>(handle->component);
        if (handle->controller) {
            fprintf(stderr, "vst3_gui_create: edit controller from component cast\n");
            fflush(stderr);
        }
    }

    if (!handle->controller) {
        fprintf(stderr, "vst3_gui_create: failed to get edit controller\n");
        fflush(stderr);
        handle->component->terminate();
        delete handle;
        return nullptr;
    }

    // Connect component and controller if separate
    if (separateController) {
        // Get connection points
        FUnknownPtr<IConnectionPoint> componentCP(handle->component);
        FUnknownPtr<IConnectionPoint> controllerCP(handle->controller);

        if (componentCP && controllerCP) {
            componentCP->connect(controllerCP);
            controllerCP->connect(componentCP);
            fprintf(stderr, "vst3_gui_create: component and controller connected\n");
            fflush(stderr);
        }

        // Synchronize state from component to controller
        MemoryStream stream;
        if (handle->component->getState(&stream) == kResultTrue) {
            stream.seek(0, IBStream::kIBSeekSet, nullptr);
            handle->controller->setComponentState(&stream);
            fprintf(stderr, "vst3_gui_create: state synchronized\n");
            fflush(stderr);
        }
    }

    // Register component handler to receive parameter change notifications
    handle->componentHandler = owned(new GuiComponentHandler(handle));
    if (handle->controller->setComponentHandler(handle->componentHandler) == kResultOk) {
        fprintf(stderr, "vst3_gui_create: component handler registered\n");
        fflush(stderr);
    } else {
        fprintf(stderr, "vst3_gui_create: WARNING - failed to register component handler\n");
        fflush(stderr);
    }

    // Create the view
    handle->view = handle->controller->createView(ViewType::kEditor);
    if (!handle->view) {
        fprintf(stderr, "vst3_gui_create: failed to create view (no GUI support?)\n");
        fflush(stderr);
        if (static_cast<void*>(handle->controller.get()) != static_cast<void*>(handle->component.get())) {
            handle->controller->terminate();
        }
        handle->component->terminate();
        delete handle;
        return nullptr;
    }

    fprintf(stderr, "vst3_gui_create: view created successfully!\n");
    fflush(stderr);
    return handle;
}

int vst3_gui_get_size(Vst3GuiHandle* handle, int* width, int* height) {
    if (!handle || !handle->view || !width || !height) {
        return VST3_GUI_ERROR_INVALID_PARAM;
    }

    ViewRect rect;
    if (handle->view->getSize(&rect) != kResultOk) {
        // Try default size
        *width = 800;
        *height = 600;
        return VST3_GUI_OK;
    }

    *width = rect.getWidth();
    *height = rect.getHeight();
    return VST3_GUI_OK;
}

int vst3_gui_attach_x11(Vst3GuiHandle* handle, uint32_t window_id) {
    if (!handle || !handle->view) {
        return VST3_GUI_ERROR_INVALID_PARAM;
    }

    if (handle->attached) {
        return VST3_GUI_OK; // Already attached
    }

    // X11 platform type
    // kPlatformTypeX11EmbedWindowID is "X11EmbedWindowID"
    if (handle->view->isPlatformTypeSupported(kPlatformTypeX11EmbedWindowID) != kResultTrue) {
        return VST3_GUI_ERROR_ATTACH_FAILED;
    }

    // Attach to X11 window
    // The window_id is passed as a pointer (X11 convention for XEmbed)
    void* parent = reinterpret_cast<void*>(static_cast<uintptr_t>(window_id));
    if (handle->view->attached(parent, kPlatformTypeX11EmbedWindowID) != kResultOk) {
        return VST3_GUI_ERROR_ATTACH_FAILED;
    }

    handle->attached = true;
    return VST3_GUI_OK;
}

void vst3_gui_detach(Vst3GuiHandle* handle) {
    if (!handle || !handle->view || !handle->attached) {
        return;
    }

    handle->view->removed();
    handle->attached = false;
}

void vst3_gui_destroy(Vst3GuiHandle* handle) {
    if (!handle) {
        return;
    }

    if (handle->attached && handle->view) {
        handle->view->removed();
    }

    handle->view = nullptr;

    if (handle->controller && static_cast<void*>(handle->controller.get()) != static_cast<void*>(handle->component.get())) {
        handle->controller->terminate();
    }
    handle->controller = nullptr;

    if (handle->component) {
        handle->component->terminate();
    }
    handle->component = nullptr;

    handle->module = nullptr;

    delete handle;
}

int vst3_gui_get_parameter_count(Vst3GuiHandle* handle) {
    if (!handle || !handle->controller) {
        return 0;
    }
    return handle->controller->getParameterCount();
}

int vst3_gui_get_parameter(Vst3GuiHandle* handle, int index, double* value) {
    if (!handle || !handle->controller || !value) {
        return VST3_GUI_ERROR_INVALID_PARAM;
    }

    int param_count = handle->controller->getParameterCount();
    if (index < 0 || index >= param_count) {
        return VST3_GUI_ERROR_INVALID_PARAM;
    }

    // Get parameter info to find the parameter ID
    ParameterInfo info;
    if (handle->controller->getParameterInfo(index, info) != kResultOk) {
        return VST3_GUI_ERROR_INVALID_PARAM;
    }

    *value = handle->controller->getParamNormalized(info.id);
    return VST3_GUI_OK;
}

int vst3_gui_set_parameter(Vst3GuiHandle* handle, int index, double value) {
    if (!handle || !handle->controller) {
        return VST3_GUI_ERROR_INVALID_PARAM;
    }

    int param_count = handle->controller->getParameterCount();
    if (index < 0 || index >= param_count) {
        return VST3_GUI_ERROR_INVALID_PARAM;
    }

    // Get parameter info to find the parameter ID
    ParameterInfo info;
    if (handle->controller->getParameterInfo(index, info) != kResultOk) {
        return VST3_GUI_ERROR_INVALID_PARAM;
    }

    // Set the normalized value
    if (handle->controller->setParamNormalized(info.id, value) != kResultOk) {
        return VST3_GUI_ERROR_INVALID_PARAM;
    }

    // Also notify the component if connected
    if (handle->component) {
        FUnknownPtr<IConnectionPoint> componentCP(handle->component);
        if (componentCP) {
            // The parameter change should propagate through the connection
            // We need to use IComponentHandler or direct notification
        }
    }

    return VST3_GUI_OK;
}

int vst3_gui_get_component_state(Vst3GuiHandle* handle, uint8_t* state_out, int state_size) {
    if (!handle || !handle->component) {
        return VST3_GUI_ERROR_INVALID_PARAM;
    }

    // Get state from component
    MemoryStream stream;
    if (handle->component->getState(&stream) != kResultOk) {
        return VST3_GUI_ERROR_GENERIC;
    }

    // Get the size
    int64 size = 0;
    stream.seek(0, IBStream::kIBSeekEnd, &size);
    stream.seek(0, IBStream::kIBSeekSet, nullptr);

    // If state_out is NULL, just return the size
    if (!state_out) {
        return static_cast<int>(size);
    }

    // Check buffer size
    if (state_size < size) {
        return VST3_GUI_ERROR_INVALID_PARAM;
    }

    // Read the state into the output buffer
    int32 bytesRead = 0;
    if (stream.read(state_out, static_cast<int32>(size), &bytesRead) != kResultOk) {
        return VST3_GUI_ERROR_GENERIC;
    }

    return static_cast<int>(bytesRead);
}

} // extern "C"
