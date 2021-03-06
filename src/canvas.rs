pub use libremarkable::framebuffer::{
    cgmath::Point2, cgmath::Vector2, common::color, common::mxcfb_rect, common::DISPLAYHEIGHT,
    common::DISPLAYWIDTH, core::Framebuffer, FramebufferBase, FramebufferDraw, FramebufferIO,
    FramebufferRefresh,
};
use libremarkable::framebuffer::{
    common::display_temp, common::dither_mode, common::waveform_mode, refresh::PartialRefreshMode,
};
use libremarkable::image;
use std::ops::DerefMut;

pub struct Canvas<'a> {
    framebuffer: Box<Framebuffer<'a>>,
}

impl<'a> Canvas<'a> {
    pub fn new() -> Self {
        Self {
            framebuffer: Box::new(Framebuffer::new("/dev/fb0")),
        }
    }

    pub fn framebuffer_mut(&mut self) -> &'static mut Framebuffer<'static> {
        unsafe {
            std::mem::transmute::<_, &'static mut Framebuffer<'static>>(
                self.framebuffer.deref_mut(),
            )
        }
    }

    pub fn clear(&mut self) {
        self.framebuffer_mut().clear();
    }

    pub fn update_full(&mut self) {
        self.framebuffer_mut().full_refresh(
            waveform_mode::WAVEFORM_MODE_GC16,
            display_temp::TEMP_USE_REMARKABLE_DRAW,
            dither_mode::EPDC_FLAG_USE_DITHERING_PASSTHROUGH,
            0,
            true,
        );
    }

    pub fn update_partial(&mut self, region: &mxcfb_rect) {
        self.framebuffer_mut().partial_refresh(
            region,
            PartialRefreshMode::Async,
            waveform_mode::WAVEFORM_MODE_GC16_FAST,
            display_temp::TEMP_USE_REMARKABLE_DRAW,
            dither_mode::EPDC_FLAG_USE_REMARKABLE_DITHER,
            0, // See documentation on DRAWING_QUANT_BITS in libremarkable/framebuffer/common.rs
            false,
        );
    }

    pub fn draw_text(&mut self, pos: Point2<Option<i32>>, text: &str, size: f32) -> mxcfb_rect {
        let mut pos = pos;
        if pos.x.is_none() || pos.y.is_none() {
            // Do dryrun to get text size
            let rect = self.framebuffer_mut().draw_text(
                Point2 {
                    x: 0.0,
                    y: DISPLAYHEIGHT as f32,
                },
                text.to_owned(),
                size,
                color::BLACK,
                true,
            );

            if pos.x.is_none() {
                // Center horizontally
                pos.x = Some(DISPLAYWIDTH as i32 / 2 - rect.width as i32 / 2);
            }

            if pos.y.is_none() {
                // Center vertically
                pos.y = Some(DISPLAYHEIGHT as i32 / 2 - rect.height as i32 / 2);
            }
        }
        let pos = Point2 {
            x: pos.x.unwrap() as f32,
            y: pos.y.unwrap() as f32,
        };

        self.framebuffer_mut()
            .draw_text(pos, text.to_owned(), size, color::BLACK, false)
    }

    pub fn draw_rect(
        &mut self,
        pos: Point2<Option<i32>>,
        size: Vector2<u32>,
        border_px: u32,
    ) -> mxcfb_rect {
        let mut pos = pos;
        if pos.x.is_none() || pos.y.is_none() {
            if pos.x.is_none() {
                // Center horizontally
                pos.x = Some(DISPLAYWIDTH as i32 / 2 - size.x as i32 / 2);
            }

            if pos.y.is_none() {
                // Center vertically
                pos.y = Some(DISPLAYHEIGHT as i32 / 2 - size.y as i32 / 2);
            }
        }
        let pos = Point2 {
            x: pos.x.unwrap(),
            y: pos.y.unwrap(),
        };

        self.framebuffer_mut()
            .draw_rect(pos, size, border_px, color::BLACK);
        mxcfb_rect {
            top: pos.y as u32,
            left: pos.x as u32,
            width: size.x,
            height: size.y,
        }
    }

