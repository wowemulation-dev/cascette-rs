//! `.product.db` writer using generated protobuf types.
//!
//! The per-install `.product.db` file is a raw serialized
//! `proto_database::ProductInstall` protobuf message that the Blizzard
//! Agent reads to identify an installed product.

use prost::Message;

use cascette_proto::proto_database;

use crate::config::InstallConfig;
use crate::error::InstallationResult;
use crate::pipeline::manifests::BuildManifests;

/// Write `.product.db` to the installation root.
///
/// Constructs a `proto_database::ProductInstall` and serializes it
/// directly. Per-install `.product.db` files contain a single
/// `ProductInstall` (not wrapped in a `Database`).
pub async fn write_product_db(
    config: &InstallConfig,
    manifests: &BuildManifests,
) -> InstallationResult<()> {
    let path = config.install_path.join(".product.db");

    let version = manifests
        .build_config
        .client_version()
        .unwrap_or("")
        .to_string();

    let install_path_str = config.install_path.to_string_lossy().to_string();

    let language_setting = proto_database::LanguageSetting {
        language: Some(config.locale.clone()),
        option: Some(proto_database::LanguageOption::LangoptionTextAndSpeech.into()),
    };

    let settings = proto_database::UserSettings {
        install_path: Some(install_path_str),
        play_region: Some(config.region.clone()),
        game_subfolder: config.game_subfolder.clone(),
        selected_text_language: Some(config.locale.clone()),
        selected_speech_language: Some(config.locale.clone()),
        languages: vec![language_setting],
        ..Default::default()
    };

    let base_state = proto_database::BaseProductState {
        installed: Some(true),
        playable: Some(true),
        update_complete: Some(true),
        current_version_str: Some(version),
        active_build_key: config.build_config.clone(),
        ..Default::default()
    };

    let cached_state = proto_database::CachedProductState {
        base_product_state: Some(base_state),
        ..Default::default()
    };

    let product_install = proto_database::ProductInstall {
        uid: Some(config.product.clone()),
        product_code: Some(config.product.clone()),
        settings: Some(settings),
        cached_product_state: Some(cached_state),
        ..Default::default()
    };

    let data = product_install.encode_to_vec();
    tokio::fs::write(&path, &data).await?;

    Ok(())
}
