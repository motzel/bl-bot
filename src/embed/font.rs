use lazy_static::lazy_static;
use ril::{Font, Image, Pixel, TextLayout, TextSegment, WrapStyle};

use ttf_parser::os2::UnicodeRanges;

lazy_static! {
    static ref ROBOTO_FONTS_BYTES: Vec<&'static [u8]> =
        vec![include_bytes!("./assets/RobotoCondensed-Bold.ttf") as &[u8],];
    pub(crate) static ref ROBOTO_FONT_FAMILY: FontFamily = FontFamily {
        fonts: ROBOTO_FONTS_BYTES
            .iter()
            .map(|b| FontWithRange {
                font: Font::from_bytes(b, 32.0).unwrap(),
                unicode_ranges: ttf_parser::Face::parse(b, 0).unwrap().unicode_ranges()
            })
            .collect(),
        y_offset: 0.0,
    };
    static ref NOTO_FONTS_BYTES: Vec<&'static [u8]> = vec![
        include_bytes!("./assets/NotoSansSC-Medium.ttf") as &[u8],
        include_bytes!("./assets/NotoSansKR-Medium.ttf") as &[u8],
        // include_bytes!("./assets/NotoSansTC-Medium.ttf") as &[u8],
        // include_bytes!("./assets/NotoSansJP-Medium.ttf") as &[u8],
        include_bytes!("./assets/NotoEmoji-Medium.ttf") as &[u8],
        // include_bytes!("./assets/NotoSans-Medium.ttf") as &[u8],
    ];
    pub(crate) static ref NOTO_FONT_FAMILY: FontFamily = FontFamily {
        fonts: NOTO_FONTS_BYTES
            .iter()
            .map(|b| FontWithRange {
                font: Font::from_bytes(b, 32.0).unwrap(),
                unicode_ranges: ttf_parser::Face::parse(b, 0).unwrap().unicode_ranges()
            })
            .collect(),
        y_offset: -0.2,
    };
}

pub(crate) struct FontWithRange {
    pub font: Font,
    pub unicode_ranges: UnicodeRanges,
}

pub(crate) struct FontFamily {
    pub fonts: Vec<FontWithRange>,
    pub y_offset: f32,
}

pub(crate) struct TextWithFontFamily {
    pub text: String,
    pub font_family: &'static FontFamily,
    pub idx: usize,
}

fn truncate_string(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        None => s,
        Some((idx, _)) => &s[..idx],
    }
}

fn clamp_text_segment<T>(text_segment: &mut TextSegment<T>, max_width: u32)
where
    T: Pixel,
{
    let str_alloc = text_segment.text.clone();
    let mut new_str = str_alloc.as_str();

    loop {
        new_str = truncate_string(new_str, new_str.chars().count() - 1);

        // new_str.truncate(new_str.len() - 1);
        if new_str.is_empty() {
            break;
        }

        text_segment.text = new_str.to_string().clone() + "...";

        let text_layout = TextLayout::new()
            .with_wrap(WrapStyle::None)
            .with_segment(text_segment);

        if text_layout.width() <= max_width {
            break;
        }
    }
}

pub(crate) fn draw_text_segment<T>(
    img: &mut Image<T>,
    text_segment: &mut TextSegment<T>,
    pos_x: u32,
    pos_y: u32,
    max_width: u32,
    center_pos_x: u32,
    center_width: u32,
) where
    T: Pixel,
{
    let mut text_layout = TextLayout::new()
        .with_wrap(WrapStyle::None)
        .with_position(pos_x, pos_y)
        .with_segment(text_segment);

    let text_width = text_layout.width();

    if text_width <= center_width {
        text_layout = text_layout
            .with_position(center_pos_x + (center_width - text_width) / 2, pos_y)
            .with_segment(text_segment);
    } else if text_width > max_width {
        clamp_text_segment(text_segment, max_width);

        text_layout = TextLayout::new()
            .with_wrap(WrapStyle::None)
            .with_position(pos_x, pos_y)
            .with_segment(text_segment);
    }

    img.draw(&text_layout);
}

