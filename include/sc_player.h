#ifndef SC_PLAYER_H
#define SC_PLAYER_H

#include <stdint.h>

// Callback function type for receiving PCM float samples (mono)
typedef void (*sc_pcm_callback)(const float* samples, uint32_t count);

// Blocking call to stream audio. Returns 0 on success.
int32_t sc_stream_track(const char* url, sc_pcm_callback callback);

// Blocking call to download track to current directory. Returns 0 on success.
int32_t sc_download_track(const char* url);

#endif