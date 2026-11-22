use alcomd_application::*;
use alcomd_store::StateStoreHandle;
use serde_json::json;

fn request(method: ExternalMethod, key: &str, fingerprint: &str) -> ExternalRequest {
    ExternalRequest {
        method,
        caller: PrincipalId::local_owner(),
        key: IdempotencyKey::parse(key).expect("key"),
        fingerprint: fingerprint.into(),
        permissions: vec![],
    }
}

#[tokio::test]
async fn external_effects_have_one_claim_and_replay_the_exact_response_after_restart() {
    let root = std::env::temp_dir().join(format!("alcomd-external-{}", OperationId::new()));
    std::fs::create_dir(&root).expect("isolated directory");
    let path = root.join("state.db");
    let store = StateStoreHandle::open(path.clone()).expect("new store");
    let mut completed = Vec::new();
    for method in [
        ExternalMethod::SettingsUpdate,
        ExternalMethod::OpenHub,
        ExternalMethod::ExportRepositories,
    ] {
        let input = request(method, "common-key", "canonical original parameters");
        let first = ExternalAcceptance::new(store.clone(), input.clone());
        let second = ExternalAcceptance::new(store.clone(), input.clone());
        let (a, b) = tokio::join!(first.claim(), second.claim());
        assert_ne!(a, b);
        let (winner, other) = if a {
            (&first, &second)
        } else {
            (&second, &first)
        };
        assert_eq!(other.observed(), Some(Ok(ExternalReceipt::OutcomeUnknown)));
        let receipt = ExternalReceipt::Result(match method {
            ExternalMethod::SettingsUpdate => {
                json!({"revision":7,"settings":{"packages":{"hiddenRepositoryPackages":["漢字".repeat(40_000)]}}})
            }
            ExternalMethod::OpenHub => json!({"outcome":"launch_requested"}),
            ExternalMethod::ExportRepositories => {
                json!({"exportedCount":3,"byteCount":90,"sha256":"a".repeat(64)})
            }
        });
        store
            .external_finish(
                input.clone(),
                *winner
                    .accepted
                    .get()
                    .expect("exactly one claimed operation"),
                receipt.clone(),
                2,
            )
            .await
            .expect("finish");
        let altered = request(method, "common-key", "other parameters");
        assert_eq!(
            store.external_admit(altered, false, 3).await,
            Err(ExternalError::Conflict)
        );
        completed.push((input, receipt));
    }
    let crash = request(ExternalMethod::OpenHub, "interrupted", "same launch");
    let ExternalAdmission::Accepted(id) = store
        .external_admit(crash.clone(), true, 4)
        .await
        .expect("admit before crash")
    else {
        panic!("claim expected")
    };
    drop(store);
    let store = StateStoreHandle::open(path).expect("reopen");
    assert!(
        !StateStore::recover(&store, 5)
            .await
            .expect("recover")
            .contains(&id)
    );
    assert_eq!(
        store.external_admit(crash, true, 6).await,
        Ok(ExternalAdmission::Replayed(ExternalReceipt::OutcomeUnknown))
    );
    for (input, receipt) in completed {
        assert_eq!(
            store.external_admit(input, true, 7).await,
            Ok(ExternalAdmission::Replayed(receipt))
        );
    }
    drop(store);
    std::fs::remove_dir_all(root).expect("remove test directory");
}
