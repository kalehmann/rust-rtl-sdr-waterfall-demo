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

mod demo;
mod dsp;
mod ui;

const WIDTH: u32 = 1024;
const HEIGHT: u32 = 800;
const CHANNELS: u32 = 3;
const BUF_SIZE: usize = (WIDTH * HEIGHT * CHANNELS) as usize;
const FFT_SIZE: usize = WIDTH as usize;
const PITCH: u32 = WIDTH * CHANNELS;

fn main() {
    demo::WaterfallDemo::new().run();
}
