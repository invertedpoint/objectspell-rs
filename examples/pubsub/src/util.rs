pub fn rand_index(max: usize) -> usize {
    use std::time::SystemTime;
    let mut nanos = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .subsec_nanos() as usize;

    // Mix the bits to eliminate repeating cycles from absolute 100ms sleeps
    nanos ^= nanos >> 16;
    nanos = nanos.wrapping_mul(0x7feb352d);
    nanos ^= nanos >> 15;
    nanos = nanos.wrapping_mul(0x846ca68b);
    nanos ^= nanos >> 16;

    nanos % max
}
