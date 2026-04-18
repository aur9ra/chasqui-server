use chasqui_core::config::ChasquiConfig;

pub fn path_to_identifier(config: &ChasquiConfig, path: &str) -> String {
    if config.serve_home && path == config.home_identifier {
        return config.home_identifier.clone();
    }
    path.to_string()
}

pub fn get_identifier_variants(raw_id: &str) -> Vec<String> {
    let mut variants = vec![raw_id.to_string()];

    if raw_id.contains('.') {
        if let Some(without_ext) = raw_id.rsplit_once('.').map(|(name, _)| name.to_string()) {
            variants.push(without_ext);
        }
    }

    variants
}