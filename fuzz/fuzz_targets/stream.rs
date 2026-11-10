#![no_main]
libfuzzer_sys::fuzz_target!(|data: &[u8]| imece_fuzz::stream::check(data));
