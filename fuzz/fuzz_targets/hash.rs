#![no_main]
use libfuzzer_sys::fuzz_target;
use ate_crypto;

fuzz_target!(|input: String| {
    let hash = ate_crypto::AteHash::from(input);
    hash.as_bytes();
});