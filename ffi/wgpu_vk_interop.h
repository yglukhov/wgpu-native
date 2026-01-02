/*
 * wgpu_vk_interop.h - Vulkan interop extensions for wgpu-native
 *
 * This header provides functions for creating wgpu resources from existing
 * Vulkan handles, enabling direct rendering to OpenXR swapchain images.
 *
 * Usage:
 *   #include "wgpu.h"             // wgpu-native C API
 *   #include "wgpu_vk_interop.h"  // Vulkan interop extensions
 *
 * The returned handles (WGPUDevice, WGPUTexture) are standard wgpu-native
 * handles that can be used with all standard wgpu-native functions.
 */

#ifndef WGPU_VK_INTEROP_H
#define WGPU_VK_INTEROP_H

#include "wgpu.h"
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Create a WGPUDevice from existing Vulkan handles.
 *
 * This allows wgpu-native to share an existing Vulkan device with OpenXR.
 * The returned device can be used with all standard wgpu-native functions.
 *
 * @param vk_instance        VkInstance handle (as uint64_t)
 * @param vk_physical_device VkPhysicalDevice handle (as uint64_t)
 * @param vk_device          VkDevice handle (as uint64_t)
 * @param queue_family_index Queue family index for graphics queue
 * @param queue_index        Queue index within the family (usually 0)
 * @return WGPUDevice handle, or NULL on failure
 */
WGPUDevice wgpuCreateDeviceFromVulkan(
    uint64_t vk_instance,
    uint64_t vk_physical_device,
    uint64_t vk_device,
    uint32_t queue_family_index,
    uint32_t queue_index
);

/**
 * Import a VkImage as a WGPUTexture.
 *
 * The returned texture can be used with standard wgpu-native functions
 * like wgpuTextureCreateView, wgpuTextureRelease, etc.
 *
 * @param device        WGPUDevice (must be created with wgpuCreateDeviceFromVulkan)
 * @param vk_image      VkImage handle (as uint64_t)
 * @param vk_format     VkFormat enum value (e.g., VK_FORMAT_R8G8B8A8_SRGB = 43)
 * @param width         Texture width
 * @param height        Texture height
 * @param array_layers  Number of array layers (1 for regular texture)
 * @param mip_levels    Number of mip levels (usually 1)
 * @param sample_count  MSAA sample count (usually 1)
 * @return WGPUTexture handle, or NULL on failure
 *
 * Note: The underlying VkImage is NOT owned by wgpu - caller is responsible
 * for its lifetime. The texture should be released with wgpuTextureRelease
 * before the VkImage is destroyed.
 */
WGPUTexture wgpuImportVulkanImage(
    WGPUDevice device,
    uint64_t vk_image,
    uint32_t vk_format,
    uint32_t width,
    uint32_t height,
    uint32_t array_layers,
    uint32_t mip_levels,
    uint32_t sample_count
);

#ifdef __cplusplus
}
#endif

#endif /* WGPU_VK_INTEROP_H */
