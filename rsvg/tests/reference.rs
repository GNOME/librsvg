//! Tests with reference images.
//!
//! This is the bulk of librsvg's black-box tests.  In principle, each test takes an SVG file, renders
//! it to a raster image, and compares that image to a reference image stored on disk.  If the images
//! are "too different", the test fails.  We allow for minor differences in rendering to account for
//! antialiasing artifacts, floating-point variations, and such.
//!

use rsvg::tests_only::{SharedImageSurface, SurfaceType};
use rsvg::{CairoRenderer, IntrinsicDimensions, Length, Loader};

use rsvg::test_utils::reference_utils::{Compare, Evaluate, Reference};
use rsvg::test_utils::{setup_font_map, setup_language};
use rsvg::{test_compare_render_output, test_svg_reference};

use std::path::{Path, PathBuf};

// The original reference images from the SVG1.1 test suite are at 72 DPI.
const TEST_SUITE_DPI: f64 = 72.0;

// https://gitlab.gnome.org/GNOME/librsvg/issues/91
//
// We were computing some offsets incorrectly if the initial transformation matrix
// passed to rsvg_handle_render_cairo() was not the identity matrix.  So,
// we create a surface with a "frame" around the destination for the image,
// and then only consider the pixels inside the frame.  This will require us
// to have a non-identity transformation (i.e. a translation matrix), which
// will test for this bug.
//
// The frame size is meant to be a ridiculous number to simulate an arbitrary
// offset.
const FRAME_SIZE: i32 = 47;

fn reference_test(path: &Path) {
    setup_language();
    setup_font_map();

    let path_base_name = path.file_stem().unwrap().to_string_lossy().into_owned();
    if path_base_name.starts_with("ignore") {
        return;
    }

    let reference = reference_path(path);

    let handle = Loader::new()
        .read_path(path)
        .unwrap_or_else(|e| panic!("could not load: {}", e));

    let renderer = CairoRenderer::new(&handle)
        .test_mode(true)
        .with_dpi(TEST_SUITE_DPI, TEST_SUITE_DPI);
    let (width, height) = image_size(renderer.intrinsic_dimensions(), TEST_SUITE_DPI);

    let surface = cairo::ImageSurface::create(
        cairo::Format::ARgb32,
        width + 2 * FRAME_SIZE,
        height + 2 * FRAME_SIZE,
    )
    .unwrap();

    {
        let cr = cairo::Context::new(&surface).expect("Failed to create a cairo context");
        cr.translate(f64::from(FRAME_SIZE), f64::from(FRAME_SIZE));
        renderer
            .render_document(
                &cr,
                &cairo::Rectangle::new(0.0, 0.0, f64::from(width), f64::from(height)),
            )
            .unwrap();
    }

    let surface = extract_rectangle(&surface, FRAME_SIZE, FRAME_SIZE, width, height).unwrap();

    let output_surf = SharedImageSurface::wrap(surface, SurfaceType::SRgb).unwrap();

    Reference::from_png(reference)
        .compare(&output_surf)
        .evaluate(&output_surf, &path_base_name);
}

/// Turns `/foo/bar/baz.svg` into `/foo/bar/baz-ref.png`.
fn reference_path(path: &Path) -> PathBuf {
    let basename = path.file_stem().unwrap();

    let mut reference_filename = basename.to_string_lossy().into_owned();
    reference_filename.push_str("-ref.png");

    path.with_file_name(reference_filename)
}

fn extract_rectangle(
    source: &cairo::ImageSurface,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) -> Result<cairo::ImageSurface, cairo::Error> {
    let dest = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h)?;
    let cr = cairo::Context::new(&dest).expect("Failed to create a cairo context");
    cr.set_source_surface(source, f64::from(-x), f64::from(-y))
        .unwrap();
    cr.paint().unwrap();
    Ok(dest)
}

/// Computes the (width, height) pixel size at which an SVG should be rendered, based on its intrinsic dimensions.
///
/// # Panics:
///
/// Will panic if none of the following conditions are met:
///
/// * Width and height both exist
/// * Width and height do not exist, but viewBox exists.
fn image_size(dim: IntrinsicDimensions, dpi: f64) -> (i32, i32) {
    let IntrinsicDimensions {
        width,
        height,
        vbox,
    } = dim;

    use rsvg::LengthUnit::*;

    if !(has_supported_unit(&width) && has_supported_unit(&height)) {
        panic!("SVG has unsupported unit type in width or height");
    }

    #[rustfmt::skip]
    let (width, height) = match (width, height, vbox) {
        (Length { length: w, unit: Percent },
         Length { length: h, unit: Percent }, vbox) if w == 1.0 && h == 1.0 => {
            if let Some(vbox) = vbox {
                (vbox.width(), vbox.height())
            } else {
                panic!("SVG with percentage width/height must have a viewBox");
            }
        }

        (Length { length: _, unit: Percent },
         Length { length: _, unit: Percent }, _) => {
            panic!("Test suite only supports percentage width/height at 100%");
        }

        (w, h, _) => {
            (normalize(&w, dpi), normalize(&h, dpi))
        }
    };

    // Keep in sync with c_api.rs
    let width = checked_i32(width.round());
    let height = checked_i32(height.round());

    (width, height)
}

// Keep in sync with c_api.rs
fn checked_i32(x: f64) -> i32 {
    cast::i32(x).expect("overflow when converting f64 to i32")
}

fn has_supported_unit(l: &Length) -> bool {
    use rsvg::LengthUnit::*;

    matches!(l.unit, Percent | Px | In | Cm | Mm | Pt | Pc)
}

const POINTS_PER_INCH: f64 = 72.0;
const CM_PER_INCH: f64 = 2.54;
const MM_PER_INCH: f64 = 25.4;
const PICA_PER_INCH: f64 = 6.0;

fn normalize(l: &Length, dpi: f64) -> f64 {
    use rsvg::LengthUnit::*;

    match l.unit {
        Px => l.length,
        In => l.length * dpi,
        Cm => l.length * dpi / CM_PER_INCH,
        Mm => l.length * dpi / MM_PER_INCH,
        Pt => l.length * dpi / POINTS_PER_INCH,
        Pc => l.length * dpi / PICA_PER_INCH,
        _ => panic!("unsupported length unit"),
    }
}

fn reftest(filename: &str) {
    let mut full_filename = PathBuf::new();
    full_filename.push("tests/fixtures/reftests");
    full_filename.push(filename);

    reference_test(&full_filename);
}

macro_rules! t {
    ($test_name:ident, $filename:expr) => {
        #[test]
        fn $test_name() {
            reftest($filename);
        }
    };
}

#[rustfmt::skip]
mod tests {
    use super::*;

