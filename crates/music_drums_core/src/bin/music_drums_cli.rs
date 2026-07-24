//! CLI for proving HID++ pulses with Options+ parked.

use music_drums_core::config::{
    list_presets, load_preset_by_id, persist_active, save_user_preset, DrumsConfig,
};
use music_drums_core::engine::fire_test_pulse;
use music_drums_core::hidpp::{HapticPulse, PulseType};
use music_drums_core::transport::{list_logi_devices, open_best_transport_with_retry};
use std::env;
use std::path::Path;
use std::process::ExitCode;
use std::time::Duration;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| "help".into());
    match cmd.as_str() {
        "devices" => match list_logi_devices() {
            Ok(lines) => {
                if lines.is_empty() {
                    println!("(no Logitech HID devices)");
                } else {
                    for line in lines {
                        println!("{line}");
                    }
                }
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        "ping" | "status" => match open_best_transport_with_retry(5, Duration::from_millis(200)) {
            Ok(t) => {
                println!("link={:?}", t.link_kind());
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                eprintln!("Tip: run Scripts/logi-mode.sh disable first.");
                eprintln!("Tip: run music-drums-cli devices to inspect HID interfaces.");
                ExitCode::FAILURE
            }
        },
        "pulse" => {
            let intensity: u8 = args.next().and_then(|s| s.parse().ok()).unwrap_or(60);
            let kind = args.next().unwrap_or_else(|| "strong".into());
            match open_best_transport_with_retry(5, Duration::from_millis(200)) {
                Ok(mut t) => {
                    if let Err(e) = t.set_haptic(true, intensity) {
                        eprintln!("set_haptic: {e}");
                        return ExitCode::FAILURE;
                    }
                    let pulse = match kind.as_str() {
                        "light" => HapticPulse::single(PulseType::Light),
                        "tick" => HapticPulse::single(PulseType::Tick),
                        "strong" => HapticPulse::single(PulseType::Strong),
                        "buzz" => HapticPulse::compound(PulseType::Strong, PulseType::Tick),
                        _ => HapticPulse::single(PulseType::Strong),
                    };
                    if let Err(e) = t.trigger(pulse) {
                        eprintln!("trigger: {e}");
                        return ExitCode::FAILURE;
                    }
                    println!(
                        "ok link={:?} intensity={intensity} pulse={kind}",
                        t.link_kind()
                    );
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        "test" => match fire_test_pulse(80) {
            Ok(link) => {
                println!("ok link={link:?}");
                ExitCode::SUCCESS
            }
            Err(e) => {
                eprintln!("error: {e}");
                ExitCode::FAILURE
            }
        },
        "presets" => {
            for p in list_presets() {
                let kind = if p.builtin { "builtin" } else { "user" };
                println!(
                    "{:>12}  {:<24}  {}  {}",
                    p.id,
                    p.name,
                    kind,
                    p.description
                );
            }
            ExitCode::SUCCESS
        }
        "preset" => {
            let sub = args.next().unwrap_or_else(|| "help".into());
            match sub.as_str() {
                "show" | "active" => {
                    let cfg = music_drums_core::config::load_active_or_default();
                    println!("{}", cfg.to_json_pretty().unwrap_or_default());
                    ExitCode::SUCCESS
                }
                "use" => {
                    let id = match args.next() {
                        Some(id) => id,
                        None => {
                            eprintln!("usage: preset use <classic|house|path.json>");
                            return ExitCode::FAILURE;
                        }
                    };
                    match load_preset_by_id(&id).and_then(|c| {
                        persist_active(&c)?;
                        Ok(c)
                    }) {
                        Ok(c) => {
                            println!("active preset: {} ({})", c.name, c.id);
                            ExitCode::SUCCESS
                        }
                        Err(e) => {
                            eprintln!("error: {e}");
                            ExitCode::FAILURE
                        }
                    }
                }
                "export" => {
                    let path = match args.next() {
                        Some(p) => p,
                        None => {
                            eprintln!("usage: preset export <path.json> [classic|house]");
                            return ExitCode::FAILURE;
                        }
                    };
                    let id = args.next();
                    let cfg = if let Some(id) = id {
                        match load_preset_by_id(&id) {
                            Ok(c) => c,
                            Err(e) => {
                                eprintln!("error: {e}");
                                return ExitCode::FAILURE;
                            }
                        }
                    } else {
                        music_drums_core::config::load_active_or_default()
                    };
                    match cfg.save_file(Path::new(&path)) {
                        Ok(()) => {
                            println!("wrote {path}");
                            ExitCode::SUCCESS
                        }
                        Err(e) => {
                            eprintln!("error: {e}");
                            ExitCode::FAILURE
                        }
                    }
                }
                "import" => {
                    let path = match args.next() {
                        Some(p) => p,
                        None => {
                            eprintln!("usage: preset import <path.json>");
                            return ExitCode::FAILURE;
                        }
                    };
                    match DrumsConfig::load_file(Path::new(&path)).and_then(|c| {
                        let path = save_user_preset(&c)?;
                        persist_active(&c)?;
                        Ok((c, path))
                    }) {
                        Ok((c, path)) => {
                            println!("imported {} → {}", c.name, path.display());
                            ExitCode::SUCCESS
                        }
                        Err(e) => {
                            eprintln!("error: {e}");
                            ExitCode::FAILURE
                        }
                    }
                }
                _ => {
                    eprintln!(
                        "preset <show|use <id>|export <path> [id]|import <path>>\n\
                         presets"
                    );
                    ExitCode::SUCCESS
                }
            }
        }
        _ => {
            eprintln!(
                "music-drums-cli <devices|ping|pulse|test|presets|preset ...>\n\
                 Park Logi Options+ first: Scripts/logi-mode.sh disable"
            );
            ExitCode::SUCCESS
        }
    }
}
