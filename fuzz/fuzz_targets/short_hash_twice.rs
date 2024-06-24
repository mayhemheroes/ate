#![no_main]
use libfuzzer_sys::fuzz_target;
use ate_crypto;

fuzz_target!(|input: (&[u8], &[u8])| {
    let hash = ate_crypto::ShortHash::from_bytes_twice(input.0, input.1);
    hash.to_bytes();
});