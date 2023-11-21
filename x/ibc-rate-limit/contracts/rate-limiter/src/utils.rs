use cosmwasm_std::{Timestamp, Uint256};

use crate::state::{Quota, RateLimit};

/// returns a percentage of the channel_value
pub fn percentage_of_channel_value(channel_value: Uint256, percentage: u8) -> Option<Uint256> {
    if percentage > 100 {
        return None;
    }
    Some(channel_value * Uint256::from_u128(percentage as u128) / Uint256::from_u128(100))
}

/// given an array of rate limits, search and return the channel value of
/// the first quota to end within the next 248 hours
pub fn parse_first_daily_quota_channel_value(
    rate_limits: &[RateLimit],
    now: Timestamp,
) -> Option<Uint256> {
    for rate_limit in rate_limits {
        // currently we are checking if the period is within the next 24 hours
        // todo: consider if this is the right approach
        if now.plus_seconds(86400) >= rate_limit.flow.period_end {
            // return channel_value instead of full struct to leverage copy_type
            return rate_limit.quota.channel_value;
        }
    }
    None
}

#[cfg(test)]
mod test {
    use cosmwasm_std::Timestamp;

    use crate::state::{Flow, Quota};
    const START_TIME: Timestamp = Timestamp::from_seconds(1700553596);
    use super::*;
    fn new_rate_limit(value: Option<u128>, end_time: Timestamp) -> RateLimit {
        RateLimit {
            quota: Quota {
                channel_value: if let Some(value) = value {
                    Some(Uint256::from_u128(value))
                } else {
                    None
                },
                name: Default::default(),
                max_percentage_recv: 0,
                max_percentage_send: 0,
                duration: 0,
            },
            flow: Flow {
                inflow: Uint256::zero(),
                outflow: Uint256::zero(),
                period_end: end_time,
            },
        }
    }
    #[test]
    fn test_percentage_of_channel_value() {
        let rate_limit_1 = new_rate_limit(Some(1_000_000_000), START_TIME.plus_seconds(86400));
        let rate_limit_2 = new_rate_limit(Some(25_000_000_000), START_TIME.plus_seconds(86400 * 2));
        let quota = parse_first_daily_quota_channel_value(
            &[rate_limit_1.clone(), rate_limit_2.clone()],
            START_TIME,
        );
        println!("{:#?}", quota);
        assert_eq!(rate_limit_1.quota.channel_value, quota);
        let percent =
            percentage_of_channel_value(rate_limit_1.quota.channel_value.unwrap(), 25).unwrap();
        assert_eq!(percent, Uint256::from_u128(250000000));
    }
}
