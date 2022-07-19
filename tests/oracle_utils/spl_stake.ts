import { BN, Program, web3 } from '@project-serum/anchor';
import { PublicKey, SYSVAR_CLOCK_PUBKEY } from '@solana/web3.js';
import Decimal from 'decimal.js';
import { IMockOracle, ITokenEntry, OracleType } from './mock_oracles';

const STAKE_POOL_ACCOUNT_SIZE: number = 529;

const createPriceFeedStakePooltoken = async (
  mockOracleProgram: Program,
  mint_total_supply: BN,
  total_liquidity: BN
) => {
  const collateralTokenFeed = new web3.Keypair();
  const provider_publickey = mockOracleProgram.provider.publicKey!;

  await mockOracleProgram.rpc.initializeStakePool(mint_total_supply, total_liquidity, {
    accounts: { oracleAccount: collateralTokenFeed.publicKey, clock: SYSVAR_CLOCK_PUBKEY },
    signers: [collateralTokenFeed],
    instructions: [
      web3.SystemProgram.createAccount({
        fromPubkey: provider_publickey,
        newAccountPubkey: collateralTokenFeed.publicKey,
        space: STAKE_POOL_ACCOUNT_SIZE,
        lamports: await mockOracleProgram.provider.connection.getMinimumBalanceForRentExemption(
          STAKE_POOL_ACCOUNT_SIZE
        ),
        programId: mockOracleProgram.programId,
      }),
    ],
  });
  return collateralTokenFeed.publicKey;
};

const setFeedPriceStakePool = async (
  mockOracleProgram: Program,
  mint_total_supply: BN,
  total_liquidity: BN,
  priceFeed: web3.PublicKey
) => {
  await mockOracleProgram.rpc.setPriceStakePool(mint_total_supply, total_liquidity, {
    accounts: { oracleAccount: priceFeed, clock: SYSVAR_CLOCK_PUBKEY },
  });
};

function liquiditiesFromPrice(price: Decimal): [BN, BN] {
  // stake pool to token rate = total_liquidity / mint_total_supply
  // total_liquidity = rate * mint_total_supply
  // fix mint_total_supply = 10^10 for a minimum of precisions
  const mint_total_supply = new BN(10).pow(new BN(10)); // So the price have a minimum of precision
  const total_liquidity_decimal = price.mul(new Decimal(10).pow(new Decimal(10)));
  const total_liquidity = new BN(total_liquidity_decimal.toNumber());
  return [mint_total_supply, total_liquidity];
}

export class StakePoolMockToken implements ITokenEntry {
  price: Decimal;
  ticker: string;
  decimals: number;
  account: PublicKey;
  program: Program;

  constructor(mockOracleProgram: Program, price: Decimal, ticker: string, decimals: number, account: PublicKey) {
    this.price = price;
    this.ticker = ticker;
    this.decimals = decimals;
    this.account = account;
    this.program = mockOracleProgram;
  }

  getType(): OracleType {
    return OracleType.SplStake;
  }

  async updatePrice(price: Decimal, decimals?: number): Promise<void> {
    if (decimals !== undefined) {
      throw 'SPL Stake mocked pool cannot change decimals number';
    }
    const supply = liquiditiesFromPrice(price);
    await setFeedPriceStakePool(this.program, supply[0], supply[1], this.account);
    this.price = price;
  }
}

export class StakePoolMockOracle implements IMockOracle {
  async createFakePriceAccount(
    mockOracleProgram: Program,
    ticker: string,
    initPrice: Decimal,
    decimals: number
  ): Promise<ITokenEntry> {
    if (decimals != 15) {
      throw 'SPL Stake pool token price dont allow to set the decimals to anything else than 15';
    }
    const supply = liquiditiesFromPrice(initPrice);
    const account = await createPriceFeedStakePooltoken(mockOracleProgram, supply[0], supply[1]);
    return new StakePoolMockToken(mockOracleProgram, initPrice, ticker, decimals, account);
  }
}
