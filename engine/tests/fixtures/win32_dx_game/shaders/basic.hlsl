cbuffer Frame : register(b0) { float4x4 mvp; };
struct VSIn { float3 pos : POSITION; float4 col : COLOR; };
struct VSOut { float4 pos : SV_POSITION; float4 col : COLOR; };
VSOut VSMain(VSIn i) { VSOut o; o.pos = mul(float4(i.pos, 1), mvp); o.col = i.col; return o; }
float4 PSMain(VSOut i) : SV_TARGET { return saturate(i.col); }
