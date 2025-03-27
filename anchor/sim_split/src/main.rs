use alloy::primitives::address;
use alloy::primitives::Address;
use base64::prelude::*;
use database::NetworkDatabase;
use database::{SqlStatement, SQL};
use rusqlite::types::Value;
use eth::parse_shares;
use types::Graffiti;
use keysplit::{
    run_keysplitter, KeygenSubcommands, Keysplit, Manual, OperatorIds, SharedKeygenOptions,
};
use openssl::pkey::Public;
use openssl::rsa::Rsa;
use rusqlite::params;
use ssv_types::parse_rsa;
use ssv_types::{ClusterId, OperatorId};
use std::fs;
use std::path::Path;
use std::str::FromStr;
use types::PublicKeyBytes;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Keystore {
    version: String,
    createdAt: String,
    shares: Vec<Share>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Share {
    data: ShareData,
    payload: Payload,
}

#[derive(Debug, Serialize, Deserialize)]
struct ShareData {
    ownerNonce: u64,
    ownerAddress: String,
    publicKey: String,
    operators: Vec<Operator>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Operator {
    id: u32,
    operatorKey: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Payload {
    publicKey: String,
    operatorIds: Vec<u32>,
    sharesData: String,
}

fn main() {
    for idx in 11..1000 {
        let file = format!("keystores/validator_keystore-{}.json", idx);
        let output = format!("keyshares/validator_split-{}.json", idx);

        let options = SharedKeygenOptions {
            keystore_path: file,
            password: "222222222222222222222222222222222222222222222222222".to_string(),
            owner: Address::ZERO,
            output_path: output,
            operators: OperatorIds(vec![1, 2, 3, 4]),
        };

        let op1 = parse_rsa(OPERATOR1).unwrap();
        let op2 = parse_rsa(OPERATOR2).unwrap();
        let op3 = parse_rsa(OPERATOR3).unwrap();
        let op4 = parse_rsa(OPERATOR4).unwrap();

        let manual = Manual {
            shared: options,
            nonce: 0,
            public_keys: vec![op1, op2, op3, op4],
        };

        let cmd = KeygenSubcommands::Manual(manual);
        let keysplit = Keysplit { subcommand: cmd };

        run_keysplitter(keysplit).unwrap()
    }

    let op1 = parse_rsa(OPERATOR1).unwrap();
    let db_path = Path::new("anchor_db.sqlite");

    let db = NetworkDatabase::new(db_path, &op1).unwrap();

    for idx in 10..1000 {
        let file = format!("keyshares/validator_split-{}.json", idx);
        let file_content = fs::read_to_string(file).unwrap();

        let keystore: Keystore = serde_json::from_str(&file_content).unwrap();
        let share = keystore.shares.first().unwrap();

        let public_key = share.payload.publicKey.clone();
        let shares_data = &share.payload.sharesData[2..];

        let operator_ids = vec![OperatorId(1), OperatorId(2), OperatorId(3), OperatorId(4)];
        let public_key = PublicKeyBytes::from_str(&public_key).unwrap();
        let shares_data = hex::decode(shares_data).unwrap();
        let cluster_id: [u8; 32] = [
            0x7a, 0x94, 0xd8, 0x7e, 0x47, 0x99, 0x48, 0x66, 0x8a, 0x5b, 0x7b, 0x8e, 0x5d, 0x88,
            0x4a, 0x2e, 0x84, 0x61, 0x06, 0x97, 0x41, 0x90, 0xdd, 0xbf, 0x65, 0xc3, 0x5a, 0x04,
            0xb5, 0x3c, 0xe3, 0x0e,
        ];
        let graff = Graffiti::default();

        let (_, shares) = parse_shares(
            shares_data,
            &operator_ids,
            &ClusterId(cluster_id),
            &public_key,
        )
        .unwrap();

        let mut conn = db.connection().unwrap();
        let tx = conn.transaction().unwrap();
        let index: usize = idx + 1;

        tx.prepare_cached(SQL[&SqlStatement::InsertValidator])
            .unwrap()
            .execute(params![
                public_key.to_string(), // validator public key
                cluster_id,             // cluster id
                index,
                Value::Blob(graff.0.to_vec())
            ])
            .unwrap();

        for share in shares {
            db.insert_share(&tx, &share, &public_key).unwrap()
        }
        tx.commit().unwrap();
        println!("finished: {:?}", idx);
    }
}

const OPERATOR1: &str = "LS0tLS1CRUdJTiBQVUJMSUMgS0VZLS0tLS0KTUlJQklqQU5CZ2txaGtpRzl3MEJBUUVGQUFPQ0FROEFNSUlCQ2dLQ0FRRUFoVUZtNUhjRmN4WXZKSGlGUzZRTgpGUUZmNHhXaHZ2M3YrcDE0cmdOOXJJQXdiNG5FbkZzcWNIOGh6Q0FFRnFPVjhwellSUFF1RTRDQkF6eVVJM1FICnV1M1ZUOUI1VTNOS1phZ1hpTlFLamx1Y0xicTFJNXZRRnozd3Q1Z25zeHFPajUrNnZreXdUS2s1SWJyb01USE4KU3hzT3QrOTZHN3UwZ1ltbUFZZnA0RjlCR042UTBRTEdrUml3Z3NvQkxsY1NvSTR3SkNicEUyazJpTXU5bkRENApEaUozd3lhTElVbk9oMHNsMmhHR1hndFVXTCttcTB2Wk8yYk5NNC80ZGxDbGl4OEZyMUVCaTAzRnh6cUNSNzV0CmJSMW8rTExSVzlsSVBMT1FrWi8xYmEyZGJVMmIrNWtaYlc1MFcwR01TbG8yU2hFcEhrV3YxZ3NrTHE0eDdqVU4KaVFJREFRQUIKLS0tLS1FTkQgUFVCTElDIEtFWS0tLS0tCg==";

const OPERATOR2: &str = "LS0tLS1CRUdJTiBQVUJMSUMgS0VZLS0tLS0KTUlJQklqQU5CZ2txaGtpRzl3MEJBUUVGQUFPQ0FROEFNSUlCQ2dLQ0FRRUF5WnJUSE44aEoyWmM2eEhYbnFWawp4ZXMxVkVWVUw3b0NKZTRXelJvR3A2SGJ5bW8rbkc0RFF4TFpaSHhXVVVwcFhtYU1OVEJwdjBYcEt6MDFFSE8yCjBvQ2ZNMHB6VU1EUHEwZ2ZBUnNNSjRCbkUvNksybFVUZFJieloxaVQ3QTJqSDhNWFBZdFRJMldjNUpPQUkwODIKcktVOWplWitDNDJENTdYa3c0NUVIUjNXRi9XMTBYdDJwY2NNamZBaVUyVEt5U25qaUpLaC9WYWMyUXJuU2NISQo5azd3NWNaamdtYmRDS3JvYjNKNXplZFpqQXBOeFpoemRNV0VGSStHWklzUGZjNkhWU2wyeG4xZjVDWHRCL1BjCkVJQVBXMFd1Ui8vbDY1TTBmRy9nZVhza2JCdk1mMm5iVjZoK3NORTNYSFcvOGVQemswUUJnOGJWT3V6TXNZbTIKL1FJREFRQUIKLS0tLS1FTkQgUFVCTElDIEtFWS0tLS0tCg==";

const OPERATOR3: &str = "LS0tLS1CRUdJTiBQVUJMSUMgS0VZLS0tLS0KTUlJQklqQU5CZ2txaGtpRzl3MEJBUUVGQUFPQ0FROEFNSUlCQ2dLQ0FRRUFpZURBS2pqb080MGlmakhhTytlSgpLdjhDVnJOUGFnTitGS09QWDMvUndEWXhtTXc4dmpkbHZsaXJlUzV6V1B5RlY3RmVZTDA5SEpmaDArS0lmMThhClA3bEVGZjdUbW1UMXJPUnpKK3BocVhrc1Q0NkxoaytrK09WcFhKbXUrWTkyS09OV3lvdG5GMVhvb0FQelFuamkKV2Q5aVJLVFBpSEx2VkV5TXlEWUxzclVZeE1ZQjFOYWRvcE9OVFlXemNWMTNsRVJseVQzeUM5Q3hWVlVWRm9EOQpTU1JxTXo1eEgrOTY5eUFxQzVydVNtdGZ1WjVPRG1CMDF6UnZNYXQ5bjQ5VzBpc0NRa09yZ3dKbzBUbzdVNWh5CkkwUHpIODdESUFYQjNUNUFicGVFaVY3TUNPYmVyYmFVRjBCK2laeU52T0NhaHU5TFBvMDlzckFSM0NlWU1lRkYKS3dJREFRQUIKLS0tLS1FTkQgUFVCTElDIEtFWS0tLS0tCg==";

const OPERATOR4: &str = "LS0tLS1CRUdJTiBQVUJMSUMgS0VZLS0tLS0KTUlJQklqQU5CZ2txaGtpRzl3MEJBUUVGQUFPQ0FROEFNSUlCQ2dLQ0FRRUF6NkhDQ1FpVTZUMXZ3RkYvWFVWdQo5dGNiZkROYlkvMnBLNG0wa1dETjVyaUdwa3hyZzVRT3hKTmwvSWdPd3BUbHd6VjVCUnpTVXFLZWRYdzZXS1FUCkVyNWxBaWhONFZ0M2kwK1lzeGhpS3NTeGVPTHk4MThVaU5VK21yamxmMmtxbmZaT0MwUDNoV1hVNUg2SHVyWU8KcW9sb2ZITEJ6U3FLYkFKOGRKQXRFL0tyNnlpZUJTSTFZNzcrTDBoT3E0c2RVNHA3aGRuL1A4KzZzekQzT3FtdwpBNWtlUS9hSDg1bmFNRUltbmZsSVF0R3V0KzgrdWxUNHJmcStDOC93VU9FRjFBYVVVWnQvM1dIN2c0MlZpU29FCnY0R1JldFM0TFJBOVBMV0pyK0dueE93MFozaVVVQUNrSkw4WGMxWUtyR0lEdDUzV0FSUHBIMDBPNGw4eHVTRnAKQVFJREFRQUIKLS0tLS1FTkQgUFVCTElDIEtFWS0tLS0tCg==";
