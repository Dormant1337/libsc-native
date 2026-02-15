# libsc-native

**libsc-native** is a high-performance, portable static library written in Rust that provides SoundCloud streaming, searching, and downloading capabilities to C and C++ applications.

By leveraging Rust's safety and modern networking stack (using `rustls` instead of OpenSSL), this library offers a self-contained, statically linked solution. It handles URL resolution, search queries, and MP3 decoding internally, exposing interleaved stereo PCM audio data via a simple C ABI.

## Features

*   **Zero OpenSSL Dependency**: Uses `rustls` to avoid complex runtime linking issues on Linux/macOS.
*   **Built-in Decoding**: Decodes MP3 streams internally using `symphonia` and outputs raw **Interleaved Stereo Float32 PCM**.
*   **Search API**: Integrated SoundCloud track search with results context management.
*   **Abortable Streaming**: Support for stop signals to interrupt blocking stream calls.
*   **Extended Error Handling**: Thread-local error reporting for detailed failure diagnostics.
*   **Static Linking**: Compiles to a single `.a` file for easy embedding into native applications.

## Building the Library

You need the Rust toolchain (cargo) installed.

1.  Clone the repository:
    ```bash
    git clone https://github.com/your-username/libsc-native.git
    cd libsc-native
    ```

2.  Build the static library:
    ```bash
    cargo build --release
    ```

The compiled artifact will be located at: `target/release/libsoundcloud_streamer.a`

## C API Reference

Include the header `include/sc_player.h`.

### Types & Callbacks

```c
// Stereo interleaved callback (L, R, L, R...)
typedef void (*sc_pcm_callback)(const float* samples, uint32_t count);

// Opaque context for search results
typedef struct SearchContext SearchContext;
```

### Functions

#### Error Management
*   `char* sc_get_last_error()`: Returns a string describing the last error. Memory must be freed using `sc_free_string`.
*   `void sc_free_string(char* s)`: Safely frees strings allocated by the library.

#### Search API
*   `SearchContext* sc_search(const char* query)`: Performs a track search. Returns a context pointer or `NULL` on failure.
*   `uint32_t sc_search_result_count(SearchContext* ctx)`: Returns the number of tracks found.
*   `const char* sc_search_result_get_title(SearchContext* ctx, uint32_t idx)`: Returns the title/artist of the result at `idx`.
*   `const char* sc_search_result_get_url(SearchContext* ctx, uint32_t idx)`: Returns the URL of the result at `idx`.
*   `void sc_search_free(SearchContext* ctx)`: Frees the search context and all associated results.

#### Playback & Download
*   `int32_t sc_stream_track(const char* url, sc_pcm_callback callback, const bool* stop_signal)`: 
    Blocks and streams audio. If `stop_signal` points to a boolean that becomes `true`, the stream will abort immediately.
*   `int32_t sc_download_track(const char* url)`: 
    Downloads the track as an MP3 file to the current directory.

## Usage Example

Below is a minimal example of performing a search and streaming the first result.

```c
#include <stdio.h>
#include <stdbool.h>
#include "sc_player.h"

void on_audio(const float* samples, uint32_t count) {
    // Forward to audio device (e.g., SDL_QueueAudio)
}

int main() {
    bool stop = false;

    // Search for a track
    SearchContext* ctx = sc_search("Kavinsky Nightcall");
    if (!ctx) return 1;

    if (sc_search_result_count(ctx) > 0) {
        const char* url = sc_search_result_get_url(ctx, 0);
        printf("Streaming: %s\n", sc_search_result_get_title(ctx, 0));
        
        // Start streaming (blocking)
        if (sc_stream_track(url, on_audio, &stop) != 0) {
            char* err = sc_get_last_error();
            fprintf(stderr, "Error: %s\n", err);
            sc_free_string(err);
        }
    }

    sc_search_free(ctx);
    return 0;
}
```

### Linking

Link against the static library and system dependencies (`pthread`, `dl`, `m`).

```bash
gcc main.c -o my_app \
    target/release/libsoundcloud_streamer.a \
    -lpthread -ldl -lm
```

## Example Player

This repository includes a full-featured TUI implementation (`examples/main.c`) utilizing **ncurses** and **SDL2**.

To build and run the example:
```bash
make
./sc_player
```

## Troubleshooting

*   **No sound / Error -4**: This library supports "Progressive" MP3 streams. Some SoundCloud tracks (Go+ or restricted) are only available via HLS (m3u8), which is not currently supported.
*   **Linking issues**: Ensure the Rust library is built with `rustls-tls` (enabled by default in this repo) to avoid OpenSSL version mismatches.