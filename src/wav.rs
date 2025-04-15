use std::io;

use hound::WavReader;

pub fn dump_header<R>(reader: &WavReader<R>)
where
	R: io::Read,
{
	println!("RIFF 标志：RIFF");
	println!("文件大小：{}", reader.len() * u32::from(reader.spec().bytes_per_sample));
	println!("文件格式：WAVE");
	println!("格式块标识：fmt");
	println!("格式块长度：16");
	println!("编码格式：{:?}", reader.spec().sample_format);
	println!("声道数：{}", reader.spec().channels);
	println!("采样频率：{} Hz", reader.spec().sample_rate);
	let block_align = u32::from(reader.spec().bytes_per_sample) * u32::from(reader.spec().channels);
	println!("传输速率：{} B/s", reader.spec().sample_rate * block_align);
	println!("数据块对齐单位 B/clock：{block_align}");
	println!("采样位数：{} bit", reader.spec().bits_per_sample);
}
