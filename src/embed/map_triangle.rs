use std::ops::DerefMut;

use ril::{
    Draw, Font, HorizontalAnchor, Image, Line, OverlayMode, Polygon, Rgba, TextLayout, TextSegment,
    VerticalAnchor, WrapStyle,
};

use crate::discord::bot::beatleader::score::MapRating;
use crate::embed::clamp;
use crate::embed::font::{draw_text_segment, ROBOTO_FONT_FAMILY};

#[derive(Default, Clone, Debug)]
pub struct Vertex {
    pub x: u32,
    pub y: u32,
}

impl Vertex {
    pub fn new(x: u32, y: u32) -> Self {
        Self { x, y }
    }

    pub fn vector(a: &Vertex, b: &Vertex) -> Vertex {
        Vertex::new(b.x - a.x, b.y - a.y)
    }

    pub fn determinant(p: &Vertex, q: &Vertex) -> f64 {
        p.x as f64 * q.y as f64 - p.y as f64 * q.x as f64
    }
    pub fn magnitude(&self) -> f64 {
        self.x as f64 * self.x as f64 + self.y as f64 * self.y as f64
    }
}

#[derive(Clone, Debug)]
pub struct MapTriangle {
    position: Vertex,
    size: u32,
    map_rating: MapRating,
    overlay: Option<OverlayMode>,
}

impl MapTriangle {
    pub fn new(position: Vertex, size: u32) -> Self {
        Self {
            position,
            size,
            map_rating: MapRating::default(),
            overlay: None,
        }
    }

    #[allow(dead_code)]
    pub const fn with_position(mut self, position: Vertex) -> Self {
        self.position = position;
        self
    }

    #[allow(dead_code)]
    pub const fn with_size(mut self, size: u32) -> Self {
        self.size = size;
        self
    }

    #[allow(dead_code)]
    pub const fn with_map_rating(mut self, map_rating: MapRating) -> Self {
        self.map_rating = map_rating;
        self
    }

    #[allow(dead_code)]
    pub const fn with_overlay_mode(mut self, mode: OverlayMode) -> Self {
        self.overlay = Some(mode);
        self
    }
}

impl Default for MapTriangle {
    fn default() -> Self {
        Self {
            position: Vertex::default(),
            size: 60,
            map_rating: MapRating::default(),
            overlay: None,
        }
    }
}

impl Draw<Rgba> for MapTriangle {
    fn draw<I: DerefMut<Target = Image<Rgba>>>(&self, mut image: I) {
        let overlay = self.overlay.unwrap_or(OverlayMode::Replace);

        let border_color = Rgba::new(255, 255, 255, 96);

        let Vertex { x: x1, y: y1 } = self.position;
        let x2 = x1 + self.size;
        let x3 = x1 + self.size / 2;
        let height = f64::max((self.size as f64 * 0.866).round(), 1.0) as u32;
        let y2 = y1 + height;

        let vertex_center =
            get_triangle_circumcenter(&self.position, &Vertex::new(x2, y1), &Vertex::new(x3, y2))
                .unwrap_or(Vertex::new(x3, y1 + height / 2));

        let triangle = &Polygon::from_vertices(vec![
            get_vertex(
                &Vertex::new(x1, y1),
                &vertex_center,
                self.map_rating.get_tech_relative(),
            ),
            get_vertex(
                &Vertex::new(x2, y1),
                &vertex_center,
                self.map_rating.get_acc_relative(),
            ),
            get_vertex(
                &Vertex::new(x3, y2),
                &vertex_center,
                self.map_rating.get_pass_relative(),
            ),
        ])
        .with_overlay_mode(overlay)
        .with_fill(get_map_rating_color(&self.map_rating))
        .with_antialiased(true);

        image.draw(triangle);

        image.draw(
            &Line::new(
                (vertex_center.x - 1, vertex_center.y),
                (vertex_center.x + 2, vertex_center.y),
                Rgba::new(0, 0, 0, 0),
            )
            .with_thickness(1)
            .with_mode(OverlayMode::Replace),
        );
        image.draw(
            &Line::new(
                (vertex_center.x, vertex_center.y - 1),
                (vertex_center.x, vertex_center.y + 2),
                Rgba::new(0, 0, 0, 0),
            )
            .with_thickness(1)
            .with_mode(OverlayMode::Replace),
        );

        image.draw(
            &Line::new((x1, y1), (x2, y1), border_color)
                .with_thickness(1)
                .with_antialiased(true),
        );
        image.draw(
            &Line::new((x1, y1), (x3, y2), border_color)
                .with_thickness(1)
                .with_antialiased(true),
        );
        image.draw(
            &Line::new((x2, y1), (x3, y2), border_color)
                .with_thickness(1)
                .with_antialiased(true),
        );

        let roboto_font = &ROBOTO_FONT_FAMILY.fonts[0].font;
        let font_size = 12.0;

        draw_stars_at_point(
            &mut image,
            self.map_rating.tech,
            x1,
            y1,
            roboto_font,
            font_size,
            HorizontalAnchor::Right,
            VerticalAnchor::Top,
        );
        draw_stars_at_point(
            &mut image,
            self.map_rating.acc,
            x2,
            y1,
            roboto_font,
            font_size,
            HorizontalAnchor::Left,
            VerticalAnchor::Top,
        );
        draw_stars_at_point(
            &mut image,
            self.map_rating.pass,
            x3,
            y2,
            roboto_font,
            font_size,
            HorizontalAnchor::Center,
            VerticalAnchor::Bottom,
        );
    }
}

