use super::*;
use crate::layout::{DisplayCommand, DisplayList};
use metis_platform::rasterizer::CornerRadius;

fn image_2x1() -> RasterImage {
    RasterImage::new(2, 1, vec![Color::RED, Color::BLUE]).expect("image")
}

#[test]
fn image_validation_rejects_empty_oversized_and_mismatched_storage() {
    for (width, height) in [(0, 1), (1, 0), (4097, 4096), (u32::MAX, 1)] {
        let error = RasterImage::new(width, height, Vec::<Color>::new())
            .expect_err("invalid image dimensions");
        assert_eq!(error.code, ErrorCode::SurfaceAllocationError);
    }
    let error = RasterImage::new(2, 1, vec![Color::RED]).expect_err("mismatched pixels");
    assert_eq!(error.code, ErrorCode::RenderFailure);
    assert!(error.message.contains("2x1"));
}

#[test]
fn rgba_bytes_preserve_row_major_channels_and_reject_length_mismatch() {
    let image = RasterImage::from_rgba_bytes(2, 1, [1, 2, 3, 4, 5, 6, 7, 8]).expect("rgba bytes");
    assert_eq!(
        image.pixels(),
        &[Color::rgba(1, 2, 3, 4), Color::rgba(5, 6, 7, 8)]
    );

    let error = RasterImage::from_rgba_bytes(2, 1, [0; 4]).expect_err("short rgba bytes");
    assert_eq!(error.code, ErrorCode::RenderFailure);
    assert!(error.message.contains("byte count"));
}

#[test]
fn placement_validation_rejects_invalid_source_and_destination() {
    let image = image_2x1();
    for source in [
        Rect::new(-1, 0, 1, 1),
        Rect::new(0, 0, 0, 1),
        Rect::new(1, 0, 2, 1),
        Rect::new(0, 1, 1, 1),
    ] {
        let error = ImagePlacement::new(
            image.clone(),
            source,
            Rect::new(0, 0, 1, 1),
            ImageSampling::Nearest,
        )
        .expect_err("invalid image source");
        assert_eq!(error.code, ErrorCode::RenderFailure);
    }
    for destination in [Rect::new(0, 0, 0, 1), Rect::new(0, 0, 1, -1)] {
        let error = ImagePlacement::new(
            image.clone(),
            Rect::new(0, 0, 2, 1),
            destination,
            ImageSampling::Nearest,
        )
        .expect_err("invalid image destination");
        assert_eq!(error.code, ErrorCode::RenderFailure);
    }
}

#[test]
fn nearest_scaling_clips_and_preserves_painter_order() {
    let image = image_2x1();
    let placement = ImagePlacement::new(
        image,
        Rect::new(0, 0, 2, 1),
        Rect::new(-1, 0, 4, 2),
        ImageSampling::Nearest,
    )
    .expect("placement");
    let mut list = DisplayList {
        commands: vec![DisplayCommand::FillRect {
            rect: Rect::new(0, 0, 3, 2),
            radius: CornerRadius::SQUARE,
            color: Color::WHITE,
        }],
    };
    list.append_image(placement).expect("append image");
    let mut framebuffer = Framebuffer::new(3, 2).expect("surface");
    list.render_to(&mut framebuffer);
    for y in 0..2 {
        assert_eq!(framebuffer.get_pixel(0, y), Color::RED);
        assert_eq!(framebuffer.get_pixel(1, y), Color::BLUE);
        assert_eq!(framebuffer.get_pixel(2, y), Color::BLUE);
    }
}

#[test]
fn orientation_transforms_preserve_grid_order_without_copying_source() {
    let image = RasterImage::new(
        2,
        3,
        (0_u8..6)
            .map(|value| Color::rgba(value, 0, 0, 255))
            .collect::<Vec<_>>(),
    )
    .expect("orientation fixture");
    let source_address = image.pixels().as_ptr();
    let cases = [
        (
            ImageTransform::Identity,
            Rect::new(0, 0, 2, 3),
            [0, 1, 2, 3, 4, 5],
        ),
        (
            ImageTransform::FlipHorizontal,
            Rect::new(0, 0, 2, 3),
            [1, 0, 3, 2, 5, 4],
        ),
        (
            ImageTransform::FlipVertical,
            Rect::new(0, 0, 2, 3),
            [4, 5, 2, 3, 0, 1],
        ),
        (
            ImageTransform::RotateClockwise,
            Rect::new(0, 0, 3, 2),
            [4, 2, 0, 5, 3, 1],
        ),
        (
            ImageTransform::RotateCounterClockwise,
            Rect::new(0, 0, 3, 2),
            [1, 3, 5, 0, 2, 4],
        ),
    ];
    for (transform, destination, expected) in cases {
        let placement = ImagePlacement::new(
            image.clone(),
            Rect::new(0, 0, 2, 3),
            destination,
            ImageSampling::Nearest,
        )
        .expect("placement")
        .with_transform(transform);
        assert_eq!(placement.transform(), transform);
        assert_eq!(placement.image().pixels().as_ptr(), source_address);
        let mut framebuffer = Framebuffer::new(
            u32::try_from(destination.width).expect("positive test width"),
            u32::try_from(destination.height).expect("positive test height"),
        )
        .expect("surface");
        placement.render_to(&mut framebuffer);
        let mut actual = Vec::with_capacity(expected.len());
        for y in 0..destination.height {
            for x in 0..destination.width {
                actual.push(framebuffer.get_pixel(x, y).r);
            }
        }
        assert_eq!(actual, expected);
    }
}

