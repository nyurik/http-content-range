# http-content-range
[![Build](https://github.com/nyurik/http-content-range/actions/workflows/ci.yaml/badge.svg)](https://github.com/nyurik/http-content-range/actions/workflows/ci.yaml)
[![Cr   ates.io](https://img.shields.io/crates/v/http-content-range.svg)](https://crates.io/crates/http-content-range)
[![Documentation](https://docs.rs/http-content-range/badge.svg)](https://docs.rs/http-content-range)

Tiny Rust lib to decode Content-Range response headers.

```rust
extern crate http_content_range;

use http_content_range::ContentRange;

fn main() {
    let content_range_str = "bytes 42-69/420";

    match ContentRange::parse(content_range_str) {
        ContentRange::Bytes(r) => {
            println!(
                "First_byte={}, last_byte={}, complete_length={}",
                r.first_byte, r.last_byte, r.complete_length,
            )
        }
        ContentRange::UnboundBytes(r) => {
            println!(
                "First_byte={}, last_byte={}, complete_length is unknown",
                r.first_byte, r.last_byte
            )
        }
        ContentRange::Unsatisfied(r) => {
            println!(
                "Unsatisfied response, complete_length={}, ",
                r.complete_length
            )
        }
        ContentRange::Unknown => {
            println!("Unable to parse")
        }
    };
}
```
