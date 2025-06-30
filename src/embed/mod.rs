use relativetime::RelativeTime;
use ril::prelude::*;

use map_triangle::Vertex;

use crate::beatleader::player::DifficultyStatus;
use crate::discord::bot::beatleader::player::Player;
use crate::discord::bot::beatleader::score::MapRatingModifier;
use crate::discord::bot::beatleader::score::Score;
use crate::discord::bot::get_binary_file;
use crate::embed::blur::gaussian_blur;
use crate::embed::font::{
    could_be_drawn, draw_multilang_text, draw_text_segment, load_noto_fonts, split_text_by_fonts,
    FontFamily, ROBOTO_FONT_FAMILY,
};
use crate::embed::map_triangle::MapTriangle;
use crate::embed::utils::{draw_rounded_rectangle, Corner};

mod blur;
mod font;
mod map_triangle;
mod triangle;
mod utils;

pub async fn embed_score(
    score: &Score,
    player: &Player,
    player_avatar_bytes: &[u8],
) -> Option<Vec<u8>> {
    const FONT_SIZE: f32 = 32.0;
    const WIDTH: u32 = 512;
    const HEIGHT: u32 = 296;
    const AVATAR_SIZE: u32 = 128;
    const BORDER_SIZE: u32 = 28;
    const BORDER_RADIUS: u32 = 32;
    const BLUR_RADIUS_BORDER: f32 = 25.0;
    const BLUR_RADIUS: f32 = 5.0;
    const PADDING: u32 = 8;

    let small_font_size = FONT_SIZE * 0.5;
    let smaller_font_size = FONT_SIZE * 0.75;
    let big_font_size = FONT_SIZE * 1.5;

    let roboto_font = &ROBOTO_FONT_FAMILY.fonts[0].font;

    // load background
    let bg_bytes = get_binary_file(&score.song_cover)
        .await
        .unwrap_or(bytes::Bytes::new());
    if bg_bytes.is_empty() {
        return None;
    }

    let Ok(mut bg) = Image::<Rgba>::from_bytes_inferred(bg_bytes.as_ref()) else {
        return None;
    };

    // resize background to WIDTH x WIDTH and crop WIDTH x HEIGHT from the center
    bg.resize(WIDTH, WIDTH, ResizeAlgorithm::Lanczos3);
    let bg_y = (WIDTH - HEIGHT) / 2;
    bg.crop(0, bg_y, WIDTH, bg_y + HEIGHT);

    // blur the background
    let mut bg_border = bg.clone();
    gaussian_blur(
        &mut bg_border.data,
        WIDTH as usize,
        HEIGHT as usize,
        BLUR_RADIUS_BORDER,
    );
    gaussian_blur(&mut bg.data, WIDTH as usize, HEIGHT as usize, BLUR_RADIUS);

    let res = std::panic::catch_unwind(|| {
        // load avatar
        let Ok(mut avatar) = Image::<Rgba>::from_bytes_inferred(player_avatar_bytes) else {
            return None;
        };
        avatar.resize(AVATAR_SIZE, AVATAR_SIZE, ResizeAlgorithm::Lanczos3);

        Some(avatar)
    });
    if res.is_err() || res.as_ref().unwrap().is_none() {
        return None;
    }

    let avatar = res.unwrap().unwrap();

    // create image
    let mut image = Image::<Rgba>::new(WIDTH, HEIGHT, Rgba::new(66, 66, 66, 1))
        .with_overlay_mode(OverlayMode::Merge);

    // add rounded corners mask & paste background blurred with BLUR_RADIUS_BORDER
    let mut bg_mask = Image::new(WIDTH, HEIGHT, L::new(0));
    draw_rounded_rectangle(
        &mut bg_mask,
        L::new(255),
        WIDTH,
        HEIGHT,
        BORDER_RADIUS,
        &[
            Corner::TopLeft,
            Corner::TopRight,
            Corner::BottomLeft,
            Corner::BottomRight,
        ],
    );
    bg_border.mask_alpha(&bg_mask);
    image.paste(0, 0, &bg_border);

    // add rounder corners inner mask & paste background blurred with BLUR_RADIUS
    let mut bg_mask = Image::new(WIDTH, HEIGHT, L::new(0));
    draw_rounded_rectangle(
        &mut bg_mask,
        L::new(255),
        WIDTH - BORDER_SIZE,
        HEIGHT - BORDER_SIZE,
        BORDER_RADIUS,
        &[
            Corner::TopLeft,
            Corner::TopRight,
            Corner::BottomLeft,
            Corner::BottomRight,
        ],
    );
    bg.mask_alpha(&bg_mask);
    image.paste(BORDER_SIZE / 2, BORDER_SIZE / 2, &bg);

    let mut overlay = Image::new(
        WIDTH - BORDER_SIZE,
        HEIGHT - BORDER_SIZE,
        Rgba::transparent(),
    );
    draw_rounded_rectangle(
        &mut overlay,
        Rgba::new(1, 1, 1, 64),
        WIDTH - BORDER_SIZE,
        HEIGHT - BORDER_SIZE,
        BORDER_RADIUS,
        &[
            Corner::TopLeft,
            Corner::TopRight,
            Corner::BottomLeft,
            Corner::BottomRight,
        ],
    );
    image.paste(BORDER_SIZE / 2, BORDER_SIZE / 2, &overlay);

    // paste masked avatar
    let mut mask = Image::new(AVATAR_SIZE, AVATAR_SIZE, BitPixel::off());
    mask.draw(
        &Ellipse::from_bounding_box(0, 0, AVATAR_SIZE, AVATAR_SIZE).with_fill(BitPixel::on()),
    );
    let avatar_pos_x = BORDER_SIZE / 2 + AVATAR_SIZE / 4;
    let avatar_pos_y = HEIGHT - BORDER_SIZE - BORDER_RADIUS / 6 - FONT_SIZE as u32 - AVATAR_SIZE;
    image.paste_with_mask(avatar_pos_x, avatar_pos_y, &avatar, &mask);

    let mut difficulty_desc = "".to_owned();
    if score.difficulty_score_rating.is_some()
        && score.difficulty_score_rating.as_ref().unwrap().stars > 0.0
    {
        let difficulty_rating = score.difficulty_score_rating.as_ref().unwrap();
        let stars = format!(
            "{:.2}*{}",
            difficulty_rating.stars,
            if difficulty_rating.modifier != MapRatingModifier::None {
                format!(" ({})", difficulty_rating.modifier)
            } else {
                "".to_owned()
            }
        );
        difficulty_desc.push_str(&stars);
    } else {
        difficulty_desc.push_str(shorten_difficulty_name(score.difficulty_name.as_str()).as_str());
    }
    let mut difficulty_text_segment =
        TextSegment::new(roboto_font, difficulty_desc, Rgba::white()).with_size(small_font_size);
    let difficulty_text_layout = TextLayout::new()
        .with_wrap(WrapStyle::None)
        .with_segment(&difficulty_text_segment);
    let difficulty_text_str_width = difficulty_text_layout.width();
    let difficulty_badge_width = difficulty_text_str_width + PADDING * 2;
    let mut difficulty = Image::new(
        difficulty_badge_width,
        small_font_size as u32 + PADDING * 3,
        Rgba::transparent(),
    );
    draw_rounded_rectangle(
        &mut difficulty,
        difficulty_color(&score.difficulty_name),
        difficulty_badge_width,
        small_font_size as u32 + PADDING * 3,
        BORDER_RADIUS,
        &[Corner::TopRight, Corner::BottomLeft],
    );
    draw_text_segment(
        &mut difficulty,
        &mut difficulty_text_segment,
        0,
        ((smaller_font_size - small_font_size) / 2.0) as u32 + PADDING,
        difficulty_badge_width,
        0,
        difficulty_badge_width,
    );
    image.paste(
        WIDTH - BORDER_SIZE / 2 - difficulty_badge_width,
        BORDER_SIZE / 2,
        &difficulty,
    );

    let mut noto_fonts_option: Option<FontFamily> = None;

    let song_name = format!("{} {}", score.song_name, score.song_sub_name);
    let text = song_name.as_str();
    let mut text_fonts = split_text_by_fonts(text, &ROBOTO_FONT_FAMILY);
    if !could_be_drawn(&text_fonts) {
        noto_fonts_option = Some(load_noto_fonts());

        text_fonts = split_text_by_fonts(text, noto_fonts_option.as_ref().unwrap());
    }

    draw_multilang_text(
        &mut image,
        text_fonts,
        Rgba::white(),
        smaller_font_size,
        BORDER_SIZE / 2 + BORDER_RADIUS / 2,
        BORDER_SIZE / 2 + BORDER_RADIUS / 4,
        WIDTH - BORDER_SIZE - BORDER_RADIUS - difficulty_badge_width,
        0,
        0,
    );

    draw_text_segment(
        &mut image,
        &mut TextSegment::new(roboto_font, score.song_mapper.clone(), Rgba::white())
            .with_size(small_font_size),
        BORDER_SIZE / 2 + BORDER_RADIUS / 2,
        BORDER_SIZE / 2 + BORDER_RADIUS / 4 + PADDING + smaller_font_size as u32,
        WIDTH - BORDER_SIZE - BORDER_RADIUS,
        0,
        0,
    );

    let speed_multiplier = match score.difficulty_score_rating.as_ref() {
        Some(difficulty_rating) => difficulty_rating.modifier.speed_multiplier(),
        None => MapRatingModifier::None.speed_multiplier(),
    };
    draw_text_segment(
        &mut image,
        &mut TextSegment::new(
            roboto_font,
            format!(
                "{} / {:.0} BPM / {:.2} NPS",
                score.song_duration.with_speed_multiplier(speed_multiplier),
                score.song_bpm as f64 * speed_multiplier,
                score.difficulty_nps * speed_multiplier,
            ),
            Rgba::white(),
        )
        .with_size(small_font_size),
        BORDER_SIZE / 2 + BORDER_RADIUS / 2,
        BORDER_SIZE / 2
            + BORDER_RADIUS / 4
            + PADDING
            + smaller_font_size as u32
            + small_font_size as u32
            + PADDING / 2,
        WIDTH - BORDER_SIZE - BORDER_RADIUS,
        0,
        0,
    );

    let text = player.name.as_str();
    let mut text_fonts = split_text_by_fonts(text, &ROBOTO_FONT_FAMILY);
    if !could_be_drawn(&text_fonts) {
        if noto_fonts_option.is_none() {
            noto_fonts_option = Some(load_noto_fonts());
        }

        text_fonts = split_text_by_fonts(text, noto_fonts_option.as_ref().unwrap());
    }

    draw_multilang_text(
        &mut image,
        text_fonts,
        Rgba::white(),
        FONT_SIZE,
        BORDER_SIZE / 2 + BORDER_RADIUS / 2,
        HEIGHT - BORDER_SIZE / 2 - BORDER_SIZE / 4 - BORDER_RADIUS / 4 - FONT_SIZE as u32,
        WIDTH - BORDER_SIZE - BORDER_RADIUS,
        avatar_pos_x - AVATAR_SIZE / 4,
        AVATAR_SIZE + AVATAR_SIZE / 2,
    );

    let stats_width =
        WIDTH - avatar_pos_x - AVATAR_SIZE - BORDER_SIZE / 2 - BORDER_RADIUS / 2 - PADDING * 4;
    let stats_pos_x = avatar_pos_x
        + AVATAR_SIZE
        + if score.difficulty_score_rating.is_some()
            && score
                .difficulty_score_rating
                .as_ref()
                .unwrap()
                .has_individual_rating()
        {
            PADDING
        } else {
            PADDING * 4
        };
    let acc_pos_y = BORDER_SIZE / 2 + BORDER_RADIUS / 2 + (FONT_SIZE * 1.9) as u32;
    draw_text_segment(
        &mut image,
        &mut TextSegment::new(
            roboto_font,
            format!("{:.2}%", score.accuracy),
            Rgba::white(),
        )
        .with_size(big_font_size),
        stats_pos_x,
        acc_pos_y,
        stats_width,
        stats_pos_x,
        stats_width,
    );

    draw_text_segment(
        &mut image,
        &mut TextSegment::new(
            roboto_font,
            format!(
                "{}{:.2} / {:.2}",
                if score.mistakes > 0 {
                    format!("{:.2}% FC • ", score.fc_accuracy)
                } else {
                    "".to_string()
                },
                score.acc_left,
                score.acc_right
            ),
            Rgba::white(),
        )
        .with_size(small_font_size),
        stats_pos_x,
        acc_pos_y + big_font_size as u32 + PADDING,
        stats_width,
        stats_pos_x,
        stats_width,
    );

    draw_text_segment(
        &mut image,
        &mut TextSegment::new(
            roboto_font,
            format!(
                "{} {} • {}",
                if !score.modifiers.is_empty() {
                    format!("{} •", score.modifiers)
                } else {
                    "".to_string()
                },
                if score.mistakes > 0 {
                    format!(
                        "{} mistake{}",
                        score.mistakes,
                        if score.mistakes > 1 {
                            "s".to_string()
                        } else {
                            "".to_string()
                        }
                    )
                } else {
                    "FC".to_string()
                },
                if score.pauses > 0 {
                    format!(
                        "{} pause{}",
                        score.pauses,
                        if score.pauses > 1 {
                            "s".to_string()
                        } else {
                            "".to_string()
                        }
                    )
                } else {
                    "No pauses".to_string()
                }
            ),
            Rgba::white(),
        )
        .with_size(small_font_size),
        stats_pos_x,
        acc_pos_y + big_font_size as u32 + PADDING + small_font_size as u32 + PADDING / 2,
        stats_width,
        stats_pos_x,
        stats_width,
    );

    draw_text_segment(
        &mut image,
        &mut TextSegment::new(
            roboto_font,
            format!(
                "#{}{}",
                score.rank,
                if score.difficulty_status == DifficultyStatus::Ranked
                    || score.difficulty_status == DifficultyStatus::Qualified
                {
                    format!(" • {:.2}pp", score.pp)
                } else {
                    "".to_string()
                }
            ),
            Rgba::white(),
        )
        .with_size(FONT_SIZE),
        stats_pos_x,
        avatar_pos_y + AVATAR_SIZE - FONT_SIZE as u32 - PADDING,
        stats_width,
        stats_pos_x,
        stats_width,
    );

    if score.difficulty_score_rating.is_some()
        && score
            .difficulty_score_rating
            .as_ref()
            .unwrap()
            .has_individual_rating()
    {
        let map_triangle = MapTriangle::new(Vertex::new(436, 74), 50)
            .with_map_rating(score.difficulty_score_rating.as_ref().unwrap().clone());
        image.draw(&map_triangle);
    }

    let mut buffer = Vec::<u8>::with_capacity(200_000);
    if image
        .encode(ril::prelude::ImageFormat::Png, &mut buffer)
        .is_ok()
    {
        return Some(buffer);
    }

    None
}

