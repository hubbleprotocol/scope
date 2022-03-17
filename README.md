# Example of crank refresh operation: 

- For simplification let's say we only refresh at most 3 prices per IX.
- In this example, we have 10 prices in scope named A, B, C, D, E, F, G, H, I, J.
- We refresh any price older than 30 slots.
- If we fire an IX, we fill the IX as much as possible (minimum refresh size is 3).
- Price age is given by source oracle (pyth) not the current slot of refresh.

## Steps

### Loop 0

- Starting ages: A: 0, B: 5, C: 10, D: 15, E: 20, F: 25, G: 30, H: 35, I: 5, J: 10
- Refresh old prices:
  1. Sort price by ages (oldest first) H, G, F, E, D, C, J, B, I, A
  2. Divide by chunk of 3
  3. For each chunk if oldest price is more than 30, refresh chunk
  4. Only one refreshed chunk H, G, F
- After refresh operation new price ages is: A: 5, B: 10, C: 15, D: 20, E: 25, F: 12, G: 3, H: 3, I: 10, J: 15 (All prices are older by 5 slots about the time to execute the ix, and just refreshed prices are not at 0 because age come from the oracle)
- Get the new oldest price: E: 25, so sleep for 5 slot (400ms*5).
- Loop

### Loop 1

- Starting ages: A: 10, B: 15, C: 20, D: 25, E: 30, F: 17, G: 8, H: 8, I: 15, J: 20
- Refresh old prices:
  1. Sort price by ages (oldest first) E, D, C, J, B, I, F, A, G, H
  2. Divide by chunk of 3
  3. For each chunk if oldest price is more than 30, refresh chunk
  4. Only one refreshed chunk E, D, C
- After refresh operation new price ages is: A: 15, B: 20, C: 3, D: 5, E: 3, F: 22, G: 13, H: 13, I: 20, J: 25
- Get the new oldest price: J: 25, so sleep for 5 slot (400ms*5).
- Loop