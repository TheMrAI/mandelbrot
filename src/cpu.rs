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

fn escape_time(c: Complex<f64>, limit: usize) -> Option<usize> {
    let mut z = Complex::<f64>::default();

    for i in 0..limit {
        if z.norm_sqr() >= 4.0 {
            return Some(i);
        }
        z = z * z + c;
    }

    None
}

fn pixel_to_point(
    bounds: (usize, usize),
    pixel: (usize, usize),
    upper_left: Complex<f64>,
    lower_right: Complex<f64>,
) -> Complex<f64> {
    let (width, height) = (
        lower_right.re - upper_left.re,
        upper_left.im - lower_right.im,
    );
    Complex {
        re: upper_left.re + pixel.0 as f64 * width / bounds.0 as f64,
        im: upper_left.im - pixel.1 as f64 * height / bounds.1 as f64,
    }
}

pub fn render(
    pixels: &mut [u32],
    bounds: (usize, usize),
    upper_left: Complex<f64>,
    lower_right: Complex<f64>,
) {
    assert!(pixels.len() == bounds.0 * bounds.1);

    for row in 0..bounds.1 {
        for column in 0..bounds.0 {
            let point = pixel_to_point(bounds, (column, row), upper_left, lower_right);
            pixels[row * bounds.0 + column] = match escape_time(point, 255) {
                None => 0,
                Some(count) => {
                    let count = count as u32;
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