    t!(a_pseudo_class_svg,                                          "a-pseudo-class.svg");
    t!(bug483_attribute_selectors_svg,                              "bug483-attribute-selectors.svg");
    t!(bug525_specificity_svg,                                      "bug525-specificity.svg");
    t!(css_import_svg,                                              "css-import.svg");
    t!(css_import_url_svg,                                          "css-import-url.svg");
    t!(duplicate_id_svg,                                            "duplicate-id.svg");
    t!(filter_component_transfer_from_reference_page_svg,           "filter-component-transfer-from-reference-page.svg");
    t!(filter_conv_bounds_svg,                                      "filter-conv-bounds.svg");
    t!(filter_conv_divisor_svg,                                     "filter-conv-divisor.svg");
    t!(filter_effects_region_svg,                                   "filter-effects-region.svg");
    t!(filter_image_from_reference_page_svg,                        "filter-image-from-reference-page.svg");
    t!(filter_kernel_unit_length_svg,                               "filter-kernel-unit-length.svg");
    t!(filter_offset_svg,                                           "filter-offset.svg");
    t!(font_shorthand_svg,                                          "font-shorthand.svg");
    t!(gzip_compressed_svg,                                         "gzip-compressed.svg");
    t!(hexchat_svg,                                                 "hexchat.svg");
    t!(ignore_filter_composite_color_interpolation_filters_svg,     "ignore-filter-composite-color-interpolation-filters.svg");
    t!(include_compressed_svg,                                      "include-compressed.svg");
    t!(include_fallback_svg,                                        "include-fallback.svg");
    t!(include_text_svg,                                            "include-text.svg");
    t!(rtl_tspan_svg,                                               "rtl-tspan.svg");
    t!(specificity_svg,                                             "specificity.svg");
    t!(structural_pseudo_classes_svg,                               "structural-pseudo-classes.svg");
    t!(style_with_xml_comments_svg,                                 "style-with-xml-comments.svg");
    t!(system_language_de_svg,                                      "system-language-de.svg");
    t!(system_language_en_svg,                                      "system-language-en.svg");
    t!(system_language_other_svg,                                   "system-language-other.svg");
    t!(text_objectboundingbox_svg,                                  "text-objectBoundingBox.svg");
    t!(xml_lang_css_inherit_svg,                                    "xml-lang-css-inherit.svg");
    t!(xml_lang_css_svg,                                            "xml-lang-css.svg");
    t!(adwaita_ac_adapter_symbolic_svg,                             "adwaita/ac-adapter-symbolic.svg");
    t!(adwaita_accessories_calculator_symbolic_svg,                 "adwaita/accessories-calculator-symbolic.svg");
    t!(adwaita_accessories_character_map_symbolic_svg,              "adwaita/accessories-character-map-symbolic.svg");
    t!(adwaita_accessories_dictionary_symbolic_svg,                 "adwaita/accessories-dictionary-symbolic.svg");
    t!(adwaita_accessories_text_editor_symbolic_svg,                "adwaita/accessories-text-editor-symbolic.svg");
    t!(adwaita_action_unavailable_symbolic_svg,                     "adwaita/action-unavailable-symbolic.svg");
    t!(adwaita_address_book_new_symbolic_svg,                       "adwaita/address-book-new-symbolic.svg");
    t!(adwaita_airplane_mode_symbolic_svg,                          "adwaita/airplane-mode-symbolic.svg");
    t!(adwaita_alarm_symbolic_svg,                                  "adwaita/alarm-symbolic.svg");
    t!(adwaita_applets_screenshooter_symbolic_svg,                  "adwaita/applets-screenshooter-symbolic.svg");
    t!(adwaita_application_certificate_symbolic_svg,                "adwaita/application-certificate-symbolic.svg");
    t!(adwaita_application_exit_symbolic_svg,                       "adwaita/application-exit-symbolic.svg");
    t!(adwaita_application_rss_xml_symbolic_svg,                    "adwaita/application-rss-xml-symbolic.svg");
    t!(adwaita_applications_engineering_symbolic_svg,               "adwaita/applications-engineering-symbolic.svg");
    t!(adwaita_applications_games_symbolic_svg,                     "adwaita/applications-games-symbolic.svg");
    t!(adwaita_applications_graphics_symbolic_svg,                  "adwaita/applications-graphics-symbolic.svg");
    t!(adwaita_applications_multimedia_symbolic_svg,                "adwaita/applications-multimedia-symbolic.svg");
    t!(adwaita_applications_science_symbolic_svg,                   "adwaita/applications-science-symbolic.svg");
    t!(adwaita_applications_system_symbolic_svg,                    "adwaita/applications-system-symbolic.svg");
    t!(adwaita_applications_utilities_symbolic_svg,                 "adwaita/applications-utilities-symbolic.svg");
    t!(adwaita_application_x_addon_symbolic_svg,                    "adwaita/application-x-addon-symbolic.svg");
    t!(adwaita_application_x_appliance_symbolic_svg,                "adwaita/application-x-appliance-symbolic.svg");
    t!(adwaita_application_x_executable_symbolic_svg,               "adwaita/application-x-executable-symbolic.svg");
    t!(adwaita_application_x_firmware_symbolic_svg,                 "adwaita/application-x-firmware-symbolic.svg");
    t!(adwaita_appointment_missed_symbolic_svg,                     "adwaita/appointment-missed-symbolic.svg");
    t!(adwaita_appointment_new_symbolic_svg,                        "adwaita/appointment-new-symbolic.svg");
    t!(adwaita_appointment_soon_symbolic_svg,                       "adwaita/appointment-soon-symbolic.svg");
    t!(adwaita_audio_card_symbolic_svg,                             "adwaita/audio-card-symbolic.svg");
    t!(adwaita_audio_headphones_symbolic_svg,                       "adwaita/audio-headphones-symbolic.svg");
    t!(adwaita_audio_headset_symbolic_svg,                          "adwaita/audio-headset-symbolic.svg");
    t!(adwaita_audio_input_microphone_symbolic_svg,                 "adwaita/audio-input-microphone-symbolic.svg");
    t!(adwaita_audio_speakers_symbolic_svg,                         "adwaita/audio-speakers-symbolic.svg");
    t!(adwaita_audio_volume_high_symbolic_svg,                      "adwaita/audio-volume-high-symbolic.svg");
    t!(adwaita_audio_volume_low_symbolic_svg,                       "adwaita/audio-volume-low-symbolic.svg");
    t!(adwaita_audio_volume_medium_symbolic_svg,                    "adwaita/audio-volume-medium-symbolic.svg");
    t!(adwaita_audio_volume_muted_symbolic_svg,                     "adwaita/audio-volume-muted-symbolic.svg");
    t!(adwaita_audio_volume_overamplified_symbolic_svg,             "adwaita/audio-volume-overamplified-symbolic.svg");
    t!(adwaita_audio_x_generic_symbolic_svg,                        "adwaita/audio-x-generic-symbolic.svg");
    t!(adwaita_auth_fingerprint_symbolic_svg,                       "adwaita/auth-fingerprint-symbolic.svg");
    t!(adwaita_auth_smartcard_symbolic_svg,                         "adwaita/auth-smartcard-symbolic.svg");
    t!(adwaita_avatar_default_symbolic_svg,                         "adwaita/avatar-default-symbolic.svg");
    t!(adwaita_battery_caution_charging_symbolic_svg,               "adwaita/battery-caution-charging-symbolic.svg");
    t!(adwaita_battery_caution_symbolic_svg,                        "adwaita/battery-caution-symbolic.svg");
    t!(adwaita_battery_empty_charging_symbolic_svg,                 "adwaita/battery-empty-charging-symbolic.svg");
    t!(adwaita_battery_empty_symbolic_svg,                          "adwaita/battery-empty-symbolic.svg");
    t!(adwaita_battery_full_charged_symbolic_svg,                   "adwaita/battery-full-charged-symbolic.svg");
    t!(adwaita_battery_full_charging_symbolic_svg,                  "adwaita/battery-full-charging-symbolic.svg");
    t!(adwaita_battery_full_symbolic_svg,                           "adwaita/battery-full-symbolic.svg");
    t!(adwaita_battery_good_charging_symbolic_svg,                  "adwaita/battery-good-charging-symbolic.svg");
    t!(adwaita_battery_good_symbolic_svg,                           "adwaita/battery-good-symbolic.svg");
    t!(adwaita_battery_low_charging_symbolic_svg,                   "adwaita/battery-low-charging-symbolic.svg");
    t!(adwaita_battery_low_symbolic_svg,                            "adwaita/battery-low-symbolic.svg");
    t!(adwaita_battery_missing_symbolic_svg,                        "adwaita/battery-missing-symbolic.svg");
    t!(adwaita_battery_symbolic_svg,                                "adwaita/battery-symbolic.svg");
    t!(adwaita_bluetooth_active_symbolic_svg,                       "adwaita/bluetooth-active-symbolic.svg");
    t!(adwaita_bluetooth_disabled_symbolic_svg,                     "adwaita/bluetooth-disabled-symbolic.svg");
    t!(adwaita_bluetooth_symbolic_svg,                              "adwaita/bluetooth-symbolic.svg");
    t!(adwaita_bookmark_new_symbolic_svg,                           "adwaita/bookmark-new-symbolic.svg");
    t!(adwaita_call_missed_symbolic_svg,                            "adwaita/call-missed-symbolic.svg");
    t!(adwaita_call_start_symbolic_svg,                             "adwaita/call-start-symbolic.svg");
    t!(adwaita_call_stop_symbolic_svg,                              "adwaita/call-stop-symbolic.svg");
    t!(adwaita_camera_photo_symbolic_svg,                           "adwaita/camera-photo-symbolic.svg");
    t!(adwaita_camera_switch_symbolic_svg,                          "adwaita/camera-switch-symbolic.svg");
    t!(adwaita_camera_video_symbolic_svg,                           "adwaita/camera-video-symbolic.svg");
    t!(adwaita_camera_web_symbolic_svg,                             "adwaita/camera-web-symbolic.svg");
    t!(adwaita_changes_allow_symbolic_svg,                          "adwaita/changes-allow-symbolic.svg");
    t!(adwaita_changes_prevent_symbolic_svg,                        "adwaita/changes-prevent-symbolic.svg");
    t!(adwaita_channel_insecure_symbolic_svg,                       "adwaita/channel-insecure-symbolic.svg");
    t!(adwaita_channel_secure_symbolic_svg,                         "adwaita/channel-secure-symbolic.svg");
    t!(adwaita_checkbox_checked_symbolic_svg,                       "adwaita/checkbox-checked-symbolic.svg");
    t!(adwaita_checkbox_mixed_symbolic_svg,                         "adwaita/checkbox-mixed-symbolic.svg");
    t!(adwaita_checkbox_symbolic_svg,                               "adwaita/checkbox-symbolic.svg");
    t!(adwaita_colorimeter_colorhug_symbolic_svg,                   "adwaita/colorimeter-colorhug-symbolic.svg");
    t!(adwaita_color_select_symbolic_svg,                           "adwaita/color-select-symbolic.svg");
    t!(adwaita_computer_apple_ipad_symbolic_svg,                    "adwaita/computer-apple-ipad-symbolic.svg");
    t!(adwaita_computer_fail_symbolic_svg,                          "adwaita/computer-fail-symbolic.svg");
    t!(adwaita_computer_symbolic_svg,                               "adwaita/computer-symbolic.svg");
    t!(adwaita_contact_new_symbolic_svg,                            "adwaita/contact-new-symbolic.svg");
    t!(adwaita_content_loading_symbolic_svg,                        "adwaita/content-loading-symbolic.svg");
    t!(adwaita_daytime_sunrise_symbolic_svg,                        "adwaita/daytime-sunrise-symbolic.svg");
    t!(adwaita_daytime_sunset_symbolic_svg,                         "adwaita/daytime-sunset-symbolic.svg");
    t!(adwaita_dialog_error_symbolic_svg,                           "adwaita/dialog-error-symbolic.svg");
    t!(adwaita_dialog_information_symbolic_svg,                     "adwaita/dialog-information-symbolic.svg");
    t!(adwaita_dialog_password_symbolic_svg,                        "adwaita/dialog-password-symbolic.svg");
    t!(adwaita_dialog_question_symbolic_svg,                        "adwaita/dialog-question-symbolic.svg");
    t!(adwaita_dialog_warning_symbolic_svg,                         "adwaita/dialog-warning-symbolic.svg");
    t!(adwaita_display_brightness_symbolic_svg,                     "adwaita/display-brightness-symbolic.svg");
    t!(adwaita_display_projector_symbolic_svg,                      "adwaita/display-projector-symbolic.svg");
    t!(adwaita_document_edit_symbolic_svg,                          "adwaita/document-edit-symbolic.svg");
    t!(adwaita_document_new_symbolic_svg,                           "adwaita/document-new-symbolic.svg");
    t!(adwaita_document_open_recent_symbolic_svg,                   "adwaita/document-open-recent-symbolic.svg");
    t!(adwaita_document_open_symbolic_svg,                          "adwaita/document-open-symbolic.svg");
    t!(adwaita_document_page_setup_symbolic_svg,                    "adwaita/document-page-setup-symbolic.svg");
    t!(adwaita_document_print_preview_symbolic_svg,                 "adwaita/document-print-preview-symbolic.svg");
    t!(adwaita_document_print_symbolic_svg,                         "adwaita/document-print-symbolic.svg");
    t!(adwaita_document_properties_symbolic_svg,                    "adwaita/document-properties-symbolic.svg");
    t!(adwaita_document_revert_symbolic_rtl_svg,                    "adwaita/document-revert-symbolic-rtl.svg");
    t!(adwaita_document_revert_symbolic_svg,                        "adwaita/document-revert-symbolic.svg");
    t!(adwaita_document_save_as_symbolic_svg,                       "adwaita/document-save-as-symbolic.svg");
    t!(adwaita_document_save_symbolic_svg,                          "adwaita/document-save-symbolic.svg");
    t!(adwaita_document_send_symbolic_svg,                          "adwaita/document-send-symbolic.svg");
    t!(adwaita_drive_harddisk_ieee1394_symbolic_svg,                "adwaita/drive-harddisk-ieee1394-symbolic.svg");
    t!(adwaita_drive_harddisk_solidstate_symbolic_svg,              "adwaita/drive-harddisk-solidstate-symbolic.svg");
    t!(adwaita_drive_harddisk_symbolic_svg,                         "adwaita/drive-harddisk-symbolic.svg");
    t!(adwaita_drive_harddisk_system_symbolic_svg,                  "adwaita/drive-harddisk-system-symbolic.svg");
    t!(adwaita_drive_harddisk_usb_symbolic_svg,                     "adwaita/drive-harddisk-usb-symbolic.svg");
    t!(adwaita_drive_multidisk_symbolic_svg,                        "adwaita/drive-multidisk-symbolic.svg");
    t!(adwaita_drive_optical_symbolic_svg,                          "adwaita/drive-optical-symbolic.svg");
    t!(adwaita_drive_removable_media_symbolic_svg,                  "adwaita/drive-removable-media-symbolic.svg");
    t!(adwaita_edit_clear_all_symbolic_svg,                         "adwaita/edit-clear-all-symbolic.svg");
    t!(adwaita_edit_clear_symbolic_rtl_svg,                         "adwaita/edit-clear-symbolic-rtl.svg");
    t!(adwaita_edit_clear_symbolic_svg,                             "adwaita/edit-clear-symbolic.svg");
    t!(adwaita_edit_copy_symbolic_svg,                              "adwaita/edit-copy-symbolic.svg");
    t!(adwaita_edit_cut_symbolic_svg,                               "adwaita/edit-cut-symbolic.svg");
    t!(adwaita_edit_delete_symbolic_svg,                            "adwaita/edit-delete-symbolic.svg");
    t!(adwaita_edit_find_replace_symbolic_svg,                      "adwaita/edit-find-replace-symbolic.svg");
    t!(adwaita_edit_find_symbolic_svg,                              "adwaita/edit-find-symbolic.svg");
    t!(adwaita_edit_paste_symbolic_svg,                             "adwaita/edit-paste-symbolic.svg");
    t!(adwaita_edit_redo_symbolic_rtl_svg,                          "adwaita/edit-redo-symbolic-rtl.svg");
    t!(adwaita_edit_redo_symbolic_svg,                              "adwaita/edit-redo-symbolic.svg");
    t!(adwaita_edit_select_all_symbolic_svg,                        "adwaita/edit-select-all-symbolic.svg");
    t!(adwaita_edit_select_symbolic_svg,                            "adwaita/edit-select-symbolic.svg");
    t!(adwaita_edit_undo_symbolic_rtl_svg,                          "adwaita/edit-undo-symbolic-rtl.svg");
    t!(adwaita_edit_undo_symbolic_svg,                              "adwaita/edit-undo-symbolic.svg");
    t!(adwaita_emblem_default_symbolic_svg,                         "adwaita/emblem-default-symbolic.svg");
    t!(adwaita_emblem_documents_symbolic_svg,                       "adwaita/emblem-documents-symbolic.svg");
    t!(adwaita_emblem_favorite_symbolic_svg,                        "adwaita/emblem-favorite-symbolic.svg");
    t!(adwaita_emblem_important_symbolic_svg,                       "adwaita/emblem-important-symbolic.svg");
    t!(adwaita_emblem_music_symbolic_svg,                           "adwaita/emblem-music-symbolic.svg");
    t!(adwaita_emblem_ok_symbolic_svg,                              "adwaita/emblem-ok-symbolic.svg");
    t!(adwaita_emblem_photos_symbolic_svg,                          "adwaita/emblem-photos-symbolic.svg");
    t!(adwaita_emblem_shared_symbolic_svg,                          "adwaita/emblem-shared-symbolic.svg");
    t!(adwaita_emblem_synchronizing_symbolic_svg,                   "adwaita/emblem-synchronizing-symbolic.svg");
    t!(adwaita_emblem_system_symbolic_svg,                          "adwaita/emblem-system-symbolic.svg");
    t!(adwaita_emblem_videos_symbolic_svg,                          "adwaita/emblem-videos-symbolic.svg");
    t!(adwaita_emoji_activities_symbolic_svg,                       "adwaita/emoji-activities-symbolic.svg");
    t!(adwaita_emoji_body_symbolic_svg,                             "adwaita/emoji-body-symbolic.svg");
    t!(adwaita_emoji_flags_symbolic_svg,                            "adwaita/emoji-flags-symbolic.svg");
    t!(adwaita_emoji_food_symbolic_svg,                             "adwaita/emoji-food-symbolic.svg");
    t!(adwaita_emoji_nature_symbolic_svg,                           "adwaita/emoji-nature-symbolic.svg");
    t!(adwaita_emoji_objects_symbolic_svg,                          "adwaita/emoji-objects-symbolic.svg");
    t!(adwaita_emoji_people_symbolic_svg,                           "adwaita/emoji-people-symbolic.svg");
    t!(adwaita_emoji_recent_symbolic_svg,                           "adwaita/emoji-recent-symbolic.svg");
    t!(adwaita_emoji_symbols_symbolic_svg,                          "adwaita/emoji-symbols-symbolic.svg");
    t!(adwaita_emoji_travel_symbolic_svg,                           "adwaita/emoji-travel-symbolic.svg");
    t!(adwaita_emote_love_symbolic_svg,                             "adwaita/emote-love-symbolic.svg");
    t!(adwaita_error_correct_symbolic_svg,                          "adwaita/error-correct-symbolic.svg");
    t!(adwaita_face_angel_symbolic_svg,                             "adwaita/face-angel-symbolic.svg");
    t!(adwaita_face_angry_symbolic_svg,                             "adwaita/face-angry-symbolic.svg");
    t!(adwaita_face_confused_symbolic_svg,                          "adwaita/face-confused-symbolic.svg");
    t!(adwaita_face_cool_symbolic_svg,                              "adwaita/face-cool-symbolic.svg");
    t!(adwaita_face_crying_symbolic_svg,                            "adwaita/face-crying-symbolic.svg");
    t!(adwaita_face_devilish_symbolic_svg,                          "adwaita/face-devilish-symbolic.svg");
    t!(adwaita_face_embarrassed_symbolic_svg,                       "adwaita/face-embarrassed-symbolic.svg");
    t!(adwaita_face_glasses_symbolic_svg,                           "adwaita/face-glasses-symbolic.svg");
    t!(adwaita_face_kiss_symbolic_svg,                              "adwaita/face-kiss-symbolic.svg");
    t!(adwaita_face_laugh_symbolic_svg,                             "adwaita/face-laugh-symbolic.svg");
    t!(adwaita_face_monkey_symbolic_svg,                            "adwaita/face-monkey-symbolic.svg");
    t!(adwaita_face_plain_symbolic_svg,                             "adwaita/face-plain-symbolic.svg");
    t!(adwaita_face_raspberry_symbolic_svg,                         "adwaita/face-raspberry-symbolic.svg");
    t!(adwaita_face_sad_symbolic_svg,                               "adwaita/face-sad-symbolic.svg");
    t!(adwaita_face_shutmouth_symbolic_svg,                         "adwaita/face-shutmouth-symbolic.svg");
    t!(adwaita_face_sick_symbolic_svg,                              "adwaita/face-sick-symbolic.svg");
    t!(adwaita_face_smile_big_symbolic_svg,                         "adwaita/face-smile-big-symbolic.svg");
    t!(adwaita_face_smile_symbolic_svg,                             "adwaita/face-smile-symbolic.svg");
    t!(adwaita_face_smirk_symbolic_svg,                             "adwaita/face-smirk-symbolic.svg");
    t!(adwaita_face_surprise_symbolic_svg,                          "adwaita/face-surprise-symbolic.svg");
    t!(adwaita_face_tired_symbolic_svg,                             "adwaita/face-tired-symbolic.svg");
    t!(adwaita_face_uncertain_symbolic_svg,                         "adwaita/face-uncertain-symbolic.svg");
    t!(adwaita_face_wink_symbolic_svg,                              "adwaita/face-wink-symbolic.svg");
    t!(adwaita_face_worried_symbolic_svg,                           "adwaita/face-worried-symbolic.svg");
    t!(adwaita_face_yawn_symbolic_svg,                              "adwaita/face-yawn-symbolic.svg");
    t!(adwaita_find_location_symbolic_svg,                          "adwaita/find-location-symbolic.svg");
    t!(adwaita_focus_legacy_systray_symbolic_svg,                   "adwaita/focus-legacy-systray-symbolic.svg");
    t!(adwaita_focus_top_bar_symbolic_svg,                          "adwaita/focus-top-bar-symbolic.svg");
    t!(adwaita_focus_windows_symbolic_svg,                          "adwaita/focus-windows-symbolic.svg");
    t!(adwaita_folder_documents_symbolic_svg,                       "adwaita/folder-documents-symbolic.svg");
    t!(adwaita_folder_download_symbolic_svg,                        "adwaita/folder-download-symbolic.svg");
    t!(adwaita_folder_drag_accept_symbolic_svg,                     "adwaita/folder-drag-accept-symbolic.svg");
    t!(adwaita_folder_music_symbolic_svg,                           "adwaita/folder-music-symbolic.svg");
    t!(adwaita_folder_new_symbolic_svg,                             "adwaita/folder-new-symbolic.svg");
    t!(adwaita_folder_open_symbolic_svg,                            "adwaita/folder-open-symbolic.svg");
    t!(adwaita_folder_pictures_symbolic_svg,                        "adwaita/folder-pictures-symbolic.svg");
    t!(adwaita_folder_publicshare_symbolic_svg,                     "adwaita/folder-publicshare-symbolic.svg");
    t!(adwaita_folder_remote_symbolic_svg,                          "adwaita/folder-remote-symbolic.svg");
    t!(adwaita_folder_saved_search_symbolic_svg,                    "adwaita/folder-saved-search-symbolic.svg");
    t!(adwaita_folder_symbolic_svg,                                 "adwaita/folder-symbolic.svg");
    t!(adwaita_folder_templates_symbolic_svg,                       "adwaita/folder-templates-symbolic.svg");
    t!(adwaita_folder_videos_symbolic_svg,                          "adwaita/folder-videos-symbolic.svg");
    t!(adwaita_folder_visiting_symbolic_svg,                        "adwaita/folder-visiting-symbolic.svg");
    t!(adwaita_font_select_symbolic_svg,                            "adwaita/font-select-symbolic.svg");
    t!(adwaita_font_x_generic_symbolic_svg,                         "adwaita/font-x-generic-symbolic.svg");
    t!(adwaita_format_indent_less_symbolic_rtl_svg,                 "adwaita/format-indent-less-symbolic-rtl.svg");
    t!(adwaita_format_indent_less_symbolic_svg,                     "adwaita/format-indent-less-symbolic.svg");
    t!(adwaita_format_indent_more_symbolic_rtl_svg,                 "adwaita/format-indent-more-symbolic-rtl.svg");
    t!(adwaita_format_indent_more_symbolic_svg,                     "adwaita/format-indent-more-symbolic.svg");
    t!(adwaita_format_justify_center_symbolic_svg,                  "adwaita/format-justify-center-symbolic.svg");
    t!(adwaita_format_justify_fill_symbolic_svg,                    "adwaita/format-justify-fill-symbolic.svg");
    t!(adwaita_format_justify_left_symbolic_svg,                    "adwaita/format-justify-left-symbolic.svg");
    t!(adwaita_format_justify_right_symbolic_svg,                   "adwaita/format-justify-right-symbolic.svg");
    t!(adwaita_format_text_bold_symbolic_svg,                       "adwaita/format-text-bold-symbolic.svg");
    t!(adwaita_format_text_direction_symbolic_rtl_svg,              "adwaita/format-text-direction-symbolic-rtl.svg");
    t!(adwaita_format_text_direction_symbolic_svg,                  "adwaita/format-text-direction-symbolic.svg");
    t!(adwaita_format_text_italic_symbolic_svg,                     "adwaita/format-text-italic-symbolic.svg");
    t!(adwaita_format_text_strikethrough_symbolic_svg,              "adwaita/format-text-strikethrough-symbolic.svg");
    t!(adwaita_format_text_underline_symbolic_svg,                  "adwaita/format-text-underline-symbolic.svg");
    t!(adwaita_gnome_power_manager_symbolic_svg,                    "adwaita/gnome-power-manager-symbolic.svg");
    t!(adwaita_goa_panel_symbolic_svg,                              "adwaita/goa-panel-symbolic.svg");
    t!(adwaita_go_bottom_symbolic_svg,                              "adwaita/go-bottom-symbolic.svg");
    t!(adwaita_go_down_symbolic_svg,                                "adwaita/go-down-symbolic.svg");
    t!(adwaita_go_first_symbolic_rtl_svg,                           "adwaita/go-first-symbolic-rtl.svg");
    t!(adwaita_go_first_symbolic_svg,                               "adwaita/go-first-symbolic.svg");
    t!(adwaita_go_home_symbolic_svg,                                "adwaita/go-home-symbolic.svg");
    t!(adwaita_go_jump_symbolic_svg,                                "adwaita/go-jump-symbolic.svg");
    t!(adwaita_go_last_symbolic_rtl_svg,                            "adwaita/go-last-symbolic-rtl.svg");
    t!(adwaita_go_last_symbolic_svg,                                "adwaita/go-last-symbolic.svg");
    t!(adwaita_go_next_symbolic_rtl_svg,                            "adwaita/go-next-symbolic-rtl.svg");
    t!(adwaita_go_next_symbolic_svg,                                "adwaita/go-next-symbolic.svg");
    t!(adwaita_go_previous_symbolic_rtl_svg,                        "adwaita/go-previous-symbolic-rtl.svg");
    t!(adwaita_go_previous_symbolic_svg,                            "adwaita/go-previous-symbolic.svg");
    t!(adwaita_go_top_symbolic_svg,                                 "adwaita/go-top-symbolic.svg");
    t!(adwaita_go_up_symbolic_svg,                                  "adwaita/go-up-symbolic.svg");
    t!(adwaita_help_about_symbolic_svg,                             "adwaita/help-about-symbolic.svg");
    t!(adwaita_help_browser_symbolic_svg,                           "adwaita/help-browser-symbolic.svg");
    t!(adwaita_help_contents_symbolic_svg,                          "adwaita/help-contents-symbolic.svg");
    t!(adwaita_help_faq_symbolic_svg,                               "adwaita/help-faq-symbolic.svg");
    t!(adwaita_image_loading_symbolic_svg,                          "adwaita/image-loading-symbolic.svg");
    t!(adwaita_image_x_generic_symbolic_svg,                        "adwaita/image-x-generic-symbolic.svg");
    t!(adwaita_inode_directory_symbolic_svg,                        "adwaita/inode-directory-symbolic.svg");
    t!(adwaita_input_dialpad_symbolic_svg,                          "adwaita/input-dialpad-symbolic.svg");
    t!(adwaita_input_gaming_symbolic_svg,                           "adwaita/input-gaming-symbolic.svg");
    t!(adwaita_input_keyboard_symbolic_svg,                         "adwaita/input-keyboard-symbolic.svg");
    t!(adwaita_input_mouse_symbolic_svg,                            "adwaita/input-mouse-symbolic.svg");
    t!(adwaita_input_tablet_symbolic_svg,                           "adwaita/input-tablet-symbolic.svg");
    t!(adwaita_input_touchpad_symbolic_svg,                         "adwaita/input-touchpad-symbolic.svg");
    t!(adwaita_insert_image_symbolic_svg,                           "adwaita/insert-image-symbolic.svg");
    t!(adwaita_insert_link_symbolic_svg,                            "adwaita/insert-link-symbolic.svg");
    t!(adwaita_insert_object_symbolic_svg,                          "adwaita/insert-object-symbolic.svg");
    t!(adwaita_insert_text_symbolic_svg,                            "adwaita/insert-text-symbolic.svg");
    t!(adwaita_keyboard_brightness_symbolic_svg,                    "adwaita/keyboard-brightness-symbolic.svg");
    t!(adwaita_list_add_symbolic_svg,                               "adwaita/list-add-symbolic.svg");
    t!(adwaita_list_remove_all_symbolic_svg,                        "adwaita/list-remove-all-symbolic.svg");
    t!(adwaita_list_remove_symbolic_svg,                            "adwaita/list-remove-symbolic.svg");
    t!(adwaita_mail_attachment_symbolic_svg,                        "adwaita/mail-attachment-symbolic.svg");
    t!(adwaita_mail_mark_important_symbolic_svg,                    "adwaita/mail-mark-important-symbolic.svg");
    t!(adwaita_mail_read_symbolic_svg,                              "adwaita/mail-read-symbolic.svg");
    t!(adwaita_mail_replied_symbolic_svg,                           "adwaita/mail-replied-symbolic.svg");
    t!(adwaita_mail_send_receive_symbolic_svg,                      "adwaita/mail-send-receive-symbolic.svg");
    t!(adwaita_mail_send_symbolic_svg,                              "adwaita/mail-send-symbolic.svg");
    t!(adwaita_mail_unread_symbolic_svg,                            "adwaita/mail-unread-symbolic.svg");
    t!(adwaita_mark_location_symbolic_svg,                          "adwaita/mark-location-symbolic.svg");
    t!(adwaita_media_eject_symbolic_svg,                            "adwaita/media-eject-symbolic.svg");
    t!(adwaita_media_flash_symbolic_svg,                            "adwaita/media-flash-symbolic.svg");
    t!(adwaita_media_floppy_symbolic_svg,                           "adwaita/media-floppy-symbolic.svg");
    t!(adwaita_media_optical_bd_symbolic_svg,                       "adwaita/media-optical-bd-symbolic.svg");
    t!(adwaita_media_optical_cd_audio_symbolic_svg,                 "adwaita/media-optical-cd-audio-symbolic.svg");
    t!(adwaita_media_optical_dvd_symbolic_svg,                      "adwaita/media-optical-dvd-symbolic.svg");
    t!(adwaita_media_optical_symbolic_svg,                          "adwaita/media-optical-symbolic.svg");
    t!(adwaita_media_playback_pause_symbolic_svg,                   "adwaita/media-playback-pause-symbolic.svg");
    t!(adwaita_media_playback_start_symbolic_rtl_svg,               "adwaita/media-playback-start-symbolic-rtl.svg");
    t!(adwaita_media_playback_start_symbolic_svg,                   "adwaita/media-playback-start-symbolic.svg");
    t!(adwaita_media_playback_stop_symbolic_svg,                    "adwaita/media-playback-stop-symbolic.svg");
    t!(adwaita_media_playlist_consecutive_symbolic_rtl_svg,         "adwaita/media-playlist-consecutive-symbolic-rtl.svg");
    t!(adwaita_media_playlist_consecutive_symbolic_svg,             "adwaita/media-playlist-consecutive-symbolic.svg");
    t!(adwaita_media_playlist_repeat_song_symbolic_rtl_svg,         "adwaita/media-playlist-repeat-song-symbolic-rtl.svg");
    t!(adwaita_media_playlist_repeat_song_symbolic_svg,             "adwaita/media-playlist-repeat-song-symbolic.svg");
    t!(adwaita_media_playlist_repeat_symbolic_rtl_svg,              "adwaita/media-playlist-repeat-symbolic-rtl.svg");
    t!(adwaita_media_playlist_repeat_symbolic_svg,                  "adwaita/media-playlist-repeat-symbolic.svg");
    t!(adwaita_media_playlist_shuffle_symbolic_rtl_svg,             "adwaita/media-playlist-shuffle-symbolic-rtl.svg");
    t!(adwaita_media_playlist_shuffle_symbolic_svg,                 "adwaita/media-playlist-shuffle-symbolic.svg");
    t!(adwaita_media_record_symbolic_svg,                           "adwaita/media-record-symbolic.svg");
    t!(adwaita_media_removable_symbolic_svg,                        "adwaita/media-removable-symbolic.svg");
    t!(adwaita_media_seek_backward_symbolic_rtl_svg,                "adwaita/media-seek-backward-symbolic-rtl.svg");
    t!(adwaita_media_seek_backward_symbolic_svg,                    "adwaita/media-seek-backward-symbolic.svg");
    t!(adwaita_media_seek_forward_symbolic_rtl_svg,                 "adwaita/media-seek-forward-symbolic-rtl.svg");
    t!(adwaita_media_seek_forward_symbolic_svg,                     "adwaita/media-seek-forward-symbolic.svg");
    t!(adwaita_media_skip_backward_symbolic_rtl_svg,                "adwaita/media-skip-backward-symbolic-rtl.svg");
    t!(adwaita_media_skip_backward_symbolic_svg,                    "adwaita/media-skip-backward-symbolic.svg");
    t!(adwaita_media_skip_forward_symbolic_rtl_svg,                 "adwaita/media-skip-forward-symbolic-rtl.svg");
    t!(adwaita_media_skip_forward_symbolic_svg,                     "adwaita/media-skip-forward-symbolic.svg");
    t!(adwaita_media_tape_symbolic_svg,                             "adwaita/media-tape-symbolic.svg");
    t!(adwaita_media_view_subtitles_symbolic_svg,                   "adwaita/media-view-subtitles-symbolic.svg");
    t!(adwaita_media_zip_symbolic_svg,                              "adwaita/media-zip-symbolic.svg");
    t!(adwaita_microphone_sensitivity_high_symbolic_svg,            "adwaita/microphone-sensitivity-high-symbolic.svg");
    t!(adwaita_microphone_sensitivity_low_symbolic_svg,             "adwaita/microphone-sensitivity-low-symbolic.svg");
    t!(adwaita_microphone_sensitivity_medium_symbolic_svg,          "adwaita/microphone-sensitivity-medium-symbolic.svg");
    t!(adwaita_microphone_sensitivity_muted_symbolic_svg,           "adwaita/microphone-sensitivity-muted-symbolic.svg");
    t!(adwaita_modem_symbolic_svg,                                  "adwaita/modem-symbolic.svg");
    t!(adwaita_multimedia_player_apple_ipod_touch_symbolic_svg,     "adwaita/multimedia-player-apple-ipod-touch-symbolic.svg");
    t!(adwaita_multimedia_player_symbolic_svg,                      "adwaita/multimedia-player-symbolic.svg");
    t!(adwaita_multimedia_volume_control_symbolic_svg,              "adwaita/multimedia-volume-control-symbolic.svg");
    t!(adwaita_network_cellular_3g_symbolic_svg,                    "adwaita/network-cellular-3g-symbolic.svg");
    t!(adwaita_network_cellular_4g_symbolic_svg,                    "adwaita/network-cellular-4g-symbolic.svg");
    t!(adwaita_network_cellular_acquiring_symbolic_svg,             "adwaita/network-cellular-acquiring-symbolic.svg");
    t!(adwaita_network_cellular_connected_symbolic_svg,             "adwaita/network-cellular-connected-symbolic.svg");
    t!(adwaita_network_cellular_edge_symbolic_svg,                  "adwaita/network-cellular-edge-symbolic.svg");
    t!(adwaita_network_cellular_gprs_symbolic_svg,                  "adwaita/network-cellular-gprs-symbolic.svg");
    t!(adwaita_network_cellular_hspa_symbolic_svg,                  "adwaita/network-cellular-hspa-symbolic.svg");
    t!(adwaita_network_cellular_no_route_symbolic_svg,              "adwaita/network-cellular-no-route-symbolic.svg");
    t!(adwaita_network_cellular_offline_symbolic_svg,               "adwaita/network-cellular-offline-symbolic.svg");
    t!(adwaita_network_cellular_signal_excellent_symbolic_svg,      "adwaita/network-cellular-signal-excellent-symbolic.svg");
    t!(adwaita_network_cellular_signal_good_symbolic_svg,           "adwaita/network-cellular-signal-good-symbolic.svg");
    t!(adwaita_network_cellular_signal_none_symbolic_svg,           "adwaita/network-cellular-signal-none-symbolic.svg");
    t!(adwaita_network_cellular_signal_ok_symbolic_svg,             "adwaita/network-cellular-signal-ok-symbolic.svg");
    t!(adwaita_network_cellular_signal_weak_symbolic_svg,           "adwaita/network-cellular-signal-weak-symbolic.svg");
    t!(adwaita_network_error_symbolic_svg,                          "adwaita/network-error-symbolic.svg");
    t!(adwaita_network_idle_symbolic_svg,                           "adwaita/network-idle-symbolic.svg");
    t!(adwaita_network_no_route_symbolic_svg,                       "adwaita/network-no-route-symbolic.svg");
    t!(adwaita_network_offline_symbolic_svg,                        "adwaita/network-offline-symbolic.svg");
    t!(adwaita_network_receive_symbolic_svg,                        "adwaita/network-receive-symbolic.svg");
    t!(adwaita_network_server_symbolic_svg,                         "adwaita/network-server-symbolic.svg");
    t!(adwaita_network_transmit_receive_symbolic_svg,               "adwaita/network-transmit-receive-symbolic.svg");
    t!(adwaita_network_transmit_symbolic_svg,                       "adwaita/network-transmit-symbolic.svg");
    t!(adwaita_network_vpn_acquiring_symbolic_svg,                  "adwaita/network-vpn-acquiring-symbolic.svg");
    t!(adwaita_network_vpn_no_route_symbolic_svg,                   "adwaita/network-vpn-no-route-symbolic.svg");
    t!(adwaita_network_vpn_symbolic_svg,                            "adwaita/network-vpn-symbolic.svg");
    t!(adwaita_network_wired_acquiring_symbolic_svg,                "adwaita/network-wired-acquiring-symbolic.svg");
    t!(adwaita_network_wired_disconnected_symbolic_svg,             "adwaita/network-wired-disconnected-symbolic.svg");
    t!(adwaita_network_wired_no_route_symbolic_svg,                 "adwaita/network-wired-no-route-symbolic.svg");
    t!(adwaita_network_wired_offline_symbolic_svg,                  "adwaita/network-wired-offline-symbolic.svg");
    t!(adwaita_network_wired_symbolic_svg,                          "adwaita/network-wired-symbolic.svg");
    t!(adwaita_network_wireless_acquiring_symbolic_svg,             "adwaita/network-wireless-acquiring-symbolic.svg");
    t!(adwaita_network_wireless_connected_symbolic_svg,             "adwaita/network-wireless-connected-symbolic.svg");
    t!(adwaita_network_wireless_encrypted_symbolic_svg,             "adwaita/network-wireless-encrypted-symbolic.svg");
    t!(adwaita_network_wireless_hotspot_symbolic_svg,               "adwaita/network-wireless-hotspot-symbolic.svg");
    t!(adwaita_network_wireless_no_route_symbolic_svg,              "adwaita/network-wireless-no-route-symbolic.svg");
    t!(adwaita_network_wireless_offline_symbolic_svg,               "adwaita/network-wireless-offline-symbolic.svg");
    t!(adwaita_network_wireless_signal_excellent_symbolic_svg,      "adwaita/network-wireless-signal-excellent-symbolic.svg");
    t!(adwaita_network_wireless_signal_good_symbolic_svg,           "adwaita/network-wireless-signal-good-symbolic.svg");
    t!(adwaita_network_wireless_signal_none_symbolic_svg,           "adwaita/network-wireless-signal-none-symbolic.svg");
    t!(adwaita_network_wireless_signal_ok_symbolic_svg,             "adwaita/network-wireless-signal-ok-symbolic.svg");
    t!(adwaita_network_wireless_signal_weak_symbolic_svg,           "adwaita/network-wireless-signal-weak-symbolic.svg");
    t!(adwaita_network_wireless_symbolic_svg,                       "adwaita/network-wireless-symbolic.svg");
    t!(adwaita_network_workgroup_symbolic_svg,                      "adwaita/network-workgroup-symbolic.svg");
    t!(adwaita_night_light_symbolic_svg,                            "adwaita/night-light-symbolic.svg");
    t!(adwaita_non_starred_symbolic_svg,                            "adwaita/non-starred-symbolic.svg");
    t!(adwaita_object_flip_horizontal_symbolic_svg,                 "adwaita/object-flip-horizontal-symbolic.svg");
    t!(adwaita_object_flip_vertical_symbolic_svg,                   "adwaita/object-flip-vertical-symbolic.svg");
    t!(adwaita_object_rotate_left_symbolic_svg,                     "adwaita/object-rotate-left-symbolic.svg");
    t!(adwaita_object_rotate_right_symbolic_svg,                    "adwaita/object-rotate-right-symbolic.svg");
    t!(adwaita_object_select_symbolic_svg,                          "adwaita/object-select-symbolic.svg");
    t!(adwaita_open_menu_symbolic_svg,                              "adwaita/open-menu-symbolic.svg");
    t!(adwaita_orientation_landscape_inverse_symbolic_svg,          "adwaita/orientation-landscape-inverse-symbolic.svg");
    t!(adwaita_orientation_landscape_symbolic_svg,                  "adwaita/orientation-landscape-symbolic.svg");
    t!(adwaita_orientation_portrait_inverse_symbolic_svg,           "adwaita/orientation-portrait-inverse-symbolic.svg");
    t!(adwaita_orientation_portrait_symbolic_svg,                   "adwaita/orientation-portrait-symbolic.svg");
    t!(adwaita_package_x_generic_symbolic_svg,                      "adwaita/package-x-generic-symbolic.svg");
    t!(adwaita_pan_down_symbolic_svg,                               "adwaita/pan-down-symbolic.svg");
    t!(adwaita_pan_end_symbolic_rtl_svg,                            "adwaita/pan-end-symbolic-rtl.svg");
    t!(adwaita_pan_end_symbolic_svg,                                "adwaita/pan-end-symbolic.svg");
    t!(adwaita_pan_start_symbolic_rtl_svg,                          "adwaita/pan-start-symbolic-rtl.svg");
    t!(adwaita_pan_start_symbolic_svg,                              "adwaita/pan-start-symbolic.svg");
    t!(adwaita_pan_up_symbolic_svg,                                 "adwaita/pan-up-symbolic.svg");
    t!(adwaita_pda_symbolic_svg,                                    "adwaita/pda-symbolic.svg");
    t!(adwaita_phone_apple_iphone_symbolic_svg,                     "adwaita/phone-apple-iphone-symbolic.svg");
    t!(adwaita_phone_symbolic_svg,                                  "adwaita/phone-symbolic.svg");
    t!(adwaita_preferences_color_symbolic_svg,                      "adwaita/preferences-color-symbolic.svg");
    t!(adwaita_preferences_desktop_accessibility_symbolic_svg,      "adwaita/preferences-desktop-accessibility-symbolic.svg");
    t!(adwaita_preferences_desktop_display_symbolic_svg,            "adwaita/preferences-desktop-display-symbolic.svg");
    t!(adwaita_preferences_desktop_font_symbolic_svg,               "adwaita/preferences-desktop-font-symbolic.svg");
    t!(adwaita_preferences_desktop_keyboard_shortcuts_symbolic_svg, "adwaita/preferences-desktop-keyboard-shortcuts-symbolic.svg");
    t!(adwaita_preferences_desktop_keyboard_symbolic_svg,           "adwaita/preferences-desktop-keyboard-symbolic.svg");
    t!(adwaita_preferences_desktop_locale_symbolic_svg,             "adwaita/preferences-desktop-locale-symbolic.svg");
    t!(adwaita_preferences_desktop_remote_desktop_symbolic_svg,     "adwaita/preferences-desktop-remote-desktop-symbolic.svg");
    t!(adwaita_preferences_desktop_screensaver_symbolic_svg,        "adwaita/preferences-desktop-screensaver-symbolic.svg");
    t!(adwaita_preferences_desktop_wallpaper_symbolic_svg,          "adwaita/preferences-desktop-wallpaper-symbolic.svg");
    t!(adwaita_preferences_other_symbolic_svg,                      "adwaita/preferences-other-symbolic.svg");
    t!(adwaita_preferences_system_details_symbolic_svg,             "adwaita/preferences-system-details-symbolic.svg");
    t!(adwaita_preferences_system_devices_symbolic_svg,             "adwaita/preferences-system-devices-symbolic.svg");
    t!(adwaita_preferences_system_network_proxy_symbolic_svg,       "adwaita/preferences-system-network-proxy-symbolic.svg");
    t!(adwaita_preferences_system_network_symbolic_svg,             "adwaita/preferences-system-network-symbolic.svg");
    t!(adwaita_preferences_system_notifications_symbolic_svg,       "adwaita/preferences-system-notifications-symbolic.svg");
    t!(adwaita_preferences_system_privacy_symbolic_svg,             "adwaita/preferences-system-privacy-symbolic.svg");
    t!(adwaita_preferences_system_search_symbolic_svg,              "adwaita/preferences-system-search-symbolic.svg");
    t!(adwaita_preferences_system_sharing_symbolic_svg,             "adwaita/preferences-system-sharing-symbolic.svg");
    t!(adwaita_preferences_system_symbolic_svg,                     "adwaita/preferences-system-symbolic.svg");
    t!(adwaita_preferences_system_time_symbolic_svg,                "adwaita/preferences-system-time-symbolic.svg");
    t!(adwaita_printer_error_symbolic_svg,                          "adwaita/printer-error-symbolic.svg");
    t!(adwaita_printer_network_symbolic_svg,                        "adwaita/printer-network-symbolic.svg");
    t!(adwaita_printer_printing_symbolic_svg,                       "adwaita/printer-printing-symbolic.svg");
    t!(adwaita_printer_symbolic_svg,                                "adwaita/printer-symbolic.svg");
    t!(adwaita_printer_warning_symbolic_svg,                        "adwaita/printer-warning-symbolic.svg");
    t!(adwaita_process_stop_symbolic_svg,                           "adwaita/process-stop-symbolic.svg");
    t!(adwaita_radio_checked_symbolic_svg,                          "adwaita/radio-checked-symbolic.svg");
    t!(adwaita_radio_mixed_symbolic_svg,                            "adwaita/radio-mixed-symbolic.svg");
    t!(adwaita_radio_symbolic_svg,                                  "adwaita/radio-symbolic.svg");
    t!(adwaita_rotation_allowed_symbolic_svg,                       "adwaita/rotation-allowed-symbolic.svg");
    t!(adwaita_rotation_locked_symbolic_svg,                        "adwaita/rotation-locked-symbolic.svg");
    t!(adwaita_scanner_symbolic_svg,                                "adwaita/scanner-symbolic.svg");
    t!(adwaita_security_high_symbolic_svg,                          "adwaita/security-high-symbolic.svg");
    t!(adwaita_security_low_symbolic_svg,                           "adwaita/security-low-symbolic.svg");
    t!(adwaita_security_medium_symbolic_svg,                        "adwaita/security-medium-symbolic.svg");
    t!(adwaita_selection_end_symbolic_rtl_svg,                      "adwaita/selection-end-symbolic-rtl.svg");
    t!(adwaita_selection_end_symbolic_svg,                          "adwaita/selection-end-symbolic.svg");
    t!(adwaita_selection_start_symbolic_rtl_svg,                    "adwaita/selection-start-symbolic-rtl.svg");
    t!(adwaita_selection_start_symbolic_svg,                        "adwaita/selection-start-symbolic.svg");
    t!(adwaita_semi_starred_symbolic_rtl_svg,                       "adwaita/semi-starred-symbolic-rtl.svg");
    t!(adwaita_semi_starred_symbolic_svg,                           "adwaita/semi-starred-symbolic.svg");
    t!(adwaita_send_to_symbolic_svg,                                "adwaita/send-to-symbolic.svg");
    t!(adwaita_software_update_available_symbolic_svg,              "adwaita/software-update-available-symbolic.svg");
    t!(adwaita_software_update_urgent_symbolic_svg,                 "adwaita/software-update-urgent-symbolic.svg");
    t!(adwaita_star_new_symbolic_svg,                               "adwaita/star-new-symbolic.svg");
    t!(adwaita_starred_symbolic_svg,                                "adwaita/starred-symbolic.svg");
    t!(adwaita_start_here_symbolic_svg,                             "adwaita/start-here-symbolic.svg");
    t!(adwaita_system_file_manager_symbolic_svg,                    "adwaita/system-file-manager-symbolic.svg");
    t!(adwaita_system_help_symbolic_svg,                            "adwaita/system-help-symbolic.svg");
    t!(adwaita_system_lock_screen_symbolic_svg,                     "adwaita/system-lock-screen-symbolic.svg");
    t!(adwaita_system_run_symbolic_svg,                             "adwaita/system-run-symbolic.svg");
    t!(adwaita_system_search_symbolic_svg,                          "adwaita/system-search-symbolic.svg");
    t!(adwaita_system_shutdown_symbolic_svg,                        "adwaita/system-shutdown-symbolic.svg");
    t!(adwaita_system_software_install_symbolic_svg,                "adwaita/system-software-install-symbolic.svg");
    t!(adwaita_system_switch_user_symbolic_svg,                     "adwaita/system-switch-user-symbolic.svg");
    t!(adwaita_system_users_symbolic_svg,                           "adwaita/system-users-symbolic.svg");
    t!(adwaita_tab_new_symbolic_svg,                                "adwaita/tab-new-symbolic.svg");
    t!(adwaita_task_due_symbolic_svg,                               "adwaita/task-due-symbolic.svg");
    t!(adwaita_task_past_due_symbolic_svg,                          "adwaita/task-past-due-symbolic.svg");
    t!(adwaita_text_editor_symbolic_svg,                            "adwaita/text-editor-symbolic.svg");
    t!(adwaita_text_x_generic_symbolic_svg,                         "adwaita/text-x-generic-symbolic.svg");
    t!(adwaita_thunderbolt_acquiring_symbolic_svg,                  "adwaita/thunderbolt-acquiring-symbolic.svg");
    t!(adwaita_thunderbolt_symbolic_svg,                            "adwaita/thunderbolt-symbolic.svg");
    t!(adwaita_tools_check_spelling_symbolic_svg,                   "adwaita/tools-check-spelling-symbolic.svg");
    t!(adwaita_touchpad_disabled_symbolic_svg,                      "adwaita/touchpad-disabled-symbolic.svg");
    t!(adwaita_tv_symbolic_svg,                                     "adwaita/tv-symbolic.svg");
    t!(adwaita_uninterruptible_power_supply_symbolic_svg,           "adwaita/uninterruptible-power-supply-symbolic.svg");
    t!(adwaita_user_available_symbolic_svg,                         "adwaita/user-available-symbolic.svg");
    t!(adwaita_user_away_symbolic_svg,                              "adwaita/user-away-symbolic.svg");
    t!(adwaita_user_bookmarks_symbolic_svg,                         "adwaita/user-bookmarks-symbolic.svg");
    t!(adwaita_user_busy_symbolic_svg,                              "adwaita/user-busy-symbolic.svg");
    t!(adwaita_user_desktop_symbolic_svg,                           "adwaita/user-desktop-symbolic.svg");
    t!(adwaita_user_home_symbolic_svg,                              "adwaita/user-home-symbolic.svg");
    t!(adwaita_user_idle_symbolic_svg,                              "adwaita/user-idle-symbolic.svg");
    t!(adwaita_user_info_symbolic_svg,                              "adwaita/user-info-symbolic.svg");
    t!(adwaita_user_invisible_symbolic_svg,                         "adwaita/user-invisible-symbolic.svg");
    t!(adwaita_user_not_tracked_symbolic_svg,                       "adwaita/user-not-tracked-symbolic.svg");
    t!(adwaita_user_offline_symbolic_svg,                           "adwaita/user-offline-symbolic.svg");
    t!(adwaita_user_status_pending_symbolic_svg,                    "adwaita/user-status-pending-symbolic.svg");
    t!(adwaita_user_trash_full_symbolic_svg,                        "adwaita/user-trash-full-symbolic.svg");
    t!(adwaita_user_trash_symbolic_svg,                             "adwaita/user-trash-symbolic.svg");
    t!(adwaita_utilities_system_monitor_symbolic_svg,               "adwaita/utilities-system-monitor-symbolic.svg");
    t!(adwaita_utilities_terminal_symbolic_svg,                     "adwaita/utilities-terminal-symbolic.svg");
    t!(adwaita_video_display_symbolic_svg,                          "adwaita/video-display-symbolic.svg");
    t!(adwaita_video_joined_displays_symbolic_svg,                  "adwaita/video-joined-displays-symbolic.svg");
    t!(adwaita_video_single_display_symbolic_svg,                   "adwaita/video-single-display-symbolic.svg");
    t!(adwaita_video_x_generic_symbolic_svg,                        "adwaita/video-x-generic-symbolic.svg");
    t!(adwaita_view_app_grid_symbolic_svg,                          "adwaita/view-app-grid-symbolic.svg");
    t!(adwaita_view_continuous_symbolic_svg,                        "adwaita/view-continuous-symbolic.svg");
    t!(adwaita_view_dual_symbolic_svg,                              "adwaita/view-dual-symbolic.svg");
    t!(adwaita_view_fullscreen_symbolic_svg,                        "adwaita/view-fullscreen-symbolic.svg");
    t!(adwaita_view_grid_symbolic_svg,                              "adwaita/view-grid-symbolic.svg");
    t!(adwaita_view_list_symbolic_svg,                              "adwaita/view-list-symbolic.svg");
    t!(adwaita_view_mirror_symbolic_svg,                            "adwaita/view-mirror-symbolic.svg");
    t!(adwaita_view_more_horizontal_symbolic_svg,                   "adwaita/view-more-horizontal-symbolic.svg");
    t!(adwaita_view_more_symbolic_svg,                              "adwaita/view-more-symbolic.svg");
    t!(adwaita_view_paged_symbolic_svg,                             "adwaita/view-paged-symbolic.svg");
    t!(adwaita_view_pin_symbolic_svg,                               "adwaita/view-pin-symbolic.svg");
    t!(adwaita_view_refresh_symbolic_svg,                           "adwaita/view-refresh-symbolic.svg");
    t!(adwaita_view_restore_symbolic_svg,                           "adwaita/view-restore-symbolic.svg");
    t!(adwaita_view_sort_ascending_symbolic_svg,                    "adwaita/view-sort-ascending-symbolic.svg");
    t!(adwaita_view_sort_descending_symbolic_svg,                   "adwaita/view-sort-descending-symbolic.svg");
    t!(adwaita_view_wrapped_symbolic_rtl_svg,                       "adwaita/view-wrapped-symbolic-rtl.svg");
    t!(adwaita_view_wrapped_symbolic_svg,                           "adwaita/view-wrapped-symbolic.svg");
    t!(adwaita_weather_clear_night_symbolic_svg,                    "adwaita/weather-clear-night-symbolic.svg");
    t!(adwaita_weather_clear_symbolic_svg,                          "adwaita/weather-clear-symbolic.svg");
    t!(adwaita_weather_few_clouds_night_symbolic_svg,               "adwaita/weather-few-clouds-night-symbolic.svg");
    t!(adwaita_weather_few_clouds_symbolic_svg,                     "adwaita/weather-few-clouds-symbolic.svg");
    t!(adwaita_weather_fog_symbolic_svg,                            "adwaita/weather-fog-symbolic.svg");
    t!(adwaita_weather_overcast_symbolic_svg,                       "adwaita/weather-overcast-symbolic.svg");
    t!(adwaita_weather_severe_alert_symbolic_svg,                   "adwaita/weather-severe-alert-symbolic.svg");
    t!(adwaita_weather_showers_scattered_symbolic_svg,              "adwaita/weather-showers-scattered-symbolic.svg");
    t!(adwaita_weather_showers_symbolic_svg,                        "adwaita/weather-showers-symbolic.svg");
    t!(adwaita_weather_snow_symbolic_svg,                           "adwaita/weather-snow-symbolic.svg");
    t!(adwaita_weather_storm_symbolic_svg,                          "adwaita/weather-storm-symbolic.svg");
    t!(adwaita_weather_windy_symbolic_svg,                          "adwaita/weather-windy-symbolic.svg");
    t!(adwaita_web_browser_symbolic_svg,                            "adwaita/web-browser-symbolic.svg");
    t!(adwaita_window_close_symbolic_svg,                           "adwaita/window-close-symbolic.svg");
    t!(adwaita_window_maximize_symbolic_svg,                        "adwaita/window-maximize-symbolic.svg");
    t!(adwaita_window_minimize_symbolic_svg,                        "adwaita/window-minimize-symbolic.svg");
    t!(adwaita_window_restore_symbolic_svg,                         "adwaita/window-restore-symbolic.svg");
    t!(adwaita_x_office_address_book_symbolic_svg,                  "adwaita/x-office-address-book-symbolic.svg");
    t!(adwaita_x_office_calendar_symbolic_svg,                      "adwaita/x-office-calendar-symbolic.svg");
    t!(adwaita_x_office_document_symbolic_svg,                      "adwaita/x-office-document-symbolic.svg");
    t!(adwaita_x_office_drawing_symbolic_svg,                       "adwaita/x-office-drawing-symbolic.svg");
    t!(adwaita_x_office_presentation_symbolic_svg,                  "adwaita/x-office-presentation-symbolic.svg");
    t!(adwaita_x_office_spreadsheet_symbolic_svg,                   "adwaita/x-office-spreadsheet-symbolic.svg");
    t!(adwaita_zoom_fit_best_symbolic_svg,                          "adwaita/zoom-fit-best-symbolic.svg");
    t!(adwaita_zoom_in_symbolic_svg,                                "adwaita/zoom-in-symbolic.svg");
    t!(adwaita_zoom_original_symbolic_svg,                          "adwaita/zoom-original-symbolic.svg");
    t!(adwaita_zoom_out_symbolic_svg,                               "adwaita/zoom-out-symbolic.svg");
    t!(bugs_a_inside_text_content_738_svg,                          "bugs/a-inside-text-content-738.svg");
    t!(bugs_a_inside_text_content_pseudo_class_738_svg,             "bugs/a-inside-text-content-pseudo-class-738.svg");
    t!(bugs_bug108_font_size_relative_svg,                          "bugs/bug108-font-size-relative.svg");
    t!(bugs_bug112_svg_delayed_attributes_svg,                      "bugs/bug112-svg-delayed-attributes.svg");
    t!(bugs_bug165_zero_length_subpath_square_linecap_svg,          "bugs/bug165-zero-length-subpath-square-linecap.svg");
    t!(bugs_bug181_inheritable_attrs_in_svg_svg,                    "bugs/bug181-inheritable-attrs-in-svg.svg");
    t!(bugs_bug241_light_source_type_svg,                           "bugs/bug241-light-source-type.svg");
    t!(bugs_bug245_negative_dashoffset_svg,                         "bugs/bug245-negative-dashoffset.svg");
    t!(bugs_bug282_drop_shadow_svg,                                 "bugs/bug282-drop-shadow.svg");
    t!(bugs_bug340047_svg,                                          "bugs/bug340047.svg");
    t!(bugs_bug363_missing_space_svg,                               "bugs/bug363-missing-space.svg");
    t!(bugs_bug372_small_arcs_svg,                                  "bugs/bug372-small-arcs.svg");
    t!(bugs_bug373_gradient_userspaceonuse_svg,                     "bugs/bug373-gradient-userspaceonuse.svg");
    t!(bugs_bug403357_svg,                                          "bugs/bug403357.svg");
    t!(bugs_bug476507_svg,                                          "bugs/bug476507.svg");
    t!(bugs_bug481_tspan_uses_at_least_first_x_svg,                 "bugs/bug481-tspan-uses-at-least-first-x.svg");
    t!(bugs_bug494_text_accumulate_dy_svg,                          "bugs/bug494-text-accumulate-dy.svg");
    t!(bugs_bug506_pattern_fallback_svg,                            "bugs/bug506-pattern-fallback.svg");
    t!(bugs_bug510_pattern_fill_opacity_svg,                        "bugs/bug510-pattern-fill-opacity.svg");
    t!(bugs_bug510_pattern_fill_svg,                                "bugs/bug510-pattern-fill.svg");
    t!(bugs_bug548_data_url_without_mimetype_svg,                   "bugs/bug548-data-url-without-mimetype.svg");
    t!(bugs_bug563933_svg,                                          "bugs/bug563933.svg");
    t!(bugs_bug587721_text_transform_svg,                           "bugs/bug587721-text-transform.svg");
    t!(bugs_bug590_mask_units_svg,                                  "bugs/bug590-mask-units.svg");
    t!(bugs_bug603550_mask_luminance_svg,                           "bugs/bug603550-mask-luminance.svg");
    t!(bugs_bug609_clippath_transform_svg,                          "bugs/bug609-clippath-transform.svg");
    t!(bugs_bug634324_blur_negative_transform_svg,                  "bugs/bug634324-blur-negative-transform.svg");
    t!(bugs_bug642_nested_tspan_dx_dy_svg,                          "bugs/bug642-nested-tspan-dx-dy.svg");
    t!(bugs_bug667_tspan_visibility_svg,                            "bugs/bug667-tspan-visibility.svg");
    t!(bugs_bug668_small_caps_svg,                                  "bugs/bug668-small-caps.svg");
    t!(bugs_bug689832_unresolved_gradient_svg,                      "bugs/bug689832-unresolved-gradient.svg");
    t!(bugs_bug718_rect_negative_rx_ry_svg,                         "bugs/bug718-rect-negative-rx-ry.svg");
    t!(bugs_bug730_font_scaling_svg,                                "bugs/bug730-font-scaling.svg");
    t!(bugs_bug738367_svg,                                          "bugs/bug738367.svg");
    t!(bugs_bug760180_svg,                                          "bugs/bug760180.svg");
    t!(bugs_bug761175_recursive_masks_svg,                          "bugs/bug761175-recursive-masks.svg");
    t!(bugs_bug761871_reset_reflection_points_svg,                  "bugs/bug761871-reset-reflection-points.svg");
    t!(bugs_bug763386_marker_coincident_svg,                        "bugs/bug763386-marker-coincident.svg");
    t!(bugs_bug776297_marker_on_non_path_elements_svg,              "bugs/bug776297-marker-on-non-path-elements.svg");
    t!(bugs_bug786372_default_style_type_svg,                       "bugs/bug786372-default-style-type.svg");
    t!(bugs_bug788_inner_svg_viewbox_svg,                           "bugs/bug788-inner-svg-viewbox.svg");
    t!(bugs_bug1128_elliptical_arcs_big_radius_svg,                 "bugs/bug1128-elliptical-arcs-big-radius.svg");
    t!(bugs_ignore_577_multiple_font_families_svg,                  "bugs/ignore-577-multiple-font-families.svg");
    t!(svg1_1_coords_trans_01_b_svg,                                "svg1.1/coords-trans-01-b.svg");
    t!(svg1_1_coords_trans_02_t_svg,                                "svg1.1/coords-trans-02-t.svg");
    t!(svg1_1_coords_trans_03_t_svg,                                "svg1.1/coords-trans-03-t.svg");
    t!(svg1_1_coords_trans_04_t_svg,                                "svg1.1/coords-trans-04-t.svg");
    t!(svg1_1_coords_trans_05_t_svg,                                "svg1.1/coords-trans-05-t.svg");
    t!(svg1_1_coords_trans_06_t_svg,                                "svg1.1/coords-trans-06-t.svg");
    t!(svg1_1_coords_trans_07_t_svg,                                "svg1.1/coords-trans-07-t.svg");
    t!(svg1_1_coords_trans_08_t_svg,                                "svg1.1/coords-trans-08-t.svg");
    t!(svg1_1_coords_trans_09_t_svg,                                "svg1.1/coords-trans-09-t.svg");
    t!(svg1_1_coords_viewattr_01_b_svg,                             "svg1.1/coords-viewattr-01-b.svg");
    t!(svg1_1_coords_viewattr_02_b_svg,                             "svg1.1/coords-viewattr-02-b.svg");
    t!(svg1_1_coords_viewattr_03_b_svg,                             "svg1.1/coords-viewattr-03-b.svg");
    t!(svg1_1_coords_viewattr_04_f_svg,                             "svg1.1/coords-viewattr-04-f.svg");
    t!(svg1_1_filters_background_01_f_svg,                          "svg1.1/filters-background-01-f.svg");
    t!(svg1_1_filters_blend_01_b_svg,                               "svg1.1/filters-blend-01-b.svg");
    t!(svg1_1_filters_color_01_b_svg,                               "svg1.1/filters-color-01-b.svg");
    t!(svg1_1_filters_color_02_b_svg,                               "svg1.1/filters-color-02-b.svg");
    t!(svg1_1_filters_composite_02_b_svg,                           "svg1.1/filters-composite-02-b.svg");
    t!(svg1_1_filters_composite_03_f_svg,                           "svg1.1/filters-composite-03-f.svg");
    t!(svg1_1_filters_composite_04_f_svg,                           "svg1.1/filters-composite-04-f.svg");
    t!(svg1_1_filters_composite_05_f_svg,                           "svg1.1/filters-composite-05-f.svg");
    t!(svg1_1_filters_comptran_01_b_svg,                            "svg1.1/filters-comptran-01-b.svg");
    t!(svg1_1_filters_conv_01_f_svg,                                "svg1.1/filters-conv-01-f.svg");
    t!(svg1_1_filters_conv_02_f_svg,                                "svg1.1/filters-conv-02-f.svg");
    t!(svg1_1_filters_conv_03_f_svg,                                "svg1.1/filters-conv-03-f.svg");
    t!(svg1_1_filters_conv_04_f_svg,                                "svg1.1/filters-conv-04-f.svg");
    t!(svg1_1_filters_conv_05_f_svg,                                "svg1.1/filters-conv-05-f.svg");
    t!(svg1_1_filters_diffuse_01_f_svg,                             "svg1.1/filters-diffuse-01-f.svg");
    t!(svg1_1_filters_displace_02_f_svg,                            "svg1.1/filters-displace-02-f.svg");
    t!(svg1_1_filters_felem_02_f_svg,                               "svg1.1/filters-felem-02-f.svg");
    t!(svg1_1_filters_gauss_01_b_svg,                               "svg1.1/filters-gauss-01-b.svg");
    t!(svg1_1_filters_gauss_02_f_svg,                               "svg1.1/filters-gauss-02-f.svg");
    t!(svg1_1_filters_gauss_03_f_svg,                               "svg1.1/filters-gauss-03-f.svg");
    t!(svg1_1_filters_image_01_b_svg,                               "svg1.1/filters-image-01-b.svg");
    t!(svg1_1_filters_image_02_b_svg,                               "svg1.1/filters-image-02-b.svg");
    t!(svg1_1_filters_image_03_f_svg,                               "svg1.1/filters-image-03-f.svg");
    t!(svg1_1_filters_image_04_f_svg,                               "svg1.1/filters-image-04-f.svg");
    t!(svg1_1_filters_image_05_f_svg,                               "svg1.1/filters-image-05-f.svg");
    t!(svg1_1_filters_light_01_f_svg,                               "svg1.1/filters-light-01-f.svg");
    t!(svg1_1_filters_light_02_f_svg,                               "svg1.1/filters-light-02-f.svg");
    t!(svg1_1_filters_light_03_f_svg,                               "svg1.1/filters-light-03-f.svg");
    t!(svg1_1_filters_light_04_f_svg,                               "svg1.1/filters-light-04-f.svg");
    t!(svg1_1_filters_light_05_f_svg,                               "svg1.1/filters-light-05-f.svg");
    t!(svg1_1_filters_morph_01_f_svg,                               "svg1.1/filters-morph-01-f.svg");
    t!(svg1_1_filters_offset_01_b_svg,                              "svg1.1/filters-offset-01-b.svg");
    t!(svg1_1_filters_overview_01_b_svg,                            "svg1.1/filters-overview-01-b.svg");
    t!(svg1_1_filters_overview_02_b_svg,                            "svg1.1/filters-overview-02-b.svg");
    t!(svg1_1_filters_overview_03_b_svg,                            "svg1.1/filters-overview-03-b.svg");
    t!(svg1_1_filters_specular_01_f_svg,                            "svg1.1/filters-specular-01-f.svg");
    t!(svg1_1_filters_tile_01_b_svg,                                "svg1.1/filters-tile-01-b.svg");
    t!(svg1_1_filters_turb_01_f_svg,                                "svg1.1/filters-turb-01-f.svg");
    t!(svg1_1_filters_turb_02_f_svg,                                "svg1.1/filters-turb-02-f.svg");
    t!(svg1_1_ignore_filters_displace_01_f_svg,                     "svg1.1/ignore-filters-displace-01-f.svg");
    t!(svg1_1_ignore_filters_example_01_b_svg,                      "svg1.1/ignore-filters-example-01-b.svg");
    t!(svg1_1_ignore_masking_path_07_b_svg,                         "svg1.1/ignore-masking-path-07-b.svg");
    t!(svg1_1_masking_filter_01_f_svg,                              "svg1.1/masking-filter-01-f.svg");
    t!(svg1_1_masking_intro_01_f_svg,                               "svg1.1/masking-intro-01-f.svg");
    t!(svg1_1_masking_mask_01_b_svg,                                "svg1.1/masking-mask-01-b.svg");
    t!(svg1_1_masking_mask_02_f_svg,                                "svg1.1/masking-mask-02-f.svg");
    t!(svg1_1_masking_opacity_01_b_svg,                             "svg1.1/masking-opacity-01-b.svg");
    t!(svg1_1_masking_path_01_b_svg,                                "svg1.1/masking-path-01-b.svg");
    t!(svg1_1_masking_path_02_b_svg,                                "svg1.1/masking-path-02-b.svg");
    t!(svg1_1_masking_path_03_b_svg,                                "svg1.1/masking-path-03-b.svg");
    t!(svg1_1_masking_path_04_b_svg,                                "svg1.1/masking-path-04-b.svg");
    t!(svg1_1_masking_path_05_f_svg,                                "svg1.1/masking-path-05-f.svg");
    t!(svg1_1_masking_path_08_b_svg,                                "svg1.1/masking-path-08-b.svg");
    t!(svg1_1_painting_control_02_f_svg,                            "svg1.1/painting-control-02-f.svg");
    t!(svg1_1_painting_marker_01_f_svg,                             "svg1.1/painting-marker-01-f.svg");
    t!(svg1_1_painting_marker_02_f_svg,                             "svg1.1/painting-marker-02-f.svg");
    t!(svg1_1_painting_marker_03_f_svg,                             "svg1.1/painting-marker-03-f.svg");
    t!(svg1_1_painting_marker_04_f_svg,                             "svg1.1/painting-marker-04-f.svg");
    t!(svg1_1_painting_marker_06_f_svg,                             "svg1.1/painting-marker-06-f.svg");
    t!(svg1_1_painting_marker_07_f_svg,                             "svg1.1/painting-marker-07-f.svg");
    t!(svg1_1_painting_marker_properties_01_f_svg,                  "svg1.1/painting-marker-properties-01-f.svg");
    t!(svg1_1_painting_stroke_01_t_svg,                             "svg1.1/painting-stroke-01-t.svg");
    t!(svg1_1_painting_stroke_02_t_svg,                             "svg1.1/painting-stroke-02-t.svg");
    t!(svg1_1_painting_stroke_03_t_svg,                             "svg1.1/painting-stroke-03-t.svg");
    t!(svg1_1_painting_stroke_04_t_svg,                             "svg1.1/painting-stroke-04-t.svg");
    t!(svg1_1_painting_stroke_05_t_svg,                             "svg1.1/painting-stroke-05-t.svg");
    t!(svg1_1_painting_stroke_06_t_svg,                             "svg1.1/painting-stroke-06-t.svg");
    t!(svg1_1_painting_stroke_07_t_svg,                             "svg1.1/painting-stroke-07-t.svg");
    t!(svg1_1_painting_stroke_08_t_svg,                             "svg1.1/painting-stroke-08-t.svg");
    t!(svg1_1_painting_stroke_09_t_svg,                             "svg1.1/painting-stroke-09-t.svg");
    t!(svg1_1_paths_data_01_t_svg,                                  "svg1.1/paths-data-01-t.svg");
    t!(svg1_1_paths_data_02_t_svg,                                  "svg1.1/paths-data-02-t.svg");
    t!(svg1_1_paths_data_03_f_svg,                                  "svg1.1/paths-data-03-f.svg");
    t!(svg1_1_paths_data_04_t_svg,                                  "svg1.1/paths-data-04-t.svg");
    t!(svg1_1_paths_data_05_t_svg,                                  "svg1.1/paths-data-05-t.svg");
    t!(svg1_1_paths_data_06_t_svg,                                  "svg1.1/paths-data-06-t.svg");
    t!(svg1_1_paths_data_07_t_svg,                                  "svg1.1/paths-data-07-t.svg");
    t!(svg1_1_paths_data_08_t_svg,                                  "svg1.1/paths-data-08-t.svg");
    t!(svg1_1_paths_data_09_t_svg,                                  "svg1.1/paths-data-09-t.svg");
    t!(svg1_1_paths_data_10_t_svg,                                  "svg1.1/paths-data-10-t.svg");
    t!(svg1_1_paths_data_12_t_svg,                                  "svg1.1/paths-data-12-t.svg");
    t!(svg1_1_paths_data_13_t_svg,                                  "svg1.1/paths-data-13-t.svg");
    t!(svg1_1_paths_data_14_t_svg,                                  "svg1.1/paths-data-14-t.svg");
    t!(svg1_1_paths_data_15_t_svg,                                  "svg1.1/paths-data-15-t.svg");
    t!(svg1_1_paths_data_16_t_svg,                                  "svg1.1/paths-data-16-t.svg");
    t!(svg1_1_paths_data_17_f_svg,                                  "svg1.1/paths-data-17-f.svg");
    t!(svg1_1_paths_data_18_f_svg,                                  "svg1.1/paths-data-18-f.svg");
    t!(svg1_1_paths_data_19_f_svg,                                  "svg1.1/paths-data-19-f.svg");
    t!(svg1_1_paths_data_20_f_svg,                                  "svg1.1/paths-data-20-f.svg");
    t!(svg1_1_pservers_grad_01_b_svg,                               "svg1.1/pservers-grad-01-b.svg");
    t!(svg1_1_pservers_grad_02_b_svg,                               "svg1.1/pservers-grad-02-b.svg");
    t!(svg1_1_pservers_grad_03_b_svg,                               "svg1.1/pservers-grad-03-b.svg");
    t!(svg1_1_pservers_grad_04_b_svg,                               "svg1.1/pservers-grad-04-b.svg");
    t!(svg1_1_pservers_grad_05_b_svg,                               "svg1.1/pservers-grad-05-b.svg");
    t!(svg1_1_pservers_grad_06_b_svg,                               "svg1.1/pservers-grad-06-b.svg");
    t!(svg1_1_pservers_grad_07_b_svg,                               "svg1.1/pservers-grad-07-b.svg");
    t!(svg1_1_pservers_grad_08_b_svg,                               "svg1.1/pservers-grad-08-b.svg");
    t!(svg1_1_pservers_grad_09_b_svg,                               "svg1.1/pservers-grad-09-b.svg");
    t!(svg1_1_pservers_grad_10_b_svg,                               "svg1.1/pservers-grad-10-b.svg");
    t!(svg1_1_pservers_grad_11_b_svg,                               "svg1.1/pservers-grad-11-b.svg");
    t!(svg1_1_pservers_grad_12_b_svg,                               "svg1.1/pservers-grad-12-b.svg");
    t!(svg1_1_pservers_grad_14_b_svg,                               "svg1.1/pservers-grad-14-b.svg");
    t!(svg1_1_pservers_grad_15_b_svg,                               "svg1.1/pservers-grad-15-b.svg");
    t!(svg1_1_pservers_grad_16_b_svg,                               "svg1.1/pservers-grad-16-b.svg");
    t!(svg1_1_pservers_grad_18_b_svg,                               "svg1.1/pservers-grad-18-b.svg");
    t!(svg1_1_pservers_grad_22_b_svg,                               "svg1.1/pservers-grad-22-b.svg");
    t!(svg1_1_pservers_grad_23_f_svg,                               "svg1.1/pservers-grad-23-f.svg");
    t!(svg1_1_pservers_grad_24_f_svg,                               "svg1.1/pservers-grad-24-f.svg");
    t!(svg1_1_pservers_grad_stops_01_f_svg,                         "svg1.1/pservers-grad-stops-01-f.svg");
    t!(svg1_1_pservers_pattern_01_b_svg,                            "svg1.1/pservers-pattern-01-b.svg");
    t!(svg1_1_pservers_pattern_02_f_svg,                            "svg1.1/pservers-pattern-02-f.svg");
    t!(svg1_1_pservers_pattern_03_f_svg,                            "svg1.1/pservers-pattern-03-f.svg");
    t!(svg1_1_pservers_pattern_04_f_svg,                            "svg1.1/pservers-pattern-04-f.svg");
    t!(svg1_1_pservers_pattern_05_f_svg,                            "svg1.1/pservers-pattern-05-f.svg");
    t!(svg1_1_pservers_pattern_06_f_svg,                            "svg1.1/pservers-pattern-06-f.svg");
    t!(svg1_1_pservers_pattern_07_f_svg,                            "svg1.1/pservers-pattern-07-f.svg");
    t!(svg1_1_pservers_pattern_08_f_svg,                            "svg1.1/pservers-pattern-08-f.svg");
    t!(svg1_1_pservers_pattern_09_f_svg,                            "svg1.1/pservers-pattern-09-f.svg");
    t!(svg1_1_shapes_intro_01_t_svg,                                "svg1.1/shapes-intro-01-t.svg");
    t!(svg1_1_shapes_intro_02_f_svg,                                "svg1.1/shapes-intro-02-f.svg");
    t!(svg1_1_struct_cond_01_t_svg,                                 "svg1.1/struct-cond-01-t.svg");
    t!(svg1_1_struct_cond_03_t_svg,                                 "svg1.1/struct-cond-03-t.svg");
    t!(svg1_1_struct_group_03_t_svg,                                "svg1.1/struct-group-03-t.svg");
    t!(svg1_1_struct_image_05_b_svg,                                "svg1.1/struct-image-05-b.svg");
    t!(svg1_1_struct_svg_03_f_svg,                                  "svg1.1/struct-svg-03-f.svg");
    t!(svg1_1_struct_symbol_01_b_svg,                               "svg1.1/struct-symbol-01-b.svg");
    t!(svg1_1_struct_use_01_t_svg,                                  "svg1.1/struct-use-01-t.svg");
    t!(svg1_1_struct_use_03_t_svg,                                  "svg1.1/struct-use-03-t.svg");
    t!(svg1_1_struct_use_04_b_svg,                                  "svg1.1/struct-use-04-b.svg");
    t!(svg1_1_struct_use_09_b_svg,                                  "svg1.1/struct-use-09-b.svg");
    t!(svg1_1_struct_use_10_f_svg,                                  "svg1.1/struct-use-10-f.svg");
    t!(svg1_1_styling_css_01_b_svg,                                 "svg1.1/styling-css-01-b.svg");
    t!(svg1_1_styling_css_02_b_svg,                                 "svg1.1/styling-css-02-b.svg");
    t!(svg1_1_styling_css_03_b_svg,                                 "svg1.1/styling-css-03-b.svg");
    t!(svg1_1_styling_css_04_f_svg,                                 "svg1.1/styling-css-04-f.svg");
    t!(svg1_1_styling_css_07_f_svg,                                 "svg1.1/styling-css-07-f.svg");
    t!(svg1_1_styling_css_08_f_svg,                                 "svg1.1/styling-css-08-f.svg");
    t!(svg1_1_text_align_01_b_svg,                                  "svg1.1/text-align-01-b.svg");
    t!(svg1_1_text_align_02_b_svg,                                  "svg1.1/text-align-02-b.svg");
    t!(svg1_1_text_align_03_b_svg,                                  "svg1.1/text-align-03-b.svg");
    t!(svg1_1_text_fonts_02_t_svg,                                  "svg1.1/text-fonts-02-t.svg");
    t!(svg1_1_text_dominant_baseline_01_svg,                        "svg1.1/text-dominant-baseline-01.svg");
    t!(svg1_1_text_text_03_b_svg,                                   "svg1.1/text-text-03-b.svg");
    t!(svg1_1_text_text_08_b_svg,                                   "svg1.1/text-text-08-b.svg");
    t!(svg1_1_text_text_10_t_svg,                                   "svg1.1/text-text-10-t.svg");
    t!(svg1_1_text_tref_01_b_svg,                                   "svg1.1/text-tref-01-b.svg");
    t!(svg1_1_text_tref_02_b_svg,                                   "svg1.1/text-tref-02-b.svg");
    t!(svg1_1_text_tref_03_b_svg,                                   "svg1.1/text-tref-03-b.svg");
    t!(svg1_1_types_basic_02_f_svg,                                 "svg1.1/types-basic-02-f.svg");
    t!(svg2_gradient_01_b_svg,                                      "svg2/gradient-01-b.svg");
    t!(svg2_mix_blend_mode_svg,                                     "svg2/mix-blend-mode.svg");
    t!(svg2_multi_filter_svg,                                       "svg2/multi-filter.svg");
    t!(svg2_paint_order_svg,                                        "svg2/paint-order.svg");
    t!(svg2_text_paint_order_svg,                                   "svg2/text-paint-order.svg");
}

