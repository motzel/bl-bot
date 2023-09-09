// taken from https://docs.rs/fastblur/0.1.1/fastblur/ and modified for Rgba
use ril::Rgba;
use std::cmp::min;

pub fn gaussian_blur(data: &mut [Rgba], width: usize, height: usize, blur_radius: f32) {
    let boxes = create_box_gauss(blur_radius, 3);
    let mut backbuf = data.to_owned();

    for box_size in boxes.iter() {
        let radius = ((box_size - 1) / 2) as usize;
        box_blur(&mut backbuf, data, width, height, radius, radius);
    }
}

#[inline]
fn create_box_gauss(sigma: f32, n: usize) -> Vec<i32> {
    if sigma > 0.0 {
        let n_float = n as f32;

        // Ideal averaging filter width
        let w_ideal = (12.0 * sigma * sigma / n_float).sqrt() + 1.0;
        let mut wl: i32 = w_ideal.floor() as i32;

        if wl % 2 == 0 {
            wl -= 1;
        };

        let wu = wl + 2;

        let wl_float = wl as f32;
        let m_ideal = (12.0 * sigma * sigma
            - n_float * wl_float * wl_float
            - 4.0 * n_float * wl_float
            - 3.0 * n_float)
            / (-4.0 * wl_float - 4.0);
        let m: usize = m_ideal.round() as usize;

        let mut sizes = Vec::<i32>::new();

        for i in 0..n {
            if i < m {
                sizes.push(wl);
            } else {
                sizes.push(wu);
            }
        }

        sizes
    } else {
        vec![1; n]
    }
}

#[inline]
fn box_blur(
    backbuf: &mut [Rgba],
    frontbuf: &mut [Rgba],
    width: usize,
    height: usize,
    blur_radius_horz: usize,
    blur_radius_vert: usize,
) {
    box_blur_horz(backbuf, frontbuf, width, height, blur_radius_horz);
    box_blur_vert(frontbuf, backbuf, width, height, blur_radius_vert);
}

#[inline]
fn box_blur_horz(
    backbuf: &[Rgba],
    frontbuf: &mut [Rgba],
    width: usize,
    height: usize,
    blur_radius: usize,
) {
    if blur_radius == 0 {
        frontbuf.copy_from_slice(backbuf);
        return;
    }

    let iarr = 1.0 / (blur_radius + blur_radius + 1) as f32;

    for i in 0..height {
        let row_start: usize = i * width; // inclusive
        let row_end: usize = (i + 1) * width - 1; // inclusive
        let mut ti: usize = i * width; // VERTICAL: $i;
        let mut li: usize = ti;
        let mut ri: usize = ti + blur_radius;

        let fv = backbuf[row_start];
        let lv = backbuf[row_end]; // VERTICAL: $backbuf[ti + $width - 1];

        let mut val_r: isize = (blur_radius as isize + 1) * isize::from(fv.r);
        let mut val_g: isize = (blur_radius as isize + 1) * isize::from(fv.g);
        let mut val_b: isize = (blur_radius as isize + 1) * isize::from(fv.b);

        // Get the pixel at the specified index, or the first pixel of the row
        // if the index is beyond the left edge of the image
        let get_left = |i: usize| {
            if i < row_start {
                fv
            } else {
                backbuf[i]
            }
        };

        // Get the pixel at the specified index, or the last pixel of the row
        // if the index is beyond the right edge of the image
        let get_right = |i: usize| {
            if i > row_end {
                lv
            } else {
                backbuf[i]
            }
        };

        for j in 0..min(blur_radius, width) {
            let bb = backbuf[ti + j]; // VERTICAL: ti + j * width
            val_r += isize::from(bb.r);
            val_g += isize::from(bb.g);
            val_b += isize::from(bb.b);
        }
        if blur_radius > width {
            val_r += (blur_radius - height) as isize * isize::from(lv.r);
            val_g += (blur_radius - height) as isize * isize::from(lv.g);
            val_b += (blur_radius - height) as isize * isize::from(lv.b);
        }

        // Process the left side where we need pixels from beyond the left edge
        for _ in 0..min(width, blur_radius + 1) {
            let bb = get_right(ri);
            ri += 1;
            val_r += isize::from(bb.r) - isize::from(fv.r);
            val_g += isize::from(bb.g) - isize::from(fv.g);
            val_b += isize::from(bb.b) - isize::from(fv.b);

            frontbuf[ti] = Rgba {
                r: round(val_r as f32 * iarr) as u8,
                g: round(val_g as f32 * iarr) as u8,
                b: round(val_b as f32 * iarr) as u8,
                a: bb.a,
            };
            ti += 1; // VERTICAL : ti += width, same with the other areas
        }

        if width > blur_radius {
            // otherwise `(width - blur_radius)` will underflow
            // Process the middle where we know we won't bump into borders
            // without the extra indirection of get_left/get_right. This is faster.
            for _ in (blur_radius + 1)..(width - blur_radius) {
                let bb1 = backbuf[ri];
                ri += 1;
                let bb2 = backbuf[li];
                li += 1;

                val_r += isize::from(bb1.r) - isize::from(bb2.r);
                val_g += isize::from(bb1.g) - isize::from(bb2.g);
                val_b += isize::from(bb1.b) - isize::from(bb2.b);

                frontbuf[ti] = Rgba {
                    r: round(val_r as f32 * iarr) as u8,
                    g: round(val_g as f32 * iarr) as u8,
                    b: round(val_b as f32 * iarr) as u8,
                    a: bb1.a,
                };
                ti += 1;
            }

            // Process the right side where we need pixels from beyond the right edge
            for _ in 0..min(width - blur_radius - 1, blur_radius) {
                let bb = get_left(li);
                li += 1;

                val_r += isize::from(lv.r) - isize::from(bb.r);
                val_g += isize::from(lv.g) - isize::from(bb.g);
                val_b += isize::from(lv.b) - isize::from(bb.b);

                frontbuf[ti] = Rgba {
                    r: round(val_r as f32 * iarr) as u8,
                    g: round(val_g as f32 * iarr) as u8,
                    b: round(val_b as f32 * iarr) as u8,
                    a: lv.a,
                };
                ti += 1;
            }
        }
    }
}

