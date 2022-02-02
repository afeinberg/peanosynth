use cpal;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use dasp::{signal, Sample, Signal};
use eframe::{egui, epi};
use env_logger::Env;
use log::info;
use serde::{Deserialize, Serialize};
use serde_json;
use std::sync::mpsc;

#[derive(Serialize, Deserialize)]
pub struct Project {
    time: usize,
    sequence: Vec<Waveform>,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum Waveform {
    Sine,
    Saw,
    Square,
    Noise,
    NoiseSimplex,
}

impl Default for Project {
    fn default() -> Self {
        let default_project = include_str!("default_project.json");
        serde_json::from_str::<Self>(default_project).unwrap()
    }
}

pub struct AudioDevice {
    device: cpal::Device,
    sample_format: cpal::SampleFormat,
    config: cpal::StreamConfig,
}

impl AudioDevice {
    pub fn default_device() -> Option<Self> {
        let host = cpal::default_host();
        host.default_output_device().and_then(|dev| {
            let config = dev.default_output_config();
            config.ok().map(|cfg| AudioDevice {
                device: dev,
                sample_format: cfg.sample_format(),
                config: cfg.into(),
            })
        })
    }
}

pub struct SynthApp {
    project: Project,
    device: AudioDevice,
}

impl SynthApp {
    pub fn new() -> Self {
        Self {
            project: Project::default(),
            device: AudioDevice::default_device().unwrap(),
        }
    }

    pub fn serialize_project(&self) -> String {
        serde_json::to_string(&self.project).unwrap()
    }

    pub fn play(&self) -> Result<(), anyhow::Error> {
        match self.device.sample_format {
            cpal::SampleFormat::F32 => self.run::<f32>(),
            cpal::SampleFormat::I16 => self.run::<i16>(),
            cpal::SampleFormat::U16 => self.run::<u16>(),
        }
    }

    fn signals_from_sequence(&self) -> impl Iterator<Item = f64> {
        let time = self.project.time;
        let config = &self.device.config;
        let hz = signal::rate(config.sample_rate.0 as f64).const_hz(440.0);
        let time_scaled = config.sample_rate.0 as usize * time;
        self.project
            .sequence
            .iter()
            .cloned()
            .fold(
                signal::equilibrium().take(0).collect::<Vec<f64>>(),
                |acc, w| {
                    let v: Vec<f64> = match w {
                        Waveform::Sine => hz.clone().sine().take(time_scaled).collect(),
                        Waveform::Saw => hz.clone().saw().take(time_scaled).collect(),
                        Waveform::Square => hz.clone().square().take(time_scaled).collect(),
                        Waveform::NoiseSimplex => {
                            hz.clone().noise_simplex().take(time_scaled).collect()
                        }
                        Waveform::Noise => signal::noise(0).take(time_scaled).collect(),
                    };
                    acc.into_iter().chain(v.into_iter()).collect()
                },
            )
            .into_iter()
    }

    /*
    fn build_signals(&self) -> impl Iterator<Item = f64> {
        let time = self.project.time;
        let config = &self.device.config;
        let hz = signal::rate(config.sample_rate.0 as f64).const_hz(440.0);
        let time_scaled = config.sample_rate.0 as usize * time;
        hz.clone()
            .sine()
            .take(time_scaled)
            .chain(hz.clone().saw().take(time_scaled))
            .chain(hz.clone().square().take(time_scaled))
            .chain(hz.clone().noise_simplex().take(time_scaled))
            .chain(signal::noise(0).take(time_scaled))
    } */

    fn run<T>(&self) -> Result<(), anyhow::Error>
    where
        T: cpal::Sample,
    {
        let device = &self.device.device;
        let config = &self.device.config;
        // Create a signal chain to play back 1 second of each oscillator at A4.
        let signals = self.signals_from_sequence();
        let mut synth = signals.map(|s| s.to_sample::<f32>() * 0.2);

        // A channel for indicating when playback has completed.
        let (complete_tx, complete_rx) = mpsc::sync_channel(1);

        // Create and run the stream.
        let err_fn = |err| eprintln!("an error occurred on stream: {}", err);
        let channels = config.channels as usize;
        let stream = device.build_output_stream(
            config,
            move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                write_data(data, channels, &complete_tx, &mut synth)
            },
            err_fn,
        )?;
        stream.play()?;

        // Wait for playback to complete.
        complete_rx.recv().unwrap();
        stream.pause()?;

        Ok(())
    }
}

impl epi::App for SynthApp {
    fn name(&self) -> &str {
        "Synth"
    }

    fn update(&mut self, ctx: &egui::CtxRef, frame: &mut epi::Frame<'_>) {
        egui::CentralPanel::default().show(&ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("Time: ");
                ui.add(egui::Slider::new(&mut self.project.time, 0..=60));
            });
            if ui.button("Play").clicked() {
                self.play().unwrap();
            }
            ui.label(format!("Time: {} seconds", self.project.time))
        });
        frame.set_window_size(ctx.used_size())
    }
}

fn write_data<T>(
    output: &mut [T],
    channels: usize,
    complete_tx: &mpsc::SyncSender<()>,
    signal: &mut dyn Iterator<Item = f32>,
) where
    T: cpal::Sample,
{
    for frame in output.chunks_mut(channels) {
        let sample = match signal.next() {
            None => {
                complete_tx.try_send(()).ok();
                0.0
            }
            Some(sample) => sample,
        };
        let value: T = cpal::Sample::from::<f32>(&sample);
        for sample in frame.iter_mut() {
            *sample = value;
        }
    }
}

fn main() {
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();
    let mut ctx = egui::CtxRef::default();
    let raw_input = egui::RawInput::default();
    ctx.begin_frame(raw_input);
    let app = SynthApp::new();
    info!("{}", app.serialize_project());
    let (_output, _what) = ctx.end_frame();
    let options = eframe::NativeOptions {
        ..Default::default()
    };
    eframe::run_native(Box::new(app), options);
}
