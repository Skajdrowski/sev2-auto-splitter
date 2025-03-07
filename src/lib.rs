#![no_std]
#![allow(non_snake_case)]

extern crate alloc;

use asr::{future::sleep, settings::Gui, Process};
use core::{str, time::Duration};
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

struct Addr {
    startAddress: u32,
    loadAddress: u32,
    splashAddress: u32,
    levelAddress: u32,
    bulletCamAddress: u32,
    objectiveAddress: u32,
    mcAddress: u32
}

impl Addr {
    fn original() -> Self {
        Self {
            startAddress: 0x689FE2,
            loadAddress: 0x67FC38,
            splashAddress: 0x653B40,
            levelAddress: 0x685F31,
            bulletCamAddress: 0x65B917,
            objectiveAddress: 0x656F3C,
            mcAddress: 0x689FD2
        }
    }
    
    fn remastered() -> Self {
        Self {
            startAddress: 0x799A77,
            loadAddress: 0x774FE3,
            splashAddress: 0x74C670,
            levelAddress: 0x7CFC7D,
            bulletCamAddress: 0x76DD17,
            objectiveAddress: 0x7CF568,
            mcAddress: 0x799A63
        }
    }
}

async fn Attacher() -> Process {
    loop {
        if let Some(process) = ["SniperEliteV2.exe", "SEV2_Remastered.exe"].into_iter().find_map(Process::attach)
        {
            return process;
        }
        asr::print_message("Waiting for game to attach...");
        sleep(Duration::from_secs(1)).await;
    }
}

async fn main() {
    let mut settings = Settings::register();
    let mut oldLevel = [0u8; 38];
    let mut oldSplash = 0;
    
    let mut baseAddress = asr::Address::new(0);
    let mut addrStruct = Addr::original();

    loop {
        let process = Attacher().await;

        process.until_closes(async {
            if let Some((base, moduleSize)) = ["SniperEliteV2.exe", "SEV2_Remastered.exe"].into_iter().find_map (
                |exe| Some((process.get_module_address(exe).ok()?, process.get_module_size(exe).ok()?))
            ) {
                baseAddress = base;
                if moduleSize == 18169856 {
                    addrStruct = Addr::remastered();
                }
            }

            loop {
                settings.update();
                let start = || {
                    if let Ok(startByte) = process.read::<u8>(baseAddress + addrStruct.startAddress) {
                        if startByte == 1 {
                            asr::timer::start();
                        }
                    }
                };
            
                let isLoading = || {
                    if let (Ok(loadByte), Ok(splashByte)) = (
                        process.read::<u8>(baseAddress + addrStruct.loadAddress),
                        process.read::<u8>(baseAddress + addrStruct.splashAddress)
                    ) {
                        if loadByte == 0 || splashByte == 0 {
                            asr::timer::pause_game_time();
                        }
                        else {
                            asr::timer::resume_game_time();
                        }
                    }
                };
                let level_bytes = process.read_vec(baseAddress + addrStruct.levelAddress, 38).ok();
                
                let mut levelSplit = || {
                    if let Some(ref levelByte) = level_bytes {
                        if levelByte != &oldLevel {
                            oldLevel.copy_from_slice(levelByte);
                            let level = str::from_utf8(levelByte).unwrap_or("").split('\0').next().unwrap_or("");
                            asr::print_message(level);
                            if level != "" && level != "nu\\Options.gui" && level != "nu\\GUIMenuCommon.asr" && level != "Tutorial\\M01_Tutorial.pc" {
                                asr::timer::split();
                            }
                        }
                    }
                };
                
                let lastSplit = || {
                    if let (Some(levelByte), Ok(bulletCamByte), Ok(objectiveByte)) = (
                        level_bytes.as_deref(),
                        process.read::<u8>(baseAddress + addrStruct.bulletCamAddress),
                        process.read::<u8>(baseAddress + addrStruct.objectiveAddress)
                    ) {
                        if str::from_utf8(levelByte).unwrap_or("").split('\0').next().unwrap_or("") == "BrandenburgGate\\M11_BrandenburgGate.pc" && bulletCamByte == 1 && objectiveByte == 3 {
                            asr::timer::split();
                        }
                    }
                };
                
                let mut individualLvl = || {
                    if let (Ok(ilSplashByte), Ok(mcByte)) = (
                        process.read::<u8>(baseAddress + addrStruct.splashAddress),
                        process.read::<u8>(baseAddress + addrStruct.mcAddress),
                    ) {
                        if mcByte == 1 {
                            asr::timer::split();
                        }
                        if ilSplashByte != oldSplash {
                            oldSplash = ilSplashByte;
                        }
                        if let Some(levelByte) = level_bytes.as_deref() {
                            if (ilSplashByte == 1 && oldSplash == 0) && str::from_utf8(levelByte).unwrap_or("").split('\0').next().unwrap_or("") != "nu\\Options.gui" {
                                asr::timer::start();
                            }
                        }
                    }
                };
                
                if settings.Full_game_run {
                    //let start_time = asr::time_util::Instant::now();
                    start();
                    //let end_time = start_time.elapsed();
                    levelSplit();
                    lastSplit();
                    //asr::print_message(&alloc::format!("Tick time: {:?}", end_time));
                }
                if settings.Individual_level {
                    individualLvl();
                }
                isLoading();
                
                sleep(Duration::from_nanos(16666667)).await;
            }
        }).await;
    }
}