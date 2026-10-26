# MPV Tuning and Hardware Decoding

## Hardware Decoding (`hwdec`)

Video decoding accounts for the majority of CPU and GPU power during media playback. The machine's dynamic GPU architecture determines optimal settings:

1. **Direct dGPU MUX Mode (`AsusMuxDgpu`):**
   - The NVIDIA GeForce RTX 5050 Mobile connects directly to the panel.
   - Setting `hwdec = "auto-safe"` engages Vulkan / NVDEC hardware acceleration.
   - Decodes 1080p60 and 4K AV1/VP9 streams with near-zero CPU usage.

2. **Optimus Hybrid Mode (`Hybrid`):**
   - The internal panel connects to the Intel iGPU, with the NVIDIA dGPU powering down into runtime D3cold sleep during 2D tasks.
   - Forcing NVDEC on the dGPU wakes the dedicated GPU and prevents it from sleeping, consuming battery.
   - Setting `hwdec = "vaapi"` offloads to the Intel UHD Graphics iGPU, or `hwdec = "no"` keeps low-resolution playback on CPU while maintaining dGPU sleep state.

Hardware decoding is configurable in `~/.config/yute/config.toml`:

```toml
[playback]
hwdec = 'auto-safe' # auto-safe | nvdec-copy | vaapi | no
```

Settings can also be cycled interactively from the Settings screen (`go` keybinding).

## Subtitle Configuration and Rate Limiting

Extracting all subtitle tracks across hundreds of languages via yt-dlp `write-subs` can trigger YouTube HTTP 429 rate limits and stream segment stalls. Subtitles are handled cleanly via MPV's native subtitle selector and `ytdl_hook`:

```ini
sub-auto=fuzzy
sub-visibility=yes
slang=en,eng,en-US,en-GB,hi,hin
```

This ensures required language subtitles are automatically selected without making redundant parallel requests or causing HTTP 429 errors.

## Video Output (`vo`)

The profile uses `vo=gpu-next` (the modern libplacebo rendering engine) rather than the legacy `vo=gpu`. On modern Wayland compositors (such as Sway or i3 with Xwayland), `gpu-next` provides seamless Vulkan video decode presentation (`VK_STD_vulkan_video_codec_av1_decode`) and prevents window swapchain presentation freezes.

## Stream Cache and Buffering

To eliminate playback stalls on high-bitrate AV1 streams:

```ini
cache=yes
demuxer-max-bytes=512MiB
demuxer-max-back-bytes=256MiB
demuxer-readahead-secs=300
stream-buffer-size=4MiB
network-timeout=15
force-seekable=yes
ytdl-raw-options-append=no-playlist=
ytdl-raw-options-append=concurrent-fragments=4
ytdl-raw-options-append=buffer-size=16k
```


## Yute stream-cache control

The Settings slider now drives dedicated MPV launch arguments and active IPC
properties: cache-secs, demuxer-readahead-secs and demuxer-max-bytes. Presets are
5/10/20/40/60 seconds with 32/64/128/192/256 MiB packet budgets; back-cache is bounded
at 32 MiB. These replace the fixed startup values in the older tuning example
above for dedicated-profile launches. cache-pause-wait is one second; the full
readahead horizon is not an initial startup delay. User-profile mode preserves
that profile's initial buffering configuration. :quality changes active quality
through file-local MPV load options and preserves position/pause.

See the [MPV cache documentation](https://mpv.io/manual/stable/#cache) for the
relationship between time and byte limits. Larger buffers cannot eliminate
YouTube extraction or network latency.
