use ril::prelude::*;

#[derive(PartialEq, Clone)]
pub(crate) enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

pub(crate) fn draw_rounded_rectangle<T>(
    img: &mut Image<T>,
    value: T,
    width: u32,
    height: u32,
    border_radius: u32,
    corners: &[Corner],
) where
    T: Pixel,
{
    // top left
    img.draw(
        &Ellipse::from_bounding_box(0, 0, border_radius, border_radius)
            .with_position(border_radius / 2, border_radius / 2)
            .with_fill(value),
    );
    // top right
    img.draw(
        &Ellipse::from_bounding_box(0, 0, border_radius, border_radius)
            .with_position(width - border_radius / 2, border_radius / 2)
            .with_fill(value),
    );
    // bottom left
    img.draw(
        &Ellipse::from_bounding_box(0, 0, border_radius, border_radius)
            .with_position(border_radius / 2, height - border_radius / 2)
            .with_fill(value),
    );
    // bottom right
    img.draw(
        &Ellipse::from_bounding_box(0, 0, border_radius, border_radius)
            .with_position(width - border_radius / 2, height - border_radius / 2)
            .with_fill(value),
    );
    // top rectangle
    img.draw(
        &Rectangle::new()
            .with_position(
                if corners.contains(&Corner::TopLeft) {
                    border_radius / 2
                } else {
                    0
                },
                0,
            )
            .with_size(
                if corners.contains(&Corner::TopLeft) {
                    if corners.contains(&Corner::TopRight) {
                        width - border_radius
                    } else {
                        width - border_radius / 2
                    }
                } else if corners.contains(&Corner::TopRight) {
                    width - border_radius / 2
                } else {
                    width
                },
                border_radius / 2,
            )
            .with_fill(value),
    );
    // middle rectangle
    img.draw(
        &Rectangle::new()
            .with_position(0, border_radius / 2)
            .with_size(width, height - border_radius)
            .with_fill(value),
    );
    // bottom rectangle
    img.draw(
        &Rectangle::new()
            .with_position(
                if corners.contains(&Corner::BottomLeft) {
                    border_radius / 2
                } else {
                    0
                },
                height - border_radius / 2,
            )
            .with_size(
                if corners.contains(&Corner::BottomLeft) {
                    if corners.contains(&Corner::BottomRight) {
                        width - border_radius
                    } else {
                        width - border_radius / 2
                    }
                } else if corners.contains(&Corner::BottomRight) {
                    width - border_radius / 2
                } else {
                    width
                },
                border_radius / 2,
            )
            .with_fill(value),
    );
}

pub(crate) fn draw_text<T>(
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

fn clamp_text_segment<T>(text_segment: &mut TextSegment<T>, max_width: u32)
where
    T: Pixel,
{
    let mut new_str = text_segment.text.clone();

    loop {
        new_str.truncate(new_str.len() - 1);
        if new_str.is_empty() {
            break;
        }

        text_segment.text = new_str.clone() + "...";

        let text_layout = TextLayout::new()
            .with_wrap(WrapStyle::None)
            .with_segment(text_segment);

        if text_layout.width() <= max_width {
            break;
        }
    }
}
