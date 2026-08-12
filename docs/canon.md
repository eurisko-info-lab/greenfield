# Canonical value encoding

## Scope

This document defines the fixed, self-describing binary encoding for the frozen Machine value algebra.

## Value constructors

The canonical `Value` algebra is:

- Unit
- Bool(bool)
- Nat(u64)
- Bytes(byte[])
- Text(UTF-8)
- Sum(tag, payload)
- Product([Value])
- Sequence([Value])
- FiniteMap([(Value, Value)])
- Digest(32 bytes)
- Ref { digest, type_digest }

## Tags

| Tag | Constructor |
| --- | --- |
| 0x00 | Unit |
| 0x01 | Bool |
| 0x02 | Nat |
| 0x03 | Bytes |
| 0x04 | Text |
| 0x05 | Sum |
| 0x06 | Product |
| 0x07 | Sequence |
| 0x08 | FiniteMap |
| 0x09 | Digest |
| 0x0A | Ref |

## Canonical unsigned varint

Unsigned integers use a single canonical varint form:

- 7 bits per byte, little-endian order.
- The high bit indicates continuation.
- The shortest possible byte sequence is required.
- The value `0` encodes as `[0x00]`.
- Encodings with extra zero continuation bytes are rejected as `NonCanonicalVarint`.

This is enforced both when decoding and when validating encoded map keys.

## Constructor rules

- `Bool` encodes as `tag` then `0x00` or `0x01`.
- `Nat` encodes as `tag` then canonical uint bytes.
- `Bytes` and `Text` encode a length prefix followed by the payload bytes.
- `Text` must be valid UTF-8.
- `Sum` encodes `tag` then the payload encoding.
- `Product` and `Sequence` encode length then each element in order.
- `FiniteMap` encodes length then each key/value pair, with keys sorted by their canonical encoded byte form. Duplicate canonical keys are rejected.
- `Digest` is always 32 bytes.
- `Ref` encodes content digest and type digest, each 32 bytes.
- Trailing bytes after the complete value are rejected.
- Decoding enforces a depth limit of 64.

## Digest

`digest(Bytes) = SHA-256(exact canonical bytes)`.

## Observational contract

The same value must produce identical bytes and digests in both Rust and Scala hosts. The canonical encoding is the interoperability boundary.
