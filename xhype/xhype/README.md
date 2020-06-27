## Build

``` 
cargo build
```

## View docs

``` 
cargo doc && python3 -m http.server --directory target/doc
```

and then open `http://0.0.0.0:8000/xhype/` in the browser

## Unit tests

``` 
cargo test -- --test-threads=1
```

## Standalone tests

``` 
KN_PATH=<vmlinuz-path> RD_PATH=<initrd.gz-path> CMD_Line=<kernel-cmd-line> RUST_LOG=warn cargo run
```

the following output is expected:

``` 
initially, a = 4, b = 2
good
a = 2, b = 100
```

Starting a Rust closure as a virtual thread require nightly Rust. With nightly rust, using the following command to test closure as a virtual thread:

``` 
KN_PATH=<vmlinuz-path> RD_PATH=<initrd.gz-path> CMD_Line=<kernel-cmd-line> RUST_LOG=warn cargo run --features "vthread_closure"
```

the following output is expected:

``` 
initially, a = 4, b = 2
good
a = 3, b = 101
```
