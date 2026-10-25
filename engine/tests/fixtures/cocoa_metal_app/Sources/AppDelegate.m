#import "AppDelegate.h"
#import "Renderer.h"
#import <MetalKit/MetalKit.h>

@implementation AppDelegate {
    Renderer *_renderer;
}
- (void)applicationDidFinishLaunching:(NSNotification *)note {
    self.window = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 800, 600)
                                              styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable | NSWindowStyleMaskResizable
                                                backing:NSBackingStoreBuffered defer:NO];
    MTKView *view = [[MTKView alloc] initWithFrame:self.window.contentView.bounds device:MTLCreateSystemDefaultDevice()];
    _renderer = [[Renderer alloc] initWithView:view];
    view.delegate = _renderer;
    self.window.contentView = view;
    NSString *settings = [[NSBundle mainBundle] pathForResource:@"settings" ofType:@"json"];
    NSLog(@"settings at %@", settings);
    [[NSUserDefaults standardUserDefaults] setBool:YES forKey:@"launchedBefore"];
    [self.window makeKeyAndOrderFront:nil];
}
@end
