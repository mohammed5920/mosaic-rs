pub(crate) mod bb_util;
pub(crate) mod file_util;
pub(crate) mod gpu_util;
pub(crate) mod vid_util;

use std::{
    thread,
    time::{Duration, Instant},
};

use parking_lot::deadlock;
use rayon::prelude::*;

pub(crate) fn benchmark<T>(label: &str, function: impl FnOnce() -> T) -> T {
    let start = Instant::now();
    let res = function();
    let duration = Instant::now().duration_since(start);
    let seconds = duration.as_secs_f32();
    let ratio = seconds.min(1.0);
    let r = (ratio * 255.0) as u8;
    let g = ((1.0 - ratio) * 255.0) as u8;
    println!(
        "{label} took \x1b[38;2;{r};{g};{}m{:#?}\x1b[0m",
        0, duration
    );
    res
}

///like np.unique
pub(crate) fn vec_unique<T: Send + Ord>(input: &mut Vec<T>) {
    input.par_sort_unstable();
    input.dedup();
}

pub(crate) fn colour_to_key(rgb: [u8; 3]) -> usize {
    let [r, g, b] = rgb;
    (r as usize) << 16 | (g as usize) << 8 | (b as usize)
}

pub(crate) fn is_power_of_two(number: u64) -> bool {
    (number as f64).log2().fract() == 0.0
}

pub(crate) fn set_panic_hook() {
    let old_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        //printing it is still useful
        old_hook(info);
        let payload = info.payload_as_str().unwrap_or("idk");
        let (file, line) = info
            .location()
            .map_or(("Not provided", 0), |l| (l.file(), l.line()));

        rfd::MessageDialog::new()
            .set_title("Do you understand what just happened?")
            .set_description(format!(
                "Your application just crashed.\n\n\
                 One file ({file}). One line ({line}).\n\n\
                 Here's what that really means 👇🧵\n\n\
                 {payload}",
            ))
            .set_level(rfd::MessageLevel::Error)
            .set_buttons(rfd::MessageButtons::Ok)
            .show();
    }));
}

pub(crate) fn detect_deadlocks() {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(10));
            let deadlocks = deadlock::check_deadlock();
            if deadlocks.is_empty() {
                continue;
            }

            println!("{} deadlocks detected", deadlocks.len());
            for (i, threads) in deadlocks.iter().enumerate() {
                println!("deadlock #{}", i);
                for t in threads {
                    println!("thread Id {:#?}", t.thread_id());
                    println!("{:#?}", t.backtrace());
                }
            }
        }
    });
}
