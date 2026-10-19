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

    pub fn setup_default_configs(&self, user_cookies: Option<&Path>) -> Result<()> {
        self.ensure_dirs()?;

        let mpv_conf_path = self.dir.join("mpv.conf");
        let mut conf = String::from(
            "# ytui dedicated mpv configuration\n\
            ytdl-format=bestvideo[height<=?1080]+bestaudio/best\n\
            hwdec=auto-safe\n\
            vo=gpu\n\
            cache=yes\n\
            demuxer-max-bytes=150M\n\
            demuxer-max-back-bytes=50M\n\
            demuxer-readahead-secs=20\n\
            osd-bar=no\n\
            keep-open=no\n\
            force-window=immediate\n",
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
        if !input_conf_path.exists() {
            let input_conf = "# Global Speed and seeking presets\n\
                w set speed 1.0\n\
                e set speed 1.5\n\
                a set speed 1.75\n\
                r set speed 2.0\n\
                d multiply speed 1.1\n\
                R add speed 0.05\n\
                E add speed -0.05\n\
                z seek -5\n\
                x seek 5\n\
                j seek -10\n\
                l seek 10\n\
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
                m cycle mute\n";
            fs::write(input_conf_path, input_conf)?;
        }

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
                "reload.lua",
                "https://raw.githubusercontent.com/4e6/mpv-reload/master/main.lua",
            ),
            (
                "quality-menu.lua",
                "https://raw.githubusercontent.com/christoph-heinrich/mpv-quality-menu/master/quality-menu.lua",
            ),
            (
                "youtube-chat.lua",
                "https://raw.githubusercontent.com/BanchouBoo/mpv-youtube-chat/master/youtube-chat.lua",
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
