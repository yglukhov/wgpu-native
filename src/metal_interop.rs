//! Metal HAL interop for wgpu-native
//!
//! Provides functions to create wgpu resources from existing Metal objects.

use std::ptr;

#[cfg(all(target_os = "macos", feature = "metal"))]
use metal::foreign_types::ForeignType;

/// Creates a WGPUTexture from an existing Metal MTLTexture.
///
/// # Safety
/// - `device` must be a valid WGPUDevice created with the Metal backend
/// - `mtl_texture` must be a valid id<MTLTexture> pointer
/// - `descriptor` must accurately describe the MTLTexture's properties
/// - The MTLTexture must remain valid for the lifetime of the returned WGPUTexture
#[no_mangle]
#[cfg(all(any(target_os = "macos", target_os = "ios", target_os = "visionos"), feature = "metal"))]
pub unsafe extern "C" fn wgpuDeviceCreateTextureFromMTLTexture(
    device: crate::wgc::id::DeviceId,
    mtl_texture: *mut std::ffi::c_void,
    descriptor: *const crate::WGPUTextureDescriptor,
) -> crate::wgc::id::TextureId {
    use wgpu_hal::api::Metal as M;

    let global = &crate::WGPU_GLOBAL;

    // Convert the void* to a Metal texture reference
    let metal_texture = metal::Texture::from_ptr(mtl_texture as *mut metal::MTLTexture);

    // Get descriptor info
    let desc = &*descriptor;

    // Create HAL texture descriptor
    let hal_desc = wgpu_hal::TextureDescriptor {
        label: if !desc.label.data.is_null() && desc.label.length > 0 {
            Some(std::str::from_utf8_unchecked(std::slice::from_raw_parts(
                desc.label.data as *const u8,
                desc.label.length,
            )))
        } else {
            None
        },
        size: wgpu_types::Extent3d {
            width: desc.size.width,
            height: desc.size.height,
            depth_or_array_layers: desc.size.depthOrArrayLayers,
        },
        mip_level_count: desc.mipLevelCount,
        sample_count: desc.sampleCount,
        dimension: crate::conv::map_texture_dimension(desc.dimension),
        format: crate::conv::map_texture_format(desc.format),
        usage: crate::conv::map_texture_usage(desc.usage),
        memory_flags: wgpu_hal::MemoryFlags::empty(),
        view_formats: vec![],
    };

    // Create HAL texture from Metal texture
    let hal_texture = <M as wgpu_hal::Api>::Device::texture_from_raw(
        metal_texture,
        &hal_desc,
    );

    // Create wgpu-core texture ID
    let (texture_id, error) = global.create_texture_from_hal::<M>(
        hal_texture,
        device,
        &hal_desc,
        None,
    );

    if let Some(err) = error {
        log::error!("Failed to create texture from Metal texture: {:?}", err);
        return ptr::null_mut();
    }

    texture_id
}

// Stub implementation for non-Apple platforms
#[no_mangle]
#[cfg(not(all(any(target_os = "macos", target_os = "ios", target_os = "visionos"), feature = "metal")))]
pub unsafe extern "C" fn wgpuDeviceCreateTextureFromMTLTexture(
    _device: crate::wgc::id::DeviceId,
    _mtl_texture: *mut std::ffi::c_void,
    _descriptor: *const crate::WGPUTextureDescriptor,
) -> crate::wgc::id::TextureId {
    panic!("wgpuDeviceCreateTextureFromMTLTexture is only available on Apple platforms with Metal backend");
}
