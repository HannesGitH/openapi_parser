RUSTFLAGS="-Zfmt-debug=none" cargo +nightly build \
  -Z build-std=std \
  -Z build-std-features="optimize_for_size" \
  --target aarch64-apple-darwin --release