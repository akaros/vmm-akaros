## Build

``` 
cargo build
```

## Test

``` 
cargo test -- --test-threads=1
```

## Run Linux guest

``` 
cargo build && KN_PATH=<vmlinuz-path> RD_PATH=<initrd.gz-path> RUST_LOG=info target/debug/vthread-rs
```
