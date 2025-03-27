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

use crate::dsp::{roll, start_reader_thread};
use crate::ui;
use crate::{BUF_SIZE, CHANNELS, FFT_SIZE, HEIGHT, WIDTH};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::render::BlendMode;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub struct WaterfallDemo {
    center_frequency: Arc<AtomicU32>,
    control_thread: Option<thread::JoinHandle<()>>,
    sample_rate: Arc<AtomicU32>,
    should_stop: Arc<AtomicBool>,
    video_buffer: Arc<Mutex<Vec<u8>>>,
}

impl WaterfallDemo {
    pub fn new() -> WaterfallDemo {
        WaterfallDemo {
            center_frequency: Arc::new(AtomicU32::new(100_000_000)),
            control_thread: None,
            sample_rate: Arc::new(AtomicU32::new(2_400_000)),
            should_stop: Arc::new(AtomicBool::new(false)),
            video_buffer: Arc::new(Mutex::new(vec![0u8; BUF_SIZE])),
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
        let sample_rate = self.sample_rate.clone();
        let should_stop = self.should_stop.clone();
        let video_buffer = self.video_buffer.clone();

        self.control_thread = Some(thread::spawn(move || {
            let (mut ctl, reader) = rtlsdr_mt::open(0)
                .expect("Could not open RTL-SDR device at index 0.");
            ctl.set_sample_rate(sample_rate.load(Ordering::Relaxed))
                .unwrap();
            ctl.set_center_freq(center_frequency.load(Ordering::Relaxed))
                .unwrap();
            ctl.disable_agc().unwrap();
            ctl.set_tuner_gain(496).unwrap();

            let reader_thread = start_reader_thread(
                reader,
                should_stop.clone(),
                video_buffer.clone(),
            );

            while !should_stop.load(Ordering::Relaxed) {
                let desired_freq = center_frequency.load(Ordering::Relaxed);
                let current_freq = ctl.center_freq();

                if current_freq != desired_freq {
                    let diff = current_freq as i32 - desired_freq as i32;
                    let sr = sample_rate.load(Ordering::Relaxed) as i32;
                    ctl.cancel_async_read();
                    ctl.set_center_freq(desired_freq).unwrap();
                    let vb = video_buffer.clone();
                    let mut raw_data = vb.lock().unwrap();
                    roll(
                        &mut raw_data,
                        vec![HEIGHT, WIDTH, CHANNELS],
                        2,
                        diff.signum() * FFT_SIZE as i32 * diff.abs() / sr,
                    );
                }
                thread::sleep(Duration::new(0, 1_000_000_000u32 / 30));
            }
            ctl.cancel_async_read();
            reader_thread.join().unwrap();
        }));
    }

    fn start_sdl2_window(&self) {
        let sdl_context = sdl2::init().unwrap();
        let video_subsystem = sdl_context.video().unwrap();
        let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string()).unwrap();
        // Font medium (16pt)
        let font_md = ui::create_font(16, &ttf_context);
        // Font small (12pt)
        let font_sm = ui::create_font(12, &ttf_context);

        let window = video_subsystem
            .window("Rust RTL-SDR waterfall demo", WIDTH, HEIGHT)
            .position_centered()
            .build()
            .unwrap();
        let mut canvas = window.into_canvas().build().unwrap();
        let texture_creator = canvas.texture_creator();

        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        canvas.present();
        canvas.set_blend_mode(BlendMode::Blend);
        let mut event_pump = sdl_context.event_pump().unwrap();

        'running: loop {
            let mut current_freq =
                self.center_frequency.load(Ordering::Relaxed);
            for event in event_pump.poll_iter() {
                match event {
                    Event::Quit { .. }
                    | Event::KeyDown {
                        keycode: Some(Keycode::Escape),
                        ..
                    } => {
                        self.should_stop.store(true, Ordering::Relaxed);
                        break 'running;
                    }
                    Event::KeyDown {
                        keycode: Some(Keycode::Left),
                        ..
                    } => {
                        current_freq -= 100_000;
                        self.center_frequency
                            .store(current_freq, Ordering::Relaxed);
                    }
                    Event::KeyDown {
                        keycode: Some(Keycode::Right),
                        ..
                    } => {
                        current_freq += 100_000;
                        self.center_frequency
                            .store(current_freq, Ordering::Relaxed);
                    }
                    _ => {}
                }
            }

            ui::render(
                &mut canvas,
                &texture_creator,
                &font_sm,
                &font_md,
                (current_freq as f64) / 1_000_000f64,
                self.video_buffer.clone(),
            );

            canvas.present();
            thread::sleep(Duration::new(0, 1_000_000_000u32 / 30));
        }
    }
}
