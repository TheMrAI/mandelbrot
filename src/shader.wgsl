
// We can do this to have the four vertices or make a buffer, fill it etc..
// This is simpler.
const vertices = array<vec4f, 4>(vec4f(-1.0, 1.0, 0.0, 1.0), vec4f(-1.0, -1.0, 0.0, 1.0), vec4f(1.0, 1.0, 0.0, 1.0), vec4f(1.0, -1.0, 0.0, 1.0));

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> @builtin(position) vec4f {
    var position: vec4f;
    // This is a weird trick, necessary as Naga doesn't allow indexing of arrays
    // with none constant values.
    if in_vertex_index == 0 {
        position = vertices[0];
    } else if in_vertex_index == 1 {
        position = vertices[1];
    } else if in_vertex_index == 2 {
        position = vertices[2];
    } else {
        position = vertices[3];
    }

    return position;
}

const upper_left = vec2f(-1.2, 0.35);
const lower_right = vec2f(-1.0, 0.2);
const width = lower_right.x - upper_left.x;
const height = upper_left.y - lower_right.y;
const bounds = vec2f(1024.0, 768.0);

// Complex {
//         re: upper_left.re + pixel.0 as f64 * width / bounds.0 as f64,
//         im: upper_left.im - pixel.1 as f64 * height / bounds.1 as f64,
//     }

@fragment
fn fs_main(@builtin(position) position: vec4f) -> @location(0) vec4<f32> {
    let point = vec2f(upper_left.x + position.x * width / bounds.x,
        upper_left.y - position.y * height / bounds.y);

    let escapes_in = escape_time(point, 255u);
    let intensity: f32 = f32(255 - escapes_in) / 255.0;
    return vec4f(intensity, intensity, intensity, 1.0);
}

fn complex_square(z: vec2f) -> vec2f {
    return vec2f(pow(z.x, 2.0) - pow(z.y, 2.0), 2.0 * z.x * z.y);
}

fn escape_time(c: vec2f, limit: u32) -> u32 {
    var z = vec2f(0.0, 0.0);

    for (var i = 0u; i < limit; i++) {
        let squared = z * z;
        if (squared.x + squared.y) >= 4.0 {
            return i;
        }
        z = complex_square(z) + c;
    }
    return 255u;
}