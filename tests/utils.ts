import { Program } from '@project-serum/anchor';
import * as mockAccountUtils from './mock_account_utils';

export enum Tokens {
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
}

export enum PriceType {
  Pyth = 0,
  SwitchboardV1 = 1,
  SwitchboardV2 = 2,
  YiToken = 3,
}

export async function createFakeAccounts(fakeOraclesProgram: Program<any>, initialTokens: any[]) {
  return await Promise.all(
    initialTokens.map(async (asset): Promise<any> => {
      console.log(`Adding ${asset.ticker.toString()}`);

      if (asset.priceType == PriceType.Pyth || asset.priceType == PriceType.YiToken) {
        return await mockAccountUtils.createPriceFeedPyth({
          oracleProgram: fakeOraclesProgram,
          initPrice: asset.price,
          expo: -asset.decimals,
        });
      } else if (asset.priceType == PriceType.SwitchboardV1) {
        return await mockAccountUtils.createPriceFeedSwitchboardV1({
          oracleProgram: fakeOraclesProgram,
          mantissa: asset.mantissa,
          scale: asset.expo,
        });
      } else if (asset.priceType == PriceType.SwitchboardV2) {
        return await mockAccountUtils.createPriceFeedSwitchboardV2({
          oracleProgram: fakeOraclesProgram,
          mantissa: asset.mantissa,
          scale: asset.expo,
        });
      }
    })
  );
}
