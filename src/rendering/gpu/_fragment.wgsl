struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) screen_uv: vec2<f32>,
};

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
  return vec4(1.);
}