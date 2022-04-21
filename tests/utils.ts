import { expect } from 'chai';
import * as chai from 'chai';
import Decimal from 'decimal.js';
import chaiDecimalJs from 'chai-decimaljs';
import { ITokenEntry, ITokenInput, OracleType } from './oracle_utils/mock_oracles';

chai.use(chaiDecimalJs(Decimal));

export enum HubbleTokens {
  SOL = 0,
  ETH,
  BTC,
  SRM,
  RAY,
  FTT,
  MSOL,
  UST,
  BNB,
  AVAX,
  STSOLUST,
  SABERMSOLSOL,
  USDHUSD,
  STSOLUSD,
  CSOL,
  CETH,
  CBTC,
  CMSOL,
}

export const initialTokens: ITokenInput[] = [
  {
    price: new Decimal('228.41550900'),
    ticker: 'SOL',
    decimals: 8,
    priceType: OracleType.Pyth,
  },
  {
    price: new Decimal('4726.59830000'),
    ticker: 'ETH',
    decimals: 8,
    priceType: OracleType.Pyth,
  },
  {
    price: new Decimal('64622.36900000'),
    ticker: 'BTC',
    decimals: 8,
    priceType: OracleType.Pyth,
  },
  {
    price: new Decimal('7.06975570'),
    ticker: 'SRM',
    decimals: 8,
    priceType: OracleType.Pyth,
  },
  {
    price: new Decimal('11.10038050'),
    ticker: 'RAY',
    decimals: 8,
    priceType: OracleType.Pyth,
  },
  {
    price: new Decimal('59.17104600'),
    ticker: 'FTT',
    decimals: 8,
    priceType: OracleType.Pyth,
  },
  {
    price: new Decimal('253.41550900'),
    ticker: 'MSOL',
    decimals: 8,
    priceType: OracleType.Pyth,
  },
  {
    price: new Decimal('228.415509'),
    ticker: 'UST',
    decimals: 8,
    priceType: OracleType.Pyth,
  },
  {
    price: new Decimal('11.10038050'),
    ticker: 'BNB',
    decimals: 8,
    priceType: OracleType.Pyth,
  },
  {
    price: new Decimal('59.17104600'),
    ticker: 'AVAX',
    decimals: 8,
    priceType: OracleType.Pyth,
  },
  {
    price: new Decimal('0.90987600'),
    ticker: 'STSOLUST',
    decimals: 8,
    priceType: OracleType.YiToken,
  },
  {
    price: new Decimal('343.92109348'),
    ticker: 'SABERMSOLSOL',
    decimals: 8,
    priceType: OracleType.SwitchboardV1,
  },
  {
    price: new Decimal('999.20334456'),
    ticker: 'USDHUSD',
    decimals: 8,
    priceType: OracleType.SwitchboardV1,
  },
  {
    ticker: 'STSOLUSD',
    price: new Decimal('474.00324002'),
    decimals: 8,
    priceType: OracleType.SwitchboardV2,
  },
  {
    ticker: 'cSOL',
    price: new Decimal('1.5'),
    decimals: 15,
    priceType: OracleType.CToken,
  },
  {
    ticker: 'cETH',
    price: new Decimal('1.2'),
    decimals: 15,
    priceType: OracleType.CToken,
  },
  {
    ticker: 'cBTC',
    price: new Decimal('0.5'),
    decimals: 15,
    priceType: OracleType.CToken,
  },
  {
    ticker: 'cMSOL',
    price: new Decimal('1.1234568'),
    decimals: 15,
    priceType: OracleType.CToken,
  },
];

export function getScopePriceDecimal(token: number, oraclePrices: any) {
  let price = oraclePrices.prices[token].price;
  let value = price.value.toNumber();
  let expo = price.exp.toNumber();
  return new Decimal(value).mul(new Decimal(10).pow(new Decimal(-expo)));
}

export function checkOraclePrice(token: number, oraclePrices: any, testTokens: ITokenEntry[]) {
  //console.log(`Check ${testTokens[token].ticker} price`);

  // Just ignore Yi token as it's not properly mocked
  if (testTokens[token].getType() == OracleType.YiToken) {
    return;
  }

  let price = oraclePrices.prices[token].price;
  let value = price.value.toNumber();
  let expo = price.exp.toNumber();
  let in_decimal = new Decimal(value).mul(new Decimal(10).pow(new Decimal(-expo)));
  expect(in_decimal).decimal.eq(testTokens[token].price);
}
