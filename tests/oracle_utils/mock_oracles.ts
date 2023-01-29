import { BN, Program, web3 } from '@project-serum/anchor';
import * as anchor from '@project-serum/anchor';
import { PublicKey } from '@solana/web3.js';
import Decimal from 'decimal.js';

import * as pyth from './pyth';
import * as switchboardV1 from './switchboard_v1';
import * as switchboardV2 from './switchboard_v2';
import * as ctokens from './ctokens';
import * as spl_stake from './spl_stake';

const mockOracleProgram = anchor.workspace.MockOracle;

export enum OracleType {
  Pyth = 0,
  SwitchboardV1 = 1,
  SwitchboardV2 = 2,
  CToken = 4,
  SplStake = 5,
  PythEMA = 6,
}

export interface ITokenInput {
  ticker: string;
  price: Decimal;
  decimals: number;
  priceType: OracleType;
}

export interface IMockOracle {
  createFakePriceAccount(
    mockOracleProgram: Program,
    ticker: string,
    init_price: Decimal,
    decimals: number
  ): Promise<ITokenEntry>;
}

export interface ITokenEntry {
  readonly price: Decimal;
  readonly ticker: string;
  readonly decimals: number;
  readonly account: PublicKey;

  updatePrice(price: Decimal, decimals?: number): Promise<any>;
  getType(): OracleType;
}

export const oracles: Record<OracleType, IMockOracle> = {
  [OracleType.Pyth]: new pyth.PythMockOracle(),
  [OracleType.SwitchboardV1]: new switchboardV1.Sb1MockOracle(),
  [OracleType.SwitchboardV2]: new switchboardV2.Sb2MockOracle(),
  [OracleType.CToken]: new ctokens.CTokenMockOracle(),
  [OracleType.SplStake]: new spl_stake.StakePoolMockOracle(),
  [OracleType.PythEMA]: new pyth.PythMockOracle(),
};

export async function createFakeAccounts(
  mockOracleProgram: Program,
  initialTokens: ITokenInput[]
): Promise<ITokenEntry[]> {
  return await Promise.all(
    initialTokens.map(async (asset): Promise<any> => {
      // console.log(`Adding ${asset.ticker.toString()}`);
      return await oracles[asset.priceType].createFakePriceAccount(
        mockOracleProgram,
        asset.ticker,
        asset.price,
        asset.decimals
      );
    })
  );
}
