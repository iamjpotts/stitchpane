# 0007: Uploading canvas images in the iced build

Status: Accepted

## Context

The iced build shows the canvas as one RGBA image, re-rendered by the core after every
change and handed to iced as a new `image::Handle`. In iced 0.14 (`iced_wgpu` 0.14.0),
an image of 2 MiB or more (`MAX_SYNC_SIZE` in `iced_wgpu/src/image/cache.rs`) is
uploaded to the GPU by a background worker thread, and iced draws nothing for it until
the upload is done. A canvas image is about 15 MB at a typical window size.

Two problems followed:

- **Flashing.** Each new handle was drawn before its upload finished, so the canvas
  showed the white window background for a frame or more. Moving the pointer over the
  empty start view re-renders the canvas, so it flashed continuously. UI-011 fixes this
  by keeping the previous image on screen until iced reports the new one allocated
  (`iced_runtime::image::allocate`).
- **Slow updates.** With one upload at a time, a drag updated the canvas about every
  100 ms, although the GPU copy itself takes 1–2 ms. Measured on Ubuntu 24.04 / i3 with
  30 Hz displays (33.3 ms per refresh), Vulkan and GL alike, the 100 ms is three
  refreshes:
  1. **The worker's submit waits for vsync.** wgpu-core 27.0.3's
     `Surface::get_current_texture` (`src/present.rs`) holds `device.fence.read()`
     while it blocks in `acquire_texture`, which with FIFO presentation lasts until the
     next vsync. `Queue::submit` (`src/device/queue.rs`) needs `device.fence.write()`,
     so the upload worker cannot submit while the main thread waits for a swapchain
     image. The worker's submit time matched the main thread's acquire time to within
     0.4 ms.
  2. **Finished uploads are collected only while drawing.** iced's `Cache::receive()`
     runs inside `upload_raster` and `trim`, during a draw, after the next swapchain
     acquire, so a finished upload is seen one refresh later.
  3. **The completion returns through the event loop.** The allocation callback sends
     a oneshot that iced's executor and proxy turn into a message. That message waits
     behind queued pointer events, each re-rendering the whole canvas (21 ms on
     average, up to 46 ms), and behind another refresh.

  When the window is idle, the same chain takes 2–5 ms.

## Options

- **Keep several uploads in flight** and show the newest ready image (UI-011, UI-012).
- **One upload at a time**, the newest frame waiting. Simple, but the update rate
  equals the three-refresh latency: about 10 updates per second at 30 Hz.
- **Unlimited uploads in flight.** Keeps pace too, but memory grows without bound while
  the worker falls behind.
- **Change the present mode** (`ICED_PRESENT_MODE`, or `vsync: false` in iced's
  settings). Measured: `immediate` updated every 11–20 ms but tears; `mailbox` was
  erratic (2–87 ms). It changes presentation for the whole window, and the environment
  variable is per user, not part of the application.
- **Render once per display refresh** instead of once per pointer event. Removes most of
  the main-thread blocking ahead of steps 1 and 3, but not the three-refresh handshake.
  It complements this decision rather than replacing it.
- **Draw each source image as its own GPU image** and move it while dragging, so a drag
  needs no upload. This removes the problem at its root but moves compositing out of the
  core, against the core/UI split in `architecture.md`.
- **Stay under the 2 MiB synchronous limit**, for example with a downscaled preview
  while dragging. Blurry at a typical window size.
- **Skip unchanged frames** (compare pixels or have the core report changes). Avoids
  needless uploads when hovering, but a changing frame still waits for its upload.
- **Paint a background behind the image** in the canvas background color. Hides the
  white flash but not the missing image.
- **Patch or fork iced or wgpu.** Fixes the cause, but carries a fork. Reporting
  upstream is preferred (see below).

## Decision

The iced build keeps up to three canvas uploads in flight (`MAX_UPLOADS` in
`stitchpane-iced`), three being the number of refreshes one upload takes. Each newly
rendered frame starts uploading at once; when the limit is reached, only the newest
waiting frame is kept and it starts when an upload finishes. A finished upload is shown
only if it is newer than the shown image. A frame stays counted until iced reports it,
so the count matches iced's uploads.

## Consequences

- During a drag the canvas updates about once per refresh (measured: every 33 ms at
  30 Hz, up from about 100 ms) and never shows the window background. It still trails
  the pointer by about three refreshes plus the render time.
- Up to five canvas images are in memory at once (shown, three uploading, one waiting):
  about 75 MB at a typical window size, about 160 MB for a full-screen 4K window.
- The headless tests do not run iced tasks, so they report allocations themselves
  (`present` in `stitchpane-iced/src/tests.rs`).
- Revisit when iced or wgpu change as below, or if the canvas moves to per-image
  drawing.

## Possible upstream improvements

These would remove the delay at its source. Not yet reported upstream; check the
current versions first.

### wgpu (seen in wgpu-core 27.0.3)

- `Surface::get_current_texture` holds the device fence lock for reading while
  `acquire_texture` blocks, which with FIFO presentation can last a whole refresh. Any
  `Queue::submit` on another thread waits for it. Releasing the lock during the blocking
  acquire, or not needing it there, would let background uploads continue while the
  main thread waits for vsync.

### iced (seen in iced 0.14.0, iced_wgpu 0.14.0, iced_winit 0.14.1)

- **Collect finished uploads when the worker reports them**, not only inside a draw.
  `Cache::receive()` runs from `upload_raster` and `trim`, so an allocation callback
  fires only after the next swapchain acquire. Handling the worker's notification on the
  event loop by running `receive()` and its callbacks would save a refresh.
- **Resolve allocations without the executor round trip.** The callback sends a oneshot
  that becomes a message through the executor and proxy, and that message queues behind
  pending window events. Delivering it straight to the event loop would save another
  refresh.
- **Keep drawing the previous image while a new one uploads**, as an option on an image
  or canvas image, so applications need not track allocations to avoid blank frames.
- **Make the synchronous upload limit configurable**, or offer a synchronous path for
  already-decoded RGBA data that changes every frame.
- **Reuse textures for repeated uploads.** The worker creates a new atlas
  (`Atlas::with_size`) for each large image. An updatable texture for images that
  change every frame would avoid the allocation and the new id per frame.
