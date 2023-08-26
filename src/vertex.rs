/// Four triangles, in the top left, top right, bottom left, and bottom right quadrants of the screen.
pub const FOUR_TRIANGLES: [Vertex; 12] = [
    // Top left triangle
    Vertex {
        position: [-1.0, -1.0, 0.0],
        normal: [0.0, 0.0, 1.0],
        tex_coords: [0.0, 1.0],
    },
    Vertex {
        position: [-1.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
        tex_coords: [0.0, 0.5],
    },
    Vertex {
        position: [0.0, 1.0, 0.0],
        normal: [0.0, 0.0, 1.0],
        tex_coords: [0.5, 1.0],
    },
    // Top right triangle
    Vertex {
        position: [0.0, 1.0, 0.0],
        normal: [0.0, 0.0, 1.0],
        tex_coords: [0.5, 1.0],
    },
    Vertex {
        position: [1.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
        tex_coords: [1.0, 0.5],
    },
    Vertex {
        position: [1.0, -1.0, 0.0],
        normal: [0.0, 0.0, 1.0],
        tex_coords: [1.0, 1.0],
    },
    // Bottom left triangle
    Vertex {
        position: [-1.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
        tex_coords: [0.0, 0.5],
    },
    Vertex {
        position: [-1.0, 1.0, 0.0],
        normal: [0.0, 0.0, 1.0],
        tex_coords: [0.0, 0.0],
    },
    Vertex {
        position: [0.0, -1.0, 0.0],
        normal: [0.0, 0.0, 1.0],
        tex_coords: [0.5, 1.0],
    },
    // Bottom right triangle
    Vertex {
        position: [0.0, -1.0, 0.0],
        normal: [0.0, 0.0, 1.0],
        tex_coords: [0.5, 1.0],
    },
    Vertex {
        position: [1.0, 1.0, 0.0],
        normal: [0.0, 0.0, 1.0],
        tex_coords: [1.0, 0.0],
    },
    Vertex {
        position: [1.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
        tex_coords: [1.0, 0.5],
    },
];

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    position: [f32; 3],
    normal: [f32; 3],
    tex_coords: [f32; 2],
}

impl Vertex {
    const ATTRIBUTES: [wgpu::VertexAttribute; 3] = wgpu::vertex_attr_array![
        0 => Float32x3,
        1 => Float32x3,
        2 => Float32x2,
    ];
    pub const BUFFER_LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &Self::ATTRIBUTES,
    };
}
