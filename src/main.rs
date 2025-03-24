use rustfft::{num_complex::Complex, Fft, FftPlanner};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::{Color, PixelFormatEnum};
use std::ops::DerefMut;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

const WIDTH: u32 = 1024;
const HEIGHT: u32 = 800;
const CHANNELS: u32 = 3;
const BUF_SIZE: usize = (WIDTH * HEIGHT * CHANNELS) as usize;
const FFT_SIZE: usize = WIDTH as usize;
const PITCH: u32 = WIDTH * CHANNELS;

struct WaterfallDemo {
    control_thread: Option<thread::JoinHandle<()>>,
    should_stop: Arc<AtomicBool>,
    video_buffer: Arc<Mutex<Vec<u8>>>,
}

fn shift<T>(buf: &mut [T], shape: Vec<u32>, axis: usize, d: i32)
where
    T: Copy + std::default::Default,
{
    let mut offset = d.abs() as usize;
    for i in axis..shape.len() {
        offset *= shape[i] as usize;
    }

    let len = shape.iter().product::<u32>() as usize;
    let mut temp = vec![T::default(); offset];

    if d < 0 {
        temp.copy_from_slice(&buf[0..offset]);
        buf.copy_within(offset..len, 0);
        buf[len - offset..len].copy_from_slice(&temp);
    } else {
        temp.copy_from_slice(&buf[len - offset..len]);
        buf.copy_within(0..(len - offset), offset);
        buf[0..offset].copy_from_slice(&temp);
    }
}

fn roll(buf: &mut [u8], shape: Vec<u32>, axis: usize, d: i32) {
    let mut offset = d.abs() as usize;
    for i in axis..shape.len() {
        offset *= shape[i] as usize;
    }

    let len = shape.iter().product::<u32>() as usize;

    if d < 0 {
        buf.copy_within(offset..len, 0);
        buf[(len - offset)..len].fill(0);
    } else {
        buf.copy_within(0..(len - offset), offset);
        buf[0..offset].fill(0);
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
            control_thread: None,
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
        let should_stop = self.should_stop.clone();
        let video_buffer = self.video_buffer.clone();
        self.control_thread = Some(thread::spawn(move || {
            let (mut ctl, mut reader) =
                rtlsdr_mt::open(0).expect("Could not open RTL-SDR device at index 0.");
            ctl.set_sample_rate(2_400_000).unwrap();
            ctl.set_center_freq(100_000_000).unwrap();

            let reader_thread = thread::spawn(move || {
                let mut planner: FftPlanner<f64> = FftPlanner::new();
                let fft = planner.plan_fft_forward(FFT_SIZE);
                reader
                    .read_async(1, 2048, |buf| {
                        work_fft(buf, fft.clone(), video_buffer.clone());
                        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 60));
                    })
                    .unwrap();
            });

            while !should_stop.load(Ordering::Relaxed) {
                thread::sleep(Duration::new(0, 1_000_000_000u32 / 30));
            }
            ctl.cancel_async_read();
            reader_thread.join().unwrap();
        }));
    }

    pub fn start_sdl2_window(&self) {
        let sdl_context = sdl2::init().unwrap();
        let video_subsystem = sdl_context.video().unwrap();

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
                canvas.copy(&texture, None, None).unwrap();
            }

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
