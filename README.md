# Floating Odds Exchange

Bonding-curve based floating odds prediction market, inspired by [FoX](https://github.com/TeamRaccoons/FOXPaper/blob/main/FOXPaper.pdf).

[Source Repository](https://github.com/ChiefWoods/floating-odds-exchange)

## How It Works

An authority initializes a binary YES/NO market, then launches it with seed liquidity so trading can begin. Traders buy bets on one side at a time: price follows a supply-ratio bonding curve (the scarcer side costs more), payment goes into the market pot, and the buyer receives tokens for the chosen outcome. Exact-in and exact-out buys both use that curve, with a slippage bound on the other leg. The authority can pause new buys without blocking claims.

When the event settles, the authority resolves the market to YES, NO, or a void refund. On YES/NO, a protocol fee is taken from the pot; holders of the winning side then claim their share of the remaining pot. If the market is refunded, both sides claim pro-rata from the pot with no fee.

## Built With

### Languages

- [![Quasar](https://img.shields.io/badge/Quasar-0e0d11?style=for-the-badge)](https://quasar-lang.com/)

## Getting Started

### Prerequisites

1. Update your Solana CLI

```sh
agave-install update
```

### Setup

1. Clone the repository

```sh
git clone https://github.com/ChiefWoods/floating-odds-exchange.git
```

2. Resync your program id

```sh
cd programs/floating-odds-exchange
quasar keys sync
```

3. Build the program

On the first build, run:

```sh
sh build-without-stub.sh
```

After that, build normally from `programs/floating-odds-exchange`:

```sh
quasar build
```

#### Testing

Run all tests.

```sh
cd programs/floating-odds-exchange
quasar test
```

## Issues

View the [open issues](https://github.com/ChiefWoods/floating-odds-exchange/issues) for a full list of proposed features and known bugs.

## Acknowledgements

### Resources

- [Shields.io](https://shields.io/)

## Contact

[chii.yuen@hotmail.com](mailto:chii.yuen@hotmail.com)
