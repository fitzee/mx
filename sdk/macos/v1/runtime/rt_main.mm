/*
 * rt_main.mm — macOS Cocoa runtime for M2 games.
 *
 * Owns: NSApplication, NSWindow, NSView, 60fps timer.
 * Calls: Game_Init / Game_Tick / Game_Shutdown (C ABI, provided by M2).
 * No ObjC types leak past rt_exports.h.
 */

#import <Cocoa/Cocoa.h>
#import <QuartzCore/QuartzCore.h>

#include "rt_exports.h"

static const uint32_t WIN_W = 800;
static const uint32_t WIN_H = 600;

/* ── App delegate ─────────────────────────────────────────────────── */

@interface M2AppDelegate : NSObject <NSApplicationDelegate>
@property (strong) NSWindow *window;
@property (strong) NSTimer  *timer;
@property (nonatomic) double  lastTime;
@end

@implementation M2AppDelegate

- (void)applicationDidFinishLaunching:(NSNotification *)note {
    NSRect frame = NSMakeRect(100, 100, WIN_W, WIN_H);
    _window = [[NSWindow alloc]
        initWithContentRect:frame
        styleMask:(NSWindowStyleMaskTitled |
                   NSWindowStyleMaskClosable |
                   NSWindowStyleMaskMiniaturizable |
                   NSWindowStyleMaskResizable)
        backing:NSBackingStoreBuffered
        defer:NO];
    [_window setTitle:@"M2 Game"];

    NSView *view = [[NSView alloc] initWithFrame:frame];
    [_window setContentView:view];
    [_window makeKeyAndOrderFront:nil];

    Game_Init((__bridge void *)view, WIN_W, WIN_H);

    _lastTime = CACurrentMediaTime();
    _timer = [NSTimer scheduledTimerWithTimeInterval:1.0 / 60.0
                                              repeats:YES
                                                block:^(NSTimer *t) {
        double now = CACurrentMediaTime();
        int32_t dt_ms = (int32_t)((now - self.lastTime) * 1000.0);
        if (dt_ms < 1) dt_ms = 1;
        self.lastTime = now;
        Game_Tick(dt_ms);
    }];
}

- (void)applicationWillTerminate:(NSNotification *)note {
    [_timer invalidate];
    _timer = nil;
    Game_Shutdown();
}

- (BOOL)applicationShouldTerminateAfterLastWindowClosed:(NSApplication *)app {
    return YES;
}

@end

/* ── Entry point ──────────────────────────────────────────────────── */

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        NSApplication *app = [NSApplication sharedApplication];
        [app setActivationPolicy:NSApplicationActivationPolicyRegular];

        M2AppDelegate *delegate = [[M2AppDelegate alloc] init];
        [app setDelegate:delegate];

        /* Build a minimal menu so Cmd-Q works */
        NSMenu *menubar = [[NSMenu alloc] init];
        NSMenuItem *appMenuItem = [[NSMenuItem alloc] init];
        [menubar addItem:appMenuItem];
        NSMenu *appMenu = [[NSMenu alloc] init];
        [appMenu addItemWithTitle:@"Quit"
                           action:@selector(terminate:)
                    keyEquivalent:@"q"];
        [appMenuItem setSubmenu:appMenu];
        [app setMainMenu:menubar];

        [app run];
    }
    return 0;
}
