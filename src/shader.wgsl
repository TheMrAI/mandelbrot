
struct Settings {
    upper_left: vec2f,
    view_width: f32,
    view_height: f32,
    window: vec2f,
};

@group(0)
@binding(0)
var<uniform> settings: Settings;

@group(0)
@binding(1)
var texture: texture_storage_2d<rgba8unorm, write>;

@compute
@workgroup_size(1)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    // TODO what is workgroup_size 1, invocation id etc???
    // The position scaling codes might look a bit confusing.
    // What we are doing is taking a pixel position and then transforming it into the mandelbrot space.
    // This would be written down as (id.x / settings.window.x) * settings.view_width, which is equivalent
    // to id.x * settings.view_width / settings.window.x. 
    let point = vec2f(settings.upper_left.x + (f32(id.x) * settings.view_width / settings.window.x),
        settings.upper_left.y - (f32(id.y) * settings.view_height / settings.window.y));

    let escapes_in = escape_time(point, 256u);
    let intensity: f32 = f32(escapes_in) / 255.0;

    textureStore(texture, vec2(i32(id.x), i32(id.y)), vec4(intensity, intensity, intensity, 1.0));
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