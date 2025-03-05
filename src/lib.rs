#![no_std]
#![allow(non_snake_case)]

extern crate alloc;

use alloc::{string::{String, ToString}};
use asr::{future::sleep, future::retry, settings::Gui, Process};
use core::time::Duration;
use dlmalloc::GlobalDlmalloc;

#[global_allocator]
static ALLOCATOR: GlobalDlmalloc = GlobalDlmalloc;

asr::async_main!(stable);
asr::panic_handler!();

#[derive(Gui)]
struct Settings {
    #[default = true]
    Full_game_run: bool,
    #[default = false]
    Individual_level: bool
}

async fn main() {
    let mut settings = Settings::register();
    let mut oldLevel = String::new();
    let mut oldSplash = 0;
    
    let mut startAddress = 0x689FE2;

    let mut loadAddress = 0x67FC38;
    let mut splashAddress = 0x653B40;

    let mut levelAddress = 0x685F31;

    let mut bulletCamAddress = 0x65B917;
    let mut objectiveAddress = 0x656F3C;
    
    let mut mcAddress = 0x689FD2;

    //asr::print_message("Autosplitter for Sniper Elite V2 started!");

    let process = retry(|| {
        ["SniperEliteV2.exe", "SEV2_Remastered.exe"].into_iter().find_map(Process::attach)
    }).await;

    process.until_closes(async {
        let baseAddress = match process.get_module_address("SniperEliteV2.exe") {
            Ok(baseAddress) => baseAddress,
            Err(_) => {
                match process.get_module_address("SEV2_Remastered.exe") {
                    Ok(baseAddress) => baseAddress,
                    Err(_) => return
                }
            }
        };
        
        let moduleSize = match process.get_module_size("SniperEliteV2.exe") {
            Ok(moduleSize) => moduleSize,
            Err(_) => {
                match process.get_module_size("SEV2_Remastered.exe") {
                    Ok(moduleSize) => moduleSize,
                    Err(_) => return
                }
            }
        };
        if moduleSize == 18169856 {
            startAddress = 0x799A77;

            loadAddress = 0x774FE3;
            splashAddress = 0x74C670;
            
            levelAddress = 0x7CFC7D;
            
            bulletCamAddress = 0x76DD17;
            objectiveAddress = 0x7CF568;
            
            mcAddress = 0x799A63;
        }

        let start = || {
            let startByte = match process.read::<u8>(baseAddress + startAddress) {
                Ok(startByte) => startByte,
                Err(_) => return
            };

            if startByte == 1 {
                asr::timer::start();
            }
        };
        
        let isLoading = || {
            let loadByte = match process.read::<u8>(baseAddress + loadAddress) {
                Ok(loadByte) => loadByte,
                Err(_) => return
            };
            let splashByte = match process.read::<u8>(baseAddress + splashAddress) {
                Ok(splashByte) => splashByte,
                Err(_) => return
            };
            
            if loadByte == 0 || splashByte == 0 {
                asr::timer::pause_game_time();
            }
            else {
                asr::timer::resume_game_time();
            }
        };

        let mut levelSplit = || {
            let levelByte = match process.read_vec(baseAddress + levelAddress, 38) {
                Ok(levelByte) => levelByte,
                Err(_) => return
            };
            let level = String::from_utf8_lossy(&levelByte).split('\0').next().unwrap_or("").to_string();
            if level != oldLevel {
                oldLevel = level.clone();
                if (level != "nu\\Options.gui" || oldLevel != "nu\\Options.gui") && level != "Tutorial\\M01_Tutorial.pc" {
                    asr::timer::split();
                }
            }
        };

        let lastSplit = || {
            let lastLevelByte = match process.read_vec(baseAddress + levelAddress, 38) {
                Ok(lastLevelByte) => lastLevelByte,
                Err(_) => return
            };
            let bulletCamByte = match process.read::<u8>(baseAddress + bulletCamAddress) {
                Ok(bulletCamByte) => bulletCamByte,
                Err(_) => return
            };
            let objectiveByte = match process.read::<u8>(baseAddress + objectiveAddress) {
                Ok(objectiveByte) => objectiveByte,
                Err(_) => return
            };

            let lastLevel = String::from_utf8_lossy(&lastLevelByte).split('\0').next().unwrap_or("").to_string();
            if lastLevel == "BrandenburgGate\\M11_BrandenburgGate.pc" && bulletCamByte == 1 && objectiveByte == 3 {
                asr::timer::split();
            }
        };

        let mut individualLvl = || {
            let ilSplashByte = match process.read::<u8>(baseAddress + splashAddress) {
                Ok(ilSplashByte) => ilSplashByte,
                Err(_) => return
            };
            let mcByte = match process.read::<u8>(baseAddress + mcAddress) {
                Ok(mcByte) => mcByte,
                Err(_) => return
            };
            let ilLevelByte = match process.read_vec(baseAddress + levelAddress, 38) {
                Ok(ilLevelByte) => ilLevelByte,
                Err(_) => return
            };
            let ilLevel = String::from_utf8_lossy(&ilLevelByte).split('\0').next().unwrap_or("").to_string();

            if mcByte == 1 {
                asr::timer::split();
            }

            if (ilSplashByte == 1 && oldSplash == 0) && ilLevel != "nu\\Options.gui" {
                asr::timer::start();
            }
            if ilSplashByte != oldSplash {
                oldSplash = ilSplashByte;
            }
        };

        loop {
            settings.update();
            if settings.Full_game_run {
                start();
                levelSplit();
                lastSplit();
            }
            if settings.Individual_level {
                individualLvl();
            }
            isLoading();

            sleep(Duration::from_micros(16666)).await;
        }
    }).await;
}