//! Обработка кадров: поворот по EXIF, центр-кроп под соотношение экрана зрителя + ресайз.

use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageDecoder, ImageFormat, ImageReader};

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

    // Телефоны не поворачивают пиксели, а пишут поворот в EXIF —
    // применяем его до кропа, иначе вертикальный кадр ляжет набок.
    let mut decoder =
        ImageReader::with_format(Cursor::new(jpeg), ImageFormat::Jpeg).into_decoder()?;
    let orientation = decoder.orientation()?;
    let mut img = DynamicImage::from_decoder(decoder)?;
    img.apply_orientation(orientation);
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
    use image::ImageEncoder;

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

    /// TIFF-блок EXIF с единственным тегом Orientation (0x0112).
    fn exif_orientation(value: u16) -> Vec<u8> {
        let mut tiff = b"II*\0\x08\0\0\0".to_vec(); // little-endian, IFD0 со смещения 8
        tiff.extend_from_slice(&1u16.to_le_bytes()); // одна запись
        tiff.extend_from_slice(&0x0112u16.to_le_bytes()); // тег Orientation
        tiff.extend_from_slice(&3u16.to_le_bytes()); // тип SHORT
        tiff.extend_from_slice(&1u32.to_le_bytes()); // одно значение
        tiff.extend_from_slice(&u32::from(value).to_le_bytes()); // само значение
        tiff.extend_from_slice(&0u32.to_le_bytes()); // следующего IFD нет
        tiff
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
    fn exif_orientation_is_applied_before_crop() {
        // Как с сенсора: 400×300, верх белый, низ чёрный. EXIF 6 — «для показа
        // повернуть на 90° по часовой»: белое должно оказаться справа.
        let img = image::RgbImage::from_fn(400, 300, |_, y| {
            if y < 150 {
                image::Rgb([255, 255, 255])
            } else {
                image::Rgb([0, 0, 0])
            }
        });
        let mut buf = Vec::new();
        let mut encoder = JpegEncoder::new_with_quality(&mut buf, 90);
        encoder.set_exif_metadata(exif_orientation(6)).unwrap();
        encoder
            .encode_image(&image::DynamicImage::ImageRgb8(img))
            .unwrap();

        let out = image::load_from_memory(&crop_to_fit(&buf, 300, 400).unwrap())
            .unwrap()
            .to_luma8();
        assert_eq!(out.dimensions(), (300, 400));
        // без поворота было бы наоборот: светлый верх, тёмный низ
        assert!(
            out.get_pixel(30, 30)[0] < 64,
            "левый верх должен быть тёмным"
        );
        assert!(
            out.get_pixel(270, 370)[0] > 192,
            "правый низ должен быть светлым"
        );
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
