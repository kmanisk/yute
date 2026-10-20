use anyhow::Result;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub struct MpvProfile {
    pub dir: PathBuf,
}

impl MpvProfile {
    pub fn new() -> Self {
        let dir = crate::config::config_dir().join("mpv");
        Self { dir }
    }

    pub fn ensure_dirs(&self) -> Result<()> {
        fs::create_dir_all(&self.dir)?;
        fs::create_dir_all(self.dir.join("scripts"))?;
        fs::create_dir_all(self.dir.join("script-opts"))?;
        Ok(())
    }

    pub fn setup_default_configs(
        &self,
        user_cookies: Option<&Path>,
        hwdec: Option<&str>,
    ) -> Result<()> {
        self.ensure_dirs()?;

        let hwdec_str = hwdec.unwrap_or("auto-safe");
        let mpv_conf_path = self.dir.join("mpv.conf");
        let mut conf = format!(
            "# ytui dedicated mpv configuration\n\
            ytdl-format=bestvideo[height<=?1080][fps<=?60]+bestaudio/best[height<=?1080]/best\n\
            hwdec={hwdec_str}\n\
            vo=gpu-next\n\
            cache=yes\n\
            demuxer-max-bytes=256MiB\n\
            demuxer-max-back-bytes=128MiB\n\
            demuxer-readahead-secs=45\n\
            demuxer-seekable-cache=yes\n\
            hr-seek=no\n\
            save-position-on-quit=yes\n\
            stream-buffer-size=4MiB\n\
            network-timeout=15\n\
            ytdl-raw-options-append=no-playlist=\n\
            ytdl-raw-options-append=concurrent-fragments=4\n\
            sub-auto=fuzzy\n\
            sub-visibility=yes\n\
            slang=en,eng,en-US,en-GB,hi,hin\n\
            osd-bar=no\n\
            keep-open=yes\n\
            force-window=yes\n",
        );
        if let Some(cp) = user_cookies {
            if cp.exists() {
                conf.push_str(&format!(
                    "ytdl-raw-options-append=cookies={}\n\
                    ytdl-raw-options-append=mark-watched=\n",
                    cp.display()
                ));
            }
        }
        let _ = fs::write(mpv_conf_path, conf);

        let input_conf_path = self.dir.join("input.conf");
        let input_conf = "# Global Speed, volume, seeking, subtitles, and chat\n\
            [ add volume 5\n\
            ' add volume -5\n\
            UP add volume 5\n\
            DOWN add volume -5\n\
            SPACE cycle pause\n\
            k cycle pause\n\
            K cycle pause\n\
            ; cycle pause\n\
            q quit\n\
            ESC set fullscreen no\n\
            f cycle fullscreen\n\
            F cycle fullscreen\n\
            i script-binding stats/display-stats-toggle\n\
            I script-binding stats/display-stats-toggle\n\
            w set speed 1.0\n\
            e set speed 1.5\n\
            a set speed 1.75\n\
            r set speed 2.0\n\
            d multiply speed 1.1\n\
            R add speed 0.05\n\
            E add speed -0.05\n\
            z seek -10\n\
            x seek 10\n\
            j seek -10\n\
            l seek 10\n\
            , seek -5\n\
            . seek 5\n\
            < add speed -0.25\n\
            > add speed 0.25\n\
            Ctrl+, frame-back-step\n\
            Ctrl+. frame-step\n\
            LEFT seek -5\n\
            RIGHT seek 5\n\
            0 seek 0 absolute-percent\n\
            1 seek 10 absolute-percent\n\
            2 seek 20 absolute-percent\n\
            3 seek 30 absolute-percent\n\
            4 seek 40 absolute-percent\n\
            5 seek 50 absolute-percent\n\
            6 seek 60 absolute-percent\n\
            7 seek 70 absolute-percent\n\
            8 seek 80 absolute-percent\n\
            9 seek 90 absolute-percent\n\
            c cycle sub-visibility\n\
            v cycle sub\n\
            V cycle sub down\n\
            h script-binding youtube-chat/chat-hidden\n\
            H script-binding youtube-chat/load-chat\n\
            m cycle mute\n";
        let _ = fs::write(input_conf_path, input_conf);

        let script_opts_dir = self.dir.join("script-opts");
        let _ = fs::create_dir_all(&script_opts_dir);
        let chat_conf = "auto-load=no\n\
            live-chat-directory=~/.local/share/youtube-live-chats\n\
            yt-dlp-path=yt-dlp\n\
            show-author=yes\n\
            author-color=random\n\
            message-color=ffffff\n\
            font-size=16\n\
            border-size=2\n\
            message-duration=10000\n\
            max-message-line-length=40\n\
            anchor=1\n";
        let _ = fs::write(script_opts_dir.join("youtube_chat.conf"), chat_conf);
        let _ = fs::write(script_opts_dir.join("mpv-youtube-chat.conf"), chat_conf);

        Ok(())
    }

    pub async fn install_recommended_scripts(&self) -> Result<()> {
        self.ensure_dirs()?;
        let scripts_dir = self.dir.join("scripts");

        let scripts = [
            (
                "sponsorblock_minimal.lua",
                "https://raw.githubusercontent.com/po5/mpv_sponsorblock/master/sponsorblock.lua",
            ),
            (
                "quality-menu.lua",
                "https://raw.githubusercontent.com/christoph-heinrich/mpv-quality-menu/master/quality-menu.lua",
            ),
        ];

        // Check if user has them locally in ~/.config/mpv/scripts/
        let user_mpv_scripts = if let Some(home) = std::env::var_os("HOME") {
            PathBuf::from(home).join(".config/mpv/scripts")
        } else {
            PathBuf::from(".config/mpv/scripts")
        };

        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .build()
            .unwrap_or_default();

        for (name, url) in scripts {
            let dest = scripts_dir.join(name);
            let local_src = user_mpv_scripts.join(name);

            if local_src.exists() {
                if let Ok(_) = fs::copy(&local_src, &dest) {
                    println!("Synced {name} from local mpv scripts");
                    continue;
                }
            }

            println!("Fetching {name} from {url}...");
            match client.get(url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    if let Ok(text) = resp.text().await {
                        fs::write(&dest, text)?;
                        println!("Saved {name} to {}", dest.display());
                    }
                }
                _ => println!("Warning: Failed to download {name}"),
            }
        }

        Ok(())
    }
}
