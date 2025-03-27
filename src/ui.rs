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

use crate::{HEIGHT, PITCH, SPECTRUM_OFFSET, WIDTH};
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
