#[macro_use]
extern crate glium;
extern crate image;

use glium::glutin;
use glium::glutin::Event;

use glium::Surface;

use std::thread;

use std::collections::VecDeque;
use std::vec::Vec;

mod math;
use math::*;

use std::time::Instant;

fn default<T: Default>() -> T {
  Default::default()
}

#[derive(Copy, Clone)]
struct Vertex {
  position: V2,
}
implement_vertex!(Vertex, position);

type STR = &'static str;

static VERTEX_SHADER: STR = r"
  #version 140
  in vec2 position;
  void main() {
    gl_Position = vec4(position, 0.0, 1.0);
  }
";

static FRAGMENT_SHADER: STR = "
  #version 140
  out vec4 color;
  void main() {
    color = vec4(1.0, 0.0, 0.0, 1.0);
  }
";

#[derive(Default)]
struct Input {
  lcommand: bool,
  rcommand: bool,
}

impl Input {
  fn new() -> Self {
    default()
  }

  fn update(&mut self, event: &glutin::Event) {
    use glium::glutin::ElementState::*;
    use glium::glutin::VirtualKeyCode::*;

    match *event {
       Event::KeyboardInput(Pressed,  _, Some(LWin)) => self.lcommand = true,
       Event::KeyboardInput(Released, _, Some(LWin)) => self.lcommand = false,
       Event::KeyboardInput(Pressed,  _, Some(RWin)) => self.rcommand = true,
       Event::KeyboardInput(Released, _, Some(RWin)) => self.rcommand = false,
       _ => {}
    }
  }

  fn command(&self) -> bool {
    self.lcommand || self.rcommand
  }
}

fn save_to_file(image_data: RGBAImageData) {
  use std::fs::File;
  use std::path::Path;

  let pixels = {
    let mut v = Vec::with_capacity(image_data.data.len() * 4);
    for (a, b, c, d) in image_data.data {
        v.push(a);
        v.push(b);
        v.push(c);
        v.push(d);
    }
    v
  };

  let buffer = image::ImageBuffer::from_raw(
    image_data.width,
    image_data.height,
    pixels
  ).expect("screenshot: from_raw failed");

  let dynamic = image::DynamicImage::ImageRgba8(buffer).flipv();

  let mut destination = File::create(&Path::new("screenshot.png"))
    .expect("screenshot: create failed");

  dynamic.save(&mut destination, image::ImageFormat::PNG)
    .expect("screenshot: save failed");
}

// Container that holds image data as vector of (u8, u8, u8, u8).
// This is used to take data from PixelBuffer and move it to another thread with the least conversions done on main thread.
pub struct RGBAImageData {
  pub data: Vec<(u8, u8, u8, u8)>,
  pub width: u32,
  pub height: u32,
}

impl glium::texture::Texture2dDataSink<(u8, u8, u8, u8)> for RGBAImageData {
  fn from_raw(data: std::borrow::Cow<[(u8, u8, u8, u8)]>, width: u32, height: u32) -> Self {
    RGBAImageData {
      data: data.into_owned(),
      width: width,
      height: height
    }
  }
}

fn screenshot(facade: &glium::backend::Facade) {
  let image_data = facade.get_context().read_front_buffer();

  save_to_file(image_data);
}

struct AsyncScreenshotTask {
  pixel_buffer: glium::texture::pixel_buffer::PixelBuffer<(u8, u8, u8, u8)>
}

