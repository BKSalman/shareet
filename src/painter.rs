use crate::{
    shapes::{Mesh, Shape},
    VertexColored,
};

#[derive(Debug)]
pub struct Painter {
    meshes: Vec<Mesh>,
    current_x: u32,
    current_y: u32,
}

impl Painter {
    pub fn new() -> Self {
        Self {
            meshes: Vec::new(),
            current_x: 0,
            current_y: 0,
        }
    }

    pub fn add_shape_absolute(&mut self, shape: Shape, color: crate::Color) {
        let color = color.rgb_f32();
        match shape {
            Shape::Rect(rect) => {
                self.meshes.push(Mesh {
                    indices: vec![0, 1, 2, 0, 2, 3],
                    vertices: vec![
                        VertexColored {
                            position: [rect.x as f32, rect.y as f32, 0.],
                            color,
                        },
                        VertexColored {
                            position: [rect.x as f32, rect.y as f32 + rect.height as f32, 0.],
                            color,
                        },
                        VertexColored {
                            position: [
                                rect.x as f32 + rect.width as f32,
                                rect.y as f32 + rect.height as f32,
                                0.,
                            ],
                            color,
                        },
                        VertexColored {
                            position: [rect.x as f32 + rect.width as f32, rect.y as f32, 0.],
                            color,
                        },
                    ],
                });
            }
            Shape::Triangle(triangle) => self.meshes.push(Mesh {
                indices: vec![0, 1, 2],
                vertices: vec![
                    VertexColored {
                        position: [triangle.a.0 as f32, triangle.a.1 as f32, 0.],
                        color,
                    },
                    VertexColored {
                        position: [triangle.b.0 as f32, triangle.b.1 as f32, 0.],
                        color,
                    },
                    VertexColored {
                        position: [triangle.c.0 as f32, triangle.c.1 as f32, 0.],
                        color,
                    },
                ],
            }),
            Shape::Circle(circle) => {
                let (vertices, indices) =
                    create_circle_vertices(circle.radius, 30, color, circle.x, circle.y);
                self.meshes.push(Mesh { indices, vertices });
            }
        }
    }

    pub fn meshes(&self) -> &Vec<Mesh> {
        &self.meshes
    }
}

fn create_circle_vertices(
    radius: f32,
    num_segments: u32,
    color: [f32; 3],
    x: i32,
    y: i32,
) -> (Vec<VertexColored>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    // Add the center vertex
    vertices.push(VertexColored {
        position: [x as f32, y as f32, 0.0],
        color,
    });

    let angle_increment = 2.0 * std::f32::consts::PI / num_segments as f32;

    for i in 0..num_segments {
        let angle = i as f32 * angle_increment;
        let angle_x = radius * angle.cos();
        let angle_y = radius * angle.sin();
        vertices.push(VertexColored {
            position: [angle_x + x as f32, angle_y + y as f32, 0.],
            color,
        });
        indices.push(0); // Index of the center vertex
        indices.push(i + 1); // Index of the outer vertex
        indices.push((i + 1) % num_segments + 1); // Index of the next outer vertex
    }

    (vertices, indices)
}
