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
        &Rectangle::at(
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
        &Rectangle::at(0, border_radius / 2)
            .with_size(width, height - border_radius)
            .with_fill(value),
    );
    // bottom rectangle
    img.draw(
        &Rectangle::at(
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
