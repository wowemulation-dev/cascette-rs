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
///
/// `total_downloaded` is the total number of bytes downloaded during the
/// install. It is recorded in `UpdateProgress.totalToDownload` to match
/// the reference Agent output.
pub async fn write_product_db(
    config: &InstallConfig,
    manifests: &BuildManifests,
    total_downloaded: u64,
) -> InstallationResult<()> {
    let path = config.install_path.join(".product.db");

    let version = manifests
        .build_config
        .client_version()
        .unwrap_or("")
        .to_string();

    let install_path_str = config.install_path.to_string_lossy().to_string();

    let lang_option = match (config.tag_query.has_speech, config.tag_query.has_text) {
        (true, true) => proto_database::LanguageOption::LangoptionTextAndSpeech,
        (true, false) => proto_database::LanguageOption::LangoptionSpeech,
        (false, true) => proto_database::LanguageOption::LangoptionText,
        (false, false) => proto_database::LanguageOption::LangoptionNone,
    };

    let language_setting = proto_database::LanguageSetting {
        language: Some(config.tag_query.locale.clone()),
        option: Some(lang_option.into()),
    };

    let selected_text = if config.tag_query.has_text {
        Some(config.tag_query.locale.clone())
    } else {
        None
    };
    let selected_speech = if config.tag_query.has_speech {
        Some(config.tag_query.locale.clone())
    } else {
        None
    };

    let settings = proto_database::UserSettings {
        install_path: Some(install_path_str),
        play_region: Some(config.region.clone()),
        desktop_shortcut: Some(proto_database::ShortcutOption::ShortcutAllUsers.into()),
        startmenu_shortcut: Some(proto_database::ShortcutOption::ShortcutAllUsers.into()),
        language_settings: Some(proto_database::LanguageSettingType::LangsettingAdvanced.into()),
        selected_text_language: selected_text,
        selected_speech_language: selected_speech,
        languages: vec![language_setting],
        additional_tags: Some(String::new()),
        version_branch: Some(String::new()),
        account_country: config.tag_query.account_country.clone(),
        geo_ip_country: config.tag_query.geo_ip_country.clone(),
        game_subfolder: config.game_subfolder.clone(),
    };

    // Resolve the install key (encoding key of the install manifest).
    let install_key = manifests
        .build_config
        .install()
        .first()
        .and_then(|bi| bi.encoding_key.clone());

    let tags_string = config.tag_query.build_info_tags();

    let base_state = proto_database::BaseProductState {
        installed: Some(true),
        playable: Some(true),
        update_complete: Some(true),
        background_download_available: Some(false),
        background_download_complete: Some(true),
        current_version_str: Some(version),
        decryption_key: Some(String::new()),
        completed_build_keys: config
            .build_config
            .as_ref()
            .map(|k| vec![k.clone()])
            .unwrap_or_default(),
        active_build_key: config.build_config.clone(),
        active_install_key: install_key,
        active_tag_string: Some(tags_string),
        ..Default::default()
    };

    let backfill = proto_database::BackfillProgress {
        progress: Some(0.0_f64),
        backgrounddownload: Some(false),
        paused: Some(false),
        ..Default::default()
    };

    let repair = proto_database::RepairProgress {
        progress: Some(0.0_f64),
    };

    let update = proto_database::UpdateProgress {
        last_disc_set_used: Some(String::new()),
        progress: Some(1.0_f64),
        disc_ignored: Some(false),
        total_to_download: Some(total_downloaded),
        download_remaining: Some(0),
        ..Default::default()
    };

    let cached_state = proto_database::CachedProductState {
        base_product_state: Some(base_state),
        backfill_progress: Some(backfill),
        repair_progress: Some(repair),
        update_progress: Some(update),
    };

    // Derive product family from product code (e.g., "wow_classic" -> "wow").
    let product_family = config
        .product
        .split('_')
        .next()
        .unwrap_or(&config.product)
        .to_string();

    let product_install = proto_database::ProductInstall {
        uid: Some(config.product.clone()),
        product_code: Some(config.product.clone()),
        settings: Some(settings),
        cached_product_state: Some(cached_state),
        product_operations: None,
        product_family: Some(product_family),
        hidden: Some(false),
        ..Default::default()
    };

    let data = product_install.encode_to_vec();
    tokio::fs::write(&path, &data).await?;

    Ok(())
}
