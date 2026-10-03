//! LibFuzzer target for `RasterImage::decode`, the image decoder for
//! application-supplied PNG, JPEG and raster-container bytes.

#![no_main]

use libfuzzer_sys::fuzz_target;
use metis_ui_lang::RasterImage;

fuzz_target!(|data: &[u8]| {
    if let Ok(image) = RasterImage::decode(data) {
        let area = u64::from(image.width()) * u64::from(image.height());
        assert_eq!(
            u64::try_from(image.pixels().len()).ok(),
            Some(area),
            "a decoded image holds one pixel per grid cell"
        );
    }
});
