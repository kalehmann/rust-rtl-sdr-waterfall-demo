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

use crate::dsp;
use crate::ui;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
use std::sync::mpsc::sync_channel;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub struct WaterfallDemo {
    center_frequency: Arc<AtomicU32>,
    control_thread: Option<thread::JoinHandle<()>>,
    fft_window: dsp::WindowType,
    gain: Arc<AtomicI32>,
    sample_rate: Arc<AtomicU32>,
    should_stop: Arc<AtomicBool>,
    ui: ui::Ui,
}

impl WaterfallDemo {
    pub fn new(
        center_frequency_hz: u32,
        fft_window: dsp::WindowType,
    ) -> WaterfallDemo {
        let center_frequency = Arc::new(AtomicU32::new(center_frequency_hz));
        let gain = Arc::new(AtomicI32::new(0));
        let sample_rate: u32 = 2_400_000;

        WaterfallDemo {
            center_frequency: center_frequency.clone(),
            control_thread: None,
            fft_window: fft_window,
            gain: gain.clone(),
            sample_rate: Arc::new(AtomicU32::new(sample_rate)),
            should_stop: Arc::new(AtomicBool::new(false)),
            ui: ui::Ui::new(
                center_frequency.clone(),
                gain.clone(),
                sample_rate,
            ),
        }
    }

    pub fn run(&mut self) {
        self.start_control_thread();
        self.start_sdl2_window();
        self.finish();
    }

    fn finish(&mut self) {
        self.should_stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.control_thread.take() {
            thread.join().unwrap();
        }
        self.should_stop.store(false, Ordering::Relaxed);
        self.control_thread = None;
    }

    fn start_control_thread(&mut self) {
        let center_frequency = self.center_frequency.clone();
        let fft_window = self.fft_window;
        let gain = self.gain.clone();
        let sample_rate = self.sample_rate.clone();
        let should_stop = self.should_stop.clone();
        let (sync_sender, receiver) = sync_channel::<dsp::FftResult>(0);
        self.ui.set_fft_receiver(receiver);

        let (mut ctl, reader) = rtlsdr_mt::open(0)
            .expect("Could not open RTL-SDR device at index 0.");
        ctl.set_sample_rate(sample_rate.load(Ordering::Relaxed))
            .unwrap();
        ctl.set_center_freq(center_frequency.load(Ordering::Relaxed))
            .unwrap();
        ctl.disable_agc().unwrap();
        ctl.set_tuner_gain(gain.load(Ordering::Relaxed)).unwrap();

        let mut gains = [0i32; 32];
        ctl.tuner_gains(&mut gains);
        self.ui.set_available_gains(gains.to_vec());

        self.control_thread = Some(thread::spawn(move || {
            let reader_thread = dsp::start_reader_thread(
                reader,
                center_frequency.clone(),
                fft_window,
                should_stop.clone(),
                sync_sender,
            );

            while !should_stop.load(Ordering::Relaxed) {
                let desired_freq = center_frequency.load(Ordering::Relaxed);
                let current_freq = ctl.center_freq();

                if current_freq != desired_freq {
                    ctl.cancel_async_read();
                    ctl.set_center_freq(desired_freq).unwrap();
                }
                let desired_gain = gain.load(Ordering::Relaxed);
                let current_gain = ctl.tuner_gain();

                if current_gain != desired_gain {
                    ctl.set_tuner_gain(desired_gain).unwrap();
                }
                thread::sleep(Duration::new(0, 1_000_000_000u32 / 30));
            }
            ctl.cancel_async_read();
            reader_thread.join().unwrap();
        }));
    }

    fn start_sdl2_window(&mut self) {
        self.ui.run();
        self.should_stop.store(true, Ordering::Relaxed);
    }
}
