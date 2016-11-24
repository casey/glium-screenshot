use super::glium;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct V2 {
  pub x: f32,
  pub y: f32,
}

unsafe impl glium::vertex::Attribute for V2 {
  fn get_type() -> glium::vertex::AttributeType {
    glium::vertex::AttributeType::F32F32
  }
}

pub fn v2(x: f32, y: f32) -> V2 {
  V2 {
    x: x,
    y: y,
  }
}
