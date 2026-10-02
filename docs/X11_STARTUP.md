# X11 startup crash on the development machine

The user's ordinary `cargo run --release --locked -p tack-app` crashed on
2026-10-02 with SIGSEGV before GPU initialization. The system core (PID 49076)
shows `libXi.so.6+0x384b`, `_XEnq`, `_XReply`, `XIQueryDevice`, and winit's
initial device enumeration. Installed libXi is 1.8.1; libX11 is 1.8.7.

Disassembly and core memory identify an XI_RawMotion event being converted during
`_XiGetExtensionVersionRequest`. libXi 1.8.1's `_XiCheckExtInit` allocates
`XInputData` with `Xmalloc`, then requests the extension version. While the
request waits, raw-motion conversion calls `_XiCheckVersion` and reads the
not-yet-assigned `vers` pointer. The core contains the invalid pointer
`0x40b1940000000000`. This establishes a startup reentrancy failure, outside
Tack's decoder and GPU code. Twelve quiet relaunches succeeded; they do not
establish whether the raw-motion trigger was exercised.

The official [upstream fix](https://gitlab.freedesktop.org/xorg/lib/libxi/-/commit/c4cbdcf747ebac89520903458e8242b0b35ada1d)
replaces that allocation with `Xcalloc`. During reentrancy `vers` is then null,
which `_XiCheckVersion` handles, instead of an arbitrary pointer. The fix is
included in [libXi 1.8.3](https://www.x.org/releases/individual/lib/libXi-1.8.3.tar.xz);
the inspected 1.8.2 source still uses `Xmalloc` and is insufficient.

## App-local correction

Run `bash tools/prepare_linux.sh` once on affected Linux hosts. It builds the
unmodified upstream source under `target/native/libXi-1.8.3`, retains its license
notices, and verifies source SHA256
`7ad60056f01af4f786cfe93b3a7707447711626fc8da2637bec71a90409babe5`.
No root privileges, system package replacement, startup download or Cargo-time
C compilation is required. The download/build is an explicit preparation step.

`tack-app/build.rs` adds a Linux-only runtime search path for that local directory.
Ordinary `cargo run` and direct execution then load the corrected library when
available. Without preparation, the dynamic loader falls back to the system
library. Native distribution packaging must provide a fixed system library or
bundle the local library and its notices; this development helper is not a final
application package. Removing `target/` removes the optional local build.

The earlier Mission 0 measurements precede this startup correction and keep their
original source/binary hashes. They do not demonstrate resistance to startup
raw-motion traffic.

## Validation on 2026-10-02

The loader trace confirms that Tack uses the project-local libXi. Independent
inspection of the installed binary confirms zero-initialization before the
extension version request. No system packages were modified.

Twelve corrected launches with synthetic mouse movement completed successfully
on the NVIDIA GeForce RTX 2060. The exact interactive command
`cargo run --release --locked -p tack-app` also initialized the GPU and created a
visible 1280 × 720 X11 window; it remained running after five seconds and was
then terminated deliberately for test cleanup. Workspace formatting, check,
Clippy, nine Rust tests, documentation, two Python tests, and cargo-deny passed.
Upstream duplicate-version warnings remain under the existing warning policy.

The old binary also passed twelve quiet launches and six launches with synthetic
mouse movement. Those inputs did not reproduce the original crash reliably;
successful launches alone do not establish that the recorded RawMotion trigger
was exercised. The diagnosis rests on the user's core and the matching upstream
fix. [Validation artifact](benchmarks/x11-startup-check.json) records both sets
of runs, the installed library hash and the interactive check. This check is
separate from the earlier Mission 0 performance measurements.
