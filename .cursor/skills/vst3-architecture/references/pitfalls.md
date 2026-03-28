# VST3/nih-plug Common Pitfalls

These are the mistakes that cause the most pain — ranging from silent DAW incompatibility to hard
crashes. Read this file before shipping anything.

---

## 1. Changing Parameter IDs After Shipping

**What happens:** Users open old projects and find all their automation and presets broken.

**How it happens:** Renaming the string in `#[id = "..."]`, reordering params without explicit IDs,
or removing a param and adding another in its place.

**Fix:** Freeze your `#[id]` strings the moment you ship v1. If you need to remove a param, keep
the `#[id]` present (even if hidden) as a dead entry, or leave a gap in a numeric scheme. Document
your ID history in a comment.

---

## 2. Allocating Memory in `process()`

**What happens:** Audio dropouts, priority inversion, occasional hard lockups.

**How it happens:** `Vec::push()`, `String::new()`, `Box::new()`, or any API that might call the
allocator inside the hot path.

**Fix:** Allocate everything in `initialize()`. Use `Vec::with_capacity()` and never grow in
`process()`. For cross-thread communication, use lock-free channels (crossbeam's `bounded(N)` with
pre-allocated slots) or atomics.

---

## 3. Locking a Mutex in `process()`

**What happens:** If any other thread holds the lock (e.g., the GUI updating state), the audio
thread blocks. Causes dropouts and can cause priority inversion under the OS scheduler.

**Fix:** Never use `Mutex` or `RwLock` in `process()`. Share state between threads using atomics
(`AtomicF32`, `AtomicBool`) or lock-free structures. For larger data, use a `triple_buffer` or
crossbeam channel with `try_recv()`/`try_send()`.

---

## 4. Silently Changing Latency at Runtime

**What happens:** The DAW's delay compensation is calculated at plugin initialization. If your
reported latency changes mid-session (e.g., user switches oversampling factor) without notifying
the host, everything downstream is out of time.

**Fix:** Call `context.set_latency_samples(new_latency)` whenever your latency changes. nih-plug
forwards this to the host, which will recalculate delay compensation. Users will hear a brief gap
or glitch as the DAW rebuilds the graph — that's expected and acceptable.

---

## 5. Assuming a Fixed Buffer Size

**What happens:** Works fine in your DAW at 512-sample blocks, crashes in another host at 2048, or
in offline render at 65536.

**How it happens:** Allocating internal delay lines or lookahead buffers sized to `512` instead of
`buffer_config.max_buffer_size`.

**Fix:** In `initialize()`, use `buffer_config.max_buffer_size` for all buffer allocations. Also
check `buffer_config.sample_rate` for anything time-based (delay lines in samples = ms * sr / 1000).

---

## 6. Only Declaring Stereo I/O

**What happens:** Some hosts (Logic, certain Reaper templates) want to instantiate a mono version
of a plugin. If you only declare stereo, the plugin may be silently skipped or the user gets confused.

**Fix:** Always declare at least a mono and stereo variant in `AUDIO_IO_LAYOUTS`. The stereo entry
should come first (it's the default). If your algorithm is inherently stereo (mid-side processing,
etc.), a mono entry isn't meaningful — but for most effects, mono compatibility is worth the few
extra lines.

---

## 7. Not Reading the Transport for Sync

**What happens:** A delay plugin that's supposed to sync to the host tempo just runs at a fixed
rate regardless of DAW BPM.

**Fix:**
```rust
if let Some(transport) = context.transport() {
    if let Some(tempo) = transport.tempo {
        let beat_duration_samples = (60.0 / tempo) * buffer_config.sample_rate as f64;
        // Use beat_duration_samples for delay time, LFO rate, etc.
    }
}
```

Also check `transport.playing` — don't advance LFO phase or consume lookahead when the transport
is stopped.

---

## 8. Shipping a Debug Build

**What happens:** Plugin runs 10–100x slower than it should. DAW appears laggy; users complain.

**Fix:** Always `cargo xtask bundle <name> --release`. The `--release` flag enables optimizations.
Debug builds are for development only.

---

## 9. Missing the `xtask` Setup

**What happens:** `cargo xtask bundle` fails with "no such subcommand: xtask".

**What's needed:**
1. A `xtask/` crate with `nih_plug_xtask::main()` in its `main.rs`
2. `xtask` listed in the workspace `[members]` in root `Cargo.toml`
3. `.cargo/config.toml` with `[alias] xtask = "run --package xtask --"`

Missing any one of these causes the command to fail in a confusing way.

---

## 10. Wrong Bundle Location on macOS

**What happens:** Plugin doesn't appear in the DAW's plugin list.

**Fix:** The `.vst3` bundle must be in:
- `~/Library/Audio/Plug-Ins/VST3/` (per-user)
- `/Library/Audio/Plug-Ins/VST3/` (system-wide)

`cargo xtask bundle` places the output in `target/bundled/`. You need to copy or symlink it:
```bash
cp -r target/bundled/MyPlugin.vst3 ~/Library/Audio/Plug-Ins/VST3/
```

Or for development, create a symlink so you don't need to copy after every build:
```bash
ln -s $(pwd)/target/bundled/MyPlugin.vst3 ~/Library/Audio/Plug-Ins/VST3/
```

---

## 11. Parameter Smoothing Causes Lag in Modulation

**What happens:** An LFO mapped to a filter cutoff sounds laggy or doesn't sweep fast enough.

**How it happens:** Setting smoothing to 100ms or more on a parameter meant to be modulated.

**Fix:** Use shorter smoothing for modulation targets (5–20ms). For discrete switches (filter type,
bypass), use `SmoothingStyle::None` — a parameter that snaps is correct; a bypass that fades 50ms
is annoying.

---

## 12. Forgetting `#[derive(Default)]` on Plugin Struct

**What happens:** Compilation error, or worse — uninitialized state at startup.

**Fix:** Implement or derive `Default` for your `Plugin` struct. In `Default::default()`, set up
any internal state (filter coefficients, delay lines) to a safe initial value — but do expensive
initialization (sample-rate-dependent values) in `initialize()`, not `default()`.
