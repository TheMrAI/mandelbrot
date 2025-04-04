
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

struct Settings {
    upper_left: vec2f,
    view_width: f32,
    view_height: f32,
    window: vec2f,
};

@group(0) @binding(0) var<uniform> settings: Settings;

@fragment
fn fs_main(@builtin(position) position: vec4f) -> @location(0) vec4<f32> {
    // The position scaling codes might look a bit confusing.
    // What we are doing is taking a pixel position and then transforming it into the mandelbrot space.
    // This would be written down as (position.x / settings.window.x) * settings.view_width, which is equivalent
    // to position.x * settings.view_width / settings.window.x. 
    let point = vec2f(settings.upper_left.x + (position.x * settings.view_width / settings.window.x),
        settings.upper_left.y - (position.y * settings.view_height / settings.window.y));

    let escapes_in = escape_time(point, 256u);
    let intensity: f32 = f32(escapes_in) / 255.0;
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
    // for now zero signals that the iteration doesn't escape
    return 0u;
}