mod common;

use scope::Configuration;
use solana_program_test::tokio;
use solana_sdk::signer::Signer;

use crate::common::{
    operations::approve_admin_cached,
    operations::set_admin_cached,
    setup::{new_keypair, setup_scope_feed},
    utils::map_anchor_error,
    utils::AnchorErrorCode,
};
// - [x] Set with wrong admin
// - [x] Approve with wrong admin_cached

#[tokio::test]
async fn test_working_set_and_approve_admin_cached() {
    let (mut ctx, scope_feed_definition) = setup_scope_feed().await;

    let admin_cached = new_keypair(&mut ctx, 100000000).await;

    set_admin_cached(&mut ctx, &scope_feed_definition, &admin_cached.pubkey())
        .await
        .unwrap();

    let config_state = ctx
        .get_anchor_account::<Configuration>(&scope_feed_definition.conf)
        .await
        .unwrap();

    assert_eq!(config_state.admin_cached, admin_cached.pubkey());
    assert_eq!(config_state.admin, ctx.admin.pubkey());

    approve_admin_cached(&mut ctx, &scope_feed_definition, &admin_cached)
        .await
        .unwrap();

    let config_state = ctx
        .get_anchor_account::<Configuration>(&scope_feed_definition.conf)
        .await
        .unwrap();

    assert_eq!(config_state.admin, admin_cached.pubkey());
}

// Set with wrong admin
#[tokio::test]
async fn test_security_set_admin_cached() {
    let (mut ctx, scope_feed_definition) = setup_scope_feed().await;

    let admin_cached = new_keypair(&mut ctx, 100000000).await;
    let admin_cached_pk = admin_cached.pubkey();

    ctx.admin = admin_cached;

    let res = set_admin_cached(&mut ctx, &scope_feed_definition, &admin_cached_pk).await;

    assert_eq!(map_anchor_error(res), AnchorErrorCode::ConstraintHasOne);
}

// Approve with wrong admin_cached
#[tokio::test]
async fn test_security_approve_admin_cached() {
    let (mut ctx, scope_feed_definition) = setup_scope_feed().await;

    let admin_cached = new_keypair(&mut ctx, 100000000).await;

    set_admin_cached(&mut ctx, &scope_feed_definition, &admin_cached.pubkey())
        .await
        .unwrap();

    let wrong_admin = new_keypair(&mut ctx, 100000000).await;

    let res = approve_admin_cached(&mut ctx, &scope_feed_definition, &wrong_admin).await;

    assert_eq!(map_anchor_error(res), AnchorErrorCode::ConstraintHasOne);
}
