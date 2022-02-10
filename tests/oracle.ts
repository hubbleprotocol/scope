import { Keypair, PublicKey, SystemProgram, SYSVAR_CLOCK_PUBKEY } from '@solana/web3.js';
import { strictEqual } from 'assert';
import * as fs from "fs";
import { Provider, Program, setProvider, workspace, BN } from "@project-serum/anchor"

describe("oracle", () => {
    const admin = Keypair.fromSecretKey(Uint8Array.from([
        241, 101, 13, 165, 53, 150, 114, 216, 162, 246, 157, 94, 156, 209, 145, 37,
        186, 13, 219, 120, 66, 196, 128, 253, 177, 46, 0, 70, 68, 211, 238, 83, 155,
        17, 157, 105, 115, 161, 0, 60, 146, 250, 19, 171, 63, 222, 211, 135, 37, 102,
        222, 216, 142, 131, 67, 196, 185, 182, 202, 219, 55, 24, 135, 90
    ]));

    const idl = JSON.parse(fs.readFileSync("./target/idl/oracle.json", "utf8"));
    const programId = new PublicKey('6jnS9rvUGxu4TpwwuCeF12Ar9Cqk2vKbufqc6Hnharnz');
    const provider = Provider.local()
    setProvider(provider);
    const program = new Program(idl, programId);


    it("Uses the workspace to invoke the initialize instruction", async () => {

        let oracleAccount = Keypair.generate();
        let price = 0;
        console.log("OracleAcc", oracleAccount.secretKey);

        await program.rpc.initialize({
            accounts: {
                admin: admin.publicKey,
                oracle: oracleAccount.publicKey,
                systemProgram: SystemProgram.programId,
            },
            signers: [admin, oracleAccount]
        });

        {
            let oracle = await program.account.oracle.fetch(oracleAccount.publicKey);
            console.log("Oracle", oracle);
        }

        let updatedSolPrice = 20;
        await program.rpc.update(
            new BN(3),      // SRM
            new BN(updatedSolPrice), {
            accounts: {
                admin: admin.publicKey,
                oracle: oracleAccount.publicKey,
                clock: SYSVAR_CLOCK_PUBKEY
            },
            signers: [admin]
        });

        {
            let oracle: any = await program.account.oracle.fetch(
                oracleAccount.publicKey
              );
            console.log("Oracle", oracle);
            strictEqual(oracle.srm.price.toNumber(), updatedSolPrice);
        }
    });
});