pub(crate) fn split_text_by_fonts(
    text: &str,
    font_family: &'static FontFamily,
) -> Vec<TextWithFontFamily> {
    text.chars()
        .map(|c| {
            (
                c,
                font_family
                    .fonts
                    .iter()
                    .enumerate()
                    .filter(|(_i, fr)| fr.unicode_ranges.contains_char(c))
                    .map(|(i, _fr)| i)
                    .collect::<Vec<usize>>(),
            )
        })
        .fold(Vec::<TextWithFontFamily>::new(), |mut acc, c| {
            let len = acc.len();

            if acc.is_empty()
                || ((!c.1.is_empty() && !c.1.contains(&acc[len - 1].idx))
                    || (c.1.is_empty() && acc[len - 1].idx != usize::MAX))
            {
                let idx = if !c.1.is_empty() { c.1[0] } else { usize::MAX };

                acc.push(TextWithFontFamily {
                    text: c.0.to_string(),
                    font_family,
                    idx,
                })
            } else {
                acc[len - 1].text += &c.0.to_string();
            }

            acc
        })
}

pub(crate) fn could_be_drawn(text_with_family: &[TextWithFontFamily]) -> bool {
    !text_with_family.iter().any(|tf| tf.idx == usize::MAX)
}

#[allow(clippy::too_many_arguments)]
fn draw_segments<T>(
    img: &mut Image<T>,
    segments: Vec<TextWithFontFamily>,
    segments_widths: Vec<u32>,
    segments_spacing: u32,
    color: T,
    size: f32,
    pos_x: u32,
    pos_y: u32,
    max_width: u32,
) where
    T: Pixel,
{
    let mut current_pos_x = pos_x;

    let mut stop = false;
    for (key, segment) in segments.iter().enumerate() {
        let text_segment = &mut TextSegment::new(
            &segment.font_family.fonts[segment.idx].font,
            segment.text.trim_end(),
            color,
        )
        .with_size(size);

        if (current_pos_x - pos_x) + segments_widths[key] > max_width {
            clamp_text_segment(text_segment, max_width - (current_pos_x - pos_x));
            stop = true;
        }

        let text_layout = TextLayout::new()
            .with_wrap(WrapStyle::None)
            .with_position(
                current_pos_x,
                (pos_y as i32 + (size * segment.font_family.y_offset) as i32) as u32,
            )
            .with_segment(text_segment);

        img.draw(&text_layout);

        if stop {
            break;
        }

        current_pos_x += segments_widths[key] + segments_spacing;
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn draw_multilang_text<T>(
    img: &mut Image<T>,
    segments: Vec<TextWithFontFamily>,
    color: T,
    size: f32,
    pos_x: u32,
    pos_y: u32,
    max_width: u32,
    center_pos_x: u32,
    center_width: u32,
) where
    T: Pixel,
{
    let segments: Vec<TextWithFontFamily> = segments
        .into_iter()
        .map(|mut tf| {
            if tf.idx == usize::MAX {
                tf.idx = 0
            }

            tf
        })
        .collect();

    let mut total_width = 0;
    let mut segments_widths = Vec::new();

    // check segments width
    for segment in segments.iter() {
        // for some reason space at the end of segment has 0 width, so we need to calculate its width
        let mut spaces_width = 0;
        let trimmed = segment.text.trim_end();
        let num_of_spaces = (segment.text.len() - trimmed.len()) as u32;
        if num_of_spaces > 0 {
            let base_width = TextLayout::new()
                .with_wrap(WrapStyle::None)
                .with_position(pos_x, pos_y)
                .with_segment(
                    &TextSegment::new(&segment.font_family.fonts[segment.idx].font, "..", color)
                        .with_size(size),
                )
                .width();
            let with_space_width = TextLayout::new()
                .with_wrap(WrapStyle::None)
                .with_position(pos_x, pos_y)
                .with_segment(
                    &TextSegment::new(&segment.font_family.fonts[segment.idx].font, ". .", color)
                        .with_size(size),
                )
                .width();
            spaces_width = (with_space_width - base_width) * num_of_spaces;
        }

        let text_layout = TextLayout::new()
            .with_wrap(WrapStyle::None)
            .with_position(pos_x, pos_y)
            .with_segment(
                &TextSegment::new(&segment.font_family.fonts[segment.idx].font, trimmed, color)
                    .with_size(size),
            );

        let segment_width = text_layout.width() + num_of_spaces * spaces_width;

        segments_widths.push(segment_width);
        total_width += segment_width;
    }

    let segments_spacing = (size * 0.0625) as u32;

    // draw
    if total_width <= center_width {
        draw_segments(
            img,
            segments,
            segments_widths,
            segments_spacing,
            color,
            size,
            center_pos_x + (center_width - total_width) / 2,
            pos_y,
            max_width,
        );
    } else {
        draw_segments(
            img,
            segments,
            segments_widths,
            segments_spacing,
            color,
            size,
            pos_x,
            pos_y,
            max_width,
        );
    }
}
