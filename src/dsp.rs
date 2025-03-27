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

use crate::{
    BUF_SIZE, CHANNELS, FFT_SIZE, HEIGHT, SPECTRUM_OFFSET, WATERFALL_OFFSET,
    WIDTH,
};
use rustfft::{num_complex::Complex, Fft, FftPlanner};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub fn start_reader_thread(
    mut reader: rtlsdr_mt::Reader,
    should_stop: Arc<AtomicBool>,
    video_buffer: Arc<Mutex<Vec<u8>>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let mut planner: FftPlanner<f64> = FftPlanner::new();
        let fft = planner.plan_fft_forward(FFT_SIZE);

        while !should_stop.load(Ordering::Relaxed) {
            reader
                .read_async(1, 2048, |buf| {
                    work_fft(buf, fft.clone(), video_buffer.clone());
                    thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
                })
                .unwrap();
        }
    })
}

pub fn work_fft(
    buf: &[u8],
    fft: Arc<dyn Fft<f64>>,
    video_buffer: Arc<Mutex<Vec<u8>>>,
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

    let mut samples: [Complex<f64>; FFT_SIZE] = buf
        .chunks(2)
        .map(|pair| Complex {
            re: (f64::from(pair[0]) - 127.0) / 127.0,
            im: (f64::from(pair[1]) - 127.0) / 127.0,
        })
        .collect::<Vec<Complex<f64>>>()
        .try_into()
        .unwrap();
    fft.process(&mut samples);

    let mut log_magnitudes = samples.map(|c| {
        // Clip values between 0 dBFS (=1) and -120 dBFS (=10^-12)
        10.0 * (c.norm_sqr() / (FFT_SIZE as f64).powi(2))
            .max(1e-12)
            .log10()
            .min(0.)
    });
    shift(
        &mut log_magnitudes,
        vec![FFT_SIZE as u32],
        1,
        (FFT_SIZE / 2) as i32,
    );

    // Draw the horizontal lines for the amplitude spectrum
    for i in (4..24).step_by(4) {
        let start = ((SPECTRUM_OFFSET + i * 10) * CHANNELS * WIDTH) as usize;
        let end = start + (CHANNELS * WIDTH) as usize;
        raw_data[start..end].fill(55);
    }

    for i in 0..FFT_SIZE {
        // Map -120 to 0 dBFS to a value between 0 and 255
        let val = (2.12 * (100.0 + log_magnitudes[i])) as u8;
        raw_data[index..index + 3].copy_from_slice(&[val, val, val]);
        index += 3;

        // Draw the amplitude spectrum.
        if i % 4 == 0 {
            let average_amplitude =
                log_magnitudes[i..i + 4].into_iter().sum::<f64>() / 4.0 * -2.0;
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

/// Shifts the buffer at d fields over the specified axis.
fn shift<T>(buf: &mut [T], shape: Vec<u32>, axis: usize, d: i32)
where
    T: Copy + std::default::Default,
{
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
    let mut temp = vec![T::default(); offset];

    for i in 0..iterations {
        let start = chunk_size * i;
        let end = chunk_size * (i + 1);
        if d < 0 {
            temp.copy_from_slice(&buf[start..(start + offset)]);
            buf.copy_within((start + offset)..end, start);
            buf[end - offset..end].copy_from_slice(&temp);
        } else {
            temp.copy_from_slice(&buf[(end - offset)..end]);
            buf.copy_within(start..(end - offset), start + offset);
            buf[start..(start + offset)].copy_from_slice(&temp);
        }
    }
}

/// Rolls the buffer at d fields over the specified axis and fills the remeining
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
