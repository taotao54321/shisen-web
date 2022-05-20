use anyhow::anyhow;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{HtmlAudioElement, HtmlImageElement, ImageBitmap};

use crate::shisen::TILE_KIND_COUNT;
use crate::util;

#[derive(Debug)]
pub struct Asset {
    imgs_tile: Vec<ImageBitmap>,
    sound_pick: HtmlAudioElement,
}

impl Asset {
    pub async fn load() -> anyhow::Result<Self> {
        let mut imgs_tile = Vec::<ImageBitmap>::with_capacity(TILE_KIND_COUNT);
        for i in 0..TILE_KIND_COUNT {
            let id = format!("asset_image_tile_{i:02}");
            let img = load_image_bitmap(&id).await?;
            imgs_tile.push(img);
        }

        let sound_pick = load_html_audio_element("asset_sound_pick");

        Ok(Self {
            imgs_tile,
            sound_pick,
        })
    }

    pub fn image_tile(&self, tile: usize) -> &ImageBitmap {
        &self.imgs_tile[tile]
    }

    pub fn sound_pick(&self) -> &HtmlAudioElement {
        &self.sound_pick
    }
}

async fn load_image_bitmap(id: &str) -> anyhow::Result<ImageBitmap> {
    let elem: HtmlImageElement = util::get_element_by_id(id);

    let future = util::window()
        .create_image_bitmap_with_html_image_element(&elem)
        .map(JsFuture::from)
        .map_err(|_| anyhow!("element '{id}': cannot start loading"))?;

    let img = future
        .await
        .map_err(|_| anyhow!("element '{id}': cannot load"))?
        .dyn_into()
        .expect("expected ImageBitmap");

    Ok(img)
}

fn load_html_audio_element(id: &str) -> HtmlAudioElement {
    util::get_element_by_id(id)
}
