This independent **experiment** is excluded from the Tack workspace. It does not
implement GIF support in Tack. Production `Cargo.lock` is unchanged.

Use the existing shared build directory to compare the stripped executables:

```sh
CARGO_TARGET_DIR=/home/gurp/softs/tack/target cargo build --release --locked --manifest-path tools/p0_image/gif_experiment/Cargo.toml
CARGO_TARGET_DIR=/home/gurp/softs/tack/target cargo build --release --locked --features gif-preview --manifest-path tools/p0_image/gif_experiment/Cargo.toml
```

Copy each result before the next build; never replace `bin/tack`. PNG is the
dependency control. GIF mode consumes only the first image through
`ImageDecoder::read_image`. Pass an owned fixture and optional poster PNG path.
Hash, logical canvas, elapsed time and cumulative read bytes are printed.

**Known experimental limitation:** `image::GifDecoder::new` consumes GIF metadata
before `set_limits`, using the underlying decoder defaults. The image buffer
limit does not cap constructor metadata. This wrapper cannot ship unchanged.
No cancellation, deadline, production cache identity, or integrated
Linked/Embedded admission contract is established here.
