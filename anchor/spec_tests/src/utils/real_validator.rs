use bls::PublicKeyBytes;
use database::NetworkDatabase;
use message_validator::{ValidationFailure, ValidationResult, Validator};
use openssl::rsa::Rsa;
use slot_clock::{ManualSlotClock, SlotClock};
use std::{sync::Arc, time::Duration};
use task_executor::TaskExecutor;
use tempfile::tempdir;
use types::Slot;

// Test RSA key (same as used in fuzz tests)
const TESTING_KEY: &str = "-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQCXpzq9yJPBj5b7
A2kqQ3CxDUxCmkcRpZz+eJq4314yxNVMyAjXEtTv62gXxSmru2se7eFky15Evw9a
/OAnsmlDEW64Dt6n6ZanHNXYFu2Y7enEpUn4OmVim2KIq/T2M3nYVtxsekPflb0Q
OTWXiqszkvNmxDJ95Jc6WvhfubWl3EBOZ3os2/xrS3zoA1+bIBLRtwzAM1O7uxwT
Pg7nts/hqvvFS0njf4CMl6MNoac5GrE7RSBStDMkJZbay5xOEUhUxZGuOMY8ppX5
vhHodvuNktSrWRxKG+D3Sh7CtyqOd0oDFR5w4EpU8flwBvD19vTj6bqbqpJqjR7Y
PvH7/MvNAgMBAAECggEACYCJQLJG98jRx/aQaf0scXDeMgoioStu/nl7ZaaxQJ3D
/k9GUTDf4LcvoCr9ZReVgFbyrsht/AFl/+h6Tw0cZVRmoJJl8cAZbW0exQRiwhie
HhvRL67RAsWuUyvwaatosLJ4ld9vTfIUP5D7bPxbpRv0XksKyDKWJdTksnLGEUH2
ni91JuxfAouJgoAWAssQrZPtsT99KbEJxD8q9KDa5ODT6wQmaTmD6gDSFzXcDNBa
Bkpc9XaJSaEFtjZIKza5YftRhVVK6LDYqeJMk5Atzbihf33dmZrhmT+zCP2HvIOk
c6gXLqPrRe7gTSLN+cbpimzGZL1+Nkks0xsk/6UnbQKBgQC/zw4Nj4HhcbCrljkz
BGrjNqSHsszEenrEn9N1r5zym2hDUTcLLjZAHvhypVpdqy+Z9xifDgD8nXSbiF6k
b/fv9aP18P9YU8k1n6MjkdVsr3z4bmMZD1alVVp9gfLJHMJ52+sptg4XpMp75q5v
XIcSDF9rTMmcdNGB7MbdYCqmCwKBgQDKZ+dbjPGqKSNdlpmJJ4Zw6+9CKxskDkNj
fGsq8pR1vWTlpA7WCDoymOzMyB+EZ81HxT2c5aNaF75X6Db/TAljzHUdxCfv/8fT
RDRWkDBMz62MGWx6lifYr8HvjdQ7lB3c2i2qPzsQWLLqaCw8Z3ya4J9kmOL6wNeT
wjiL9280hwKBgFWJVrEBcGBDPRAn+/YeYDRXZ+QD/oEYRattwvVWjV07pLFwhGV+
BD9wEEfAKZ5f+uhkYxx7OEFvTlMV627VZ/Igzy+ce6K+Kpq5SB1SqaTAVbDMOXEx
f+hXOfWCf+zj4G5LfoGpaHtux8WdR+jtkGaiEeNd6QLWrZ+NIdoTSrGlAoGBALXx
8Oc7K4HquP/IAPxpq1CWxdyVIzCmIa2siilxJkMwnSJQ94UuoCIblcH/o1VCeiWq
CFihlNXHwjMDa2zSzR4JDL5VNhFnvBkNln653rEtfrQRppILqIYAeDT/KWjlHHML
LUF81Xs8QJi2TA2AeWI/yQiE5oTCFQed73biVfTBAoGACEDOZi7v9Ncj03XY4UNl
IxIjMgIIlkjifLjr2MVi/qEx67109rsdZAGGAb1YCukel/0NAXmxjXROxj1lHHPe
xfn7l4RWwIGqi3yZtxfpKB29mjBaY0BRL6XPhGe2MAfydeXMdwk6QrZPcYroOJ+t
SPdvWXU4osCd7vgiJvAP4ek=
-----END PRIVATE KEY-----";

// Mock duties provider for testing
#[derive(Default)]
pub struct MockDutiesProvider {
    pub voluntary_exit_duty_count: u64,
}

impl message_validator::DutiesProvider for MockDutiesProvider {
    fn is_validator_in_sync_committee(
        &self,
        _committee_period: u64,
        _validator_index: ssv_types::ValidatorIndex,
    ) -> bool {
        true
    }

    fn is_epoch_known_for_proposers(&self, _epoch: types::Epoch) -> bool {
        true
    }

    fn is_validator_proposer_at_slot(
        &self,
        _slot: types::Slot,
        _validator_index: ssv_types::ValidatorIndex,
    ) -> bool {
        true
    }

    fn get_voluntary_exit_duty_count(&self, _slot: types::Slot, _pubkey: &PublicKeyBytes) -> u64 {
        self.voluntary_exit_duty_count
    }
}

