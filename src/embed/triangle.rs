use crate::embed::map_triangle::Vertex;
use colorgrad::{BlendMode, Color, Gradient, Interpolation};
use ril::prelude::*;
use ril::Rgba;
use std::default::Default;
use std::ops::DerefMut;

#[derive(Clone, Debug)]
pub struct VertexWithColor<P: Pixel> {
    pub vertex: Vertex,
    pub color: P,
}

impl<P: Pixel> Default for VertexWithColor<P> {
    fn default() -> Self {
        Self {
            vertex: Vertex::default(),
            color: P::default(),
        }
    }
}
#[allow(dead_code)]
impl<P: Pixel> VertexWithColor<P> {
    pub fn new(vertex: Vertex, color: P) -> Self {
        Self { vertex, color }
    }
}

#[derive(Clone, Debug)]
pub struct TriangleGradient<P: Pixel> {
    vertices: [VertexWithColor<P>; 3],
    blend_mode: Option<BlendMode>,
    interpolation: Option<Interpolation>,
    overlay: Option<OverlayMode>,
}

impl<P: Pixel> Default for TriangleGradient<P> {
    fn default() -> Self {
        Self {
            vertices: [
                VertexWithColor::default(),
                VertexWithColor::default(),
                VertexWithColor::default(),
            ],
            blend_mode: None,
            interpolation: None,
            overlay: None,
        }
    }
}

#[allow(dead_code)]
impl<P: Pixel> TriangleGradient<P> {
    pub fn new(v1: VertexWithColor<P>, v2: VertexWithColor<P>, v3: VertexWithColor<P>) -> Self {
        Self::default().with_vertices(v1, v2, v3)
    }

    pub fn with_vertices(
        mut self,
        v1: VertexWithColor<P>,
        v2: VertexWithColor<P>,
        v3: VertexWithColor<P>,
    ) -> Self {
        self.vertices = [v1, v2, v3];
        self.vertices
            .sort_unstable_by(|v1, v2| v1.vertex.y.cmp(&v2.vertex.y));

        self
    }

    pub const fn with_blend_mode(mut self, mode: BlendMode) -> Self {
        self.blend_mode = Some(mode);
        self
    }

    pub const fn with_interpolation(mut self, mode: Interpolation) -> Self {
        self.interpolation = Some(mode);
        self
    }

    pub const fn with_overlay_mode(mut self, mode: OverlayMode) -> Self {
        self.overlay = Some(mode);
        self
    }

    fn color_from_gradient(t: f64, gradient: &Gradient) -> P {
        let (r, g, b, a) = gradient.at(t).to_linear_rgba_u8();
        P::from_raw_parts(ColorType::Rgba, 8, &[r, g, b, a]).unwrap_or_default()
    }
}

#[allow(dead_code)]
impl<P: Pixel> Draw<P> for TriangleGradient<P> {
    fn draw<I: DerefMut<Target = Image<P>>>(&self, mut image: I) {
        let blend_mode = self.blend_mode.unwrap_or(BlendMode::LinearRgb);
        let interpolation = self.interpolation.unwrap_or(Interpolation::Linear);
        let overlay = self.overlay.unwrap_or(OverlayMode::Replace);

        let rgba_colors: Vec<Rgba> = self.vertices.iter().map(|v| v.color.as_rgba()).collect();

        let Vertex { x: x0, y: y0 } = self.vertices[0].vertex;
        let Vertex { x: x1, y: y1 } = self.vertices[1].vertex;
        let Vertex { x: x2, y: y2 } = self.vertices[2].vertex;

        let edge_02 = interpolate_u32(y0, x0, y2, x2, true);
        let mut edge_12 = interpolate_u32(y1, x1, y2, x2, true);
        let mut edge_012 = interpolate_u32(y0, x0, y1, x1, false);

        let gradient_012 = colorgrad::CustomGradient::new()
            .domain(&[y0 as f64, y1 as f64, y2 as f64])
            .colors(&[
                Color::from_rgba8(
                    rgba_colors[0].r,
                    rgba_colors[0].g,
                    rgba_colors[0].b,
                    rgba_colors[0].a,
                ),
                Color::from_rgba8(
                    rgba_colors[1].r,
                    rgba_colors[1].g,
                    rgba_colors[1].b,
                    rgba_colors[1].a,
                ),
                Color::from_rgba8(
                    rgba_colors[2].r,
                    rgba_colors[2].g,
                    rgba_colors[2].b,
                    rgba_colors[2].a,
                ),
            ])
            .mode(blend_mode)
            .interpolation(interpolation)
            .build()
            .unwrap();

        let gradient_02 = colorgrad::CustomGradient::new()
            .domain(&[y0 as f64, y2 as f64])
            .colors(&[
                Color::from_rgba8(
                    rgba_colors[0].r,
                    rgba_colors[0].g,
                    rgba_colors[0].b,
                    rgba_colors[0].a,
                ),
                Color::from_rgba8(
                    rgba_colors[2].r,
                    rgba_colors[2].g,
                    rgba_colors[2].b,
                    rgba_colors[2].a,
                ),
            ])
            .mode(blend_mode)
            .interpolation(interpolation)
            .build()
            .unwrap();

        edge_012.append(&mut edge_12);

        let middle_idx = (edge_012.len() as f64 / 2.0).floor() as usize;
        let (x_left, x_right, gradient_left, gradient_right) =
            if edge_02.get(middle_idx) < edge_012.get(middle_idx) {
                (&edge_02, &edge_012, &gradient_02, &gradient_012)
            } else {
                (&edge_012, &edge_02, &gradient_012, &gradient_02)
            };

        for (idx, left) in x_left.iter().enumerate() {
            let l_color = Self::color_from_gradient(y0 as f64 + idx as f64, gradient_left);
            let r_color = Self::color_from_gradient(y0 as f64 + idx as f64, gradient_right);

            let gradient = LinearGradient::new()
                .with_color(l_color)
                .with_color(r_color)
                .with_blend_mode(blend_mode)
                .with_interpolation(interpolation);

            image.draw(
                &Rectangle::from_bounding_box(
                    *left,
                    y0 + idx as u32,
                    x_right.get(idx).unwrap_or(left) + 1,
                    y0 + idx as u32 + 1,
                )
                .with_fill(gradient)
                .with_overlay_mode(overlay),
            )
        }
    }
}

// Interpolate dependent values d0->d1 over independent values i0->i1
#[allow(dead_code)]
fn interpolate_f64(i0: u32, d0: f64, i1: u32, d1: f64, with_last: bool) -> Vec<f64> {
    if i0 == i1 {
        return vec![d0];
    }

    let a = (d1 as i32 - d0 as i32) as f64 / (i1 as i32 - i0 as i32) as f64;

    if with_last {
        (i0..=i1)
            .map(|i| d0 + (i as i32 - i0 as i32) as f64 * a)
            .collect()
    } else {
        (i0..i1)
            .map(|i| d0 + (i as i32 - i0 as i32) as f64 * a)
            .collect()
    }
}

fn interpolate_u32(i0: u32, d0: u32, i1: u32, d1: u32, with_last: bool) -> Vec<u32> {
    if i0 == i1 {
        return vec![d0];
    }

    let a = (d1 as i32 - d0 as i32) as f64 / (i1 as i32 - i0 as i32) as f64;

    if with_last {
        (i0..=i1)
            .map(|i| (d0 as f64 + (i as i32 - i0 as i32) as f64 * a).round() as u32)
            .collect()
    } else {
        (i0..i1)
            .map(|i| (d0 as f64 + (i as i32 - i0 as i32) as f64 * a).round() as u32)
            .collect()
    }
}
