use yute::{
    app::Screen,
    commands::{self, Command, History},
};
#[test]
fn parse_commands_and_arguments() {
    assert_eq!(
        commands::parse(":search rust async").unwrap(),
        Command::Search("rust async".into())
    );
    assert_eq!(
        commands::parse(":volume +5").unwrap(),
        Command::Volume(5.0, true)
    );
    assert_eq!(
        commands::parse(":seek -30").unwrap(),
        Command::Seek(-30.0, true)
    );
    assert_eq!(
        commands::parse(":seek 1:20").unwrap(),
        Command::Seek(80.0, false)
    );
    assert_eq!(
        commands::parse(":watch-later").unwrap(),
        Command::Navigate(Screen::WatchLater)
    );
    assert_eq!(
        commands::parse(":wl").unwrap(),
        Command::Navigate(Screen::WatchLater)
    );
    assert_eq!(
        commands::parse(":queue next").unwrap(),
        Command::QueueAdd(true)
    );
}
#[test]
fn invalid_commands_never_panic_or_dispatch() {
    for input in [
        "",
        ":",
        ":unknown-command",
        ":home extra",
        ":search",
        ":volume NaN",
        ":volume 101",
        ":seek 1:99",
        ":seek 99999999999999999999999999:30",
        ":seek inf",
        ":queue wat",
        ":watch-later add",
        ":copy cookie",
        ":set graphics chromium",
    ] {
        assert!(
            commands::parse(input).is_err(),
            "{input:?} must fail with a useful error"
        );
    }
}
#[test]
fn set_command_exposes_validated_ui_and_playback_settings() {
    for input in [
        ":set icons nerd",
        ":set columns auto",
        ":set columns 8",
        ":set cache-seconds 40",
        ":set codec av1",
        ":set hwdec nvdec",
        ":set autoplay off",
        ":set history-sync on",
        ":set thumbnail-cache 512",
        ":set region IN",
    ] {
        assert!(commands::parse(input).is_ok(), "{input}");
    }
    for input in [
        ":set columns 0",
        ":set columns 9",
        ":set cache-seconds 999",
        ":set icons emoji",
        ":set autoplay maybe",
        ":set thumbnail-cache 4",
    ] {
        assert!(commands::parse(input).is_err(), "{input}");
    }
    assert_eq!(commands::completions("set col"), vec!["set columns"]);
    assert_eq!(
        commands::completions("set sidebar-at-launch r"),
        vec!["set sidebar-at-launch remember"]
    );
    assert!(matches!(
        commands::parse(":set sidebar-at-launch remember"),
        Ok(Command::Set(key, value)) if key == "sidebar-at-launch" && value == "remember"
    ));
    assert!(commands::parse(":set sidebar-at-launch maybe").is_err());
    assert!(commands::completions("set codec ").contains(&"set codec av1".to_string()));
}

#[test]
fn completion_and_aliases_are_unambiguous() {
    assert_eq!(commands::completions("his"), vec!["history"]);
    assert!(commands::completions("q").contains(&"queue".to_string()));
    assert!(commands::completions("q").contains(&"quit".to_string()));
    assert_eq!(commands::completions("queue a"), vec!["queue add"]);
    assert_eq!(commands::completions("comment r"), vec!["comment reply"]);
    assert_eq!(
        commands::parse(":comment delete").unwrap(),
        Command::Comment("delete".into())
    );
    assert!(commands::parse(":comment post").is_err());
    assert_eq!(commands::parse(":q").unwrap(), Command::Quit(false));
    assert_eq!(commands::parse(":logout").unwrap(), Command::Logout);
    assert_eq!(
        commands::parse(":logout second account").unwrap(),
        Command::Account("remove second account".into())
    );
}
#[test]
fn session_history_preserves_draft_and_is_bounded() {
    let mut history = History::default();
    assert_eq!(history.previous("draft"), "draft");
    for i in 0..120 {
        history.record(&format!("search {i}"));
    }
    assert_eq!(history.previous("draft"), "search 119");
    assert_eq!(history.previous("ignored"), "search 118");
    assert_eq!(history.next_entry(), "search 119");
    assert_eq!(history.next_entry(), "draft");
    for _ in 0..200 {
        history.previous("ignored");
    }
    assert_eq!(history.previous("ignored"), "search 20");
}
#[test]
fn command_help_and_keyboard_help_are_separate() {
    let help = commands::help(Some("search")).unwrap();
    assert!(help.contains("search <query>"));
    assert!(help.contains("linux kernel"));
    assert!(commands::help(Some("queue"))
        .unwrap()
        .contains("queue [add"));
    assert!(commands::help(Some("not-a-command")).is_err());
}

#[test]
fn quality_command_accepts_limits_and_rejects_unbounded_input() {
    use yute::commands::{completions, parse, Command};
    assert_eq!(parse(":quality"), Ok(Command::Quality(None)));
    assert_eq!(
        parse(":quality 720"),
        Ok(Command::Quality(Some("720p".into())))
    );
    assert_eq!(
        parse(":quality 4k"),
        Ok(Command::Quality(Some("2160p".into())))
    );
    for arg in ["NaN", "-1", "50000000", "best extra"] {
        assert!(parse(&format!(":quality {arg}")).is_err());
    }
    assert!(completions("quality 7").contains(&"quality 720p".into()));
}
