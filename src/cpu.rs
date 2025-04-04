use std::sync::Arc;

use num::Complex;
use winit::window::Window;

pub struct Cpu {
    pub context: softbuffer::Context<Arc<Window>>,
    pub surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
}

impl Cpu {
    pub fn new(window: Arc<Window>) -> Self {
        let context = softbuffer::Context::new(Arc::clone(&window)).unwrap();
        let surface = softbuffer::Surface::new(&context, window).unwrap();

        Cpu { context, surface }
    }
}

fn escape_time(c: Complex<f32>, limit: usize) -> Option<usize> {
    let mut z = Complex::<f32>::default();

    for i in 0..limit {
        if z.norm_sqr() >= 4.0 {
            return Some(i);
        }
        z = z * z + c;
    }

    None
}

fn pixel_to_view(
    pixel: (u32, u32),
    upper_left: Complex<f32>,
    view_resolution: (f32, f32),   // real and imaginary axes
    window_resolution: (u32, u32), // x and y axes
) -> Complex<f32> {
    Complex {
        re: upper_left.re
            + (pixel.0 as f32 * view_resolution.0 as f32 / window_resolution.0 as f32),
        im: upper_left.im
            - (pixel.1 as f32 * view_resolution.1 as f32 / window_resolution.1 as f32),
    }
}

pub fn render(
    pixels: &mut [u32],
    upper_left: Complex<f32>,
    view_resolution: (f32, f32),
    window_resolution: (u32, u32),
) {
    assert!(pixels.len() == window_resolution.0 as usize * window_resolution.1 as usize);

    for row in 0..window_resolution.1 {
        for column in 0..window_resolution.0 {
            let point = pixel_to_view(
                (column, row),
                upper_left,
                view_resolution,
                window_resolution,
            );

            let index = (row * window_resolution.0 + column) as usize;
            pixels[index] = match escape_time(point, 256) {
                None => 0,
                Some(count) => {
                    let count = count as u32;
                    // softbuffer data representation: https://docs.rs/softbuffer/latest/softbuffer/struct.Buffer.html#data-representation
                    // Shifting in the escape time for all color (RGB) channels.
                    count << 16 | count << 8 | count
                }
            }
        }
    }
}

// pub fn run() {
//     let upper_left = Complex { re: -1.2, im: 0.35 };
//     let lower_right = Complex { re: -1.0, im: 0.2 };
//     let mut pixels = vec![0; 4000 * 3000];

//     let bounds = (4000, 3000);
//     let threads = 8;
//     let rows_per_band = bounds.1 / threads + 1;

//     {
//         let bands: Vec<&mut [u8]> = pixels.chunks_mut(rows_per_band * bounds.0).collect();
//         crossbeam::scope(|spawner| {
//             for (i, band) in bands.into_iter().enumerate() {
//                 let top = rows_per_band * i;
//                 let height = band.len() / bounds.0;
//                 let band_bounds = (bounds.0, height);
//                 let band_upper_left = pixel_to_point(bounds, (0, top), upper_left, lower_right);
//                 let band_lower_right =
//                     pixel_to_point(bounds, (bounds.0, top + height), upper_left, lower_right);

//                 spawner.spawn(move |_| {
//                     render(band, band_bounds, band_upper_left, band_lower_right);
//                 });
//             }
//         })
//         .unwrap();
//     }
// }
