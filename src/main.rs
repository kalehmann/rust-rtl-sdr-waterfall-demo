use rustfft::{num_complex::Complex, Fft, FftPlanner};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::{Color, PixelFormatEnum};
use sdl2::rect::Rect;
use sdl2::rwops::RWops;
use std::ops::DerefMut;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const ANDIKA_BOLD_TTF: &[u8] = include_bytes!("../assets/Andika/Andika-Bold.ttf");
const WIDTH: u32 = 1024;
const HEIGHT: u32 = 700;
const CHANNELS: u32 = 3;
const BUF_SIZE: usize = (WIDTH * HEIGHT * CHANNELS) as usize;
const FFT_SIZE: usize = WIDTH as usize;
const PITCH: u32 = WIDTH * CHANNELS;

struct WaterfallDemo {
    center_frequency: Arc<AtomicU32>,
    control_thread: Option<thread::JoinHandle<()>>,
    sample_rate: Arc<AtomicU32>,
    should_stop: Arc<AtomicBool>,
    video_buffer: Arc<Mutex<Vec<u8>>>,
}

fn start_reader_thread(
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

fn render_text_centered<S, T>(
    text: &str,
    x: i32,
    y: i32,
    font: &sdl2::ttf::Font,
    canvas: &mut sdl2::render::Canvas<S>,
    texture_creator: &sdl2::render::TextureCreator<T>,
) where
    S: sdl2::render::RenderTarget,
{
    let surface = font
        .render(text)
        .blended(Color::RGBA(255, 255, 255, 255))
        .unwrap();
    let texture = texture_creator
        .create_texture_from_surface(&surface)
        .unwrap();
    let sdl2::render::TextureQuery { width, height, .. } = texture.query();
    let rect = Rect::new(
        x - (width / 2) as i32,
        y - (height / 2) as i32,
        width,
        height,
    );
    canvas.copy(&texture, None, rect).unwrap();
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

fn work_fft(buf: &[u8], fft: Arc<dyn Fft<f64>>, video_buffer: Arc<Mutex<Vec<u8>>>) {
    let mut raw_data = video_buffer.lock().unwrap();

    roll(&mut raw_data, vec![HEIGHT, WIDTH, CHANNELS], 1, -1);

    let mut samples: [Complex<f64>; FFT_SIZE] = buf
        .chunks(2)
        .map(|pair| Complex {
            re: f64::from(pair[0]),
            im: f64::from(pair[1]),
        })
        .collect::<Vec<Complex<f64>>>()
        .try_into()
        .unwrap();
    fft.process(&mut samples);

    let mut magnitudes = samples.map(|c| c.norm());
    shift(&mut magnitudes, vec![WIDTH], 1, (WIDTH / 2) as i32);
    let mut index = (WIDTH * (HEIGHT - 1) * CHANNELS) as usize;
    for i in 0usize..FFT_SIZE {
        let logmag = (10.0 * magnitudes[i].powi(2).log10()) as i32;
        for _ in 0..CHANNELS {
            raw_data[index] = (100 + logmag) as u8;
            index += 1;
        }
    }
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

    pub fn finish(&mut self) {
        self.should_stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.control_thread.take() {
            thread.join().unwrap();
        }
        self.should_stop.store(false, Ordering::Relaxed);
        self.control_thread = None;
    }

    pub fn start_control_thread(&mut self) {
        let center_frequency = self.center_frequency.clone();
        let sample_rate = self.sample_rate.clone();
        let should_stop = self.should_stop.clone();
        let video_buffer = self.video_buffer.clone();

        self.control_thread = Some(thread::spawn(move || {
            let (mut ctl, reader) =
                rtlsdr_mt::open(0).expect("Could not open RTL-SDR device at index 0.");
            ctl.set_sample_rate(sample_rate.load(Ordering::Relaxed))
                .unwrap();
            ctl.set_center_freq(center_frequency.load(Ordering::Relaxed))
                .unwrap();

            let reader_thread =
                start_reader_thread(reader, should_stop.clone(), video_buffer.clone());

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

    pub fn start_sdl2_window(&self) {
        let sdl_context = sdl2::init().unwrap();
        let video_subsystem = sdl_context.video().unwrap();
        let ttf_context = sdl2::ttf::init().map_err(|e| e.to_string()).unwrap();
        let mut font = ttf_context
            .load_font_from_rwops(RWops::from_bytes(ANDIKA_BOLD_TTF).unwrap(), 32)
            .unwrap();
        font.set_style(sdl2::ttf::FontStyle::BOLD);

        let window = video_subsystem
            .window("Rust RTL-SDR waterfall demo", WIDTH, HEIGHT + 100)
            .position_centered()
            .build()
            .unwrap();
        let mut canvas = window.into_canvas().build().unwrap();
        let texture_creator = canvas.texture_creator();

        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        canvas.present();
        let mut event_pump = sdl_context.event_pump().unwrap();

        'running: loop {
            canvas.set_draw_color(Color::RGB(0, 0, 0));
            canvas.clear();
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
                        self.center_frequency.store(
                            self.center_frequency.load(Ordering::Relaxed) - 100_000,
                            Ordering::Relaxed,
                        );
                    }
                    Event::KeyDown {
                        keycode: Some(Keycode::Right),
                        ..
                    } => {
                        self.center_frequency.store(
                            self.center_frequency.load(Ordering::Relaxed) + 100_000,
                            Ordering::Relaxed,
                        );
                    }
                    _ => {}
                }
            }

            {
                let mut raw_data = self.video_buffer.lock().unwrap();
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
                canvas.copy(&texture, r, r).unwrap();
            }
            let freq_mhz = (self.center_frequency.load(Ordering::Relaxed) as f64) / 1_000_000f64;
            render_text_centered(
                &format!("{freq_mhz:.3} MHz").to_string(),
                (WIDTH / 2) as i32,
                750,
                &font,
                &mut canvas,
                &texture_creator,
            );

            canvas.present();
            thread::sleep(Duration::new(0, 1_000_000_000u32 / 30));
        }
    }
}

fn main() {
    let mut demo = WaterfallDemo::new();
    demo.start_control_thread();
    demo.start_sdl2_window();
    demo.finish();
}