#[inline]
fn box_blur_vert(
    backbuf: &[Rgba],
    frontbuf: &mut [Rgba],
    width: usize,
    height: usize,
    blur_radius: usize,
) {
    if blur_radius == 0 {
        frontbuf.copy_from_slice(backbuf);
        return;
    }

    let iarr = 1.0 / (blur_radius + blur_radius + 1) as f32;

    for i in 0..width {
        let col_start = i; //inclusive
        let col_end = i + width * (height - 1); //inclusive
        let mut ti: usize = i;
        let mut li: usize = ti;
        let mut ri: usize = ti + blur_radius * width;

        let fv = backbuf[col_start];
        let lv = backbuf[col_end];

        let mut val_r: isize = (blur_radius as isize + 1) * isize::from(fv.r);
        let mut val_g: isize = (blur_radius as isize + 1) * isize::from(fv.g);
        let mut val_b: isize = (blur_radius as isize + 1) * isize::from(fv.b);

        // Get the pixel at the specified index, or the first pixel of the column
        // if the index is beyond the top edge of the image
        let get_top = |i: usize| {
            if i < col_start {
                fv
            } else {
                backbuf[i]
            }
        };

        // Get the pixel at the specified index, or the last pixel of the column
        // if the index is beyond the bottom edge of the image
        let get_bottom = |i: usize| {
            if i > col_end {
                lv
            } else {
                backbuf[i]
            }
        };

        for j in 0..min(blur_radius, height) {
            let bb = backbuf[ti + j * width];
            val_r += isize::from(bb.r);
            val_g += isize::from(bb.g);
            val_b += isize::from(bb.b);
        }
        if blur_radius > height {
            val_r += (blur_radius - height) as isize * isize::from(lv.r);
            val_g += (blur_radius - height) as isize * isize::from(lv.g);
            val_b += (blur_radius - height) as isize * isize::from(lv.b);
        }

        for _ in 0..min(height, blur_radius + 1) {
            let bb = get_bottom(ri);
            ri += width;
            val_r += isize::from(bb.r) - isize::from(fv.r);
            val_g += isize::from(bb.g) - isize::from(fv.g);
            val_b += isize::from(bb.b) - isize::from(fv.b);

            frontbuf[ti] = Rgba {
                r: round(val_r as f32 * iarr) as u8,
                g: round(val_g as f32 * iarr) as u8,
                b: round(val_b as f32 * iarr) as u8,
                a: bb.a,
            };
            ti += width;
        }

        if height > blur_radius {
            // otherwise `(height - blur_radius)` will underflow
            for _ in (blur_radius + 1)..(height - blur_radius) {
                let bb1 = backbuf[ri];
                ri += width;
                let bb2 = backbuf[li];
                li += width;

                val_r += isize::from(bb1.r) - isize::from(bb2.r);
                val_g += isize::from(bb1.g) - isize::from(bb2.g);
                val_b += isize::from(bb1.b) - isize::from(bb2.b);

                frontbuf[ti] = Rgba {
                    r: round(val_r as f32 * iarr) as u8,
                    g: round(val_g as f32 * iarr) as u8,
                    b: round(val_b as f32 * iarr) as u8,
                    a: bb1.a,
                };
                ti += width;
            }

            for _ in 0..min(height - blur_radius - 1, blur_radius) {
                let bb = get_top(li);
                li += width;

                val_r += isize::from(lv.r) - isize::from(bb.r);
                val_g += isize::from(lv.g) - isize::from(bb.g);
                val_b += isize::from(lv.b) - isize::from(bb.b);

                frontbuf[ti] = Rgba {
                    r: round(val_r as f32 * iarr) as u8,
                    g: round(val_g as f32 * iarr) as u8,
                    b: round(val_b as f32 * iarr) as u8,
                    a: lv.a,
                };
                ti += width;
            }
        }
    }
}

#[inline]
fn round(mut x: f32) -> f32 {
    x += 12582912.0;
    x -= 12582912.0;
    x
}
