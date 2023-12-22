pub mod solanacompass {
    // Example of a fee struct at https://solanacompass.com/api/fees
    // ```json
    // {
    //   "1": {
    //     "min": 5001,
    //     "max": 10005001,
    //     "avg": 42916.688320592366,
    //     "priorityTx": 29171,
    //     "nonVotes": 38484,
    //     "priorityRatio": 0.7580033260575824,
    //     "avgCuPerBlock": 38492648,
    //     "blockspaceUsageRatio": 0.8019301663194445
    //   },
    //   "5": {
    //     "min": 0,
    //     "max": 25005000,
    //     "avg": 63320.30489703489,
    //     "priorityTx": 141310,
    //     "nonVotes": 182315,
    //     "priorityRatio": 0.7750870745687409,
    //     "avgCuPerBlock": 35336259,
    //     "blockspaceUsageRatio": 0.7361720596527779
    //   },
    //   "15": {
    //     "min": 0,
    //     "max": 240005000,
    //     "avg": 73912.08067500319,
    //     "priorityTx": 376176,
    //     "nonVotes": 485472,
    //     "priorityRatio": 0.7748665216531541,
    //     "avgCuPerBlock": 35243894,
    //     "blockspaceUsageRatio": 0.7342477776587059
    //   }
    // }
    // ```

    use serde::{Deserialize, Serialize};

    struct Fee {
        min: u64,
        max: u64,
        avg: f64,
        priorityTx: u64,
        nonVotes: u64,
        priorityRatio: f64,
        avgCuPerBlock: u64,
        blockspaceUsageRatio: f64,
    }
}
