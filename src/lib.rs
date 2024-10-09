#![doc = include_str!("../README.md")]

use crate::utils::{fail_if, is_whitespace, IterExt};

mod utils;

const PREFIX: &[u8] = b"bytes";

/// HTTP Content-Range response header representation.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ContentRange {
    /// Regular bytes range response with status 206
    Bytes(ContentRangeBytes),
    /// Regular bytes range response with status 206
    UnboundBytes(ContentRangeUnbound),
    /// Server response with status 416
    Unsatisfied(ContentRangeUnsatisfied),
    /// Header cannot be parsed. This includes non-standard response with status 206
    Unknown,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ContentRangeBytes {
    pub first_byte: u64,
    pub last_byte: u64,
    pub complete_length: u64,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ContentRangeUnbound {
    pub first_byte: u64,
    pub last_byte: u64,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct ContentRangeUnsatisfied {
    pub complete_length: u64,
}

impl ContentRange {
    /// Parses Content-Range HTTP header string as per
    /// [RFC 7233](https://httpwg.org/specs/rfc7233.html#header.content-range).
    ///
    /// `header` is the HTTP Content-Range header (e.g. `bytes 0-9/30`).
    ///
    /// This parser is a bit more lenient than the official RFC, it allows spaces and tabs between everything.
    /// See <https://httpwg.org/specs/rfc7233.html#rfc.section.4.2>
    ///
    /// ```
    /// use http_content_range::{ContentRange, ContentRangeBytes, ContentRangeUnbound, ContentRangeUnsatisfied};
    /// assert_eq!(ContentRange::parse("bytes 42-69/420"),
    ///     ContentRange::Bytes(ContentRangeBytes{first_byte: 42, last_byte: 69, complete_length: 420}));
    ///
    /// // complete_length is unknown
    /// assert_eq!(ContentRange::parse("bytes 42-69/*"),
    ///    ContentRange::UnboundBytes(ContentRangeUnbound{first_byte: 42, last_byte: 69}));
    ///
    /// // response is unsatisfied
    /// assert_eq!(ContentRange::parse("bytes */420"),
    ///   ContentRange::Unsatisfied(ContentRangeUnsatisfied{complete_length: 420}));
    /// ```
    #[must_use]
    pub fn parse(header: &str) -> ContentRange {
        Self::parse_bytes(header.as_bytes())
    }

    /// Same as [`parse`](Self::parse) but parses directly from the byte array
    #[must_use]
    pub fn parse_bytes(header: &[u8]) -> ContentRange {
        Self::parse_opt(header).unwrap_or(ContentRange::Unknown)
    }

    /// Internal implementation of parsing, easier to return Option midway with `?`.
    /// From <https://httpwg.org/specs/rfc7233.html#rfc.section.4.2>
    /// Valid bytes responses:
    ///   Content-Range: bytes 42-1233/1234
    ///   Content-Range: bytes 42-1233/*
    ///   Content-Range: bytes */1233
    ///
    /// ```none
    ///   Content-Range       = byte-content-range
    ///                       / other-content-range
    ///
    ///   byte-content-range  = bytes-unit SP
    ///                         ( byte-range-resp / unsatisfied-range )
    ///
    ///   byte-range-resp     = byte-range "/" ( complete-length / "*" )
    ///   byte-range          = first-byte-pos "-" last-byte-pos
    ///   unsatisfied-range   = "*/" complete-length
    ///
    ///   complete-length     = 1*DIGIT
    ///
    ///   other-content-range = other-range-unit SP other-range-resp
    ///   other-range-resp    = *CHAR
    /// ```
    fn parse_opt(header: &[u8]) -> Option<ContentRange> {
        if !header.starts_with(PREFIX) {
            return None;
        }

        let mut iter = header[PREFIX.len()..].iter().peekable();

        // must start with a space
        fail_if(!is_whitespace(*iter.next()?))?;
        let res = if iter.skip_spaces()? == b'*' {
            // Unsatisfied range
            iter.next()?; // consume '*'
            iter.parse_separator(b'/')?;
            ContentRange::Unsatisfied(ContentRangeUnsatisfied {
                complete_length: iter.parse_u64()?,
            })
        } else {
            // byte range
            let first_byte = iter.parse_u64()?;
            iter.parse_separator(b'-')?;
            let last_byte = iter.parse_u64()?;
            fail_if(first_byte > last_byte)?;
            if iter.parse_separator(b'/')? == b'*' {
                // unbound byte range, consume '*'
                iter.next()?;
                ContentRange::UnboundBytes(ContentRangeUnbound {
                    first_byte,
                    last_byte,
                })
            } else {
                let complete_length = iter.parse_u64()?;
                fail_if(last_byte >= complete_length)?;
                ContentRange::Bytes(ContentRangeBytes {
                    first_byte,
                    last_byte,
                    complete_length,
                })
            }
        };

        // verify there is nothing left
        match iter.skip_spaces() {
            None => Some(res),
            Some(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn new_bytes(first_byte: u64, last_byte: u64, complete_length: u64) -> ContentRange {
        ContentRange::Bytes(ContentRangeBytes {
            first_byte,
            last_byte,
            complete_length,
        })
    }

    fn new_unbound(first_byte: u64, last_byte: u64) -> ContentRange {
        ContentRange::UnboundBytes(ContentRangeUnbound {
            first_byte,
            last_byte,
        })
    }

    fn new_unsatisfied(complete_length: u64) -> ContentRange {
        ContentRange::Unsatisfied(ContentRangeUnsatisfied { complete_length })
    }

    #[test]
    fn test_parse() {
        let tests = vec![
            // Valid
            ("bytes 0-9/20", new_bytes(0, 9, 20)),
            ("bytes\t 0 \t -\t \t  \t9 / 20   ", new_bytes(0, 9, 20)),
            ("bytes */20", new_unsatisfied(20)),
            ("bytes   *\t\t/  20    ", new_unsatisfied(20)),
            ("bytes 0-9/*", new_unbound(0, 9)),
            ("bytes   0  -    9  /  *   ", new_unbound(0, 9)),
            //
            // Errors
            //
            ("", ContentRange::Unknown),
            ("b", ContentRange::Unknown),
            ("foo", ContentRange::Unknown),
            ("foo 1-2/3", ContentRange::Unknown),
            (" bytes 1-2/3", ContentRange::Unknown),
            ("bytes -2/3", ContentRange::Unknown),
            ("bytes 1-/3", ContentRange::Unknown),
            ("bytes 1-2/", ContentRange::Unknown),
            ("bytes 1-2/a", ContentRange::Unknown),
            ("bytes1-2/3", ContentRange::Unknown),
            ("bytes=1-2/3", ContentRange::Unknown),
            ("bytes a-2/3", ContentRange::Unknown),
            ("bytes 1-a/3", ContentRange::Unknown),
            ("bytes 0x01-0x02/3", ContentRange::Unknown),
            ("bytes 1-2/a", ContentRange::Unknown),
            (
                "bytes 1111111111111111111111111111111111111111111-2/1",
                ContentRange::Unknown,
            ),
            ("bytes 1-3/20 1", ContentRange::Unknown),
            ("bytes 1-3/* 1", ContentRange::Unknown),
            ("bytes */1 1", ContentRange::Unknown),
            ("bytes 1-0/20", ContentRange::Unknown),
            ("bytes 1-20/20", ContentRange::Unknown),
            ("bytes 1-21/20", ContentRange::Unknown),
        ];

        for (header, expected) in tests {
            let res = ContentRange::parse(header);

            match expected {
                ContentRange::Bytes(expected) => {
                    if let ContentRange::Bytes(res) = res {
                        assert_eq!(
                            res, expected,
                            "parseContentRange(\"{header}\") = {res:?}, want {expected:?}"
                        );
                    } else {
                        panic!("parseContentRange(\"{header}\") = {res:?}, want {expected:?}");
                    }
                }
                ContentRange::UnboundBytes(expected) => {
                    if let ContentRange::UnboundBytes(res) = res {
                        assert_eq!(
                            res, expected,
                            "parseContentRange(\"{header}\") = {res:?}, want {expected:?}"
                        );
                    } else {
                        panic!("parseContentRange(\"{header}\") = {res:?}, want {expected:?}");
                    }
                }
                ContentRange::Unsatisfied(expected) => {
                    if let ContentRange::Unsatisfied(res) = res {
                        assert_eq!(
                            res, expected,
                            "parseContentRange(\"{header}\") = {res:?}, want {expected:?}"
                        );
                    } else {
                        panic!("parseContentRange(\"{header}\") = {res:?}, want {expected:?}");
                    }
                }
                ContentRange::Unknown => {
                    assert_eq!(
                        res, expected,
                        "parseContentRange(\"{header}\") = {res:?}, want {expected:?}"
                    );
                }
            }
        }
    }
}
