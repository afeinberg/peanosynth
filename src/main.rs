#![allow(unused_imports)]
use cpal;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use dasp::{signal, Sample, Signal};
use eframe::{egui, epi};
use egui::*;
use std::sync::mpsc;

// TODO: Move more parameters to this state.
pub struct SynthApp {
    time: usize,
}
impl epi::App for SynthApp {
    fn name(&self) -> &str {
        "Synth"
    }

    fn update(&mut self, ctx: &egui::CtxRef, frame: &mut epi::Frame<'_>) {
        egui::CentralPanel::default().show(&ctx, |ui| {
            let host = cpal::default_host();
            let device = host
                .default_output_device()
                .expect("failed to find a default output device");
            let config = device.default_output_config().unwrap();
            ui.horizontal( |ui| {
                ui.label("Time: ");
                ui.add(egui::Slider::new(&mut self.time, 0..=60));
            });
            if ui.button("Play").clicked() {
                let sample_format = config.sample_format();
                if sample_format == cpal::SampleFormat::F32 {
                    run::<f32>(&device, self.time, &config.into()).unwrap()
                } else if sample_format == cpal::SampleFormat::I16 {
                    run::<i16>(&device, self.time, &config.into()).unwrap()
                } else {
                    run::<u16>(&device, self.time, &config.into()).unwrap()
                }
            }
            ui.label(format!("Time: {} seconds", self.time))
        });
        frame.set_window_size(ctx.used_size())
    }
}

fn main() {
    let mut ctx = egui::CtxRef::default();
    let raw_input = egui::RawInput::default();
    ctx.begin_frame(raw_input);
    let app = SynthApp { time: 2 };
    let (_output, _what) = ctx.end_frame();
    let options = eframe::NativeOptions {
        ..Default::default()
    };
    eframe::run_native(Box::new(app), options);
}

// TODO: Allow passing in the chain as configuration. Make this part of application
// state.
fn run<T>(
    device: &cpal::Device,
    time: usize,
    config: &cpal::StreamConfig,
) -> Result<(), anyhow::Error>
where
    T: cpal::Sample,
{
    // Create a signal chain to play back 1 second of each oscillator at A4.
    let hz = signal::rate(config.sample_rate.0 as f64).const_hz(440.0);
    let time_scaled = config.sample_rate.0 as usize * time;
    let mut synth = hz
        .clone()
        .sine()
        .take(time_scaled)
        .chain(hz.clone().saw().take(time_scaled))
        .chain(hz.clone().square().take(time_scaled))
        .chain(hz.clone().noise_simplex().take(time_scaled))
        .chain(signal::noise(0).take(time_scaled))
        .map(|s| s.to_sample::<f32>() * 0.2);

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
