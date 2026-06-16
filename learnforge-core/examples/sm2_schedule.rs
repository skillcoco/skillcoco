//! Demonstrate the SuperMemo 2 (SM-2) review-scheduling algorithm.
//!
//! Runs ten successive reviews with quality `4` through `sm2_calculate` and
//! prints the `(repetitions, ease_factor, interval)` triple after each.
//! Also shows the failure-reset rule by injecting one `q=2` review in the
//! middle of the sequence.
//!
//! The math is documented in `docs/SM2.md`.
//!
//! Run with:
//!
//! ```bash
//! cargo run -p learnforge-core --example sm2_schedule
//! ```

use learnforge_core::sm2::sm2_calculate;

fn main() {
    // (quality, label) pairs — one failure injected mid-sequence to show
    // the reset rule.
    let reviews: [(i32, &str); 10] = [
        (4, "correct, mild hesitation"),
        (4, "correct, mild hesitation"),
        (4, "correct, mild hesitation"),
        (4, "correct, mild hesitation"),
        (2, "FAILED (quality<3 → reset)"),
        (4, "correct, mild hesitation"),
        (4, "correct, mild hesitation"),
        (5, "perfect recall"),
        (5, "perfect recall"),
        (5, "perfect recall"),
    ];

    let mut repetitions: i32 = 0;
    let mut ease_factor: f64 = 2.5;
    let mut interval: f64 = 0.0;

    println!("Starting state: repetitions=0, EF=2.50, interval=0");
    println!();
    println!("step  q  outcome                      repetitions  EF      interval (days)");
    println!("----  -  --------------------------   -----------  ------  ---------------");

    for (i, (q, label)) in reviews.iter().enumerate() {
        let r = sm2_calculate(*q, repetitions, ease_factor, interval);
        repetitions = r.repetitions;
        ease_factor = r.ease_factor;
        interval = r.interval;
        println!(
            "  {:<3} {}  {:<27}  {:<11}  {:.2}    {:.2}",
            i + 1,
            q,
            label,
            repetitions,
            ease_factor,
            interval
        );
    }

    println!();
    println!(
        "Final state: repetitions={repetitions}, EF={ease_factor:.2}, interval={interval:.2} days"
    );
}
