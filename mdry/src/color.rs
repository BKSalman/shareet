#[derive(Debug, Clone, Copy)]
pub struct Color {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

impl Color {
    pub fn hex(hex: &str) -> Option<Self> {
        if let Some(hex) = hex.strip_prefix("#") {
            if hex.len() != 6 {
                return None;
            }

            if hex.chars().any(|c| !c.is_ascii_hexdigit()) {
                return None;
            }

            let hex: Vec<_> = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
                .collect();

            return Some(Self {
                r: hex[0],
                g: hex[1],
                b: hex[2],
                a: 255,
            });
        }

        None
    }
    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
    pub fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, a: 255 }
    }

    pub fn rgba_f32(&self) -> [f32; 4] {
        [
            self.r as f32 / 255.,
            self.g as f32 / 255.,
            self.b as f32 / 255.,
            self.a as f32 / 255.,
        ]
    }

    pub fn rgba_f64(&self) -> [f64; 4] {
        [
            self.r as f64 / 255.,
            self.g as f64 / 255.,
            self.b as f64 / 255.,
            self.a as f64 / 255.,
        ]
    }

    pub fn rgb_f32(&self) -> [f32; 3] {
        [
            self.r as f32 / 255.,
            self.g as f32 / 255.,
            self.b as f32 / 255.,
        ]
    }
}

impl Into<wgpu::Color> for Color {
    fn into(self) -> wgpu::Color {
        let color = self.rgba_f64();
        wgpu::Color {
            r: color[0],
            g: color[1],
            b: color[2],
            a: color[3],
        }
    }
}

impl Into<glyphon::Color> for Color {
    fn into(self) -> glyphon::Color {
        glyphon::Color::rgba(self.r, self.g, self.b, self.a)
    }
}