test_compare_render_output!(
    marker_orient_auto_start_reverse,
    100,
    100,
    br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
    <defs>
      <marker id="marker" orient="auto-start-reverse" viewBox="0 0 10 10"
              refX="0" refY="5" markerWidth="10" markerHeight="10"
              markerUnits="userSpaceOnUse">
        <path d="M0,0 L10,5 L0,10 Z" fill="green"/>
      </marker>
    </defs>
  
    <path d="M20,50 L80,50" marker-start="url(#marker)" marker-end="url(#marker)" stroke-width="10" stroke="black"/>
  </svg>"##,

    br##"<?xml version="1.0" encoding="UTF-8" standalone="no"?>
    <svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
      <path
         d="M 20,55 10,50 20,45 Z"
         id="triangle1" fill="green"/>
      <path
         d="m 80,45 10,5 -10,5 z"
         id="triangle2" fill="green"/>
      <rect
         id="rectangle"
         width="60"
         height="10"
         x="20"
         y="45" fill="black"/>
    </svg>"##,
);

test_compare_render_output!(
    marker_context_stroke_fill,
    400,
    400,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg xmlns="http://www.w3.org/2000/svg" width="400" height="400">
      <style>
        .path1 {
          fill: none;
          stroke-width: 4px;
          marker: url(#marker1);
        }
    
        .path2 {
          fill: darkblue;
          stroke: mediumseagreen;
          stroke-width: 4px;
          marker: url(#marker2);
        }
      </style>
    
      <path class="path1" d="M20,20 L200,20 L380,20" stroke="lime"/>
    
      <path class="path2" d="M20,40 h360 v320 h-360 v-320 Z"/>
    
      <marker id="marker1" markerWidth="12" markerHeight="12" refX="6" refY="6"
              markerUnits="userSpaceOnUse">
        <circle cx="6" cy="6" r="3"
                fill="white" stroke="context-stroke" stroke-width="2"/>
      </marker>
    
      <marker id="marker2" markerWidth="12" markerHeight="12" refX="6" refY="6"
              markerUnits="userSpaceOnUse">
        <!-- Note that here the paint is reversed:
             fill=context-stroke,
             stroke=context-fill 
        -->
        <circle cx="6" cy="6" r="3"
                fill="context-stroke" stroke="context-fill" stroke-width="2"/>
      </marker>
    </svg>"##,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg xmlns="http://www.w3.org/2000/svg" width="400" height="400">
      <path d="M20,20 L200,20 L380,20" stroke="lime" stroke-width="4"/>
      <circle cx="20" cy="20" r="3" stroke-width="2" fill="white" stroke="lime"/>
      <circle cx="200" cy="20" r="3" stroke-width="2" fill="white" stroke="lime"/>
      <circle cx="380" cy="20" r="3" stroke-width="2" fill="white" stroke="lime"/>
    
      <path class="path2" d="M20,40 h360 v320 h-360 v-320 Z" fill="darkblue"
            stroke="mediumseagreen" stroke-width="4"/>
      <circle cx="20"  cy="40"  r="3" fill="mediumseagreen" stroke="darkblue" stroke-width="2"/>
      <circle cx="20"  cy="40"  r="3" fill="mediumseagreen" stroke="darkblue" stroke-width="2"/>
      <circle cx="380" cy="40"  r="3" fill="mediumseagreen" stroke="darkblue" stroke-width="2"/>
      <circle cx="380" cy="360" r="3" fill="mediumseagreen" stroke="darkblue" stroke-width="2"/>
      <circle cx="20"  cy="360" r="3" fill="mediumseagreen" stroke="darkblue" stroke-width="2"/>
      <circle cx="20"  cy="40"  r="3" fill="mediumseagreen" stroke="darkblue" stroke-width="2"/>
    </svg>
    "##,
);

test_compare_render_output!(
    image_auto_width_height,
    30,
    30,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg xmlns="http://www.w3.org/2000/svg" width="30" height="30">
      <image
        href="data:;base64,iVBORw0KGgoAAAANSUhEUgAAAAoAAAAKCAIAAAACUFjqAAAAFElEQVQY02Nk+M+ABzAxMIxKYwIAQC0BEwZFOw4AAAAASUVORK5CYII="
        x="10" y="10"/>
    </svg>"##,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg xmlns="http://www.w3.org/2000/svg" width="30" height="30">
      <rect x="10" y="10" width="10" height="10" fill="lime"/>
    </svg>"##,
);

test_compare_render_output!(
    rect_auto_width_height,
    30,
    30,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg xmlns="http://www.w3.org/2000/svg" width="30" height="30">
      <rect x="10" y="10" width="auto" height="auto" fill="lime"/>
    </svg>"##,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg xmlns="http://www.w3.org/2000/svg" width="30" height="30">
    </svg>"##
);

test_compare_render_output!(
    svg_auto_width_height,
    30,
    30,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg xmlns="http://www.w3.org/2000/svg" width="30" height="30">
      <svg xmlns="http://www.w3.org/2000/svg" width="auto" height="auto">
        <rect x="10" y="10" width="100%" height="100%" fill="lime"/>
      </svg>
    </svg>"##,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg xmlns="http://www.w3.org/2000/svg" width="30" height="30">
      <rect x="10" y="10" width="100%" height="100%" fill="lime"/>
    </svg>"##,
);

test_compare_render_output!(
    use_context_stroke,
    100,
    20,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg width="100" height="20" viewBox="0 0 40 10" xmlns="http://www.w3.org/2000/svg">
  <g id="group">
    <circle cx="5" cy="5" r="4" stroke="context-stroke" fill="black"/>
    <circle cx="14" cy="5" r="4" stroke="context-fill"/>
  </g>
  <use href="#group" x="20" stroke="blue" fill="yellow"/>
  <!--
  Modified from: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/use
  -->
</svg>
    "##,
    br##"<?xml version="1.0" encoding="UTF-8"?>
    <svg width="100" height="20" viewBox="0 0 40 10" xmlns="http://www.w3.org/2000/svg">
    <circle cx="5" cy="5" r="4" fill="black"/>
    <circle cx="14" cy="5" r="4" fill="black"/>
    <circle cx="25" cy="5" r="4" stroke="blue" fill="black"/>
    <circle cx="34" cy="5" r="4" stroke="yellow" fill="yellow"/>
    <!--
    Modified from: https://developer.mozilla.org/en-US/docs/Web/SVG/Element/use
    -->
  </svg>
    "##,
);

test_svg_reference!(
    isolation,
    "tests/fixtures/reftests/svg2-reftests/isolation.svg",
    "tests/fixtures/reftests/svg2-reftests/isolation-ref.svg"
);

test_svg_reference!(
    mask_and_opacity,
    "tests/fixtures/reftests/svg2-reftests/mask-and-opacity.svg",
    "tests/fixtures/reftests/svg2-reftests/mask-and-opacity-ref.svg"
);

test_svg_reference!(
    gaussian_blur_nonpositive_913,
    "tests/fixtures/reftests/svg2-reftests/bug913-gaussian-blur-nonpositive.svg",
    "tests/fixtures/reftests/svg2-reftests/bug913-gaussian-blur-nonpositive-ref.svg"
);

test_svg_reference!(
    bug_880_horizontal_vertical_stroked_lines,
    "tests/fixtures/reftests/bugs-reftests/bug880-stroke-wide-line.svg",
    "tests/fixtures/reftests/bugs-reftests/bug880-stroke-wide-line-ref.svg"
);

test_svg_reference!(
    bug_92_symbol_clip,
    "tests/fixtures/reftests/bugs-reftests/bug92-symbol-clip.svg",
    "tests/fixtures/reftests/bugs-reftests/bug92-symbol-clip-ref.svg"
);

test_svg_reference!(
    bug_875_svg_use_width_height,
    "tests/fixtures/reftests/bugs-reftests/bug875-svg-use-width-height.svg",
    "tests/fixtures/reftests/bugs-reftests/bug875-svg-use-width-height-ref.svg"
);

test_svg_reference!(
    bug_885_vector_effect_non_scaling_stroke,
    "tests/fixtures/reftests/bugs-reftests/bug885-vector-effect-non-scaling-stroke.svg",
    "tests/fixtures/reftests/bugs-reftests/bug885-vector-effect-non-scaling-stroke-ref.svg"
);

test_svg_reference!(
    bug_930_invalid_clip_path_transform,
    "tests/fixtures/reftests/bugs-reftests/bug930-invalid-clip-path-transform.svg",
    "tests/fixtures/reftests/bugs-reftests/bug930-invalid-clip-path-transform-ref.svg"
);

test_svg_reference!(
    bug_985_image_rendering_property,
    "tests/fixtures/reftests/svg2-reftests/image-rendering-985.svg",
    "tests/fixtures/reftests/svg2-reftests/image-rendering-985-ref.svg"
);

test_svg_reference!(
    bug_992_use_symbol_cascade,
    "tests/fixtures/reftests/bugs/use-symbol-cascade-992.svg",
    "tests/fixtures/reftests/bugs/use-symbol-cascade-992-ref.svg"
);

test_svg_reference!(
    color_types,
    "tests/fixtures/reftests/color-types.svg",
    "tests/fixtures/reftests/color-types-ref.svg"
);

// Note that this uses the same reference file as color-types.svg - the result ought to be the same.
test_svg_reference!(
    color_property_color_types,
    "tests/fixtures/reftests/color-property-color-types.svg",
    "tests/fixtures/reftests/color-types-ref.svg"
);

test_svg_reference!(
    color_types_unsupported,
    "tests/fixtures/reftests/color-types-unsupported.svg",
    "tests/fixtures/reftests/color-types-unsupported-ref.svg"
);

test_svg_reference!(
    invalid_gradient_transform,
    "tests/fixtures/reftests/invalid-gradient-transform.svg",
    "tests/fixtures/reftests/invalid-gradient-transform-ref.svg"
);

test_svg_reference!(
    xinclude_data_url,
    "tests/fixtures/reftests/xinclude-data-url.svg",
    "tests/fixtures/reftests/xinclude-data-url-ref.svg"
);

test_svg_reference!(
    xinclude_non_utf8,
    "tests/fixtures/reftests/xinclude-non-utf8.svg",
    "tests/fixtures/reftests/xinclude-non-utf8-ref.svg"
);

test_svg_reference!(
    markers_arc_segments,
    "tests/fixtures/reftests/markers-arc-segments.svg",
    "tests/fixtures/reftests/markers-arc-segments-ref.svg"
);

test_svg_reference!(
    bug_1121_feimage_embedded_svg,
    "tests/fixtures/reftests/bugs-reftests/bug1121-feimage-embedded-svg.svg",
    "tests/fixtures/reftests/bugs-reftests/bug1121-feimage-embedded-svg-ref.svg"
);
