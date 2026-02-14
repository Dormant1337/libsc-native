# libsc-native

**libsc-native** is a high-performance, portable static library written in Rust that provides SoundCloud streaming and downloading capabilities to C and C++ applications.

By leveraging Rust's safety and modern networking stack (using `rustls` instead of OpenSSL), this library offers a self-contained, statically linked solution that handles URL resolution, HTTP streaming, and MP3 decoding internally, exposing raw PCM audio data via a simple C ABI.

## Features

*   **Zero OpenSSL Dependency**: Uses `rustls` to avoid complex runtime linking issues on Linux/macOS.
*   **Built-in Decoding**: Decodes MP3 streams internally and outputs raw Float32 PCM audio.
*   **Simple C API**: Minimalist interface with just two primary functions.
*   **Static Linking**: Compiles to a single `.a` file for easy embedding.
*   **Smart Resolution**: Automatically handles SoundCloud client ID resolution and API interaction.

## Building the Library

You need the Rust toolchain installed.

1.  Clone the repository:
    ```bash
    git clone https://github.com/your-username/libsc-native.git
    cd libsc-native
    ```

2.  Build the static library:
    ```bash
    cargo build --release
    ```

The compiled artifact will be located at:
`target/release/libsoundcloud_streamer.a`

## C API Reference

Include the header `include/sc_player.h`.

### Types

```c
/**
 * Callback function prototype for receiving audio data.
 * @param samples Pointer to the array of float samples (PCM Float32).
 * @param count   Number of samples in the buffer.
 */
typedef void (*sc_pcm_callback)(const float* samples, uint32_t count);
```

### Functions

#### `sc_stream_track`

Resolves a SoundCloud URL, decodes the audio stream, and invokes the callback with raw PCM data in real-time. This function blocks until the track finishes or an error occurs.

```c
int32_t sc_stream_track(const char* url, sc_pcm_callback callback);
```

*   **Returns**: `0` on success, negative error code on failure.

#### `sc_download_track`

Resolves a SoundCloud URL and downloads the track as an MP3 file to the current working directory. The filename is generated automatically based on the artist and title.

```c
int32_t sc_download_track(const char* url);
```

*   **Returns**: `0` on success, negative error code on failure.

## Usage Example

Below is a minimal example of how to link and use the library in a C program.

**main.c**
```c
#include <stdio.h>
#include "sc_player.h"

// Simple callback to handle audio data
void on_audio_data(const float* samples, uint32_t count) {
    // Process audio (e.g., send to SDL2, PortAudio, or write to stdout)
    printf("Received %d samples\n", count);
}

int main() {
    const char* url = "https://soundcloud.com/artist/track";
    
    printf("Streaming track...\n");
    int result = sc_stream_track(url, on_audio_data);
    
    if (result != 0) {
        fprintf(stderr, "Error streaming track: %d\n", result);
        return 1;
    }
    
    return 0;
}
```

### Linking

When linking against `libsc-native`, you must include the system libraries required by the Rust runtime (`pthread`, `dl`, `m`).

```bash
gcc main.c -o my_app \
    target/release/libsoundcloud_streamer.a \
    -lpthread -ldl -lm
```

## Example Player

This repository includes a full-featured example implementation (`examples/main.c`) utilizing **ncurses** for the UI and **SDL2** for audio playback.

To build the example player:

```bash
make
./sc_player "https://soundcloud.com/some-artist/some-track"
```