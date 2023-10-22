use crate::VertexColored;

#[derive(Debug, Clone)]
pub struct Mesh {
    pub indices: Vec<u32>,
    pub vertices: Vec<VertexColored>,
}

#[derive(Debug)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

pub struct Circle {
    pub x: i32,
    pub y: i32,
    pub radius: f32,
}

pub struct Triangle {
    pub a: (i32, i32),
    pub b: (i32, i32),
    pub c: (i32, i32),
}

pub enum Shape {
    Rect(Rect),
    Circle(Circle),
    Triangle(Triangle),
}
