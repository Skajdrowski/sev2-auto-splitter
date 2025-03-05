#![no_std]

extern crate alloc;

use alloc::string::{String, ToString};
use asr::{future::sleep, settings::Gui, Process};
use core::time::Duration;
use dlmalloc::GlobalDlmalloc;

#[global_allocator]
static ALLOCATOR: GlobalDlmalloc = GlobalDlmalloc;

asr::async_main!(stable);
asr::panic_handler!();

#[derive(Gui)]
#[allow(non_snake_case)]
struct Settings {
    #[default = true]
    Enable_autosplitter: bool,
}

async fn main() {
    let mut settings = Settings::register();

    let mut old_level = String::new();
    asr::print_message("Autosplitter for Sniper Elite V2 started!");

    loop {
        let process = Process::wait_attach("SniperEliteV2.exe").await;

        process.until_closes(async {
            let baseaddress = match process.get_module_address("SniperEliteV2.exe") {
                Ok(addr) => addr,
                Err(_) => return
            };

            loop {
                settings.update();

                // Check if the timer should start
                if let Ok(start) = process.read::<u8>(baseaddress + 0x689FE2) {
                    if start == 1 {
                        asr::timer::start();
                    }
                }

                // Read the current level
                if let Ok(level_bytes) = process.read_vec(baseaddress + 0x685F31, 38) {
                    let level = String::from_utf8_lossy(&level_bytes).split('\0').next().unwrap_or("").to_string();
                    if level != old_level {
                        old_level = level.clone();
                        asr::print_message(&level);
                        if (level != "nu\\Options.gui" || old_level != "nu\\Options.gui") && level != "Tutorial\\M01_Tutorial.pc" {
                            asr::timer::split();
                        }
                    }

                    // Check for specific conditions to split
                    if let (Ok(bulletcam), Ok(objective)) = (process.read::<u8>(baseaddress + 0x65B917), process.read::<u8>(baseaddress + 0x656F3C)) {
                        if level == "BrandenburgGate\\M11_BrandenburgGate.pc" && bulletcam == 1 && objective == 3 {
                            asr::timer::split();
                        }
                    }

                    // Check loading state
                    if let (Ok(loading), Ok(splash)) = (process.read::<u8>(baseaddress + 0x67FC38), process.read::<u8>(baseaddress + 0x653B40)) {
                        if loading == 0 || splash == 0 {
                            asr::timer::pause_game_time();
                        } else {
                            asr::timer::resume_game_time();
                        }
                    }
                }
                sleep(Duration::from_micros(16666)).await;
            }
        }).await;
    }
}