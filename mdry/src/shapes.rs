use crate::VertexColored;

#[derive(Debug, Clone)]
pub struct Mesh {
    pub indices: Vec<u32>,
    pub vertices: Vec<VertexColored>,
}

impl Mesh {
    pub fn add_offset(mut self, offset: f32) -> Self {
        for vertex in self.vertices.iter_mut() {
            vertex.add_offset_mut(offset);
        }

        self
    }
}

#[derive(Debug, Clone)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: u32,
    pub height: u32,
    pub color: crate::color::Color,
}

#[derive(Debug)]
pub struct Circle {
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub color: crate::color::Color,
}

#[derive(Debug)]
pub struct Triangle {
    pub a: (f32, f32),
    pub b: (f32, f32),
    pub c: (f32, f32),
    pub color: crate::color::Color,
}

#[derive(Debug)]
pub enum Shape {
    Rect(Rect),
    Circle(Circle),
    Triangle(Triangle),
}
