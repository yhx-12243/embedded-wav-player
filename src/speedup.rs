use alsa::{
    pcm::{Format,},
};

pub const FRAME_LENGTH: usize = 1024;

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

fn process<T: Into<f64>>(chunk: &[u8]) -> Vec<f64> {
    let chunk_t = unsafe {
        core::slice::from_raw_parts(chunk.as_ptr().cast::<T>(), chunk.len() / core::mem::size_of::<T>())
    };
    chunk_t.iter().map(Into::into).collect()
}

pub fn ola_process(byte_chunk: &[u8], format: Format, multipier: u8, channel_num: u8) -> Vec<u8> {
    let output_stride = FRAME_LENGTH/2;
    let input_stride: usize = output_stride * multiplier / 2;
    let data_vec = match format{
        Format::S8 => process::<i8>(byte_chunk),
        Format::S16LE => process::<i16>(byte_chunk),
        Format::S32LE => process::<i32>(byte_chunk),
        Format::FloatLE => process::<f32>(byte_chunk),
        Format::Float64LE => process::<f64>(byte_chunk),
        _ => panic!("Don't panic!")
    };
    assert(data_vec.len()==FRAME_LENGTH*2*channel_num);
    let 
    for i in 0..channel_num {
        for j in  i..
    }
}