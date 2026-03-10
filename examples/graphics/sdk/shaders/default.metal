#include <metal_stdlib>
using namespace metal;

struct VertexOut {
    float4 position [[position]];
    float4 color;
};

vertex VertexOut vertex_main(uint vid [[vertex_id]],
                              const device float *verts [[buffer(0)]]) {
    uint base = vid * 6;
    VertexOut out;
    out.position = float4(verts[base], verts[base + 1], 0.0, 1.0);
    out.color    = float4(verts[base + 2], verts[base + 3],
                          verts[base + 4], verts[base + 5]);
    return out;
}

fragment float4 fragment_main(VertexOut in [[stage_in]]) {
    return in.color;
}