impl AsyncScreenshotTask {
  fn new(facade: &glium::backend::Facade) -> Self {
    // Get information about current framebuffer
    let dimensions = facade.get_context().get_framebuffer_dimensions();
    let rect = glium::Rect { left: 0, bottom: 0, width: dimensions.0, height: dimensions.1 };
    let blit_target = glium::BlitTarget { left: 0, bottom: 0, width: dimensions.0 as i32, height: dimensions.1 as i32 };

    // Create temporary texture and blit the default front buffer to it
    let texture = glium::texture::Texture2d::empty(facade, dimensions.0, dimensions.1).unwrap();
    let framebuffer = glium::framebuffer::SimpleFrameBuffer::new(facade, &texture).unwrap();
    framebuffer.blit_from_frame(&rect, &blit_target, glium::uniforms::MagnifySamplerFilter::Nearest);

    // Read it into new pixel buffer
    let pixel_buffer = texture.read_to_pixel_buffer();

    AsyncScreenshotTask {
      pixel_buffer: pixel_buffer,
    }
  }

  fn read_image_data(self) -> RGBAImageData {
    self.pixel_buffer.read_as_texture_2d().unwrap()
  }
}

fn main() {
  use glium::{DisplayBuild, Surface};
  let display = glutin::WindowBuilder::new().build_glium().unwrap();

  let vertex1 = Vertex { position: v2(-0.5, -0.5) };
  let vertex2 = Vertex { position: v2( 0.0,  0.5) };
  let vertex3 = Vertex { position: v2( 0.5, -0.25) };
  let shape = vec![vertex1, vertex2, vertex3];

  let vertex_buffer = glium::VertexBuffer::new(&display, &shape).unwrap();
  let indices = glium::index::NoIndices(glium::index::PrimitiveType::TrianglesList);

  let program = glium::Program::from_source(&display, VERTEX_SHADER, FRAGMENT_SHADER, None).unwrap();

  let mut input = Input::new();

  let mut frame: u64 = 0;

  let mut frame_times = &mut [0; 60];

  // Amount of frames to wait for the pixel data to arrive from GPU
  const SCREENSHOT_FRAME_DELAY: u64 = 5; // Tune this based on your requirements. Bigger value means the screenshot will be picked up later. If the value is too small, the main thread will block waiting for the image.
  let mut screenshot_tasks = VecDeque::<(u64, AsyncScreenshotTask)>::new();

  loop {
    let start = Instant::now();

    let mut target = display.draw();
    target.clear_color(0.0, 0.0, 1.0, 1.0);

    let draw_result = target.draw(
      &vertex_buffer,
      &indices, &program,
      &glium::uniforms::EmptyUniforms,
      &default()
    );

    if let Err(error) = draw_result {
      panic!("draw failed: {}", error);
    }

    if let Err(error) = target.finish() {
      panic!("finish failed: {}", error);
    }

    for event in display.poll_events() {
      use glium::glutin::Event::*;
      use glium::glutin::ElementState::*;
      use glium::glutin::VirtualKeyCode::*;

      input.update(&event);

      match event {
        Closed                             => return,
        KeyboardInput(Pressed, _, Some(Q)) => if input.command() { return },
        KeyboardInput(Pressed, _, Some(S)) => screenshot(&display),
        KeyboardInput(Pressed, _, Some(A)) => screenshot_tasks.push_back((frame + SCREENSHOT_FRAME_DELAY, AsyncScreenshotTask::new(&display))),
        _                                  => {}
      }
    }

    // Check if there are any screenshots queue for pickup on this frame
    if screenshot_tasks.front().map(|p| p.0) == Some(frame) {
      let (_, task) = screenshot_tasks.pop_front().unwrap();

      let image_data = task.read_image_data();

      thread::spawn(move || {
        save_to_file(image_data);
      });
    }

    let elapsed = start.elapsed();
    let ms = elapsed.as_secs() * 1000
      + (elapsed.subsec_nanos() as u64) / 1000000;
    let i = frame as usize % frame_times.len();
    frame_times[i] = ms;

    if frame % 60 == 0 {
      let mut sum = 0u64;
      let mut max = 0u64;
      for time in frame_times.iter() {
        sum += *time;
        max = std::cmp::max(max, *time);
      }
      let average = sum as f64 / frame_times.len() as f64;
      println!("{}ms/{}ms AVE/MAX", average as u64, max);
    }

    frame += 1;
  }
}
