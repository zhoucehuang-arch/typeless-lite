use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use parking_lot::Mutex;
use thiserror::Error;

use crate::types::MicrophoneDevice;

const TARGET_SAMPLE_RATE: u32 = 16_000;

pub trait AudioConsumer: Send + Sync {
    fn consume_pcm_chunk(&self, pcm: &[u8]);
}

#[derive(Debug, Error)]
pub enum RecorderError {
    #[error("录音设备不可用: {0}")]
    EngineFailed(String),
}

pub struct Recorder {
    stop: Arc<AtomicBool>,
    join: Mutex<Option<JoinHandle<()>>>,
}

impl Recorder {
    pub fn start(
        device_name: Option<String>,
        consumer: Arc<dyn AudioConsumer>,
        level_handler: Arc<dyn Fn(f32) + Send + Sync>,
    ) -> Result<(Self, Receiver<RecorderError>), RecorderError> {
        let stop = Arc::new(AtomicBool::new(false));
        let stop_for_thread = Arc::clone(&stop);
        let (startup_tx, startup_rx) = channel::<Result<(), RecorderError>>();
        let (runtime_tx, runtime_rx) = channel::<RecorderError>();
        let join = thread::Builder::new()
            .name("typeless-recorder".into())
            .spawn(move || {
                run_audio_thread(
                    device_name,
                    consumer,
                    level_handler,
                    stop_for_thread,
                    startup_tx,
                    runtime_tx,
                );
            })
            .map_err(|err| RecorderError::EngineFailed(err.to_string()))?;

        startup_rx
            .recv()
            .map_err(|err| RecorderError::EngineFailed(err.to_string()))??;

        Ok((
            Self {
                stop,
                join: Mutex::new(Some(join)),
            },
            runtime_rx,
        ))
    }

    pub fn stop(self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(join) = self.join.lock().take() {
            let _ = join.join();
        }
    }
}

pub fn list_input_devices() -> Result<Vec<MicrophoneDevice>, RecorderError> {
    let host = cpal::default_host();
    let default_name = host.default_input_device().and_then(|dev| dev.name().ok());
    let devices = host
        .input_devices()
        .map_err(|err| RecorderError::EngineFailed(err.to_string()))?;
    let mut result = Vec::new();
    for device in devices {
        let Ok(name) = device.name() else {
            continue;
        };
        result.push(MicrophoneDevice {
            is_default: default_name.as_deref() == Some(name.as_str()),
            name,
        });
    }
    Ok(result)
}

fn run_audio_thread(
    device_name: Option<String>,
    consumer: Arc<dyn AudioConsumer>,
    level_handler: Arc<dyn Fn(f32) + Send + Sync>,
    stop: Arc<AtomicBool>,
    startup_tx: Sender<Result<(), RecorderError>>,
    runtime_tx: Sender<RecorderError>,
) {
    let stream = match build_input_stream(device_name, consumer, level_handler, runtime_tx) {
        Ok(stream) => stream,
        Err(err) => {
            let _ = startup_tx.send(Err(err));
            return;
        }
    };
    if let Err(err) = stream.play() {
        let _ = startup_tx.send(Err(RecorderError::EngineFailed(err.to_string())));
        return;
    }
    let _ = startup_tx.send(Ok(()));
    while !stop.load(Ordering::SeqCst) {
        thread::sleep(std::time::Duration::from_millis(40));
    }
    let _ = stream.pause();
    drop(stream);
}

