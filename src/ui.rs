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

pub const WIDTH: u32 = 1024;
const HEIGHT: u32 = 800;
const CHANNELS: u32 = 3;
const BUF_SIZE: usize = (WIDTH * HEIGHT * CHANNELS) as usize;
const PITCH: u32 = WIDTH * CHANNELS;
const SPECTRUM_OFFSET: u32 = 30;
const WATERFALL_OFFSET: u32 = 300;

use crate::dsp::FftResult;
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{BlendMode, Canvas, TextureQuery};
use sdl2::rwops::RWops;
use sdl2::ttf::{Font, FontStyle, Sdl2TtfContext};
use std::ops::DerefMut;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const ANDIKA_BOLD_TTF: &[u8] =
    include_bytes!("../assets/Andika/Andika-Bold.ttf");

pub struct Ui {
    canvas: Canvas<sdl2::video::Window>,
    center_frequency: Arc<AtomicU32>,
    color_map: Vec<[u8; 3]>,
    event_pump: sdl2::EventPump,
    fft_recv: Option<Receiver<FftResult>>,
    sample_rate: u32,
    texture_creator: sdl2::render::TextureCreator<sdl2::video::WindowContext>,
    pub video_buffer: Arc<Mutex<Vec<u8>>>,
}

impl Ui {
    pub fn new(center_frequency: Arc<AtomicU32>, sample_rate: u32) -> Ui {
        let sdl_context = sdl2::init().unwrap();
        let event_pump = sdl_context.event_pump().unwrap();
        let video_subsystem = sdl_context.video().unwrap();
        let window = video_subsystem
            .window("Rust RTL-SDR waterfall demo", WIDTH, HEIGHT)
            .position_centered()
            .build()
            .unwrap();
        let canvas = window.into_canvas().build().unwrap();
        let texture_creator = canvas.texture_creator();

        Ui {
            canvas: canvas,
            center_frequency: center_frequency,
            color_map: interpolate_color_map(
                vec![[255, 200, 20], [250, 110, 20], [60, 0, 45], [30, 20, 50]],
                121,
            ),
            event_pump: event_pump,
            fft_recv: None,
            sample_rate: sample_rate,
            texture_creator: texture_creator,
            video_buffer: Arc::new(Mutex::new(vec![0u8; BUF_SIZE])),
        }
    }

    pub fn set_fft_receiver(&mut self, receiver: Receiver<FftResult>) {
        self.fft_recv = Some(receiver);
    }

    pub fn run(&mut self) {
        let mut current_frequency =
            self.center_frequency.load(Ordering::Relaxed);
        let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string()).unwrap();
        // Font medium (16pt)
        let font_md = create_font(16, &ttf_context);
        // Font small (12pt)
        let font_sm = create_font(12, &ttf_context);

        self.canvas.set_blend_mode(BlendMode::Blend);
        'running: loop {
            for event in self.event_pump.poll_iter() {
                match event {
                    Event::Quit { .. }
                    | Event::KeyDown {
                        keycode: Some(Keycode::Escape),
                        ..
                    } => {
                        break 'running;
                    }
                    Event::KeyDown {
                        keycode: Some(Keycode::Left),
                        ..
                    } => {
                        self.center_frequency.store(
                            self.center_frequency.load(Ordering::Relaxed)
                                - 100_000,
                            Ordering::Relaxed,
                        );
                    }
                    Event::KeyDown {
                        keycode: Some(Keycode::Right),
                        ..
                    } => {
                        self.center_frequency.store(
                            self.center_frequency.load(Ordering::Relaxed)
                                + 100_000,
                            Ordering::Relaxed,
                        );
                    }
                    _ => {}
                }
            }

            match &self.fft_recv {
                Some(recv) => match recv.recv() {
                    Ok(result) => {
                        if result.center_frequency != current_frequency {
                            let diff = current_frequency as i32
                                - result.center_frequency as i32;
                            let mut raw_data =
                                self.video_buffer.lock().unwrap();
                            roll(
                                &mut raw_data,
                                vec![HEIGHT, WIDTH, CHANNELS],
                                2,
                                diff.signum() * WIDTH as i32 * diff.abs()
                                    / self.sample_rate as i32,
                            );
                            current_frequency = result.center_frequency;
                        }
                        self.update_video_buffer(result);
                    }
                    Err(..) => {}
                },
                None => {}
            }
            self.render(&font_md, &font_sm, current_frequency);

