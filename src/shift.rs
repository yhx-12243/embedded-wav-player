use crate::fmt_impl::Fmt;

pub const BLOCK_SIZE: usize = 512;
pub const ADDITION: usize = 128;
pub const MAX_BUFFER_SIZE: usize = buffer_size(4);

pub const FRAME_LENGTH: usize = BLOCK_SIZE*2;

const HANNING_WINDOW: [f64; FRAME_LENGTH] = const{
    let mut temp: [f64; FRAME_LENGTH] = [0.0; FRAME_LENGTH];
    for i in 0..FRAME_LENGTH {
        temp[i] = 0.5 * (1.0 - (2.0 * std::f64::consts::PI * i as f64 / (FRAME_LENGTH as f64 - 1.0)).cos())
    }
    temp
};

pub fn mult_hanning_window<f64>(input_array: &[f64; FRAME_LENGTH]) -> [f64; FRAME_LENGTH] {
    let ret = [0.0; FRAME_LENGTH];
    for i in 0..FRAME_LENGTH {
        ret[i] = input_array[i] * HANNING_WINDOW[i]
    }
    ret
}

#[inline(always)]
pub const fn buffer_size(multiplier: u8) -> usize {
    multiplier as usize * BLOCK_SIZE + ADDITION
}

#[inline(always)]
pub const fn one_time_consume(multiplier: u8) -> usize {
    multiplier as usize * const { BLOCK_SIZE / 2 }
}

fn process_channel(src: &[f64; MAX_BUFFER_SIZE], multiplier: u8, dst: &mut [f64; BLOCK_SIZE]) {
    let n = buffer_size(multiplier);
    let m = one_time_consume(multiplier);
    // TODO: Time-scale Modificaiton

    weighted_part1 = mult_hanning_window(&src[..FRAME_LENGTH]);
    weighted_part2 = mult_hanning_window(&src[m..m+FRAME_LENGTH]);
    for i in 0..BLOCK_SIZE {
        dst[i] = weighted_part1[BLOCK_SIZE+i]+weighted_part2[i];
    }

    // ==== DUMMY CODE BELOW ====
    
    // 谢谢烈火！

    // ==== DUMMY CODE ABOVE ====
}

/// (ret: 输入消耗量，输出写入量)
pub fn process<S: Fmt>(mut r#in: &[S], channels: usize, multiplier: u8, mut out: &mut [S]) -> (usize, usize) {
    if multiplier == 2 { // fast path, without interleave/transpose
        let l = r#in.len().min(out.len());
        out[..l].copy_from_slice(&r#in[..l]);
        return (channels * BLOCK_SIZE, channels * BLOCK_SIZE);
    }

    let n = buffer_size(multiplier);
    let m = one_time_consume(multiplier);

    let mut v = [0f64; MAX_BUFFER_SIZE];
    let mut scratch = [0f64; BLOCK_SIZE];

    let mut consume = 0;
    let mut produce = 0;

    while r#in.len() >= channels * n && out.len() >= channels * BLOCK_SIZE {
        let block = &r#in[..channels * n];

        for i in 0..channels {
            for j in 0..n {
                v[j] = unsafe { block.get_unchecked(j * channels + i) }.to_f64();
            }
            process_channel(&v, multiplier, &mut scratch);
            for j in 0..BLOCK_SIZE {
                *unsafe { out.get_unchecked_mut(j * channels + i) } = S::from_f64(scratch[j]);
            }
        }

        consume += channels * m;
        produce += channels * BLOCK_SIZE;

        r#in = &r#in[channels * m..];
        out = &mut out[channels * BLOCK_SIZE..];
    }

    (consume, produce)
}
