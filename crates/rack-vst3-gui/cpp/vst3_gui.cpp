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

// Forward declarations
struct Vst3GuiHandle;
class GuiComponentHandler;

// Helper to parse UID from string
static bool string_to_uid(const char* str, VST3::UID& uid) {
    if (!str) return false;
    auto uid_opt = VST3::UID::fromString(std::string(str));
    if (!uid_opt) return false;
    uid = *uid_opt;
    return true;
}

// Helper macros for logging
#define LOG_DEBUG(fmt, ...) do { fprintf(stderr, fmt "\n", ##__VA_ARGS__); fflush(stderr); } while(0)

// ComponentHandler to receive parameter change notifications from the plugin GUI
class GuiComponentHandler : public IComponentHandler {
public:
    explicit GuiComponentHandler(Vst3GuiHandle* owner) : owner_(owner), refCount_(1) {}
    virtual ~GuiComponentHandler() = default;

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
        auto count = --refCount_;
        if (count == 0) delete this;
        return count;
    }

    // IComponentHandler
    tresult PLUGIN_API beginEdit(ParamID) override { return kResultOk; }
    tresult PLUGIN_API performEdit(ParamID id, ParamValue valueNormalized) override;
    tresult PLUGIN_API endEdit(ParamID) override { return kResultOk; }
    tresult PLUGIN_API restartComponent(int32) override { return kResultOk; }

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
    bool attached = false;
    bool separateController = false;

    std::mutex paramChangesMutex;
    std::vector<std::pair<ParamID, ParamValue>> paramChanges;

    void cleanup() {
        view = nullptr;
        if (controller && separateController) {
            controller->terminate();
        }
        controller = nullptr;
        if (component) {
            component->terminate();
        }
        component = nullptr;
        module = nullptr;
    }
};

// Implement performEdit after Vst3GuiHandle is defined
tresult PLUGIN_API GuiComponentHandler::performEdit(ParamID id, ParamValue valueNormalized) {
    if (!owner_) return kResultOk;

    std::lock_guard<std::mutex> lock(owner_->paramChangesMutex);
    owner_->paramChanges.push_back({id, valueNormalized});
    LOG_DEBUG("performEdit: param %u = %f", id, valueNormalized);
    return kResultOk;
}

// Helper: Load VST3 module
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

// Helper: Create and initialize component
static bool create_component(Vst3GuiHandle* handle, const VST3::UID& plugin_uid) {
    const auto& factory = handle->module->getFactory();
    handle->component = factory.createInstance<IComponent>(plugin_uid);
    if (!handle->component) {
        LOG_DEBUG("vst3_gui_create: failed to create component");
        return false;
    }
    LOG_DEBUG("vst3_gui_create: component created");

    if (handle->component->initialize(FUnknownPtr<IHostApplication>(new HostApplication())) != kResultOk) {
        LOG_DEBUG("vst3_gui_create: failed to initialize component");
        return false;
    }
    LOG_DEBUG("vst3_gui_create: component initialized");
    return true;
}