            self.canvas.present();
            thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
        }
    }

    pub fn update_video_buffer(&self, fft_result: FftResult) {
        let mut raw_data = self.video_buffer.lock().unwrap();
        let mut index = (WATERFALL_OFFSET * CHANNELS * WIDTH) as usize;
        roll(
            &mut raw_data[index..BUF_SIZE],
            vec![HEIGHT - WATERFALL_OFFSET, WIDTH, CHANNELS],
            1,
            1,
        );
        raw_data[0..index].fill(0);

        // Draw the horizontal lines for the amplitude spectrum
        for i in (4..24).step_by(4) {
            let start =
                ((SPECTRUM_OFFSET + i * 10) * CHANNELS * WIDTH) as usize;
            let end = start + (CHANNELS * WIDTH) as usize;
            raw_data[start..end].fill(55);
        }

        for i in 0..WIDTH as usize {
            // Map -120 to 0 dBFS to a value between 0 and 255
            let l = (-1. * fft_result.log_magnitudes[i]) as usize;
            raw_data[index..index + 3].copy_from_slice(&self.color_map[l]);
            index += 3;

            // Draw the amplitude spectrum.
            if i % 4 == 0 {
                let average_amplitude = fft_result.log_magnitudes[i..i + 4]
                    .into_iter()
                    .sum::<f64>()
                    / 4.0
                    * -2.0;
                let mut offset =
                    (((SPECTRUM_OFFSET + average_amplitude as u32) * WIDTH
                        + i as u32)
                        * CHANNELS) as usize;
                for _ in 0..4 {
                    raw_data[offset..offset + 3]
                        .copy_from_slice(&[210, 0, 120]);
                    offset += 3;
                }
            }
        }
    }

    fn render(
        &mut self,
        font_md: &Font,
        font_sm: &Font,
        current_frequency: u32,
    ) {
        self.render_video_buffer();
        self.canvas.set_draw_color(Color::RGB(40, 5, 55));
        self.canvas.fill_rect(Rect::new(0, 0, WIDTH, 30)).unwrap();
        self.canvas.fill_rect(Rect::new(0, 270, WIDTH, 30)).unwrap();
        self.canvas.set_draw_color(Color::RGBA(45, 225, 230, 50));
        self.canvas.fill_rect(Rect::new(0, 30, 70, 240)).unwrap();
        let freq_mhz = current_frequency as f64 / 1_000_000.;

        self.render_text_centered(
            &format!("{freq_mhz:.3} MHz").to_string(),
            (WIDTH / 2) as i32,
            285,
            &font_md,
        );
        for i in (20..101).step_by(20) {
            self.render_text_centered(
                &format!("-{i} dBFS").to_string(),
                35,
                SPECTRUM_OFFSET as i32 + 2 * i,
                &font_sm,
            );
        }
    }

    fn render_text_centered(
        &mut self,
        text: &str,
        x: i32,
        y: i32,
        font: &Font,
    ) {
        let surface = font
            .render(text)
            .blended(Color::RGBA(255, 255, 255, 255))
            .unwrap();
        let texture = self
            .texture_creator
            .create_texture_from_surface(&surface)
            .unwrap();
        let TextureQuery { width, height, .. } = texture.query();
        let rect = Rect::new(
            x - (width / 2) as i32,
            y - (height / 2) as i32,
            width,
            height,
        );
        self.canvas.copy(&texture, None, rect).unwrap();
    }

    fn render_video_buffer(&mut self) {
        let mut raw_data = self.video_buffer.lock().unwrap();
        let surface = sdl2::surface::Surface::from_data(
            raw_data.deref_mut().as_mut_slice(),
            WIDTH,
            HEIGHT,
            PITCH,
            PixelFormatEnum::RGB24,
        )
        .unwrap();
        let texture = self
            .texture_creator
            .create_texture_from_surface(&surface)
            .unwrap();
        let r = Rect::new(0, 0, WIDTH, HEIGHT);
        self.canvas.copy(&texture, None, r).unwrap();
    }
}

fn create_font<'a>(
    point_size: u16,
    ttf_context: &'a Sdl2TtfContext,
) -> Font<'a, 'a> {
    let mut font = ttf_context
        .load_font_from_rwops(
            RWops::from_bytes(ANDIKA_BOLD_TTF).unwrap(),
            point_size,
        )
        .unwrap();
    font.set_style(FontStyle::BOLD);

    return font;
}

fn interpolate_color_map(
    colors: Vec<[u8; 3]>,
    map_size: usize,
) -> Vec<[u8; 3]> {
    let mut result: Vec<[u8; 3]> = vec![[0, 0, 0]; map_size];
    // Steps between two colors
    let s = map_size as f64 / (colors.len() - 1) as f64;

    for i in 0..map_size {
        let start = colors[(i as f64 / s).floor() as usize];
        let end = colors[(i as f64 / s).ceil() as usize];
	// Offset from the start to the end color from 0 to 1
        let o = (i as f64 % s) / s;
        result[i] = [
            (start[0] as f64 + (end[0] as f64 - start[0] as f64) * o) as u8,
            (start[1] as f64 + (end[1] as f64 - start[1] as f64) * o) as u8,
            (start[2] as f64 + (end[2] as f64 - start[2] as f64) * o) as u8,
        ];
    }

    return result;
}

/// Rolls the buffer at d fields over the specified axis and fills the remaining
/// space with zeros.
fn roll(buf: &mut [u8], shape: Vec<u32>, axis: usize, d: i32) {
    let mut offset = d.abs() as usize;
    let mut iterations = 1;
    for i in axis..shape.len() {
        offset *= shape[i] as usize;
    }
    for i in 0..(axis - 1) {
        iterations *= shape[i] as usize;
    }

    let len = shape.iter().product::<u32>() as usize;
    let chunk_size = len / iterations;
    for i in 0..iterations {
        let start = chunk_size * i;
        let end = chunk_size * (i + 1);
        if d < 0 {
            buf.copy_within((start + offset)..end, start);
            buf[(end - offset)..end].fill(0);
        } else {
            buf.copy_within(start..(end - offset), start + offset);
            buf[start..(start + offset)].fill(0);
        }
    }
}
