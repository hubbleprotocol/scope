# Guideline

## Config Parameters

### Collateral Tokens / Kamino Tokens
- Minimum update delay: 50 seconds
- Minimum job results: 2 (at least 60% of the jobs, depending on how many jobs; e.g. for 2 jobs use all of them, for 3 we can have only 2)
- Minimum oracle results: 4
- Oracle batch size: 6
- Force report period: 0 / empty
- Variance threshold: 0 / empty

### Reward Tokens
- Minimum update delay: 120 seconds
- Minimum job results: 2 (at least 60% of the jobs, depending on how many jobs; e.g. for 2 jobs use all of them, for 3 we can have only 2)
- Minimum oracle results: 4
- Oracle batch size: 6
- Force report period: 0 / empty
- Variance threshold: 0 / empty


### Terminology
- `Minimum update delay`: the minimum amount of time between 2 updates; i.e. there needs to pass at least `minimum_update_delay` from the last update to the next update; if `minimum_update_delay` is bigger the price refreshes less often and the Swtichboard feed costs less
- `Variance threshold`: the minimum percentage in terms of price difference between 2 consecutive prices that will trigger the price to update
