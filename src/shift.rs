use std::{char::MAX, sync::LazyLock};

use crate::fmt_impl::Fmt;

pub const BLOCK_SIZE: usize = 512;
pub const ADDITION: usize = 128;
pub const MAX_BUFFER_SIZE: usize = buffer_size(4);

pub const FRAME_LENGTH: usize = BLOCK_SIZE * 2;

static HANNING_WINDOW: LazyLock<[f64; FRAME_LENGTH]> = LazyLock::new(|| {
    let mut temp = [0.0; FRAME_LENGTH];
    for i in 0..FRAME_LENGTH {
        temp[i] = 0.5 * (1.0 - (core::f64::consts::TAU * i as f64 / FRAME_LENGTH as f64).cos())
    }
    temp
});

fn mult_hanning_window(input_array: &[f64; FRAME_LENGTH]) -> [f64; FRAME_LENGTH] {
    let mut ret = [0.0; FRAME_LENGTH];
    for i in 0..FRAME_LENGTH {
        ret[i] = input_array[i] * HANNING_WINDOW[i]
    }
    ret
}

fn correlate(x: &[f64], y: &[f64], out: &mut [f64]) {
    for k in 0..=y.len() - x.len() {
        let mut sum = 0.0;
        for i in 0..x.len() {
            sum += x[i] * y[k + i];
        }
        out[k] = sum;
    }
}

#[inline(always)]
pub const fn buffer_size(multiplier: u8) -> usize {
    one_time_consume(multiplier) + FRAME_LENGTH + ADDITION
}

#[inline(always)]
/// effectively (`BLOCK_SIZE` * speed)
pub const fn one_time_consume(multiplier: u8) -> usize {
    multiplier as usize * const { BLOCK_SIZE / 2 }
}

fn compute_offset(src: &[f64; MAX_BUFFER_SIZE], multiplier: u8) -> usize {
    todo!()
}

fn process_channel(src: &[f64; MAX_BUFFER_SIZE], multiplier: u8, offset: usize, dst: &mut [f64; BLOCK_SIZE]) -> usize {
    let n = buffer_size(multiplier);
    let m = one_time_consume(multiplier);

    let overlap_part1 = src[..FRAME_LENGTH].as_array::<FRAME_LENGTH>().unwrap();
    let ref_part = src[BLOCK_SIZE..BLOCK_SIZE + FRAME_LENGTH].as_array::<FRAME_LENGTH>().unwrap();
    let slide_window = &src[m - ADDITION / 2..m + FRAME_LENGTH + ADDITION / 2];
    let mut result = [0.0; ADDITION + 1];
    correlate(ref_part, &slide_window, &mut result);
    let argmax = result.iter().enumerate()
        .max_by(|(_, x), (_, y)| x.total_cmp(y))
        .unwrap().0 + m - ADDITION / 2;
    let overlap_part2 = src[argmax..argmax + FRAME_LENGTH].as_array::<FRAME_LENGTH>().unwrap();

    for i in 0..BLOCK_SIZE {
        dst[i] = overlap_part1[BLOCK_SIZE + i] * HANNING_WINDOW[BLOCK_SIZE + i] + overlap_part2[i] * HANNING_WINDOW[i];
    }

    m
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

        let mut consume_now = m;
        let mut offset = 0;

        for i in 0..channels {
            for j in 0..n {
                v[j] = unsafe { block.get_unchecked(j * channels + i) }.to_f64();
            }
            if i == 0 {
                offset = compute_offset(&v, multiplier);
            }
            let c = process_channel(&v, multiplier, offset, &mut scratch);
            if i == 0 {
                consume_now = c;
            }
            for j in 0..BLOCK_SIZE {
                *unsafe { out.get_unchecked_mut(j * channels + i) } = S::from_f64(scratch[j]);
            }
        }

        consume += channels * consume_now;
        produce += channels * BLOCK_SIZE;

        r#in = &r#in[channels * consume_now..];
        out = &mut out[channels * BLOCK_SIZE..];
    }

    (consume, produce)
}
