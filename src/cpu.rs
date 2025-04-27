use std::thread;

use num::Complex;
use winit::dpi::PhysicalSize;

fn escape_time(c: Complex<f32>, limit: usize) -> Option<usize> {
    assert!(limit <= 256, "Limit must not exceed 256.");
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
    view_resolution: &PhysicalSize<f32>,
    window_resolution: &PhysicalSize<u32>,
) -> Complex<f32> {
    Complex {
        re: upper_left.re
            + (pixel.0 as f32 * view_resolution.width / window_resolution.width as f32),
        im: upper_left.im
            - (pixel.1 as f32 * view_resolution.height / window_resolution.height as f32),
    }
}

pub fn render(
    pixels: &mut [u32],
    upper_left: Complex<f32>,
    view_resolution: &PhysicalSize<f32>,
    window_resolution: &PhysicalSize<u32>,
) {
    debug_assert!(pixels.len() == window_resolution.width as usize * window_resolution.height as usize);

    let thread_count = match std::thread::available_parallelism() {
        Ok(parallelism) => parallelism.get(),
        Err(_) => 4,
    };
    let band_height = std::cmp::max(window_resolution.height / thread_count as u32, 50);

    {
        let bands = pixels
            .chunks_mut((window_resolution.width * band_height) as usize)
            .collect::<Vec<&mut [u32]>>();

        fn render_chunk(
            band: &mut [u32],
            band_i: u32,
            band_height: u32,
            upper_left: Complex<f32>,
            view_resolution: &PhysicalSize<f32>,
            window_resolution: &PhysicalSize<u32>,
        ) {
            let start_row = band_height * band_i;
            let height = band.len() as u32 / window_resolution.width;
            let end_row = start_row + height;

            for row in start_row..end_row {
                for column in 0..window_resolution.width {
                    let point = pixel_to_view(
                        (column, row),
                        upper_left,
                        view_resolution,
                        window_resolution,
                    );
                    // within the given band
                    let pixel_index = (row - start_row) * window_resolution.width + column;
                    band[pixel_index as usize] = match escape_time(point, 256) {
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

        thread::scope(|s| {
            let last_band = bands.len() - 1;
            for (band_i, band) in bands.into_iter().enumerate() {
                // for all but the last chunk we spawn a new thread
                // for the last we already have the current thread available
                if band_i != last_band {
                    s.spawn(move || {
                        render_chunk(
                            band,
                            band_i as u32,
                            band_height,
                            upper_left,
                            view_resolution,
                            window_resolution,
                        )
                    });
                } else {
                    render_chunk(
                        band,
                        band_i as u32,
                        band_height,
                        upper_left,
                        view_resolution,
                        window_resolution,
                    )
                }
            }
        });
    }
}
