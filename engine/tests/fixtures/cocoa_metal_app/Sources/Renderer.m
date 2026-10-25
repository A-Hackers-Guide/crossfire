#import "Renderer.h"
#import <Metal/Metal.h>
#import <AVFoundation/AVFoundation.h>
#include <mach/mach_time.h>
#include "geometry.h"

@implementation Renderer {
    id<MTLDevice> _device;
    id<MTLCommandQueue> _queue;
    id<MTLRenderPipelineState> _pipeline;
    AVAudioPlayer *_music;
    uint64_t _start;
}
- (instancetype)initWithView:(MTKView *)view {
    if ((self = [super init])) {
        _device = view.device;
        _queue = [_device newCommandQueue];
        id<MTLLibrary> lib = [_device newDefaultLibrary];
        MTLRenderPipelineDescriptor *d = [MTLRenderPipelineDescriptor new];
        d.vertexFunction = [lib newFunctionWithName:@"vertex_main"];
        d.fragmentFunction = [lib newFunctionWithName:@"fragment_main"];
        d.colorAttachments[0].pixelFormat = view.colorPixelFormat;
        _pipeline = [_device newRenderPipelineStateWithDescriptor:d error:nil];
        _start = mach_absolute_time();
        dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
            NSLog(@"loaded %d vertices", geometry_vertex_count());
        });
    }
    return self;
}
- (void)drawInMTKView:(MTKView *)view {
    id<MTLCommandBuffer> cb = [_queue commandBuffer];
    id<MTLRenderCommandEncoder> enc = [cb renderCommandEncoderWithDescriptor:view.currentRenderPassDescriptor];
    [enc setRenderPipelineState:_pipeline];
    [enc drawPrimitives:MTLPrimitiveTypeTriangle vertexStart:0 vertexCount:geometry_vertex_count()];
    [enc endEncoding];
    [cb presentDrawable:view.currentDrawable];
    [cb commit];
}
- (void)mtkView:(MTKView *)view drawableSizeWillChange:(CGSize)size {}
@end
