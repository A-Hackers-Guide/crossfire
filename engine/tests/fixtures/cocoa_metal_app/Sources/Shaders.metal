#include <metal_stdlib>
using namespace metal;
vertex float4 vertex_main(uint vid [[vertex_id]]) {
    float2 p[3] = { float2(0, 0.6), float2(-0.6, -0.6), float2(0.6, -0.6) };
    return float4(p[vid], 0, 1);
}
fragment float4 fragment_main() { return float4(1, 0.5, 0.1, 1); }
