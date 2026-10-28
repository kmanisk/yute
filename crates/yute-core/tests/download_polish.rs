use yute_core::{
    config::Config,
    downloads::{self, Mode, Progress, Request},
};
fn args(mode: Mode, playlist: bool, ffmpeg: bool) -> Vec<String> {
    let c = downloads::command(
        &Request {
            url: "https://www.youtube.com/playlist?list=fixture;$(touch NEVER)".into(),
            playlist,
            mode,
            ffmpeg,
        },
        &Config::default(),
    )
    .unwrap();
    c.as_std()
        .get_args()
        .map(|v| v.to_string_lossy().into_owned())
        .collect()
}
#[test]
fn command_arguments_cover_ceilings_audio_conversion_archive_and_templates() {
    let a = args(Mode::Video(Some(720)), true, false);
    assert!(a.contains(&"bestvideo[height<=720]+bestaudio/best[height<=720]".into()));
    assert!(a.contains(&"--ignore-errors".into()));
    assert!(a.iter().any(|s| s.contains("playlist_index)02d")));
    assert!(a.iter().any(|s| s.contains("download-archive-")));
    assert!(a.last().unwrap().contains("$(touch NEVER)"));
    assert_eq!(a[a.len() - 2], "--");
    assert!(!a.iter().any(|s| s.contains("cookies")));
    assert!(a.contains(&"--no-simulate".into()));
    for kind in ["best", "opus", "aac", "mp3", "flac", "m4a"] {
        let a = args(Mode::Audio(kind.into()), false, true);
        assert_eq!(
            a.contains(&"--extract-audio".into()),
            matches!(kind, "mp3" | "flac" | "m4a")
        );
    }
    assert!(downloads::command(
        &Request {
            url: "fixture".into(),
            playlist: false,
            mode: Mode::Audio("mp3".into()),
            ffmpeg: false
        },
        &Config::default()
    )
    .is_err());
}
#[test]
fn structured_progress_and_failures_do_not_echo_raw_errors() {
    let mut p = Progress::default();
    downloads::update_progress(
        &mut p,
        "YUTE_PROGRESS:3:24:{\"downloaded_bytes\":72,\"total_bytes\":100}",
    );
    assert_eq!(p.item, Some(3));
    assert_eq!(p.total, Some(24));
    assert_eq!(p.percent, "72%");
    downloads::update_progress(&mut p, "YUTE_DONE");
    downloads::update_progress(&mut p, "ERROR: private URL must never be logged");
    assert_eq!((p.completed, p.failed), (1, 1));
}
#[tokio::test]
async fn fake_playlist_continues_after_one_failure_and_reports_completion() {
    let mut cmd = tokio::process::Command::new("sh");
    cmd.args([
        "-c",
        "printf 'YUTE_DONE\\n'; printf 'ERROR: unavailable\\n' >&2; printf 'YUTE_DONE\\n'",
    ]);
    let (tx, _) = tokio::sync::mpsc::unbounded_channel();
    let p = downloads::run_command(cmd, tx).await.unwrap();
    assert_eq!((p.completed, p.failed), (2, 1));
}
#[tokio::test]
async fn cancellation_kills_owned_download_process() {
    let tmp = tempfile::tempdir().unwrap();
    let pid = tmp.path().join("pid");
    let mut cmd = tokio::process::Command::new("sh");
    cmd.args(["-c", "echo $$ > \"$1\"; exec sleep 30", "fixture"])
        .arg(&pid);
    let (tx, _) = tokio::sync::mpsc::unbounded_channel();
    let task = tokio::spawn(downloads::run_command(cmd, tx));
    for _ in 0..50 {
        if pid.exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    let pid = std::fs::read_to_string(pid).unwrap();
    task.abort();
    let _ = task.await;
    for _ in 0..50 {
        if !std::path::Path::new(&format!("/proc/{}", pid.trim())).exists() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    panic!("owned child survived cancellation");
}
#[test]
fn baked_thumbnail_bars_are_removed_only_with_strong_evidence() {
    use image::{DynamicImage, Rgb, RgbImage};
    let mut img = RgbImage::from_pixel(480, 360, Rgb([0, 0, 0]));
    for y in 45..315 {
        for x in 0..480 {
            img.put_pixel(x, y, Rgb([180, 90, 40]));
        }
    }
    let crop = yute_core::media::thumbs::remove_thumbnail_bars(&DynamicImage::ImageRgb8(img));
    assert_eq!((crop.width(), crop.height()), (480, 270));
    let dark = DynamicImage::ImageRgb8(RgbImage::from_pixel(480, 360, Rgb([0, 0, 0])));
    assert_eq!(
        yute_core::media::thumbs::remove_thumbnail_bars(&dark).height(),
        360
    );
}
