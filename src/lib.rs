//! # http-content-range
//!
//! HTTP Content-Range response header parser.
//! Inspired by https://github.com/bancek/rust-http-range library.

use crate::utils::{fail_if, is_whitespace, IterExt};

mod utils;

static PREFIX: &[u8] = b"bytes";
const PREFIX_LEN: usize = 5;

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
    /// See https://httpwg.org/specs/rfc7233.html#rfc.section.4.2
    pub fn parse(header: &str) -> ContentRange {
        Self::parse_bytes(header.as_bytes())
    }

    /// Same as [parse] but parses directly from the byte array
    pub fn parse_bytes(header: &[u8]) -> ContentRange {
        Self::parse_opt(header).unwrap_or(ContentRange::Unknown)
    }

    /// Internal implementation of parsing, easier to return Option midway with `?`.
    /// From https://httpwg.org/specs/rfc7233.html#rfc.section.4.2
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

        let mut iter = header[PREFIX_LEN..].iter().peekable();

        // must start with a space
        fail_if(!is_whitespace(*iter.next()?))?;
        let res = if iter.skip_spaces()? == b'*' {
            // Unsatisfied range
            iter.next()?; // consume '*'
            iter.parse_separator(b'/')?;
            ContentRange::Unsatisfied {
                0: ContentRangeUnsatisfied {
                    complete_length: iter.parse_u64()?,
                },
            }
        } else {
            // byte range
            let first_byte = iter.parse_u64()?;
            iter.parse_separator(b'-')?;
            let last_byte = iter.parse_u64()?;
            fail_if(first_byte > last_byte)?;
            if iter.parse_separator(b'/')? == b'*' {
                // unbound byte range, consume '*'
                iter.next()?;
                ContentRange::UnboundBytes {
                    0: ContentRangeUnbound {
                        first_byte,
                        last_byte,
                    },
                }
            } else {
                let complete_length = iter.parse_u64()?;
                fail_if(last_byte >= complete_length)?;
                ContentRange::Bytes {
                    0: ContentRangeBytes {
                        first_byte,
                        last_byte,
                        complete_length,
                    },
                }
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

    struct T(&'static str, ContentRange);

    fn new_bytes(first_byte: u64, last_byte: u64, complete_length: u64) -> ContentRange {
        ContentRange::Bytes {
            0: ContentRangeBytes {
                first_byte,
                last_byte,
                complete_length,
            },
        }
    }

    fn new_unbound(first_byte: u64, last_byte: u64) -> ContentRange {
        ContentRange::UnboundBytes {
            0: ContentRangeUnbound {
                first_byte,
                last_byte,
            },
        }
    }

    fn new_unsatisfied(complete_length: u64) -> ContentRange {
        ContentRange::Unsatisfied {
            0: ContentRangeUnsatisfied { complete_length },
        }
    }

    #[test]
    fn test_parse() {
        let tests = vec![
            // Valid
            T("bytes 0-9/20", new_bytes(0, 9, 20)),
            T("bytes\t 0 \t -\t \t  \t9 / 20   ", new_bytes(0, 9, 20)),
            T("bytes */20", new_unsatisfied(20)),
            T("bytes   *\t\t/  20    ", new_unsatisfied(20)),
            T("bytes 0-9/*", new_unbound(0, 9)),
            T("bytes   0  -    9  /  *   ", new_unbound(0, 9)),
            //
            // Errors
            //
            T("", ContentRange::Unknown),
            T("b", ContentRange::Unknown),
            T("foo", ContentRange::Unknown),
            T("foo 1-2/3", ContentRange::Unknown),
            T(" bytes 1-2/3", ContentRange::Unknown),
            T("bytes -2/3", ContentRange::Unknown),
            T("bytes 1-/3", ContentRange::Unknown),
            T("bytes 1-2/", ContentRange::Unknown),
            T("bytes 1-2/a", ContentRange::Unknown),
            T("bytes1-2/3", ContentRange::Unknown),
            T("bytes=1-2/3", ContentRange::Unknown),
            T("bytes a-2/3", ContentRange::Unknown),
            T("bytes 1-a/3", ContentRange::Unknown),
            T("bytes 0x01-0x02/3", ContentRange::Unknown),
            T("bytes 1-2/a", ContentRange::Unknown),
            T(
                "bytes 1111111111111111111111111111111111111111111-2/1",
                ContentRange::Unknown,
            ),
            T("bytes 1-3/20 1", ContentRange::Unknown),
            T("bytes 1-3/* 1", ContentRange::Unknown),
            T("bytes */1 1", ContentRange::Unknown),
            T("bytes 1-0/20", ContentRange::Unknown),
            T("bytes 1-20/20", ContentRange::Unknown),
            T("bytes 1-21/20", ContentRange::Unknown),
        ];

        for t in tests {
            let header = t.0;
            let expected = t.1;

            let res = ContentRange::parse(header);

            match expected {
                ContentRange::Bytes(expected) => {
                    if let ContentRange::Bytes(res) = res {
                        assert_eq!(
                            res, expected,
                            "parseContentRange(\"{header}\") = {:?}, want {:?}",
                            res, expected
                        );
                    } else {
                        panic!(
                            "parseContentRange(\"{header}\") = {:?}, want {:?}",
                            res, expected
                        );
                    }
                }
                ContentRange::UnboundBytes(expected) => {
                    if let ContentRange::UnboundBytes(res) = res {
                        assert_eq!(
                            res, expected,
                            "parseContentRange(\"{header}\") = {:?}, want {:?}",
                            res, expected
                        );
                    } else {
                        panic!(
                            "parseContentRange(\"{header}\") = {:?}, want {:?}",
                            res, expected
                        );
                    }
                }
                ContentRange::Unsatisfied(expected) => {
                    if let ContentRange::Unsatisfied(res) = res {
                        assert_eq!(
                            res, expected,
                            "parseContentRange(\"{header}\") = {:?}, want {:?}",
                            res, expected
                        );
                    } else {
                        panic!(
                            "parseContentRange(\"{header}\") = {:?}, want {:?}",
                            res, expected
                        );
                    }
                }
                ContentRange::Unknown => {
                    assert_eq!(
                        res, expected,
                        "parseContentRange(\"{header}\") = {:?}, want {:?}",
                        res, expected
                    );
                }
            }
        }
    }
}
