/* Platform interface used by the game loop. */
#ifndef PLATFORM_H
#define PLATFORM_H
int  platform_window_open(int width, int height, const char *title);
int  platform_window_poll(int *key_pressed);
void platform_window_present(const unsigned int *pixels, int width, int height);
void platform_window_close(void);
int  platform_audio_open(int sample_rate);
void platform_audio_beep(float frequency, float seconds);
void platform_audio_close(void);
#endif
