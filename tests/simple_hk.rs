extern crate hk;
use hk::HegselmannKrause;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmp_naive_tree() {
        let mut hk1 = HegselmannKrause::new(100, 0., 1., 13);
        let mut hk2 = HegselmannKrause::new(100, 0., 1., 13);

        // test that the two methods will yield identical results for 100 sweeps
        for _ in 0..100 {
            hk1.sweep_naive();
            hk2.sweep_tree();
            assert!(hk1 == hk2);
        }
    }
}
