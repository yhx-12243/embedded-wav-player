use std::io;

use alsa::{
    Direction, PCM,
    pcm::{Access, HwParams},
};
use hound::WavReader;

use crate::util::{PlayError, cvt_format};

pub fn dump_header<R>(reader: &WavReader<R>)
where
    R: io::Read,
{
    println!("RIFF 标志：RIFF");
    println!(
        "文件大小：{}",
        reader.len() * u32::from(reader.spec().bytes_per_sample)
    );
    println!("文件格式：WAVE");
    println!("格式块标识：fmt");
    println!("格式块长度：16");
    println!("编码格式：{:?}", reader.spec().sample_format);
    println!("声道数：{}", reader.spec().channels);
    println!("采样频率：{} Hz", reader.spec().sample_rate);
    let block_align = u32::from(reader.spec().bytes_per_sample) * u32::from(reader.spec().channels);
    println!("传输速率：{} B/s", reader.spec().sample_rate * block_align);
    println!("数据块对齐单位：{block_align} B/block");
    println!("采样位数：{} bit", reader.spec().bits_per_sample);
}

pub fn play<R>(reader: &mut WavReader<R>) -> Result<(), PlayError>
where
    R: io::Read,
{
    // 打开 PCM 设备，分配 snd_pcm_hw_params_t 结构体，配置空间初始化
    let pcm = PCM::new("default", Direction::Playback, false)?;
    let mut params = HwParams::any(&pcm)?;

    // 设置交错模式
    params.set_access(Access::RWInterleaved)?;

    // 设置样本长度 (位数)
    params.set_format(cvt_format(reader.spec())?)?;

    // 设置采样率
    params.set_rate_near(reader.spec().sample_rate, 0)?;

    // 设置通道数
    params.set_channels(reader.spec().channels)?;

    pcm.hw_params(&params)?;

    Ok(())
}
