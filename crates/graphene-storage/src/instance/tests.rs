use super::*;
use graphene_core::{ErrorCode, InstanceId, OperationId};

#[test]
fn shared_and_exclusive_instance_leases() {
    let temp = tempfile::tempdir().unwrap();
    let paths = InstancePaths::new(temp.path());
    let store = InstanceLeaseStore::new(paths.clone());
    let instance_id = InstanceId::new();

    // Acquire shared lease
    let lease1 = store.acquire_shared(instance_id).expect("shared lease 1");
    assert_eq!(lease1.instance_id(), instance_id);

    // Concurrent shared lease succeeds
    let lease2 = store.acquire_shared(instance_id).expect("shared lease 2");
    assert_eq!(lease2.instance_id(), instance_id);

    // Exclusive lease fails with InstanceBusy while shared lease is held
    let err = store.acquire_exclusive(instance_id).unwrap_err();
    assert_eq!(err.code, ErrorCode::InstanceBusy);

    drop(lease1);
    drop(lease2);

    // Carrier file persists after unlock!
    assert!(paths.lock_carrier_path(instance_id).exists());

    // Acquire exclusive lease
    let ex_lease = store
        .acquire_exclusive(instance_id)
        .expect("exclusive lease");
    assert_eq!(ex_lease.instance_id(), instance_id);

    // Shared and exclusive both fail while exclusive is held
    let err_sh = store.acquire_shared(instance_id).unwrap_err();
    assert_eq!(err_sh.code, ErrorCode::InstanceBusy);

    let err_ex = store.acquire_exclusive(instance_id).unwrap_err();
    assert_eq!(err_ex.code, ErrorCode::InstanceBusy);

    drop(ex_lease);

    // Carrier file persists after unlock!
    assert!(paths.lock_carrier_path(instance_id).exists());
}

#[test]
fn sorted_pair_lease_acquisition_prevents_deadlocks() {
    let temp = tempfile::tempdir().unwrap();
    let paths = InstancePaths::new(temp.path());
    let store = InstanceLeaseStore::new(paths);
    let id_a = InstanceId::new();
    let id_b = InstanceId::new();

    let (lease_b, lease_a) = store.acquire_exclusive_pair(id_b, id_a).unwrap();
    assert_eq!(lease_b.instance_id(), id_b);
    assert_eq!(lease_a.instance_id(), id_a);
}

#[test]
fn document_bounded_read_and_atomic_write() {
    let temp = tempfile::tempdir().unwrap();
    let doc_path = temp.path().join("instance.json");

    write_document_atomic(&doc_path, b"{\"hello\":\"world\"}").unwrap();
    let bytes = read_document_bounded(&doc_path, 1024).unwrap();
    assert_eq!(bytes, b"{\"hello\":\"world\"}");

    // Test exceeding bounds
    let err = read_document_bounded(&doc_path, 5).unwrap_err();
    assert_eq!(err.code, ErrorCode::InstanceInvalid);
}

#[test]
fn staging_create_only_publication_and_quarantine_deletion() {
    let temp = tempfile::tempdir().unwrap();
    let paths = InstancePaths::new(temp.path());
    let id = InstanceId::new();
    let op_id = OperationId::new();

    // Create staging
    let staging = InstanceStagingTree::create(&paths.staging_root(), op_id).unwrap();
    staging
        .write_file("instance.json", b"{\"id\":\"inst-1\"}")
        .unwrap();
    staging
        .write_file(".minecraft/options.txt", b"fov:70")
        .unwrap();

    let dest = paths.instance_root(id);
    staging.publish_create_only(&dest).unwrap();

    assert!(dest.join("instance.json").exists());
    assert!(dest.join(".minecraft/options.txt").exists());

    // Second publish to existing target fails
    let staging2 = InstanceStagingTree::create(&paths.staging_root(), OperationId::new()).unwrap();
    let err = staging2.publish_create_only(&dest).unwrap_err();
    assert_eq!(err.code, ErrorCode::InstallCommitFailed);

    // Delete via quarantine
    let del_op = OperationId::new();
    let trash_dir = paths.trash_dir(id, del_op);
    InstanceTrash::quarantine(&dest, &trash_dir).unwrap();

    assert!(!dest.exists());
    assert!(trash_dir.join("instance.json").exists());

    // Cleanup trash
    InstanceTrash::cleanup_quarantined_tree(&trash_dir).unwrap();
    assert!(!trash_dir.exists());
}
