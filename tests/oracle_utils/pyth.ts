import { BN, Program, web3 } from '@project-serum/anchor';
import { parsePriceData } from '@pythnetwork/client';
import { PublicKey, SYSVAR_CLOCK_PUBKEY } from '@solana/web3.js';
import Decimal from 'decimal.js';
import * as anchor from '@project-serum/anchor';
import { IMockOracle, ITokenEntry, OracleType } from './mock_oracles';

const PYTH_PRICE_ACCOUNT_SIZE: number = 3312;

export enum PriceStatus {
  Unknown = 0,
  Trading = 1,
  Halted = 2,
  Auction = 3,
}

export const createPriceFeed = async (
  mockOracleProgram: Program,
  initPrice: Decimal,
  confidence?: BN,
  expo: number = -8
) => {
  const conf = confidence || new BN(0);
  const collateralTokenFeed = new web3.Keypair();

  await mockOracleProgram.rpc.initializePyth(
    new BN(initPrice.mul(new Decimal(10).pow(new Decimal(-expo))).toNumber()),
    expo,
    conf,
    {
      accounts: {
        oracleAccount: collateralTokenFeed.publicKey,
        clock: SYSVAR_CLOCK_PUBKEY,
      },
      signers: [collateralTokenFeed],
      instructions: [
        web3.SystemProgram.createAccount({
          fromPubkey: mockOracleProgram.provider.wallet.publicKey,
          newAccountPubkey: collateralTokenFeed.publicKey,
          space: PYTH_PRICE_ACCOUNT_SIZE,
          lamports: await mockOracleProgram.provider.connection.getMinimumBalanceForRentExemption(
            PYTH_PRICE_ACCOUNT_SIZE
          ),
          programId: mockOracleProgram.programId,
        }),
      ],
    }
  );
  return collateralTokenFeed.publicKey;
};
export const setFeedPrice = async (mockOracleProgram: Program, newPrice: Decimal, priceFeed: web3.PublicKey) => {
  const info = await mockOracleProgram.provider.connection.getAccountInfo(priceFeed);
  //@ts-expect-error
  const data = parsePriceData(info.data);
  const newPriceBn = new BN(newPrice.mul(new Decimal(10).pow(new Decimal(-data.exponent))).toNumber());
  await mockOracleProgram.rpc.setPricePyth(newPriceBn, {
    accounts: { oracleAccount: priceFeed, clock: SYSVAR_CLOCK_PUBKEY },
  });
};

export class PythMockToken implements ITokenEntry {
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
    return OracleType.Pyth;
  }

  async updatePrice(price: Decimal, decimals?: number): Promise<void> {
    await setFeedPrice(this.program, price, this.account);
    this.price = price;
    if (decimals !== undefined) {
      throw 'Pyth mock cannot change decimals number after init';
    }
  }
}

export class PythMockOracle implements IMockOracle {
  async createFakePriceAccount(
    mockOracleProgram: Program,
    ticker: string,
    initPrice: Decimal,
    decimals: number
  ): Promise<ITokenEntry> {
    const account = await createPriceFeed(mockOracleProgram, initPrice, undefined, -decimals);
    return new PythMockToken(mockOracleProgram, initPrice, ticker, decimals, account);
  }
}