#[test]
fn affine_transform_validates_and_preserves_grid_order() {
    let yellow = Color::rgb(255, 200, 0);
    let image = RasterImage::new(2, 2, vec![Color::RED, Color::GREEN, Color::BLUE, yellow])
        .expect("affine fixture");
    let affine = AffineTransform::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0).expect("identity affine");
    assert_eq!(affine, AffineTransform::identity());
    let placement = ImagePlacement::new(
        image.clone(),
        Rect::new(0, 0, 2, 2),
        Rect::new(0, 0, 2, 2),
        ImageSampling::Nearest,
    )
    .expect("placement")
    .with_transform(ImageTransform::Affine(affine));
    let mut framebuffer = Framebuffer::new(2, 2).expect("surface");
    placement.render_to(&mut framebuffer);
    assert_eq!(
        [
            framebuffer.get_pixel(0, 0),
            framebuffer.get_pixel(1, 0),
            framebuffer.get_pixel(0, 1),
            framebuffer.get_pixel(1, 1),
        ],
        [Color::RED, Color::GREEN, Color::BLUE, yellow]
    );

    let horizontal_flip =
        AffineTransform::new(-1.0, 0.0, 0.0, 1.0, 1.0, 0.0).expect("horizontal flip affine");
    let placement = ImagePlacement::new(
        image,
        Rect::new(0, 0, 2, 2),
        Rect::new(0, 0, 2, 2),
        ImageSampling::Nearest,
    )
    .expect("placement")
    .with_transform(ImageTransform::Affine(horizontal_flip));
    let mut framebuffer = Framebuffer::new(2, 2).expect("surface");
    placement.render_to(&mut framebuffer);
    assert_eq!(
        [
            framebuffer.get_pixel(0, 0),
            framebuffer.get_pixel(1, 0),
            framebuffer.get_pixel(0, 1),
            framebuffer.get_pixel(1, 1),
        ],
        [Color::GREEN, Color::RED, yellow, Color::BLUE]
    );
}

#[test]
fn affine_transform_rejects_nonfinite_and_singular_matrices() {
    for coefficients in [
        [f64::NAN, 0.0, 0.0, 1.0, 0.0, 0.0],
        [f64::INFINITY, 0.0, 0.0, 1.0, 0.0, 0.0],
        [1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    ] {
        let error = AffineTransform::new(
            coefficients[0],
            coefficients[1],
            coefficients[2],
            coefficients[3],
            coefficients[4],
            coefficients[5],
        )
        .expect_err("invalid affine matrix");
        assert_eq!(error.code, ErrorCode::RenderFailure);
    }
}

#[test]
fn affine_transform_scales_into_a_clipped_destination() {
    let image = image_2x1();
    let transform = AffineTransform::new(0.5, 0.0, 0.0, 1.0, 0.25, 0.0).expect("scale affine");
    let placement = ImagePlacement::new(
        image,
        Rect::new(0, 0, 2, 1),
        Rect::new(0, 0, 4, 1),
        ImageSampling::Nearest,
    )
    .expect("placement")
    .with_transform(ImageTransform::Affine(transform));
    let mut framebuffer = Framebuffer::new(4, 1).expect("surface");
    placement.render_to(&mut framebuffer);
    assert_eq!(
        [
            framebuffer.get_pixel(0, 0),
            framebuffer.get_pixel(1, 0),
            framebuffer.get_pixel(2, 0),
            framebuffer.get_pixel(3, 0),
        ],
        [
            Color::TRANSPARENT,
            Color::RED,
            Color::BLUE,
            Color::TRANSPARENT
        ]
    );
}

#[test]
fn image_alpha_composites_over_existing_surface() {
    let image = RasterImage::new(1, 1, vec![Color::rgba(0, 0, 0, 128)]).expect("image");
    let placement = ImagePlacement::new(
        image,
        Rect::new(0, 0, 1, 1),
        Rect::new(0, 0, 1, 1),
        ImageSampling::Nearest,
    )
    .expect("placement");
    let mut list = DisplayList::default();
    list.append_image(placement).expect("append image");
    let mut framebuffer = Framebuffer::new(1, 1).expect("surface");
    framebuffer.clear(Color::WHITE);
    list.render_to(&mut framebuffer);
    assert_eq!(framebuffer.get_pixel(0, 0), Color::rgba(127, 127, 127, 255));
}
