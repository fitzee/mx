/*
 * mtl_wrap.h — Pure C ABI wrapper around Metal.
 *
 * All Metal/ObjC objects stay behind opaque void* handles.
 * Caller owns the +1 retain on every handle returned; pass to
 * mtl_release() when done.
 */

#ifndef MTL_WRAP_H
#define MTL_WRAP_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef void *MtlDevice;
typedef void *MtlQueue;
typedef void *MtlLayer;
typedef void *MtlLibrary;
typedef void *MtlPipeline;
typedef void *MtlBuffer;

/* Device & queue */
MtlDevice   mtl_create_device(void);
MtlQueue    mtl_create_queue(MtlDevice device);

/* Layer (swapchain) */
MtlLayer    mtl_attach_layer(MtlDevice device, void *nsview,
                              uint32_t width, uint32_t height);
void        mtl_layer_resize(MtlLayer layer,
                              uint32_t width, uint32_t height);

/* Shader libraries */
MtlLibrary  mtl_load_default_library(MtlDevice device);
MtlLibrary  mtl_load_library(MtlDevice device, const char *path);
MtlLibrary  mtl_compile_library(MtlDevice device, const char *source);

/* Pipeline */
MtlPipeline mtl_create_pipeline(MtlDevice device, MtlLibrary library,
                                 const char *vertex_fn,
                                 const char *fragment_fn);

/* Buffers */
MtlBuffer   mtl_create_buffer(MtlDevice device, const void *data,
                               uint32_t size);
void        mtl_update_buffer(MtlBuffer buffer, const void *data,
                               uint32_t size);

/* Draw + present */
void        mtl_draw(MtlQueue queue, MtlLayer layer,
                      MtlPipeline pipeline, MtlBuffer vertex_buffer,
                      uint32_t vertex_count,
                      const float clear_rgba[4]);

/* Release any handle */
void        mtl_release(void *handle);

#ifdef __cplusplus
}
#endif

#endif /* MTL_WRAP_H */
