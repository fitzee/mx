/*
 * mtl_wrap.mm — Pure C ABI Metal wrapper (ObjC++ / ARC).
 *
 * Every returned handle carries a +1 retain count owned by the caller.
 * Pass handles to mtl_release() when done.
 */

#import <Metal/Metal.h>
#import <QuartzCore/CAMetalLayer.h>
#import <AppKit/AppKit.h>

#include "mtl_wrap.h"

/* ── Bridge helpers ───────────────────────────────────────────────── */

static inline id objc(void *h) { return (__bridge id)h; }

static inline void *opaque_retain(id obj) {
    return (__bridge_retained void *)obj;
}

static inline id opaque_consume(void *h) {
    return (__bridge_transfer id)h;
}

/* ── C API ────────────────────────────────────────────────────────── */

extern "C" {

MtlDevice mtl_create_device(void) {
    id<MTLDevice> dev = MTLCreateSystemDefaultDevice();
    if (!dev) {
        NSLog(@"mtl_wrap: no Metal device");
        return NULL;
    }
    return opaque_retain(dev);
}

MtlQueue mtl_create_queue(MtlDevice device) {
    id<MTLDevice> dev = (id<MTLDevice>)objc(device);
    return opaque_retain([dev newCommandQueue]);
}

MtlLayer mtl_attach_layer(MtlDevice device, void *ns_view,
                           uint32_t width, uint32_t height) {
    id<MTLDevice> dev = (id<MTLDevice>)objc(device);
    NSView *view = (NSView *)objc(ns_view);

    CAMetalLayer *layer = [CAMetalLayer layer];
    layer.device          = dev;
    layer.pixelFormat     = MTLPixelFormatBGRA8Unorm;
    layer.drawableSize    = CGSizeMake(width, height);
    layer.framebufferOnly = YES;

    view.wantsLayer = YES;
    view.layer      = layer;

    return opaque_retain(layer);
}

void mtl_layer_resize(MtlLayer layer, uint32_t width, uint32_t height) {
    CAMetalLayer *l = (CAMetalLayer *)objc(layer);
    l.drawableSize = CGSizeMake(width, height);
}

MtlLibrary mtl_load_default_library(MtlDevice device) {
    id<MTLDevice> dev = (id<MTLDevice>)objc(device);
    id<MTLLibrary> lib = [dev newDefaultLibrary];
    if (!lib) return NULL;
    return opaque_retain(lib);
}

MtlLibrary mtl_load_library(MtlDevice device, const char *path) {
    id<MTLDevice> dev = (id<MTLDevice>)objc(device);
    NSError *err = nil;
    NSURL *url = [NSURL fileURLWithPath:[NSString stringWithUTF8String:path]];
    id<MTLLibrary> lib = [dev newLibraryWithURL:url error:&err];
    if (!lib) {
        NSLog(@"mtl_wrap: load_library failed: %@", err);
        return NULL;
    }
    return opaque_retain(lib);
}

MtlLibrary mtl_compile_library(MtlDevice device, const char *source) {
    id<MTLDevice> dev = (id<MTLDevice>)objc(device);
    NSString *src = [NSString stringWithUTF8String:source];
    NSError *err = nil;
    id<MTLLibrary> lib = [dev newLibraryWithSource:src options:nil error:&err];
    if (!lib) {
        NSLog(@"mtl_wrap: compile_library failed: %@", err);
        return NULL;
    }
    return opaque_retain(lib);
}

MtlPipeline mtl_create_pipeline(MtlDevice device, MtlLibrary library,
                                 const char *vertex_fn,
                                 const char *fragment_fn) {
    id<MTLDevice>  dev = (id<MTLDevice>)objc(device);
    id<MTLLibrary> lib = (id<MTLLibrary>)objc(library);

    id<MTLFunction> vert =
        [lib newFunctionWithName:[NSString stringWithUTF8String:vertex_fn]];
    id<MTLFunction> frag =
        [lib newFunctionWithName:[NSString stringWithUTF8String:fragment_fn]];
    if (!vert || !frag) {
        NSLog(@"mtl_wrap: shader function not found (vert=%p frag=%p)",
              vert, frag);
        return NULL;
    }

    MTLRenderPipelineDescriptor *desc =
        [[MTLRenderPipelineDescriptor alloc] init];
    desc.vertexFunction   = vert;
    desc.fragmentFunction = frag;
    desc.colorAttachments[0].pixelFormat = MTLPixelFormatBGRA8Unorm;

    NSError *err = nil;
    id<MTLRenderPipelineState> pso =
        [dev newRenderPipelineStateWithDescriptor:desc error:&err];
    if (!pso) {
        NSLog(@"mtl_wrap: pipeline creation failed: %@", err);
        return NULL;
    }
    return opaque_retain(pso);
}

MtlBuffer mtl_create_buffer(MtlDevice device, const void *data,
                              uint32_t size) {
    id<MTLDevice> dev = (id<MTLDevice>)objc(device);
    id<MTLBuffer> buf;
    if (data) {
        buf = [dev newBufferWithBytes:data
                               length:(NSUInteger)size
                              options:MTLResourceStorageModeShared];
    } else {
        buf = [dev newBufferWithLength:(NSUInteger)size
                               options:MTLResourceStorageModeShared];
    }
    return opaque_retain(buf);
}

void mtl_update_buffer(MtlBuffer buffer, const void *data, uint32_t size) {
    id<MTLBuffer> buf = (id<MTLBuffer>)objc(buffer);
    memcpy(buf.contents, data, (size_t)size);
}

void mtl_draw(MtlQueue queue, MtlLayer layer, MtlPipeline pipeline,
              MtlBuffer vertex_buffer, uint32_t vertex_count,
              const float clear_rgba[4]) {
    @autoreleasepool {
        id<MTLCommandQueue> q = (id<MTLCommandQueue>)objc(queue);
        CAMetalLayer *l       = (CAMetalLayer *)objc(layer);
        id<MTLRenderPipelineState> pso =
            (id<MTLRenderPipelineState>)objc(pipeline);
        id<MTLBuffer> vbuf = (id<MTLBuffer>)objc(vertex_buffer);

        id<CAMetalDrawable> drawable = [l nextDrawable];
        if (!drawable) return;

        double r = 0, g = 0, b = 0, a = 1;
        if (clear_rgba) {
            r = clear_rgba[0]; g = clear_rgba[1];
            b = clear_rgba[2]; a = clear_rgba[3];
        }

        MTLRenderPassDescriptor *rpd =
            [MTLRenderPassDescriptor renderPassDescriptor];
        rpd.colorAttachments[0].texture     = drawable.texture;
        rpd.colorAttachments[0].loadAction  = MTLLoadActionClear;
        rpd.colorAttachments[0].storeAction = MTLStoreActionStore;
        rpd.colorAttachments[0].clearColor  =
            MTLClearColorMake(r, g, b, a);

        id<MTLCommandBuffer> cmd = [q commandBuffer];
        id<MTLRenderCommandEncoder> enc =
            [cmd renderCommandEncoderWithDescriptor:rpd];

        [enc setRenderPipelineState:pso];
        [enc setVertexBuffer:vbuf offset:0 atIndex:0];
        [enc drawPrimitives:MTLPrimitiveTypeTriangle
                vertexStart:0
                vertexCount:vertex_count];
        [enc endEncoding];

        [cmd presentDrawable:drawable];
        [cmd commit];
    }
}

void mtl_release(void *handle) {
    if (handle) {
        (void)opaque_consume(handle);
    }
}

} /* extern "C" */
