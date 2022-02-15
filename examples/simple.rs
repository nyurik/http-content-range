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
