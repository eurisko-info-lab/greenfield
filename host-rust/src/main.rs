use std::{fs, path::Path};
use stratum_rust::{fixture_json, hex_encode, parse_fixture_text, digest_bytes, encode_value};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        eprintln!("usage: stratum-rust fixture <fixture-file>");
        std::process::exit(1);
    }

    let path = Path::new(&args[2]);
    let text = fs::read_to_string(path).unwrap_or_else(|err| {
        eprintln!("failed to read fixture: {err}");
        std::process::exit(1);
    });
    let fixture_id = path.file_stem().unwrap_or_default().to_string_lossy();
    let value = parse_fixture_text(&text).unwrap_or_else(|| {
        eprintln!("unsupported fixture format: {text}");
        std::process::exit(1);
    });
    let bytes = encode_value(&value).unwrap();
    let digest = digest_bytes(&bytes);
    let outcome_digest = digest_bytes(&hex_encode(&digest).into_bytes());
    println!(
        "{{\"fixture_id\":\"{fixture_id}\",\"encoded_hex\":\"{}\",\"digest_hex\":\"{}\",\"outcome_digest\":\"{}\",\"steps\":1,\"receipt_digests\":[]}}",
        hex_encode(&bytes),
        hex_encode(&digest),
        hex_encode(&outcome_digest)
    );
    let _ = fixture_json;
}
