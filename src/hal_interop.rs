//! HAL interop functions for creating wgpu-native handles from raw Vulkan handles
//! This module allows importing existing Vulkan devices/textures into wgpu-native
//!
//! C API:
//! - wgpuCreateDeviceFromVulkan: Create WGPUDevice from existing Vulkan handles
//! - wgpuImportVulkanImage: Import VkImage as WGPUTexture

use std::borrow::Cow;
use std::ffi::CStr;
use std::sync::{atomic, Arc, Weak};
use parking_lot::Mutex;
use hal::api::Vulkan;
use ash::vk::{self, Handle};

use crate::{
    native, Context, WGPUAdapterImpl, WGPUDeviceImpl, WGPUTextureImpl,
    QueueId, ErrorSinkRaw, TextureData,
};

/// Default error handlers for HAL-imported devices
struct HalInteropCallbacks;

impl HalInteropCallbacks {
    fn default_device_lost() -> crate::DeviceLostCallback {
        crate::DEFAULT_DEVICE_LOST_HANDLER
    }
}

// ============================================================================
// C API for Vulkan Interop
// ============================================================================

/// Convert VkFormat to wgpu TextureFormat
fn vk_format_to_wgpu(vk_format: i32) -> Option<wgt::TextureFormat> {
    match vk_format {
        43 => Some(wgt::TextureFormat::Rgba8UnormSrgb),  // VK_FORMAT_R8G8B8A8_SRGB
        37 => Some(wgt::TextureFormat::Rgba8Unorm),      // VK_FORMAT_R8G8B8A8_UNORM
        50 => Some(wgt::TextureFormat::Bgra8UnormSrgb),  // VK_FORMAT_B8G8R8A8_SRGB
        44 => Some(wgt::TextureFormat::Bgra8Unorm),      // VK_FORMAT_B8G8R8A8_UNORM
        97 => Some(wgt::TextureFormat::Rgba16Float),     // VK_FORMAT_R16G16B16A16_SFLOAT
        64 => Some(wgt::TextureFormat::Rgb10a2Unorm),    // VK_FORMAT_A2B10G10R10_UNORM_PACK32
        129 => Some(wgt::TextureFormat::Depth24PlusStencil8), // VK_FORMAT_D24_UNORM_S8_UINT
        126 => Some(wgt::TextureFormat::Depth32Float),   // VK_FORMAT_D32_SFLOAT
        _ => None,
    }
}

/// Create a WGPUDevice from existing Vulkan handles.
///
/// This allows wgpu-native to share an existing Vulkan device with OpenXR.
///
/// # Safety
/// All Vulkan handles must be valid and the device must support the graphics queue.
#[no_mangle]
#[cfg(feature = "vulkan")]
pub unsafe extern "C" fn wgpuCreateDeviceFromVulkan(
    vk_instance: u64,
    vk_physical_device: u64,
    vk_device: u64,
    queue_family_index: u32,
    _queue_index: u32,
) -> native::WGPUDevice {
    #[cfg(target_os = "android")]
    {
        use log::LevelFilter;
        android_logger::init_once(
            android_logger::Config::default()
                .with_max_level(LevelFilter::Info)
                .with_tag("wgpu-native"),
        );
    }

    log::info!("wgpuCreateDeviceFromVulkan called");

    let result = std::panic::catch_unwind(|| {
        create_device_from_vulkan_impl(
            vk_instance,
            vk_physical_device,
            vk_device,
            queue_family_index,
        )
    });

    match result {
        Ok(Some(device)) => device,
        Ok(None) => {
            log::error!("wgpuCreateDeviceFromVulkan failed");
            std::ptr::null_mut()
        }
        Err(e) => {
            log::error!("wgpuCreateDeviceFromVulkan panicked: {:?}", e);
            std::ptr::null_mut()
        }
    }
}

