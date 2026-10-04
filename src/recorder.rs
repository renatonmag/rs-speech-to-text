use std::{
    io::Cursor,
    sync::{Arc, Mutex, mpsc},
    thread::JoinHandle,
};

use anyhow::{Context as _, Result, anyhow};
use cpal::{
    FromSample, SampleFormat, SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};

type Samples = Arc<Mutex<Vec<f32>>>;

/// An in-progress microphone recording. The cpal stream lives on its own
/// thread because it is not `Send`.
pub struct Recording {
    stop_tx: mpsc::Sender<()>,
    thread: JoinHandle<Result<()>>,
    samples: Samples,
    sample_rate: u32,
}

impl Recording {
    pub fn start() -> Result<Self> {
        let samples: Samples = Arc::default();
        let (stop_tx, stop_rx) = mpsc::channel();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<u32>>();

        let thread_samples = samples.clone();
        let thread = std::thread::spawn(move || -> Result<()> {
            let stream = match open_stream(thread_samples) {
                Ok((stream, rate)) => {
                    let _ = ready_tx.send(Ok(rate));
                    stream
                }
                Err(e) => {
                    let _ = ready_tx.send(Err(anyhow!("{e:#}")));
                    return Err(e);
                }
            };
            let _ = stop_rx.recv();
            drop(stream);
            Ok(())
        });

        let sample_rate = ready_rx
            .recv()
            .map_err(|_| anyhow!("recorder thread exited"))??;

        Ok(Self {
            stop_tx,
            thread,
            samples,
            sample_rate,
        })
    }

    /// Stop recording and return the captured audio as a mono 16-bit WAV.
    pub fn stop(self) -> Result<Vec<u8>> {
        let _ = self.stop_tx.send(());
        self.thread
            .join()
            .map_err(|_| anyhow!("recorder thread panicked"))??;
        let samples = std::mem::take(&mut *self.samples.lock().unwrap());
        encode_wav(&samples, self.sample_rate)
    }
}

fn open_stream(samples: Samples) -> Result<(cpal::Stream, u32)> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .context("no input device available")?;
    let supported = device.default_input_config()?;
    let format = supported.sample_format();
    let config = supported.config();
    let rate = config.sample_rate;

    let stream = match format {
        SampleFormat::F32 => build::<f32>(&device, config, samples)?,
        SampleFormat::I16 => build::<i16>(&device, config, samples)?,
        SampleFormat::U16 => build::<u16>(&device, config, samples)?,
        SampleFormat::I32 => build::<i32>(&device, config, samples)?,
        other => return Err(anyhow!("unsupported sample format {other:?}")),
    };
    stream.play()?;
    Ok((stream, rate))
}

fn build<T>(
    device: &cpal::Device,
    config: cpal::StreamConfig,
    samples: Samples,
) -> Result<cpal::Stream>
where
    T: SizedSample,
    f32: FromSample<T>,
{
    let channels = config.channels as usize;
    let stream = device.build_input_stream::<T, _, _>(
        config,
        move |data: &[T], _| {
            let mut out = samples.lock().unwrap();
            for frame in data.chunks(channels) {
                let sum: f32 = frame.iter().map(|s| s.to_sample::<f32>()).sum();
                out.push(sum / channels as f32);
            }
        },
        |err| eprintln!("audio input error: {err}"),
        None,
    )?;
    Ok(stream)
}

pub fn encode_wav(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut cursor, spec)?;
        for &s in samples {
            writer.write_sample((s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)?;
        }
        writer.finalize()?;
    }
    Ok(cursor.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_readable_wav() {
        let rate = 16_000;
        let samples: Vec<f32> = (0..rate)
            .map(|i| (i as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin() * 0.5)
            .collect();
        let wav = encode_wav(&samples, rate).unwrap();

        let reader = hound::WavReader::new(Cursor::new(wav)).unwrap();
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().sample_rate, rate);
        assert_eq!(reader.len(), rate);
    }
}
