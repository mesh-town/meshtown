//! The loss-burst counter of the `lossy_link` self-checks.
//! Plugged in as `#[path = "common/lossy_link_gaps.rs"] mod gaps;` by the model and shape checks.

pub fn gaps(seqs: &[u32], total: u32) -> Vec<u32> {
    let mut seen = vec![false; total as usize];
    seqs.iter().for_each(|s| seen[*s as usize] = true);
    let mut runs = Vec::new();
    let mut run = 0;
    for ok in seen {
        match ok {
            false => run += 1,
            true if run > 0 => runs.push(std::mem::take(&mut run)),
            true => {}
        }
    }
    if run > 0 {
        runs.push(run);
    }
    runs
}