#[cfg(feature = "vulkan")]
unsafe fn create_device_from_vulkan_impl(
    vk_instance: u64,
    vk_physical_device: u64,
    vk_device: u64,
    queue_family_index: u32,
) -> Option<native::WGPUDevice> {
    use hal::Api;

    let entry = ash::Entry::load().ok()?;
    let vk_instance_handle = vk::Instance::from_raw(vk_instance);
    let vk_phys_device = vk::PhysicalDevice::from_raw(vk_physical_device);
    let vk_device_handle = vk::Device::from_raw(vk_device);

    let ash_instance = ash::Instance::load(entry.static_fn(), vk_instance_handle);
    let ash_device = ash::Device::load(ash_instance.fp_v1_0(), vk_device_handle);

    // Log physical device name
    let props = ash_instance.get_physical_device_properties(vk_phys_device);
    let name = CStr::from_ptr(props.device_name.as_ptr());
    log::info!("Physical device: {:?}", name);

    // Create HAL instance from existing Vulkan instance
    let hal_instance = <Vulkan as Api>::Instance::from_raw(
        entry.clone(),
        ash_instance.clone(),
        vk::API_VERSION_1_1,
        0, // android_sdk_version
        None, // debug_utils
        vec![], // extensions we don't need to track
        wgt::InstanceFlags::empty(),
        wgt::MemoryBudgetThresholds::default(),
        false, // create_surfaces
        None, // debug_callback
    ).ok()?;

    // Expose adapter from physical device
    let hal_exposed_adapter = hal_instance.expose_adapter(vk_phys_device)?;

    // Create wgpu-core context from HAL instance
    let context = Arc::new(wgc::global::Global::from_hal_instance::<Vulkan>(
        "wgpu-vk-interop",
        hal_instance,
    ));

    // Register adapter with context
    let adapter_id = context.create_adapter_from_hal(hal_exposed_adapter.into(), None);

    let adapter = Arc::new(WGPUAdapterImpl {
        context: context.clone(),
        id: adapter_id,
    });

    // Create HAL OpenDevice from the existing Vulkan device
    let hal_adapter_guard = context.adapter_as_hal::<Vulkan>(adapter_id)?;
    let hal_open_device = hal_adapter_guard.device_from_raw(
        ash_device,
        None, // extension_fns
        &[], // enabled_extensions
        wgt::Features::empty(),
        &wgt::MemoryHints::default(),
        queue_family_index,
        0, // queue_index
    ).ok()?;
    drop(hal_adapter_guard);

    // Create device descriptor
    let desc = wgt::DeviceDescriptor {
        label: Some(Cow::Borrowed("vk-interop-device")),
        required_features: wgt::Features::empty(),
        required_limits: wgt::Limits::default(),
        memory_hints: wgt::MemoryHints::default(),
        experimental_features: Default::default(),
        trace: Default::default(),
    };

    // Create device from HAL
    let (device_id, queue_id) = context.create_device_from_hal(
        adapter_id,
        hal_open_device.into(),
        &desc,
        None, // instance_flags
        None, // trace_path
    ).ok()?;

    log::info!("wgpu device created successfully");

    // Create device impl
    let device_impl = Arc::new_cyclic(|weak_device: &Weak<WGPUDeviceImpl>| {
        let error_sink = ErrorSinkRaw::new(
            HalInteropCallbacks::default_device_lost(),
            weak_device.clone(),
        );
        let error_sink = Arc::new(Mutex::new(error_sink));

        WGPUDeviceImpl {
            context: context.clone(),
            id: device_id,
            queue: Arc::new(QueueId {
                context: context.clone(),
                id: queue_id,
            }),
            error_sink,
        }
    });

    // Keep adapter alive by leaking it (will be cleaned up when device is released)
    std::mem::forget(adapter);

    Some(Arc::into_raw(device_impl) as native::WGPUDevice)
}

