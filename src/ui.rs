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

const SPECTRUM_OFFSET: u32 = 30;
const WATERFALL_OFFSET: u32 = 300;

use crate::dsp::FftResult;
use crate::{BUF_SIZE, CHANNELS, FFT_SIZE, HEIGHT, PITCH, WIDTH};
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::render::{Canvas, RenderTarget, TextureCreator, TextureQuery};
use sdl2::rwops::RWops;
use sdl2::ttf::{Font, FontStyle, Sdl2TtfContext};
use std::ops::DerefMut;
use std::sync::{Arc, Mutex};

const ANDIKA_BOLD_TTF: &[u8] =
    include_bytes!("../assets/Andika/Andika-Bold.ttf");

pub fn create_font<'a>(
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

pub fn update_video_buffer(
    video_buffer: Arc<Mutex<Vec<u8>>>,
    fft_result: FftResult,
) {
    let mut raw_data = video_buffer.lock().unwrap();
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
        let start = ((SPECTRUM_OFFSET + i * 10) * CHANNELS * WIDTH) as usize;
        let end = start + (CHANNELS * WIDTH) as usize;
        raw_data[start..end].fill(55);
    }

    for i in 0..FFT_SIZE {
        // Map -120 to 0 dBFS to a value between 0 and 255
        let val = (2.12 * (100.0 + fft_result.log_magnitudes[i])) as u8;
        raw_data[index..index + 3].copy_from_slice(&[val, val, val]);
        index += 3;

        // Draw the amplitude spectrum.
        if i % 4 == 0 {
            let average_amplitude =
                fft_result.log_magnitudes[i..i + 4].into_iter().sum::<f64>()
                    / 4.0
                    * -2.0;
            let mut offset = (((SPECTRUM_OFFSET + average_amplitude as u32)
                * WIDTH
                + i as u32)
                * CHANNELS) as usize;
            for _ in 0..4 {
                raw_data[offset..offset + 3].copy_from_slice(&[210, 0, 120]);
                offset += 3;
            }
        }
    }
}

pub fn render<S, T>(
    canvas: &mut Canvas<S>,
    texture_creator: &TextureCreator<T>,
    font_sm: &Font,
    font_md: &Font,
    freq_mhz: f64,
    video_buffer: Arc<Mutex<Vec<u8>>>,
) where
    S: RenderTarget,
{
    render_video_buffer(canvas, texture_creator, video_buffer);
    canvas.set_draw_color(Color::RGB(40, 5, 55));
    canvas.fill_rect(Rect::new(0, 0, WIDTH, 30)).unwrap();
    canvas.fill_rect(Rect::new(0, 270, WIDTH, 30)).unwrap();
    canvas.set_draw_color(Color::RGBA(45, 225, 230, 50));
    canvas.fill_rect(Rect::new(0, 30, 70, 240)).unwrap();

    render_text_centered(
        &format!("{freq_mhz:.3} MHz").to_string(),
        (WIDTH / 2) as i32,
        285,
        font_md,
        canvas,
        texture_creator,
    );
    for i in (20..101).step_by(20) {
        render_text_centered(
            &format!("-{i} dBFS").to_string(),
            35,
            SPECTRUM_OFFSET as i32 + 2 * i,
            font_sm,
            canvas,
            texture_creator,
        );
    }
}

fn render_video_buffer<S, T>(
    canvas: &mut Canvas<S>,
    texture_creator: &TextureCreator<T>,
    video_buffer: Arc<Mutex<Vec<u8>>>,
) where
    S: RenderTarget,
{
    let mut raw_data = video_buffer.lock().unwrap();
    let surface = sdl2::surface::Surface::from_data(
        raw_data.deref_mut().as_mut_slice(),
        WIDTH,
        HEIGHT,
        PITCH,
        PixelFormatEnum::RGB24,
    )
    .unwrap();
    let texture = texture_creator
        .create_texture_from_surface(&surface)
        .unwrap();
    let r = Rect::new(0, 0, WIDTH, HEIGHT);
    canvas.copy(&texture, None, r).unwrap();
}

fn render_text_centered<S, T>(
    text: &str,
    x: i32,
    y: i32,
    font: &Font,
    canvas: &mut Canvas<S>,
    texture_creator: &TextureCreator<T>,
) where
    S: RenderTarget,
{
    let surface = font
        .render(text)
        .blended(Color::RGBA(255, 255, 255, 255))
        .unwrap();
    let texture = texture_creator
        .create_texture_from_surface(&surface)
        .unwrap();
    let TextureQuery { width, height, .. } = texture.query();
    let rect = Rect::new(
        x - (width / 2) as i32,
        y - (height / 2) as i32,
        width,
        height,
    );
    canvas.copy(&texture, None, rect).unwrap();
}

/// Rolls the buffer at d fields over the specified axis and fills the remaining
/// space with zeros.
pub fn roll(buf: &mut [u8], shape: Vec<u32>, axis: usize, d: i32) {
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
