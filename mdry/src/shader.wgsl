struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

struct UniformBuffer {
    screen_size: vec2<f32>,
    // Uniform buffers need to be at least 16 bytes in WebGL.
    // See https://github.com/gfx-rs/wgpu/issues/2072
    _padding: vec2<u32>,
};

@group(0) @binding(0) var<uniform> uniform_buffer: UniformBuffer;

fn position_from_screen(screen_pos: vec3<f32>) -> vec4<f32> {
    return vec4<f32>(
        2.0 * screen_pos.x / uniform_buffer.screen_size.x - 1.0,
        1.0 - 2.0 * screen_pos.y / uniform_buffer.screen_size.y,
        0.0,
        1.0,
    );
}

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = position_from_screen(model.position);
    out.color = model.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(pow(in.color, vec3(2.2/1.0)), 1.0);
}

