## Hubble scope

## TODO list

Step 1:
- [x] Minimum tests for the bot
- [ ] Increase amount of prices to 512
- [ ] Find refresh size limit and use it in bot
- [ ] Look into bot logs

Step 2:
- [ ] deploy on devnet

Step 3:
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

To discuss:
- [ ] Strategy to have multiple bots running : Proposal: detect stall with different time threshold. Each bot start when stall is detected + return to sleep based on monitor of contract interractions?
