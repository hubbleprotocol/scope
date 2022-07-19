import { BN, Program, web3 } from '@project-serum/anchor';
import { PublicKey, SYSVAR_CLOCK_PUBKEY } from '@solana/web3.js';
import Decimal from 'decimal.js';
import { IMockOracle, ITokenEntry, OracleType } from './mock_oracles';

const SWITCHBOARD_V2_ACCOUNT_SIZE: number = 3851;

export const createPriceFeedSwitchboardV2 = async (mockOracleProgram: Program, mantissa: BN, scale: BN) => {
  const collateralTokenFeed = new web3.Keypair();
  const provider_publickey = mockOracleProgram.provider.publicKey!;

  await mockOracleProgram.rpc.initializeSwitchboardV2(mantissa, scale, {
    accounts: { oracleAccount: collateralTokenFeed.publicKey, clock: SYSVAR_CLOCK_PUBKEY },
    signers: [collateralTokenFeed],
    instructions: [
      web3.SystemProgram.createAccount({
        fromPubkey: provider_publickey,
        newAccountPubkey: collateralTokenFeed.publicKey,
        space: SWITCHBOARD_V2_ACCOUNT_SIZE,
        lamports: await mockOracleProgram.provider.connection.getMinimumBalanceForRentExemption(
          SWITCHBOARD_V2_ACCOUNT_SIZE
        ),
        programId: mockOracleProgram.programId,
      }),
    ],
  });
  return collateralTokenFeed.publicKey;
};

export const setFeedPriceSwitchboardV2 = async (
  mockOracleProgram: Program,
  mantissa: BN,
  scale: BN,
  priceFeed: web3.PublicKey
) => {
  const info = await mockOracleProgram.provider.connection.getAccountInfo(priceFeed);
  await mockOracleProgram.rpc.setPriceSwitchboardV2(mantissa, scale, {
    accounts: { oracleAccount: priceFeed, clock: SYSVAR_CLOCK_PUBKEY },
  });
};

export class Sb2MockToken implements ITokenEntry {
  price: Decimal;
  ticker: string;
  decimals: number;
  account: PublicKey;
  program: Program<any>;

  constructor(mockOracleProgram: Program, price: Decimal, ticker: string, decimals: number, account: PublicKey) {
    this.price = price;
    this.ticker = ticker;
    this.decimals = decimals;
    this.account = account;
    this.program = mockOracleProgram;
  }

  getType(): OracleType {
    return OracleType.SwitchboardV2;
  }

  async updatePrice(price: Decimal, decimals?: number): Promise<void> {
    let scale = decimals ?? this.decimals;
    const mantissa = new BN(price.mul(new Decimal(10).pow(new Decimal(scale))).toString());
    await setFeedPriceSwitchboardV2(this.program, mantissa, new BN(scale), this.account);
    this.price = price;
    this.decimals = scale;
  }
}

export class Sb2MockOracle implements IMockOracle {
  async createFakePriceAccount(
    mockOracleProgram: Program,
    ticker: string,
    initPrice: Decimal,
    decimals: number
  ): Promise<ITokenEntry> {
    const mantissa = new BN(initPrice.mul(new Decimal(10).pow(new Decimal(decimals))).toString());
    const account = await createPriceFeedSwitchboardV2(mockOracleProgram, mantissa, new BN(decimals));
    return new Sb2MockToken(mockOracleProgram, initPrice, ticker, decimals, account);
  }
}
