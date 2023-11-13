#[macro_export]
macro_rules! assert_fuzzy_eq {
    ($actual:expr, $expected:expr, $epsilon:expr) => {
        let eps = $epsilon as i128;
        let act = $actual as i128;
        let exp = $expected as i128;
        let diff = (act - exp).abs();
        if diff > eps {
            panic!(
                "Actual {} Expected {} diff {} Epsilon {}",
                $actual, $expected, diff, eps
            );
        }
    };

    ($actual:expr, $expected:expr, $epsilon:expr, $type:ty) => {
        let eps = $epsilon as $type;
        let act = $actual as $type;
        let exp = $expected as $type;
        let diff = (act - exp).abs();
        if diff > eps {
            panic!(
                "Actual {} Expected {} diff {} Epsilon {}",
                $actual, $expected, diff, eps
            );
        }
    };
}
