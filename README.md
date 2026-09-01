## Description

A simple arbitrage bot using artemis and aloy. 
consists of 3 main modules: 
- `collector`: collects sync events from a set of uniswap v2 pools (examples given in `pools.toml`.
- `strategy`: finds possible arbitrage opportunities by estimating fees, and using UniSwap's constant product AMM.
- `executor`: bundles the actions found by strategy into a FlashBots bundle for atomic execution.

