#![no_main]
use libfuzzer_sys::fuzz_target;
use ate_crypto;

fuzz_target!(|input: String| {
    let hash = ate_crypto::ShortHash::from(input);
    hash.to_bytes();
});