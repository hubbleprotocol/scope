## Hubble scope

## TODO list

Step 1:
- [x] Minimum tests for the bot
- [x] Increase amount of prices to 512
- [x] Find refresh size limit and use it in bot
- [x] Look into bot logs

Step 2:
- [x] deploy on devnet

Step 3:
- [x] Strategy to have multiple bots running : Detect state stall with different time threshold. Each bot start when stall is detected + return to sleep based on monitor of contract interractions?
- [ ] Tests for security checks
- [ ] Connect to hubble (price diff)

Step 4:
- [ ] More tests for failure/edge cases

Step 5:
- [ ] Add support for non pyth tokens, switchboard, index prices

Low prio:
- [ ] Update to last pyth version
- [ ] Crank only when price change
- [ ] Autorefresh of mapping in crank mode?
