# librsvg-rebind

The *librsvg-rebind* crates allow to use *librsvg*'s C-API from rust. Since *librsvg* is written in Rust, a [native Rust API](https://crates.io/crates/librsvg) does exist. However, statically linking the library into a binary might not be desired in all cases. In these cases, *librsvg* can be linked dynamically and can reduce the Rust binary size by about 5 MB.