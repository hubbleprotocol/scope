use crate::Result;

pub mod solanacompass {
    use crate::errors::ErrorKind;

    use super::*;

    use std::collections::HashMap;

    use serde::Deserialize;

    #[derive(Deserialize, Debug)]
    struct Entry {
        name: String,
        data: HashMap<String, f64>,
    }

    /// Propose a recommended fee based on the fees in the last 5 minutes
    ///
    /// Fee is in microlamports per CU
    pub async fn get_last_5_min_median_fee() -> Result<u64> {
        let url = "https://solanacompass.com/statistics/priorityFees?type=avg";
        let resp = reqwest::get(url).await?.json::<Vec<Entry>>().await?;

        if &resp[1].name != "Median Fee (60s Avg)" {
            return Err(ErrorKind::SolanaCompassReturnInvalid);
        }
        let mean_fees_data = &resp[1].data;
        let fees = [
            mean_fees_data["2 mins ago"],
            mean_fees_data["3 mins ago"],
            mean_fees_data["4 mins ago"],
            mean_fees_data["5 mins ago"],
        ];
        let avg_median_fee = fees.into_iter().sum::<f64>() / fees.len() as f64;
        // Fee is in SOL per tx (assuming 200_000 CU), convert to micro lamports per CU
        let fee = (avg_median_fee * (10.0_f64.powi(9)) * 1_000_000.0 / 200_000.0) as u64;

        Ok(fee)
    }

    #[cfg(test)]
    mod test {
        use super::*;

        #[tokio::test]
        async fn test_get_last_5_min_median_fee() {
            let fee = get_last_5_min_median_fee().await.unwrap();
            println!("fee: {}", fee);
            assert!(fee > 0);
        }
    }
}
