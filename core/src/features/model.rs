use crate::features::assets::audio::model::AudioAsset;
use crate::features::assets::images::model::ImageAsset;
use crate::features::assets::videos::model::VideoAsset;
use crate::features::pages::model::Page;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Feature {
    Page(Page),
    Video(VideoAsset),
    Audio(AudioAsset),
    Image(ImageAsset),
}

#[derive(Serialize)]
#[serde(tag = "type", content = "data")]
pub enum JsonFeature {
    Page(crate::features::pages::model::JsonPage),
    Video(VideoAsset),
    Audio(AudioAsset),
    Image(ImageAsset),
}

impl From<Feature> for JsonFeature {
    fn from(f: Feature) -> Self {
        match f {
            Feature::Page(p) => JsonFeature::Page((&p).into()),
            Feature::Video(v) => JsonFeature::Video(v),
            Feature::Audio(a) => JsonFeature::Audio(a),
            Feature::Image(i) => JsonFeature::Image(i),
        }
    }
}

pub fn match_feature_to_type(f: &Feature) -> FeatureType {
    match f {
        Feature::Page(_) => FeatureType::Page,
        Feature::Video(_) => FeatureType::Video,
        Feature::Audio(_) => FeatureType::Audio,
        Feature::Image(_) => FeatureType::Image,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeatureType {
    Page,
    Video,
    Audio,
    Image,
}