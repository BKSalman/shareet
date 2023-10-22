use glyphon::{Resolution, SwashCache, TextArea, TextBounds};

pub struct TextRenderer {
    pub texts: Vec<Text>,
    pub renderer: glyphon::TextRenderer,
    pub cache: SwashCache,
    pub font_system: glyphon::FontSystem,
    pub atlas: glyphon::TextAtlas,
}

#[derive(Debug)]
pub struct Text {
    pub x: i32,
    pub y: i32,
    pub color: glyphon::Color,
    pub content: String,
    pub bounds: TextBounds,
    pub buffer: glyphon::Buffer,
}

impl TextRenderer {
    pub fn add_text(&mut self, text: Text) {
        self.texts.push(text);
    }
    pub fn resize(&mut self, width: f32, height: f32, aspect_ratio: f32) {
        for text in self.texts.iter_mut() {
            text.buffer.set_size(
                &mut self.font_system,
                width * aspect_ratio,
                height * aspect_ratio,
            );
        }
    }
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        texts: &[Text],
    ) -> Result<(), wgpu::SurfaceError> {
        let text_areas = texts.iter().map(|t| TextArea {
            buffer: &t.buffer,
            left: t.x as f32,
            top: t.y as f32,
            scale: 1.0,
            bounds: t.bounds,
            default_color: t.color,
        });

        self.renderer
            .prepare(
                device,
                queue,
                &mut self.font_system,
                &mut self.atlas,
                Resolution { width, height },
                text_areas,
                &mut self.cache,
            )
            .unwrap();
        Ok(())
    }

    pub fn render<'rp>(
        &'rp self,
        render_pass: &mut wgpu::RenderPass<'rp>,
    ) -> Result<(), crate::Error> {
        self.renderer.render(&self.atlas, render_pass)?;

        Ok(())
    }

    pub fn trim(&mut self) {
        self.atlas.trim();
    }
}
