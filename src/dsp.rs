/* Copyright (c) 2025 by Karsten Lehmann <mail@kalehmann.de>
 *
 *   This file is part of rust-rtl-sdr-waterfall-demo.
 *
 *   rust-rtl-sdr-waterfall-demo is free software: you can redistribute it
 *   and/or modify it under the terms of the GNU Affero General Public License
 *   as published by the Free Software Foundation, either version 3 of the
 *   License, or (at your option) any later version.
 *
 *   rust-rtl-sdr-waterfall-demo is distributed in the hope that it will be
 *   useful, but WITHOUT ANY WARRANTY; without even the implied warranty of
 *   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero
 *   General Public License for more details.
 *
 *   You should have received a copy of the GNU Affero General Public License
 *   along with rust-rtl-sdr-waterfall-demo. If not, see
 *   <https://www.gnu.org/licenses/>. */

use rustfft::{num_complex::Complex, Fft, FftPlanner};
use std::ops::DerefMut;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::thread;

const FFT_SIZE: usize = crate::ui::WIDTH as usize;

pub fn start_reader_thread(
    mut reader: rtlsdr_mt::Reader,
    center_frequency: Arc<AtomicU32>,
    fft_window: WindowType,
    should_stop: Arc<AtomicBool>,
    sender: SyncSender<FftResult>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let signal_processor = SignalProcessor::new(fft_window);

        while !should_stop.load(Ordering::Relaxed) {
            let cf = center_frequency.load(Ordering::Relaxed);
            match reader.read_async(1, 2048, |buf| {
                let mut result = signal_processor.process_signal(buf);
                result.center_frequency = cf;
                match sender.try_send(result) {
                    Ok(..) => {}
                    Err(..) => {}
                }
            }) {
                Ok(..) => {}
                Err(..) => {}
            }
        }
    })
}

#[derive(clap::ValueEnum, Clone, Copy, Debug)]
pub enum WindowType {
    Bartlett,
    Rectangular,
}

pub struct FftResult {
    pub avg: f64,
    pub center_frequency: u32,
    pub log_magnitudes: Vec<f64>,
    pub peak: Option<(usize, f64)>,
}

struct SignalProcessor {
    fft: Arc<dyn Fft<f64>>,
    window: WindowType,
}

impl SignalProcessor {
    pub fn new(window: WindowType) -> SignalProcessor {
        let mut planner: FftPlanner<f64> = FftPlanner::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);

        SignalProcessor {
            fft: fft,
            window: window,
        }
    }

    pub fn process_signal(&self, buf: &[u8]) -> FftResult {
        let mut signal_vector = buf
            .chunks(2)
            .map(|pair| Complex {
                re: (f64::from(pair[0]) - 127.0) / 127.0,
                im: (f64::from(pair[1]) - 127.0) / 127.0,
            })
            .collect::<Vec<Complex<f64>>>();
        let signal = signal_vector.deref_mut();
        self.apply_window(signal);
        return self.work_fft(signal);
    }

    fn apply_window(&self, signal: &mut [Complex<f64>]) {
        match self.window {
            WindowType::Bartlett => {
                for i in 0..FFT_SIZE / 2 {
                    let factor = i as f64 / (FFT_SIZE / 2) as f64;
                    signal[i] *= factor;
                    signal[FFT_SIZE - i - 1] *= factor;
                }
            }
            WindowType::Rectangular => {
                // Do nothing
            }
        }
    }

    fn work_fft(&self, signal: &mut [Complex<f64>]) -> FftResult {
        self.fft.process(signal);

        let mut result = FftResult {
            avg: 0.0f64,
            center_frequency: 0,
            log_magnitudes: vec![0.0f64; FFT_SIZE],
            peak: None,
        };
        let mut peak: (usize, f64) = (0, -120.0);

        for (i, c) in signal.into_iter().enumerate() {
            let index = (i + FFT_SIZE / 2) % FFT_SIZE;
            let logmag = 10.0
                * (c.norm_sqr() / (FFT_SIZE as f64).powi(2))
                    .max(1e-12)
                    .log10()
                    .min(0.);
            result.log_magnitudes[index] = logmag;
            result.avg += logmag;
            if logmag > peak.1 {
                peak = (index, logmag);
            }
        }
        result.avg /= FFT_SIZE as f64;
        if peak.1 > result.avg + 20.0 {
            result.peak = Some(peak);
        }

        return result;
    }
}
