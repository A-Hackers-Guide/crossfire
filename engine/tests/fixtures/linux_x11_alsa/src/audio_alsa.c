/* ALSA audio backend. */
#include <alsa/asoundlib.h>
#include <math.h>
#include "platform.h"

static snd_pcm_t *pcm;
static int rate;

int platform_audio_open(int sample_rate)
{
    rate = sample_rate;
    if (snd_pcm_open(&pcm, "default", SND_PCM_STREAM_PLAYBACK, 0) < 0) return 0;
    return snd_pcm_set_params(pcm, SND_PCM_FORMAT_S16_LE, SND_PCM_ACCESS_RW_INTERLEAVED, 1, (unsigned)rate, 1, 50000) == 0;
}

void platform_audio_beep(float frequency, float seconds)
{
    short buf[4096];
    int n = (int)(seconds * rate);
    if (n > 4096) n = 4096;
    for (int i = 0; i < n; i++) buf[i] = (short)(8000 * sinf(2.0f * 3.14159f * frequency * i / rate));
    if (pcm) snd_pcm_writei(pcm, buf, (snd_pcm_uframes_t)n);
}

void platform_audio_close(void)
{
    if (pcm) { snd_pcm_drain(pcm); snd_pcm_close(pcm); }
}
