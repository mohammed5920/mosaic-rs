struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) screen_uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let uv = vec2<f32>(f32((in_vertex_index << 1u) & 2u), f32(in_vertex_index & 2u));
    out.clip_position = vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
    out.screen_uv = vec2<f32>(uv.x, 1.0 - uv.y); 
    return out;
}
