/* X11 window backend. */
#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <stdlib.h>
#include "platform.h"

static Display *dpy;
static Window win;
static GC gc;
static XImage *img;

int platform_window_open(int width, int height, const char *title)
{
    dpy = XOpenDisplay(NULL);
    if (!dpy) return 0;
    int screen = DefaultScreen(dpy);
    win = XCreateSimpleWindow(dpy, RootWindow(dpy, screen), 0, 0, width, height, 0, 0, 0);
    XStoreName(dpy, win, title);
    XSelectInput(dpy, win, KeyPressMask | ExposureMask);
    XMapWindow(dpy, win);
    gc = XCreateGC(dpy, win, 0, NULL);
    return 1;
}

int platform_window_poll(int *key_pressed)
{
    *key_pressed = 0;
    while (XPending(dpy)) {
        XEvent e;
        XNextEvent(dpy, &e);
        if (e.type == KeyPress) {
            KeySym ks = XLookupKeysym(&e.xkey, 0);
            *key_pressed = (int)ks;
        }
    }
    return 1;
}

void platform_window_present(const unsigned int *pixels, int width, int height)
{
    if (!img)
        img = XCreateImage(dpy, DefaultVisual(dpy, 0), 24, ZPixmap, 0, malloc((size_t)width * height * 4), width, height, 32, 0);
    for (int i = 0; i < width * height; i++) ((unsigned int *)img->data)[i] = pixels[i];
    XPutImage(dpy, win, gc, img, 0, 0, 0, 0, width, height);
    XFlush(dpy);
}

void platform_window_close(void)
{
    if (img) XDestroyImage(img);
    XCloseDisplay(dpy);
}
