use chrono::{Duration, Utc};
use config::mysql::MysqlConfig;
use nodemanage::{
    AgentRegistration, MySqlNodeRepository, Node, NodeManager, NodeRepository, NodeStatus,
    NoopRsAgentInstaller, PaginationParams,
};
use std::env;
use uuid::Uuid;

fn test_mysql_config() -> MysqlConfig {
    // Test precondition:
    // - These integration tests target the shared MySQL instance already used by the deployed stack.
    // - They are allowed to create/drop nodemanage-prefixed tables only.
    MysqlConfig {
        host: env::var("MYSQL_HOST")
            .unwrap_or_else(|_| "test-mysql.bkbase-test.svc.cluster.local".to_string()),
        port: env::var("MYSQL_PORT")
            .unwrap_or_else(|_| "3306".to_string())
            .parse()
            .unwrap_or(3306),
        user: env::var("MYSQL_USER").unwrap_or_else(|_| "root".to_string()),
        password: env::var("MYSQL_PASSWORD").unwrap_or_else(|_| "testpass".to_string()),
        database: env::var("MYSQL_DATABASE").unwrap_or_else(|_| "prompt".to_string()),
        max_connections: 10,
        min_connections: 1,
        connect_timeout_secs: 10,
    }
}

async fn test_repository() -> MySqlNodeRepository {
    let table_prefix = format!("node_test_{}_", Uuid::new_v4().simple());
    let repository = MySqlNodeRepository::new(test_mysql_config(), table_prefix)
        .await
        .expect("mysql repository should initialize");

    repository
        .reset_for_tests()
        .await
        .expect("mysql test tables should reset");

    repository
}

#[tokio::test]
#[ignore = "requires reachable MySQL test environment; run with --ignored and MYSQL_* overrides if needed"]
async fn mysql_repository_can_create_fetch_update_list_and_delete_nodes() {
    let repository = test_repository().await;

    let node = Node::new(
        "mysql-worker-1".to_string(),
        "http://mysql-worker-1:8080".to_string(),
        vec!["gpu".to_string(), "batch".to_string()],
    );

    let created = repository.create(node).await.unwrap();
    let fetched = repository.get(&created.id).await.unwrap().unwrap();

    assert_eq!(fetched.id, created.id);
    assert_eq!(fetched.labels, vec!["gpu".to_string(), "batch".to_string()]);

    let mut updated = fetched.clone();
    updated.name = "mysql-worker-1-renamed".to_string();
    updated.status = NodeStatus::Maintenance;
    updated.labels = vec!["maintenance".to_string()];
    updated.updated_at = Utc::now();

    let persisted = repository.update(updated.clone()).await.unwrap();
    assert_eq!(persisted.name, "mysql-worker-1-renamed");
    assert_eq!(persisted.status, NodeStatus::Maintenance);
    assert_eq!(persisted.labels, vec!["maintenance".to_string()]);

    let listed = repository.list(PaginationParams::new(1, 10)).await.unwrap();
    assert_eq!(listed.total, 1);
    assert_eq!(listed.items.len(), 1);
    assert_eq!(listed.items[0].id, created.id);

    let deleted = repository.delete(&created.id).await.unwrap();
    assert!(deleted);
    assert!(repository.get(&created.id).await.unwrap().is_none());
}

#[tokio::test]
#[ignore = "requires reachable MySQL test environment; run with --ignored and MYSQL_* overrides if needed"]
async fn mysql_repository_uses_only_prefixed_tables() {
    let repository = test_repository().await;

    let tables = repository
        .list_tables_for_tests()
        .await
        .expect("mysql tables should be queryable");

    assert!(tables.iter().all(|name| name.starts_with("node_test_")));
    assert!(tables.iter().any(|name| name.ends_with("_nodes")));
}

#[tokio::test]
#[ignore = "requires reachable MySQL test environment; run with --ignored and MYSQL_* overrides if needed"]
async fn mysql_repository_lists_newest_nodes_first() {
    let repository = test_repository().await;

    let mut first = Node::new("first".to_string(), "http://first:8080".to_string(), vec![]);
    first.created_at = Utc::now() - Duration::seconds(2);
    first.updated_at = first.created_at;
    let first = repository.create(first).await.unwrap();

    let mut second = Node::new(
        "second".to_string(),
        "http://second:8080".to_string(),
        vec![],
    );
    second.created_at = Utc::now();
    second.updated_at = second.created_at;
    let second = repository.create(second).await.unwrap();

    let listed = repository.list(PaginationParams::new(1, 10)).await.unwrap();
    assert_eq!(listed.total, 2);
    assert_eq!(listed.items[0].id, second.id);
    assert_eq!(listed.items[1].id, first.id);
}

#[tokio::test]
#[ignore = "requires reachable MySQL test environment; run with --ignored and MYSQL_* overrides if needed"]
async fn mysql_repository_supports_idempotent_agent_registration_flow() {
    let repository = test_repository().await;
    let manager = NodeManager::new(repository.clone(), NoopRsAgentInstaller);

    let first = manager
        .register_agent(AgentRegistration {
            agent_id: "agent-mysql-1".to_string(),
            hostname: "worker-a".to_string(),
            endpoint: "http://worker-a:19090".to_string(),
            labels: vec!["edge".to_string()],
        })
        .await
        .unwrap();

    let second = manager
        .register_agent(AgentRegistration {
            agent_id: "agent-mysql-1".to_string(),
            hostname: "worker-a".to_string(),
            endpoint: "http://worker-a:29090".to_string(),
            labels: vec!["edge".to_string(), "gpu".to_string()],
        })
        .await
        .unwrap();

    let listed = repository.list(PaginationParams::new(1, 10)).await.unwrap();

    assert_eq!(first.id, second.id);
    assert_eq!(listed.total, 1);
    assert_eq!(listed.items[0].endpoint, "http://worker-a:29090");
    assert_eq!(
        listed.items[0].labels,
        vec!["edge".to_string(), "gpu".to_string()]
    );
}
