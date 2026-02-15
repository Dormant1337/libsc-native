#ifndef SC_PLAYER_H
#define SC_PLAYER_H

#include <stdint.h>
#include <stdbool.h>

// Stereo interleaved callback (L, R, L, R...)
typedef void (*sc_pcm_callback)(const float* samples, uint32_t count);

typedef struct SearchContext SearchContext;

// Error Handling
char* sc_get_last_error(void);
void sc_free_string(char* s);

// Search API
SearchContext* sc_search(const char* query);
uint32_t sc_search_result_count(SearchContext* ctx);
const char* sc_search_result_get_title(SearchContext* ctx, uint32_t idx);
const char* sc_search_result_get_url(SearchContext* ctx, uint32_t idx);
void sc_search_free(SearchContext* ctx);

// Playback & Download
// stop_signal can be NULL. If pointer provided, set *stop_signal = true to abort stream.
int32_t sc_stream_track(const char* url, sc_pcm_callback callback, const bool* stop_signal);
int32_t sc_download_track(const char* url);

#endif