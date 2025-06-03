use crate::fmt_impl::Fmt;

pub const BLOCK_SIZE: usize = 1024;

/// out size should be (`BLOCK_SIZE` * multiplier / 2)
pub fn process_channel(v: &[f64; 2 * BLOCK_SIZE], multiplier: u8, out: &mut [f64]) {
    match multiplier {
        1 => (),
        2 => out[..BLOCK_SIZE].copy_from_slice(&v[..BLOCK_SIZE]),
        3 => (),
        4 => (),
        _ => panic!("invalid value"),
    }
}

/// (ret: 输入消耗量，输出写入量)
pub fn process<S: Fmt>(block: &[S], multiplier: u8, out: &mut [S]) -> (usize, usize) {
    let l = block.len();
    assert!(l % (2 * BLOCK_SIZE) == 0 && l > 0 && multiplier > 0);
    if multiplier == 2 { // fast path
        out[..l].copy_from_slice(block);
        return (l, l);
    }

    let channels = l / (2 * BLOCK_SIZE);
    let mut v = [0f64; 2 * BLOCK_SIZE];
    let mut scratch = [0f64; 2 * BLOCK_SIZE];
    let out_len = const { BLOCK_SIZE / 2 } * usize::from(multiplier);
    let scratch = &mut scratch[..out_len];
    for i in 0..channels {
        for j in 0..BLOCK_SIZE {
            v[j] = unsafe { block.get_unchecked(j * channels + i) }.to_f64();
        }
        process_channel(&v, multiplier, scratch);
        for j in 0..out_len {
            out[j * channels + i] = S::from_f64(scratch[j]);
        }
    }

    (channels * BLOCK_SIZE, channels * out_len)
}