#[allow(unused_assignments)]
pub async fn embed_profile(
    player: &Player,
    player_avatar_bytes: &[u8],
    player_cover_bytes: &[u8],
) -> Option<Vec<u8>> {
    const FONT_SIZE: f32 = 32.0;
    const WIDTH: u32 = 512;
    const HEIGHT: u32 = 296;
    const AVATAR_SIZE: u32 = 128;
    const BORDER_SIZE: u32 = 28;
    const BORDER_RADIUS: u32 = 32;
    const BLUR_RADIUS_BORDER: f32 = 25.0;
    const BLUR_RADIUS: f32 = 7.5;
    const PADDING: u32 = 8;

    let small_font_size = FONT_SIZE * 0.5;
    let smaller_font_size = FONT_SIZE * 0.75;
    let big_font_size = FONT_SIZE * 1.5;

    let roboto_font = &ROBOTO_FONT_FAMILY.fonts[0].font;

    // load background
    let Ok(mut bg) = Image::<Rgba>::from_bytes_inferred(player_cover_bytes) else {
        return None;
    };

    // resize background to WIDTH x WIDTH and crop WIDTH x HEIGHT from the center
    bg.resize(WIDTH, WIDTH, ResizeAlgorithm::Lanczos3);
    let bg_y = (WIDTH - HEIGHT) / 2;
    bg.crop(0, bg_y, WIDTH, bg_y + HEIGHT);

    // blur the background
    let mut bg_border = bg.clone();
    gaussian_blur(
        &mut bg_border.data,
        WIDTH as usize,
        HEIGHT as usize,
        BLUR_RADIUS_BORDER,
    );
    gaussian_blur(&mut bg.data, WIDTH as usize, HEIGHT as usize, BLUR_RADIUS);

    // load avatar
    let Ok(mut avatar) = Image::<Rgba>::from_bytes_inferred(player_avatar_bytes) else {
        return None;
    };
    avatar.resize(AVATAR_SIZE, AVATAR_SIZE, ResizeAlgorithm::Lanczos3);

    // create image
    let mut image = Image::<Rgba>::new(WIDTH, HEIGHT, Rgba::new(66, 66, 66, 1))
        .with_overlay_mode(OverlayMode::Merge);

    // add rounded corners mask & paste background blurred with BLUR_RADIUS_BORDER
    let mut bg_mask = Image::new(WIDTH, HEIGHT, L::new(0));
    draw_rounded_rectangle(
        &mut bg_mask,
        L::new(255),
        WIDTH,
        HEIGHT,
        BORDER_RADIUS,
        &[
            Corner::TopLeft,
            Corner::TopRight,
            Corner::BottomLeft,
            Corner::BottomRight,
        ],
    );
    bg_border.mask_alpha(&bg_mask);
    image.paste(0, 0, &bg_border);

    // add rounder corners inner mask & paste background blurred with BLUR_RADIUS
    let mut bg_mask = Image::new(WIDTH, HEIGHT, L::new(0));
    draw_rounded_rectangle(
        &mut bg_mask,
        L::new(255),
        WIDTH - BORDER_SIZE,
        HEIGHT - BORDER_SIZE,
        BORDER_RADIUS,
        &[
            Corner::TopLeft,
            Corner::TopRight,
            Corner::BottomLeft,
            Corner::BottomRight,
        ],
    );
    bg.mask_alpha(&bg_mask);
    image.paste(BORDER_SIZE / 2, BORDER_SIZE / 2, &bg);

    let mut overlay = Image::new(
        WIDTH - BORDER_SIZE,
        HEIGHT - BORDER_SIZE,
        Rgba::transparent(),
    );
    draw_rounded_rectangle(
        &mut overlay,
        Rgba::new(1, 1, 1, 64),
        WIDTH - BORDER_SIZE,
        HEIGHT - BORDER_SIZE,
        BORDER_RADIUS,
        &[
            Corner::TopLeft,
            Corner::TopRight,
            Corner::BottomLeft,
            Corner::BottomRight,
        ],
    );
    image.paste(BORDER_SIZE / 2, BORDER_SIZE / 2, &overlay);

    // paste masked avatar
    let mut mask = Image::new(AVATAR_SIZE, AVATAR_SIZE, BitPixel::off());
    mask.draw(
        &Ellipse::from_bounding_box(0, 0, AVATAR_SIZE, AVATAR_SIZE).with_fill(BitPixel::on()),
    );
    let avatar_pos_x = BORDER_SIZE / 2 + AVATAR_SIZE / 4;
    let avatar_pos_y = HEIGHT - BORDER_SIZE - BORDER_RADIUS / 6 - FONT_SIZE as u32 - AVATAR_SIZE;
    image.paste_with_mask(avatar_pos_x, avatar_pos_y, &avatar, &mask);

    if !player.is_verified {
        let mut not_verified_text_segment =
            TextSegment::new(roboto_font, "Not verified", Rgba::white()).with_size(small_font_size);
        let not_verified_text_layout = TextLayout::new()
            .with_wrap(WrapStyle::None)
            .with_segment(&not_verified_text_segment);
        let not_verified_text_str_width = not_verified_text_layout.width();
        let not_verified_badge_width = not_verified_text_str_width + PADDING * 2;
        let mut not_verified = Image::new(
            not_verified_badge_width,
            small_font_size as u32 + PADDING * 3,
            Rgba::transparent(),
        );
        draw_rounded_rectangle(
            &mut not_verified,
            Rgba::new(191, 42, 66, 192),
            not_verified_badge_width,
            small_font_size as u32 + PADDING * 3,
            BORDER_RADIUS,
            &[Corner::TopRight, Corner::BottomLeft],
        );
        draw_text_segment(
            &mut not_verified,
            &mut not_verified_text_segment,
            0,
            ((smaller_font_size - small_font_size) / 2.0) as u32 + PADDING,
            not_verified_badge_width,
            0,
            not_verified_badge_width,
        );
        image.paste(
            WIDTH - BORDER_SIZE / 2 - not_verified_badge_width,
            BORDER_SIZE / 2,
            &not_verified,
        );
    }

    draw_text_segment(
        &mut image,
        &mut TextSegment::new(roboto_font, format!("#{}", player.rank), Rgba::white())
            .with_size(FONT_SIZE),
        BORDER_SIZE / 2 + BORDER_RADIUS / 2,
        HEIGHT
            - BORDER_SIZE
            - BORDER_RADIUS / 4
            - FONT_SIZE as u32
            - AVATAR_SIZE
            - FONT_SIZE as u32
            - (PADDING as f32 * 3.5) as u32,
        WIDTH - BORDER_SIZE - BORDER_RADIUS,
        avatar_pos_x,
        AVATAR_SIZE,
    );

    draw_text_segment(
        &mut image,
        &mut TextSegment::new(
            roboto_font,
            format!("#{} peak", player.peak_rank),
            Rgba::white(),
        )
        .with_size(small_font_size),
        BORDER_SIZE / 2 + BORDER_RADIUS / 2,
        HEIGHT
            - BORDER_SIZE
            - BORDER_RADIUS / 4
            - FONT_SIZE as u32
            - AVATAR_SIZE
            - FONT_SIZE as u32
            - (PADDING as f32 * 3.5) as u32
            + FONT_SIZE as u32
            + PADDING / 2,
        WIDTH - BORDER_SIZE - BORDER_RADIUS,
        avatar_pos_x - AVATAR_SIZE / 4,
        AVATAR_SIZE + AVATAR_SIZE / 2,
    );

    let mut noto_fonts_option = None;

    let text = &player.name;
    let mut text_fonts = split_text_by_fonts(text, &ROBOTO_FONT_FAMILY);
    if !could_be_drawn(&text_fonts) {
        if noto_fonts_option.is_none() {
            noto_fonts_option = Some(load_noto_fonts());
        }

        text_fonts = split_text_by_fonts(text, noto_fonts_option.as_ref().unwrap());
    }

    draw_multilang_text(
        &mut image,
        text_fonts,
        Rgba::white(),
        FONT_SIZE,
        BORDER_SIZE / 2 + BORDER_RADIUS / 2,
        HEIGHT - BORDER_SIZE / 2 - BORDER_SIZE / 4 - BORDER_RADIUS / 4 - FONT_SIZE as u32,
        WIDTH - BORDER_SIZE - BORDER_RADIUS,
        avatar_pos_x - AVATAR_SIZE / 4,
        AVATAR_SIZE + AVATAR_SIZE / 2,
    );

    let stats_width =
        WIDTH - avatar_pos_x - AVATAR_SIZE - BORDER_SIZE / 2 - BORDER_RADIUS / 2 - PADDING * 2;
    let stats_pos_x = avatar_pos_x + AVATAR_SIZE + PADDING * 2;
    let stats_pos_y = BORDER_SIZE / 2 + BORDER_RADIUS / 2 + (FONT_SIZE * 1.25) as u32;
    draw_text_segment(
        &mut image,
        &mut TextSegment::new(roboto_font, format!("{:.2}pp", player.pp), Rgba::white())
            .with_size(big_font_size),
        stats_pos_x,
        stats_pos_y,
        stats_width,
        stats_pos_x,
        stats_width,
    );

    draw_text_segment(
        &mut image,
        &mut TextSegment::new(
            roboto_font,
            format!(
                "{:.2} top pp{}",
                player.top_pp,
                if player.last_scores_fetch.is_some() {
                    format!(" • {:.2} +1pp", player.plus_1pp)
                } else {
                    "".to_owned()
                }
            ),
            Rgba::white(),
        )
        .with_size(small_font_size),
        stats_pos_x,
        stats_pos_y + (big_font_size * 1.2) as u32 + PADDING,
        stats_width,
        stats_pos_x,
        stats_width,
    );

    draw_text_segment(
        &mut image,
        &mut TextSegment::new(
            roboto_font,
            format!(
                "{}{:.2}% avg acc",
                if player.last_scores_fetch.is_some() {
                    format!("{:.2}* top stars • ", player.top_stars)
                } else {
                    "".to_owned()
                },
                player.avg_ranked_accuracy
            ),
            Rgba::white(),
        )
        .with_size(small_font_size),
        stats_pos_x,
        stats_pos_y + (big_font_size * 1.2) as u32 + PADDING + small_font_size as u32 + PADDING / 2,
        stats_width,
        stats_pos_x,
        stats_width,
    );

    let mut y_offset = 0;
    if player.last_scores_fetch.is_some() {
        draw_text_segment(
            &mut image,
            &mut TextSegment::new(
                roboto_font,
                if player.last_ranked_paused_at.is_some() {
                    let mut relative_time = player.last_ranked_paused_at.unwrap().to_relative();
                    if relative_time == "1 months ago" {
                        "1 month ago".clone_into(&mut relative_time);
                    }
                    format!("Last paused {relative_time}")
                } else {
                    "Never paused".to_owned()
                },
                Rgba::white(),
            )
            .with_size(small_font_size),
            stats_pos_x,
            stats_pos_y
                + (big_font_size * 1.2) as u32
                + PADDING
                + (small_font_size as u32 + PADDING / 2) * 2
                + y_offset,
            stats_width,
            stats_pos_x,
            stats_width,
        );

        y_offset += small_font_size as u32 + PADDING / 2;
    }

    if !player.clans.is_empty() {
        draw_text_segment(
            &mut image,
            &mut TextSegment::new(roboto_font, player.clans.join(" • "), Rgba::white())
                .with_size(small_font_size),
            stats_pos_x,
            stats_pos_y
                + (big_font_size * 1.2) as u32
                + PADDING
                + (small_font_size as u32 + PADDING / 2) * 2
                + y_offset,
            stats_width,
            stats_pos_x,
            stats_width,
        );

        y_offset += small_font_size as u32 + PADDING / 2;
    }

    let mut buffer = Vec::<u8>::with_capacity(200_000);
    if image
        .encode(ril::prelude::ImageFormat::Png, &mut buffer)
        .is_ok()
    {
        return Some(buffer);
    }

    None
}

fn difficulty_color(name: &str) -> Rgba {
    match name {
        "Easy" => Rgba::new(60, 179, 113, 192),
        "Normal" => Rgba::new(89, 176, 244, 192),
        "Hard" => Rgba::new(255, 99, 71, 192),
        "Expert" => Rgba::new(191, 42, 66, 192),
        "ExpertPlus" => Rgba::new(143, 72, 219, 192),
        _ => Rgba::new(128, 128, 128, 192),
    }
}

fn shorten_difficulty_name(name: &str) -> String {
    match name {
        "ExpertPlus" => "Expert+".to_string(),
        _ => name.to_string(),
    }
}

pub(crate) fn clamp<T: PartialOrd>(input: T, min: T, max: T) -> T {
    debug_assert!(min <= max, "min must be less than or equal to max");
    if input < min {
        min
    } else if input > max {
        max
    } else {
        input
    }
}
