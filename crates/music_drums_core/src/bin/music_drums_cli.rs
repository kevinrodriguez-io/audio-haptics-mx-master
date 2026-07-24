//! CLI for proving HID++ pulses with Options+ parked.

use music_drums_core::engine::fire_test_pulse;
use music_drums_core::hidpp::{HapticPulse, PulseType};
use music_drums_core::transport::{list_logi_devices, open_best_transport_with_retry};
use std::env;
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
            let intensity: u8 = args
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(60);
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
        _ => {
            eprintln!(
                "music-drums-cli <devices|ping|pulse [intensity] [light|tick|strong|buzz]|test>\n\
                 Park Logi Options+ first: Scripts/logi-mode.sh disable"
            );
            ExitCode::SUCCESS
        }
    }
}
