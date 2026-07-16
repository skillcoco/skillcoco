//! Demonstrate the Bayesian Knowledge Tracing (BKT) update rule.
//!
//! Runs a synthetic observation sequence (5 correct, 2 incorrect, 3 more
//! correct) through `update_mastery` and prints the mastery trajectory.
//! The math is documented in `docs/BKT.md`.
//!
//! Run with:
//!
//! ```bash
//! cargo run -p skillcoco-core --example bkt_update
//! ```

use skillcoco_core::bkt::{update_mastery, BKTParams, MASTERY_THRESHOLD};

fn main() {
    let params = BKTParams::default();

    println!(
        "BKT defaults: p_know={:.2}, p_learn={:.2}, p_guess={:.2}, p_slip={:.2}",
        params.p_know, params.p_learn, params.p_guess, params.p_slip
    );
    println!("Mastery threshold: {MASTERY_THRESHOLD}");
    println!();

    // 5 correct, 2 incorrect, 3 more correct.
    let observations = [true, true, true, true, true, false, false, true, true, true];

    let mut mastery = params.p_know;
    println!("step  outcome     mastery   crossed?");
    println!("----  --------    --------  --------");
    println!("  0   (prior)     {mastery:.4}    ");
    let mut crossed_at: Option<usize> = None;

    for (i, &correct) in observations.iter().enumerate() {
        mastery = update_mastery(&params, mastery, correct);
        let outcome = if correct { "correct" } else { "incorrect" };
        let crossed_now = mastery >= MASTERY_THRESHOLD && crossed_at.is_none();
        if crossed_now {
            crossed_at = Some(i + 1);
        }
        let crossed_marker = if crossed_now { "<- crossed" } else { "" };
        println!(
            "  {:<3} {:<10}  {:.4}    {}",
            i + 1,
            outcome,
            mastery,
            crossed_marker
        );
    }

    println!();
    if let Some(step) = crossed_at {
        println!("Mastery threshold crossed at step {step}.");
    } else {
        println!("Mastery threshold NOT crossed within {} steps.", observations.len());
    }
    println!("Final mastery estimate: {mastery:.4}");
}