// Helper: Get edit controller (separate or cast from component)
static bool get_controller(Vst3GuiHandle* handle) {
    const auto& factory = handle->module->getFactory();

    // Try separate controller first
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

    // Fall back to casting component to controller
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

// Helper: Connect component and controller, sync state
static void connect_and_sync(Vst3GuiHandle* handle) {
    if (!handle->separateController) return;

    FUnknownPtr<IConnectionPoint> componentCP(handle->component);
    FUnknownPtr<IConnectionPoint> controllerCP(handle->controller);

    if (componentCP && controllerCP) {
        componentCP->connect(controllerCP);
        controllerCP->connect(componentCP);
        LOG_DEBUG("vst3_gui_create: component and controller connected");
    }

    MemoryStream stream;
    if (handle->component->getState(&stream) == kResultTrue) {
        stream.seek(0, IBStream::kIBSeekSet, nullptr);
        handle->controller->setComponentState(&stream);
        LOG_DEBUG("vst3_gui_create: state synchronized");
    }
}

// Helper: Register component handler
static void register_handler(Vst3GuiHandle* handle) {
    handle->componentHandler = owned(new GuiComponentHandler(handle));
    auto result = handle->controller->setComponentHandler(handle->componentHandler);
    LOG_DEBUG("vst3_gui_create: component handler %s",
              result == kResultOk ? "registered" : "registration FAILED");
}

// Helper: Create plugin view
static bool create_view(Vst3GuiHandle* handle) {
    handle->view = handle->controller->createView(ViewType::kEditor);
    if (!handle->view) {
        LOG_DEBUG("vst3_gui_create: failed to create view (no GUI support?)");
        return false;
    }
    LOG_DEBUG("vst3_gui_create: view created successfully!");
    return true;
}

extern "C" {

Vst3GuiHandle* vst3_gui_create(const char* path, const char* uid) {
    if (!path || !uid) {
        LOG_DEBUG("vst3_gui_create: null path or uid");
        return nullptr;
    }
    LOG_DEBUG("vst3_gui_create: path=%s uid=%s", path, uid);

    VST3::UID plugin_uid;
    if (!string_to_uid(uid, plugin_uid)) {
        LOG_DEBUG("vst3_gui_create: failed to parse UID: %s", uid);
        return nullptr;
    }
    LOG_DEBUG("vst3_gui_create: UID parsed successfully");

    auto handle = new(std::nothrow) Vst3GuiHandle();
    if (!handle) {
        LOG_DEBUG("vst3_gui_create: failed to allocate handle");
        return nullptr;
    }

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

int vst3_gui_get_size(Vst3GuiHandle* handle, int* width, int* height) {
    if (!handle || !handle->view || !width || !height) {
        return VST3_GUI_ERROR_INVALID_PARAM;
    }

    ViewRect rect;
    if (handle->view->getSize(&rect) != kResultOk) {
        *width = 800;
        *height = 600;
        return VST3_GUI_OK;
    }

    *width = rect.getWidth();
    *height = rect.getHeight();
    return VST3_GUI_OK;
}

int vst3_gui_attach_x11(Vst3GuiHandle* handle, uint32_t window_id) {
    if (!handle || !handle->view) return VST3_GUI_ERROR_INVALID_PARAM;
    if (handle->attached) return VST3_GUI_OK;

    if (handle->view->isPlatformTypeSupported(kPlatformTypeX11EmbedWindowID) != kResultTrue) {
        return VST3_GUI_ERROR_ATTACH_FAILED;
    }

    void* parent = reinterpret_cast<void*>(static_cast<uintptr_t>(window_id));
    if (handle->view->attached(parent, kPlatformTypeX11EmbedWindowID) != kResultOk) {
        return VST3_GUI_ERROR_ATTACH_FAILED;
    }

    handle->attached = true;
    return VST3_GUI_OK;
}

void vst3_gui_detach(Vst3GuiHandle* handle) {
    if (!handle || !handle->view || !handle->attached) return;
    handle->view->removed();
    handle->attached = false;
}

void vst3_gui_destroy(Vst3GuiHandle* handle) {
    if (!handle) return;

    if (handle->attached && handle->view) {
        handle->view->removed();
    }
    handle->cleanup();
    delete handle;
}

int vst3_gui_get_parameter_count(Vst3GuiHandle* handle) {
    if (!handle || !handle->controller) return 0;
    return handle->controller->getParameterCount();
}

int vst3_gui_get_parameter(Vst3GuiHandle* handle, int index, double* value) {
    if (!handle || !handle->controller || !value) return VST3_GUI_ERROR_INVALID_PARAM;

    int param_count = handle->controller->getParameterCount();
    if (index < 0 || index >= param_count) return VST3_GUI_ERROR_INVALID_PARAM;

    ParameterInfo info;
    if (handle->controller->getParameterInfo(index, info) != kResultOk) {
        return VST3_GUI_ERROR_INVALID_PARAM;
    }

    *value = handle->controller->getParamNormalized(info.id);
    return VST3_GUI_OK;
}

int vst3_gui_set_parameter(Vst3GuiHandle* handle, int index, double value) {
    if (!handle || !handle->controller) return VST3_GUI_ERROR_INVALID_PARAM;

    int param_count = handle->controller->getParameterCount();
    if (index < 0 || index >= param_count) return VST3_GUI_ERROR_INVALID_PARAM;

    ParameterInfo info;
    if (handle->controller->getParameterInfo(index, info) != kResultOk) {
        return VST3_GUI_ERROR_INVALID_PARAM;
    }

    if (handle->controller->setParamNormalized(info.id, value) != kResultOk) {
        return VST3_GUI_ERROR_INVALID_PARAM;
    }

    return VST3_GUI_OK;
}

int vst3_gui_get_component_state(Vst3GuiHandle* handle, uint8_t* state_out, int state_size) {
    if (!handle || !handle->component) return VST3_GUI_ERROR_INVALID_PARAM;

    MemoryStream stream;
    if (handle->component->getState(&stream) != kResultOk) {
        return VST3_GUI_ERROR_GENERIC;
    }

    int64 size = 0;
    stream.seek(0, IBStream::kIBSeekEnd, &size);
    stream.seek(0, IBStream::kIBSeekSet, nullptr);

    if (!state_out) return static_cast<int>(size);
    if (state_size < size) return VST3_GUI_ERROR_INVALID_PARAM;

    int32 bytesRead = 0;
    if (stream.read(state_out, static_cast<int32>(size), &bytesRead) != kResultOk) {
        return VST3_GUI_ERROR_GENERIC;
    }

    return static_cast<int>(bytesRead);
}

} // extern "C"