fn get_vertex(vertex: &Vertex, center: &Vertex, value: f64) -> (u32, u32) {
    let vector_x = vertex.x as f64 - center.x as f64;
    let vector_y = vertex.y as f64 - center.y as f64;

    (
        f64::max(center.x as f64 + vector_x * value, 0.0) as u32,
        f64::max(center.y as f64 + vector_y * value, 0.0) as u32,
    )
}

#[allow(clippy::too_many_arguments)]
fn draw_stars_at_point<I: DerefMut<Target = Image<Rgba>>>(
    image: &mut I,
    stars: f64,
    x1: u32,
    y1: u32,
    font: &Font,
    font_size: f32,
    horizontal_anchor: HorizontalAnchor,
    vertical_anchor: VerticalAnchor,
) {
    let text = format!("{stars:.2}*");

    let mut tech_text_segment = TextSegment::new(font, text, Rgba::white()).with_size(font_size);
    let tech_text_layout = TextLayout::new()
        .with_wrap(WrapStyle::None)
        .with_segment(&tech_text_segment);
    let tech_text_str_width = tech_text_layout.width();

    let x_offset_multiplier = match horizontal_anchor {
        HorizontalAnchor::Left => 8.0,
        HorizontalAnchor::Center => 6.0,
        HorizontalAnchor::Right => 5.0,
    };

    let y_offset: f32 = match vertical_anchor {
        VerticalAnchor::Top => font_size + 3.0,
        VerticalAnchor::Center => font_size / 2.0,
        VerticalAnchor::Bottom => -font_size - 4.0,
    };

    let pos_x = x1 - (tech_text_str_width as f64 * x_offset_multiplier / 12.0) as u32;
    let pos_y = y1 - y_offset as u32;
    draw_text_segment(
        image,
        &mut tech_text_segment,
        pos_x,
        pos_y,
        tech_text_str_width,
        pos_x,
        tech_text_str_width,
    );
}

fn get_map_rating_color(map_rating: &MapRating) -> Rgba {
    let max_rating = map_rating.get_max_rating();

    Rgba::new(
        clamp(map_rating.tech / max_rating * 255.0, 0.0, 255.0) as u8,
        clamp(map_rating.pass / max_rating * 255.0, 0.0, 255.0) as u8,
        clamp(map_rating.acc / max_rating * 255.0, 0.0, 255.0) as u8,
        255,
    )
}

fn get_triangle_circumcenter(v1: &Vertex, v2: &Vertex, v3: &Vertex) -> Option<Vertex> {
    let d = Vertex::vector(v1, v2);
    let e = Vertex::vector(v1, v3);

    let bl = d.magnitude();
    let cl = e.magnitude();
    let det = Vertex::determinant(&d, &e);

    let x = (e.y as f64 * bl - d.y as f64 * cl) * (0.5 / det);
    let y = (d.x as f64 * cl - e.x as f64 * bl) * (0.5 / det);

    if bl != 0.0 && cl != 0.0 && det != 0.0 {
        Some(Vertex::new(
            v1.x + x.round() as u32,
            v1.y + y.round() as u32,
        ))
    } else {
        None
    }
}
