use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::{Color, PixelFormatEnum};
use std::time::Duration;

fn start_sdl2_window<F>(f: F)
where
    F: Fn(&mut [u8; 1024 * 800 * 3]),
{
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem
        .window("Rust RTL-SDR waterfall demo", 1024, 800)
        .position_centered()
        .build()
        .unwrap();
    let mut canvas = window.into_canvas().build().unwrap();
    let texture_creator = canvas.texture_creator();

    canvas.set_draw_color(Color::RGB(0, 0, 0));
    canvas.clear();
    canvas.present();
    let mut event_pump = sdl_context.event_pump().unwrap();

    let mut data: [u8; 1024 * 800 * 3] = vec![0u8; 1024 * 800 * 3]
        .into_iter()
        .collect::<Vec<u8>>()
        .try_into()
        .unwrap();

    'running: loop {
        canvas.set_draw_color(Color::RGB(0, 0, 0));
        canvas.clear();
        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. }
                | Event::KeyDown {
                    keycode: Some(Keycode::Escape),
                    ..
                } => break 'running,
                _ => {}
            }
        }

        f(&mut data);

        let surface = sdl2::surface::Surface::from_data(
            &mut data,
            1024,
            800,
            1024 * 3,
            PixelFormatEnum::RGB24,
        )
        .unwrap();
        let texture = texture_creator
            .create_texture_from_surface(&surface)
            .unwrap();
        canvas.copy(&texture, None, None).unwrap();

        canvas.present();
        ::std::thread::sleep(Duration::new(0, 1_000_000_000u32 / 30));
    }
}

fn main() {
    start_sdl2_window(|buffer| {
	// Draw the waterfall here
        buffer[1] = 255;
    });
}
