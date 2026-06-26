//! Обработка кадров: центр-кроп под соотношение экрана зрителя + ресайз.

use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{GenericImageView, ImageFormat, ImageReader};

const JPEG_QUALITY: u8 = 85;

#[derive(Debug, thiserror::Error)]
pub enum CropError {
    #[error("не удалось обработать изображение: {0}")]
    Image(#[from] image::ImageError),
    #[error("размеры должны быть больше нуля")]
    ZeroDimension,
}

/// Центр-кроп до соотношения `target_w`×`target_h`, затем точный ресайз.
/// Целочисленная арифметика в u64: без потери точности и переполнений.
#[allow(clippy::cast_possible_truncation)] // значения ограничены исходными размерами кадра
pub fn crop_to_fit(jpeg: &[u8], target_w: u32, target_h: u32) -> Result<Vec<u8>, CropError> {
    if target_w == 0 || target_h == 0 {
        return Err(CropError::ZeroDimension);
    }

    let img = ImageReader::with_format(Cursor::new(jpeg), ImageFormat::Jpeg).decode()?;
    let (src_w, src_h) = img.dimensions();

    // Ширина кропа при полной высоте; если не влезает — наоборот.
    let full_height_w = u64::from(src_h) * u64::from(target_w) / u64::from(target_h);
    let (crop_w, crop_h) = if full_height_w <= u64::from(src_w) {
        ((full_height_w as u32).max(1), src_h)
    } else {
        let full_width_h = u64::from(src_w) * u64::from(target_h) / u64::from(target_w);
        (src_w, (full_width_h as u32).max(1))
    };

    let x = (src_w - crop_w) / 2;
    let y = (src_h - crop_h) / 2;
    let result =
        img.crop_imm(x, y, crop_w, crop_h)
            .resize_exact(target_w, target_h, FilterType::Triangle);

    let mut buf = Vec::new();
    JpegEncoder::new_with_quality(&mut buf, JPEG_QUALITY).encode_image(&result)?;
    Ok(buf)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(clippy::cast_possible_truncation)] // x % 256 всегда влезает в u8
    fn jpeg(w: u32, h: u32) -> Vec<u8> {
        let img = image::RgbImage::from_fn(w, h, |x, y| {
            image::Rgb([(x % 256) as u8, (y % 256) as u8, 128])
        });
        let mut buf = Vec::new();
        JpegEncoder::new_with_quality(&mut buf, 80)
            .encode_image(&image::DynamicImage::ImageRgb8(img))
            .unwrap();
        buf
    }

    fn dimensions(jpeg: &[u8]) -> (u32, u32) {
        image::load_from_memory(jpeg).unwrap().dimensions()
    }

    #[test]
    fn landscape_source_to_portrait_target() {
        let out = crop_to_fit(&jpeg(400, 200), 90, 180).unwrap();
        assert_eq!(dimensions(&out), (90, 180));
    }

    #[test]
    fn portrait_source_to_landscape_target() {
        let out = crop_to_fit(&jpeg(200, 400), 180, 90).unwrap();
        assert_eq!(dimensions(&out), (180, 90));
    }

    #[test]
    fn same_aspect_is_pure_resize() {
        let out = crop_to_fit(&jpeg(400, 800), 100, 200).unwrap();
        assert_eq!(dimensions(&out), (100, 200));
    }

    #[test]
    fn zero_dimension_is_rejected() {
        assert!(matches!(
            crop_to_fit(&jpeg(10, 10), 0, 100),
            Err(CropError::ZeroDimension)
        ));
    }

    #[test]
    fn garbage_input_is_rejected() {
        assert!(matches!(
            crop_to_fit(b"not a jpeg", 100, 100),
            Err(CropError::Image(_))
        ));
    }
}