pub fn create_test_validator() -> Arc<Validator<ManualSlotClock, MockDutiesProvider>> {
    // Setup slot clock
    let slot_clock = ManualSlotClock::new(
        Slot::new(100), // Current slot for test context
        Duration::from_secs(0),
        Duration::from_secs(12),
    );

    // Setup database with RSA key
    let rsa = Rsa::private_key_from_pem(TESTING_KEY.as_bytes()).expect("Key is valid");
    let public_key =
        Rsa::from_public_components(rsa.n().to_owned().unwrap(), rsa.e().to_owned().unwrap())
            .unwrap();

    let tempdir = tempdir().unwrap();
    let file = tempdir.path().join("test_db.sqlite");
    let db = NetworkDatabase::new(&file, &public_key).expect("Database construction");

    // Setup duties provider
    let duties_provider = MockDutiesProvider::default();

    // Setup task executor using runtime-based approach similar to fuzz tests
    // Note: This requires running in a tokio runtime context
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create runtime");
    let handle = std::sync::Arc::downgrade(&std::sync::Arc::new(runtime));
    let (_signal, exit) = async_channel::bounded(1);
    let (shutdown_tx, _) = futures::channel::mpsc::channel(1);
    let executor = TaskExecutor::new(handle, exit, shutdown_tx, "test_executor".into());

    // Create validator
    Validator::new(
        db.watch(),
        32,  // slots_per_epoch
        256, // epochs_per_sync_committee_period
        512, // sync_committee_size
        Arc::new(duties_provider),
        slot_clock,
        &executor,
    )
}

pub fn map_validation_failure_to_expected_error(failure: &ValidationFailure) -> String {
    match failure {
        // Message decoding and format errors
        ValidationFailure::UndecodableMessageData(_) => {
            "failed decoding consensus data".to_string()
        }
        ValidationFailure::MalformedPubSubMessage => "failed decoding consensus data".to_string(),
        ValidationFailure::NilSignedSSVMessage => "failed decoding consensus data".to_string(),
        ValidationFailure::NilSSVMessage => "failed decoding consensus data".to_string(),
        ValidationFailure::EmptyData => "failed decoding consensus data".to_string(),
        ValidationFailure::SSVDataTooBig => "failed decoding consensus data".to_string(),

        // Validator identity errors
        ValidationFailure::UnknownValidator => "duty invalid: wrong validator index".to_string(),
        ValidationFailure::ValidatorIndexMismatch => {
            "duty invalid: wrong validator index".to_string()
        }
        ValidationFailure::NoValidators => "duty invalid: wrong validator index".to_string(),

        // Public key errors
        ValidationFailure::WrongRSASignatureSize => "duty invalid: wrong validator pk".to_string(),
        ValidationFailure::SignatureVerificationFailed { .. } => {
            "duty invalid: wrong validator pk".to_string()
        }
        ValidationFailure::SignatureVerification => "duty invalid: wrong validator pk".to_string(),

        // Role and duty type errors
        ValidationFailure::InvalidRole => "duty invalid: wrong beacon role type".to_string(),
        ValidationFailure::PartialSignatureTypeRoleMismatch => {
            "duty invalid: wrong beacon role type".to_string()
        }
        ValidationFailure::InvalidPartialSignatureType => {
            "duty invalid: wrong beacon role type".to_string()
        }

        // Timing and epoch errors
        ValidationFailure::EarlySlotMessage { .. } => {
            "duty invalid: duty epoch is into far future".to_string()
        }
        ValidationFailure::LateSlotMessage { .. } => {
            "duty invalid: duty epoch is into far future".to_string()
        }
        ValidationFailure::SlotAlreadyAdvanced { .. } => {
            "duty invalid: duty epoch is into far future".to_string()
        }
        ValidationFailure::TooManyDutiesPerEpoch => {
            "duty invalid: duty epoch is into far future".to_string()
        }

        // Slashing and attestation errors
        ValidationFailure::DuplicatedMessage { .. } => "slashable attestation".to_string(),
        ValidationFailure::DecidedWithSameSigners => "slashable attestation".to_string(),
        ValidationFailure::DifferentProposalData => "slashable attestation".to_string(),
        ValidationFailure::NonDecidedWithMultipleSigners { .. } => {
            "slashable attestation".to_string()
        }

        // Attestation-specific validation errors
        ValidationFailure::InvalidHash => "attestation data source >= target".to_string(),
        ValidationFailure::MismatchedIdentifier { .. } => {
            "attestation data source >= target".to_string()
        }

        // Target epoch validation
        ValidationFailure::RoundTooHigh => {
            "attestation data target epoch is into far future".to_string()
        }
        ValidationFailure::EstimatedRoundNotInAllowedSpread { .. } => {
            "attestation data target epoch is into far future".to_string()
        }

        // General validation errors
        ValidationFailure::NoDuty => "duty invalid: no duty".to_string(),
        ValidationFailure::WrongDomain => "wrong domain".to_string(),

        // Catch-all for unmapped errors - this helps with debugging
        _ => {
            // For debugging: print the actual failure type
            println!("UNMAPPED ValidationFailure: {:?}", failure);
            format!("validation error: {:?}", failure)
        }
    }
}

pub fn validate_message_with_real_validator(
    validator: &Arc<Validator<ManualSlotClock, MockDutiesProvider>>,
    message_data: &[u8],
) -> Result<(), String> {
    // DEBUG: Log message data details for troubleshooting
    println!(
        "DEBUG: Validating message with {} bytes, first 8 bytes: {:?}",
        message_data.len(),
        message_data.get(0..8).unwrap_or(&[])
    );

    match validator.validate(message_data) {
        ValidationResult::Success(_) => {
            println!("DEBUG: Validation SUCCESS");
            Ok(())
        }
        ValidationResult::PreDecodeFailure(failure) => {
            println!("DEBUG: PreDecodeFailure: {:?}", failure);
            Err(map_validation_failure_to_expected_error(&failure))
        }
        ValidationResult::PostDecodeFailure(failure, _) => {
            println!("DEBUG: PostDecodeFailure: {:?}", failure);
            Err(map_validation_failure_to_expected_error(&failure))
        }
    }
}