fn build_input_stream(
    device_name: Option<String>,
    consumer: Arc<dyn AudioConsumer>,
    level_handler: Arc<dyn Fn(f32) + Send + Sync>,
    runtime_tx: Sender<RecorderError>,
) -> Result<cpal::Stream, RecorderError> {
    let host = cpal::default_host();
    let device = select_input_device(&host, device_name.as_deref())?;
    let supported = device
        .default_input_config()
        .map_err(|err| RecorderError::EngineFailed(err.to_string()))?;
    let sample_format = supported.sample_format();
    let config: StreamConfig = supported.config();
    let input_sample_rate = config.sample_rate.0;
    let channels = config.channels as usize;
    let state = Arc::new(Mutex::new(ResampleState::new(input_sample_rate)));
    let err_tx = runtime_tx.clone();
    let err_fn = move |err| {
        let _ = err_tx.send(RecorderError::EngineFailed(err.to_string()));
    };

    match sample_format {
        SampleFormat::F32 => device
            .build_input_stream(
                &config,
                move |data: &[f32], _| process_input(data, channels, &state, &consumer, &level_handler),
                err_fn,
                None,
            )
            .map_err(|err| RecorderError::EngineFailed(err.to_string())),
        SampleFormat::I16 => device
            .build_input_stream(
                &config,
                move |data: &[i16], _| process_input(data, channels, &state, &consumer, &level_handler),
                err_fn,
                None,
            )
            .map_err(|err| RecorderError::EngineFailed(err.to_string())),
        SampleFormat::U16 => device
            .build_input_stream(
                &config,
                move |data: &[u16], _| process_input(data, channels, &state, &consumer, &level_handler),
                err_fn,
                None,
            )
            .map_err(|err| RecorderError::EngineFailed(err.to_string())),
        other => Err(RecorderError::EngineFailed(format!(
            "unsupported sample format: {other:?}"
        ))),
    }
}

fn select_input_device(
    host: &cpal::Host,
    name: Option<&str>,
) -> Result<cpal::Device, RecorderError> {
    if let Some(name) = name.filter(|value| !value.trim().is_empty()) {
        let devices = host
            .input_devices()
            .map_err(|err| RecorderError::EngineFailed(err.to_string()))?;
        for device in devices {
            if device.name().ok().as_deref() == Some(name) {
                return Ok(device);
            }
        }
    }
    host.default_input_device()
        .ok_or_else(|| RecorderError::EngineFailed("没有找到默认麦克风".into()))
}

trait ToF32Sample {
    fn to_f32_sample(self) -> f32;
}

impl ToF32Sample for f32 {
    fn to_f32_sample(self) -> f32 {
        self.clamp(-1.0, 1.0)
    }
}

impl ToF32Sample for i16 {
    fn to_f32_sample(self) -> f32 {
        self as f32 / i16::MAX as f32
    }
}

impl ToF32Sample for u16 {
    fn to_f32_sample(self) -> f32 {
        (self as f32 - 32768.0) / 32768.0
    }
}

fn process_input<T: Copy + ToF32Sample>(
    data: &[T],
    channels: usize,
    state: &Arc<Mutex<ResampleState>>,
    consumer: &Arc<dyn AudioConsumer>,
    level_handler: &Arc<dyn Fn(f32) + Send + Sync>,
) {
    if channels == 0 || data.is_empty() {
        return;
    }
    let mut mono = Vec::with_capacity(data.len() / channels);
    for frame in data.chunks(channels) {
        let sum = frame.iter().map(|sample| sample.to_f32_sample()).sum::<f32>();
        mono.push((sum / channels as f32).clamp(-1.0, 1.0));
    }
    let samples = state.lock().resample(&mono);
    if samples.is_empty() {
        return;
    }
    let mut rms_sum = 0.0f32;
    let mut pcm = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        rms_sum += sample * sample;
        let int = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        pcm.extend_from_slice(&int.to_le_bytes());
    }
    consumer.consume_pcm_chunk(&pcm);
    let sample_count = pcm.len().max(2) / 2;
    let level = ((rms_sum / sample_count as f32).sqrt() * 4.0).clamp(0.0, 1.0);
    level_handler(level);
}

struct ResampleState {
    input_sample_rate: u32,
    cursor: f64,
}

impl ResampleState {
    fn new(input_sample_rate: u32) -> Self {
        Self {
            input_sample_rate,
            cursor: 0.0,
        }
    }

    fn resample(&mut self, input: &[f32]) -> Vec<f32> {
        if input.is_empty() {
            return Vec::new();
        }
        if self.input_sample_rate == TARGET_SAMPLE_RATE {
            return input.to_vec();
        }
        let ratio = self.input_sample_rate as f64 / TARGET_SAMPLE_RATE as f64;
        let mut output = Vec::new();
        while self.cursor < input.len() as f64 {
            let idx = self.cursor.floor() as usize;
            let frac = self.cursor - idx as f64;
            let a = input[idx];
            let b = input.get(idx + 1).copied().unwrap_or(a);
            output.push(a + (b - a) * frac as f32);
            self.cursor += ratio;
        }
        self.cursor -= input.len() as f64;
        output
    }
}
