/*
pub mod protocol {
    tonic::include_proto!("protocol");
}
*/

pub mod cgo;
pub mod consumer;
pub mod flow;
pub mod materialize;
pub mod protocol;
pub mod recoverylog;

mod read;

use serde::{Deserializer, Serializer};

/// This exists to enable conditional serialization of optional u32 fields where 0 represents a
/// missing or unset value. See `build.rs` for references to this function in the serde attributes.
fn u32_is_0(i: &u32) -> bool {
    *i == 0
}

pub fn deserialize_duration<'a, D>(d: D) -> Result<Option<prost_types::Duration>, D::Error>
where
    D: Deserializer<'a>,
{
    let dur: Option<std::time::Duration> = humantime_serde::deserialize(d)?;
    Ok(dur.map(Into::into))
}
// serialize_duration is lossy: it only serializes whole numbers of positive seconds.
// If the Duration is negative, None is returned. If the Duration includes nanos,
// they're dropped. This is because:
// * std::time::Duration only represents positive durations, and indeed all durations
//   within Gazette & Flow are never negative.
// * The protobuf mapping of prost_type::Duration to JSON requires that they be
//   fractional seconds, with an "s" suffix. Meanwhile, humantime parses only
//   integer time segments (including "s"). Therefor, we use integer seconds as
//   a lowest common denominator.
pub fn serialize_duration<S>(dur: &Option<prost_types::Duration>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match dur {
        Some(prost_types::Duration { seconds, .. }) if *seconds >= 0 => {
            s.serialize_str(&format!("{}s", seconds))
        }
        _ => s.serialize_none(),
    }
}

/// Message UUID flags defined by Gazette, and used by Flow.
/// C.f. Gazette's `message` package, where these are originally defined.
pub mod message_flags {
    /// MASK is the low 10 bits of UuidParts::producer_and_flags.
    /// It's the bit of a Gazette message UUID which are used to carry flag values.
    pub const MASK: u64 = 0x3ff;
    /// OUTSIDE_TXN marks the message is immediately commit.
    pub const OUTSIDE_TXN: u64 = 0x0;
    /// CONTINUE_TXN marks the message as transactional, such that it must
    /// be committed by a future ACK_TXN before it may be processed.
    pub const CONTINUE_TXN: u64 = 0x1;
    /// ACK_TXN marks the message as an acknowledgement of a committed transaction.
    /// On reading a ACK, the reader may process previous CONTINUE_TXN messages
    /// which are now considered to have committed.
    pub const ACK_TXN: u64 = 0x2;
}

pub mod collection {
    use crate::flow::{CollectionSpec, Projection};
    pub trait CollectionExt {
        fn get_projection(&self, field: impl AsRef<str>) -> Option<&Projection>;
    }

    impl CollectionExt for CollectionSpec {
        fn get_projection(&self, field: impl AsRef<str>) -> Option<&Projection> {
            let field = field.as_ref();
            self.projections.iter().find(|p| p.field == field)
        }
    }
}

pub mod arena {
    use crate::flow::Slice;
    use std::borrow::Borrow;
    use std::io;

    /// Extension trait with helper functions for working with arenas.
    pub trait ArenaExt {
        fn is_valid(&self, slice: Slice) -> bool;

        fn bytes(&self, slice: Slice) -> &[u8];

        fn add_bytes<B: AsRef<[u8]>>(&mut self, bytes: &B) -> Slice;

        fn writer(&mut self) -> ArenaWriter;
    }

    impl ArenaExt for Vec<u8> {
        fn is_valid(&self, slice: Slice) -> bool {
            let slice = slice.borrow();
            slice.begin <= slice.end && slice.end as usize <= self.len()
        }
        fn bytes(&self, slice: Slice) -> &[u8] {
            let slice = slice.borrow();
            &self[slice.begin as usize..slice.end as usize]
        }

        fn add_bytes<B: AsRef<[u8]>>(&mut self, bytes: &B) -> Slice {
            let src = bytes.as_ref();
            let start = self.len();
            self.extend_from_slice(src);
            let end = self.len();
            Slice {
                begin: start as u32,
                end: end as u32,
            }
        }

        fn writer<'a>(&'a mut self) -> ArenaWriter<'a> {
            let start = self.len();
            ArenaWriter { arena: self, start }
        }
    }

    pub struct ArenaWriter<'a> {
        arena: &'a mut Vec<u8>,
        start: usize,
    }
    impl<'a> ArenaWriter<'a> {
        pub fn slice(&self) -> Slice {
            Slice {
                begin: self.start as u32,
                end: self.arena.len() as u32,
            }
        }
        pub fn finish(self) -> Slice {
            self.slice()
        }
    }

    impl<'a> io::Write for ArenaWriter<'a> {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.arena.extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use flow::{inference, CollectionSpec, Inference, Projection};

    #[test]
    fn test_serde_round_trip_of_collection_spec() {
        fn s(i: &str) -> String {
            String::from(i)
        }

        let expected = CollectionSpec {
            name: String::from("testCollection"),
            schema_uri: String::from("test://test/schema.json"),
            key_ptrs: vec![String::from("/a"), String::from("/b")],
            uuid_ptr: s(""),
            partition_fields: Vec::new(),
            ack_json_template: Vec::new(),
            journal_spec: None,
            projections: vec![
                Projection {
                    ptr: s("/a"),
                    field: s("field_a"),
                    user_provided: true,
                    is_partition_key: false,
                    is_primary_key: true,
                    inference: Some(Inference {
                        title: s("the title from a"),
                        description: s(""),
                        types: vec![s("string")],
                        must_exist: true,
                        string: Some(inference::String {
                            content_type: s(""),
                            format: s("email"),
                            is_base64: false,
                            max_length: 0,
                        }),
                    }),
                },
                Projection {
                    ptr: s("/b"),
                    field: s("b"),
                    user_provided: false,
                    is_partition_key: false,
                    is_primary_key: true,
                    inference: Some(Inference {
                        title: s(""),
                        description: s("the description from b"),
                        types: vec![s("integer")],
                        must_exist: true,
                        string: None,
                    }),
                },
            ],
        };

        let serialized = serde_json::to_string_pretty(&expected).unwrap();
        insta::assert_snapshot!(serialized);
        let actual = serde_json::from_str::<CollectionSpec>(&serialized).unwrap();
        assert_eq!(expected, actual);
    }
}
