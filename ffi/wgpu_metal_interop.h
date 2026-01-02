/*
 * wgpu_metal_interop.h - Metal interop extensions for wgpu-native
 *
 * This header provides functions for creating wgpu resources from existing
 * Metal handles, enabling interoperability with CompositorServices and other
 * Metal-based frameworks on Apple platforms.
 *
 * Usage:
 *   #include "wgpu.h"              // wgpu-native C API
 *   #include "wgpu_metal_interop.h" // Metal interop extensions
 *
 * The returned handles (WGPUTexture) are standard wgpu-native handles that
 * can be used with all standard wgpu-native functions.
 */

#ifndef WGPU_METAL_INTEROP_H
#define WGPU_METAL_INTEROP_H

#include "wgpu.h"

#if defined(__APPLE__)

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Creates a WGPUTexture from an existing Metal MTLTexture.
 *
 * This function allows wrapping an externally-created MTLTexture as a wgpu texture,
 * enabling interoperability between wgpu and Metal rendering pipelines. This is
 * useful for rendering to CompositorServices drawables on visionOS or integrating
 * with other Metal-based frameworks.
 *
 * @param device      The wgpu device (must be using the Metal backend)
 * @param mtlTexture  A pointer to an id<MTLTexture>
 * @param descriptor  The texture descriptor describing the MTLTexture's properties.
 *                    The descriptor must accurately match the MTLTexture's format,
 *                    dimensions, mip levels, and usage flags.
 * @return A new WGPUTexture wrapping the Metal texture, or NULL on failure.
 *
 * @note The MTLTexture must remain valid for the lifetime of the returned WGPUTexture.
 *       The caller is responsible for ensuring the MTLTexture is not destroyed while
 *       the WGPUTexture is in use.
 *
 * @note The device must have been created with the Metal backend. Using this function
 *       with a device using a different backend will result in undefined behavior.
 *
 * @note The underlying MTLTexture is NOT owned by wgpu - caller is responsible
 *       for its lifetime. The texture should be released with wgpuTextureRelease
 *       before the MTLTexture is destroyed.
 */
WGPUTexture wgpuDeviceCreateTextureFromMTLTexture(
    WGPUDevice device,
    void * mtlTexture,
    WGPUTextureDescriptor const * descriptor);

#ifdef __cplusplus
}
#endif

#endif /* __APPLE__ */

#endif /* WGPU_METAL_INTEROP_H */