/// Import a VkImage as a WGPUTexture.
///
/// # Safety
/// The device must have been created with wgpuCreateDeviceFromVulkan.
/// The VkImage must be valid and owned by the same Vulkan device.
#[no_mangle]
#[cfg(feature = "vulkan")]
pub unsafe extern "C" fn wgpuImportVulkanImage(
    device: native::WGPUDevice,
    vk_image: u64,
    vk_format: u32,
    width: u32,
    height: u32,
    array_layers: u32,
    mip_levels: u32,
    sample_count: u32,
) -> native::WGPUTexture {
    if device.is_null() {
        return std::ptr::null_mut();
    }

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        import_vulkan_image_impl(
            device,
            vk_image,
            vk_format as i32,
            width,
            height,
            array_layers,
            mip_levels,
            sample_count,
        )
    }));

    match result {
        Ok(Some(texture)) => texture,
        Ok(None) => {
            log::error!("wgpuImportVulkanImage failed");
            std::ptr::null_mut()
        }
        Err(e) => {
            log::error!("wgpuImportVulkanImage panicked: {:?}", e);
            std::ptr::null_mut()
        }
    }
}

#[cfg(feature = "vulkan")]
unsafe fn import_vulkan_image_impl(
    device: native::WGPUDevice,
    vk_image: u64,
    vk_format: i32,
    width: u32,
    height: u32,
    array_layers: u32,
    mip_levels: u32,
    sample_count: u32,
) -> Option<native::WGPUTexture> {
    let wgpu_format = vk_format_to_wgpu(vk_format)?;

    // Get device impl from raw pointer
    let device_arc = Arc::from_raw(device as *const WGPUDeviceImpl);
    // Increment ref count since we don't want to drop it
    let device_impl = Arc::clone(&device_arc);
    std::mem::forget(device_arc);

    let context = &device_impl.context;
    let device_id = device_impl.id;

    let size = wgt::Extent3d {
        width,
        height,
        depth_or_array_layers: array_layers,
    };

    let hal_usage = wgt::TextureUses::COLOR_TARGET | wgt::TextureUses::COPY_DST;

    // Get HAL device using public API
    let hal_device_guard = context.device_as_hal::<Vulkan>(device_id)?;

    // Create HAL texture from raw VkImage
    let hal_texture = hal_device_guard.texture_from_raw(
        vk::Image::from_raw(vk_image),
        &hal::TextureDescriptor {
            label: Some("imported_vk_image"),
            size,
            mip_level_count: mip_levels,
            sample_count,
            dimension: wgt::TextureDimension::D2,
            format: wgpu_format,
            usage: hal_usage,
            memory_flags: hal::MemoryFlags::empty(),
            view_formats: vec![],
        },
        None, // drop_guard - don't drop since we don't own the image
    );

    // Drop the guard before calling create_texture_from_hal (which also locks)
    drop(hal_device_guard);

    // Create texture descriptor
    let desc = wgt::TextureDescriptor {
        label: Some(Cow::Borrowed("imported_vk_image")),
        size,
        mip_level_count: mip_levels,
        sample_count,
        dimension: wgt::TextureDimension::D2,
        format: wgpu_format,
        usage: wgt::TextureUsages::RENDER_ATTACHMENT | wgt::TextureUsages::COPY_DST,
        view_formats: vec![],
    };

    // Register texture with wgpu-core
    let (texture_id, maybe_error) = context.create_texture_from_hal(
        Box::new(hal_texture),
        device_id,
        &desc,
        None,
    );

    if let Some(e) = maybe_error {
        log::error!("Failed to create texture from HAL: {:?}", e);
        return None;
    }

    log::info!("Imported texture: {}x{}", width, height);

    // Create texture impl
    let native_format = crate::conv::to_native_texture_format(wgpu_format)
        .unwrap_or(native::WGPUTextureFormat_Undefined);

    let texture_impl = Arc::new(WGPUTextureImpl {
        context: context.clone(),
        id: texture_id,
        error_sink: device_impl.error_sink.clone(),
        data: TextureData {
            usage: desc.usage.bits() as u64,
            dimension: native::WGPUTextureDimension_2D,
            size: native::WGPUExtent3D {
                width: desc.size.width,
                height: desc.size.height,
                depthOrArrayLayers: desc.size.depth_or_array_layers,
            },
            format: native_format,
            mip_level_count: desc.mip_level_count,
            sample_count: desc.sample_count,
        },
        surface_id: None,
        has_surface_presented: Arc::new(atomic::AtomicBool::new(false)),
    });

    Some(Arc::into_raw(texture_impl) as native::WGPUTexture)
}
