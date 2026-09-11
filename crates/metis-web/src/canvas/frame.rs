//! Borrowed frame contract shared by browser consumers.

/// A borrowed row-major straight-alpha RGBA8 frame.
///
/// Implement this seam on a consumer-owned presentation value. The browser
/// host validates the dimensions and byte count before handing the borrowed
/// view to the Moirai canvas provider, so the frame does not acquire a second
/// pixel allocation or retain format-specific metadata.
pub trait CanvasFrame {
    /// Returns the frame width in device pixels.
    fn width(&self) -> u32;

    /// Returns the frame height in device pixels.
    fn height(&self) -> u32;

    /// Returns contiguous row-major RGBA8 bytes.
    fn rgba(&self) -> &[u8];
}

#[cfg(test)]
mod tests {
    use super::CanvasFrame;

    struct Fixture {
        pixels: [u8; 8],
    }

    impl CanvasFrame for Fixture {
        fn width(&self) -> u32 {
            2
        }

        fn height(&self) -> u32 {
            1
        }

        fn rgba(&self) -> &[u8] {
            &self.pixels
        }
    }

    #[test]
    fn frame_seam_preserves_borrowed_dimensions_and_storage() {
        let fixture = Fixture { pixels: [1; 8] };
        assert_eq!(fixture.width(), 2);
        assert_eq!(fixture.height(), 1);
        assert_eq!(fixture.rgba(), &[1; 8]);
    }
}
