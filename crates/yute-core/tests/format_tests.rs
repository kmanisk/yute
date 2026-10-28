use yute_core::media::formats::{AvailableFormats, MediaKind};
fn formats() -> AvailableFormats {
    AvailableFormats::from_json(include_bytes!("fixtures/formats/available.json")).unwrap()
}
#[test]
fn typed_metadata_preserves_actual_stream_properties() {
    let formats = formats();
    assert_eq!(formats.formats.len(), 7);
    assert_eq!(formats.exact("18").unwrap().kind, MediaKind::Muxed);
    let av1 = formats.exact("399").unwrap();
    assert!(av1.hdr);
    assert_eq!(av1.fps, Some(60.));
    assert_eq!(av1.bitrate, Some(5_200_000));
    assert_eq!(av1.filesize, None);
    assert_eq!(
        formats.exact("140").unwrap().audio_language.as_deref(),
        Some("hi")
    );
}
#[test]
fn resolution_precedes_codec_and_ceiling_is_never_exceeded() {
    let formats = formats();
    assert_eq!(
        formats
            .video(Some(1080), Some(1080), Some("av1"))
            .unwrap()
            .id,
        "399"
    );
    assert_eq!(
        formats
            .video(Some(1080), Some(720), Some("av1"))
            .unwrap()
            .id,
        "247"
    );
    assert_eq!(
        formats.video(Some(1080), None, Some("h264")).unwrap().id,
        "137"
    );
    assert!(formats.video(Some(240), None, None).is_err());
    let low = AvailableFormats::from_json(
        br#"{"formats":[{"format_id":"low","height":720,"vcodec":"vp9","acodec":"opus"}]}"#,
    )
    .unwrap();
    assert_eq!(
        low.video(Some(1080), None, Some("av1")).unwrap().height,
        Some(720)
    );
}
#[test]
fn exact_selection_pairs_audio_and_does_not_mutate_preferences() {
    let formats = formats();
    assert_eq!(formats.selector("399").unwrap(), "399+251");
    assert_eq!(formats.selector("18").unwrap(), "18");
    assert_eq!(formats.best_audio(None).unwrap().id, "251");
    assert_eq!(formats.best_audio(Some("aac")).unwrap().id, "140");
    assert_eq!(formats.selector("140").unwrap(), "140");
    assert!(formats.selector("missing").is_err());
}
#[test]
fn live_missing_metadata_and_invalid_responses() {
    let live = AvailableFormats::from_json(
        br#"{"is_live":true,"formats":[{"format_id":"live","vcodec":"avc1","acodec":"aac"}]}"#,
    )
    .unwrap();
    assert!(live.live);
    assert!(live.video(None, Some(1080), None).is_err());
    assert!(live.video(None, None, None).is_ok());
    for invalid in [
        br#"{}"#.as_slice(),
        br#"{"formats":[]}"#,
        br#"{"formats":[{"format_id":"x+all","vcodec":"avc1"}]}"#,
        b"not json",
    ] {
        assert!(AvailableFormats::from_json(invalid).is_err());
    }
}
#[tokio::test]
async fn cancelled_resolution_never_spawns_a_probe() {
    let token = tokio_util::sync::CancellationToken::new();
    token.cancel();
    assert!(yute_core::media::formats::FormatResolver::default()
        .resolve("fixture", token)
        .await
        .is_err());
}