    pub fn fill_rect(
        &mut self,
        pos: Point2<Option<i32>>,
        size: Vector2<u32>,
        clr: color,
    ) -> mxcfb_rect {
        let mut pos = pos;
        if pos.x.is_none() || pos.y.is_none() {
            if pos.x.is_none() {
                // Center horizontally
                pos.x = Some(DISPLAYWIDTH as i32 / 2 - size.x as i32 / 2);
            }

            if pos.y.is_none() {
                // Center vertically
                pos.y = Some(DISPLAYHEIGHT as i32 / 2 - size.y as i32 / 2);
            }
        }
        let pos = Point2 {
            x: pos.x.unwrap(),
            y: pos.y.unwrap(),
        };

        self.framebuffer_mut().fill_rect(pos, size, clr);
        mxcfb_rect {
            top: pos.y as u32,
            left: pos.x as u32,
            width: size.x,
            height: size.y,
        }
    }

    pub fn draw_button(
        &mut self,
        pos: Point2<Option<i32>>,
        text: &str,
        font_size: f32,
        vgap: u32,
        hgap: u32,
    ) -> mxcfb_rect {
        let text_rect = self.draw_text(pos, text, font_size);
        self.draw_rect(
            Point2 {
                x: Some((text_rect.left - hgap) as i32),
                y: Some((text_rect.top - vgap) as i32),
            },
            Vector2 {
                x: hgap + text_rect.width + hgap,
                y: vgap + text_rect.height + vgap,
            },
            5,
        )
    }

    /// Image that can be overlayed white respecting the previous pixels.
    /// This way transparent images can work.
    fn calc_overlay_image(
        &mut self,
        pos: Point2<i32>,
        img: &image::DynamicImage,
    ) -> image::RgbImage {
        let rgba = img.to_rgba();
        let mut rgb = img.to_rgb();
        for (x, y, pixel) in rgba.enumerate_pixels() {
            let color_pix = [
                pixel[0] as f32 / 255.0,
                pixel[1] as f32 / 255.0,
                pixel[2] as f32 / 255.0,
            ];
            let color_alpha = (255 - pixel[3]) as f32 / 255.0;

            let orig_pixel = self
                .framebuffer_mut()
                .read_pixel(Point2 {
                    x: pos.x as u32 + x as u32,
                    y: pos.y as u32 + y as u32,
                })
                .to_rgb8();
            let new_rgb_f32 = image::Rgb([
                color_pix[0] * (1.0 - color_alpha) + (orig_pixel[0] as f32 / 255.0) * color_alpha,
                color_pix[1] * (1.0 - color_alpha) + (orig_pixel[1] as f32 / 255.0) * color_alpha,
                color_pix[2] * (1.0 - color_alpha) + (orig_pixel[2] as f32 / 255.0) * color_alpha,
            ]);

            let new_rgb_u8: image::Rgb<u8> = image::Rgb([
                (new_rgb_f32[0] * 255.0) as u8,
                (new_rgb_f32[1] * 255.0) as u8,
                (new_rgb_f32[2] * 255.0) as u8,
            ]);

            rgb.put_pixel(x, y, new_rgb_u8);
        }

        rgb
    }

    pub fn draw_image(
        &mut self,
        pos: Point2<i32>,
        img: &image::DynamicImage,
        is_transparent: bool,
    ) -> mxcfb_rect {
        let rgb_img = if is_transparent {
            self.calc_overlay_image(pos, img)
        } else {
            img.to_rgb()
        };

        self.framebuffer_mut().draw_image(&rgb_img, pos);
        mxcfb_rect {
            top: pos.y as u32,
            left: pos.x as u32,
            width: rgb_img.width(),
            height: rgb_img.height(),
        }
    }

    pub fn is_hitting(pos: Point2<u16>, hitbox: mxcfb_rect) -> bool {
        (pos.x as u32) >= hitbox.left
            && (pos.x as u32) < (hitbox.left + hitbox.width)
            && (pos.y as u32) >= hitbox.top
            && (pos.y as u32) < (hitbox.top + hitbox.height)
    }
}
