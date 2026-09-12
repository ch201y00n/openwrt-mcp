//! Host microbenchmark; no router calls. Run with --release.
use async_trait::async_trait;
use openwrt_mcp_adapters::{AuditConfig, AuditWriter};
use openwrt_mcp_core::{Access, Category, Grant, Permission, Policy, PreparedAction, Requirement};
use openwrt_mcp_runtime::{Backend, Dispatcher, Limits, RuntimeError};
use serde_json::{Value, json};
use std::{hint::black_box, sync::Arc, time::Instant};

struct Fixture;
#[async_trait]
impl Backend for Fixture {
    async fn execute(&self, _: &PreparedAction, _: &Limits) -> Result<Value, RuntimeError> {
        Ok(json!({"uptime": 100}))
    }
}

fn percentile(samples: &mut [u128], percent: usize) -> u128 {
    samples.sort_unstable();
    samples[(samples.len() - 1) * percent / 100]
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let base = openwrt_mcp_features::catalog(vec![]).unwrap();
    let template = base.get("system_info").unwrap();
    let mut custom = Vec::new();
    for index in 0..1000 {
        let mut op = template.clone();
        op.name = format!("fixture_{index}");
        op.requirements = vec![Requirement {
            category: Category::System,
            permission: Permission::Read,
        }];
        custom.push(op);
    }
    let catalog = openwrt_mcp_features::catalog(custom).unwrap();
    let mut policy = Policy::default();
    policy.categories.insert(
        Category::System,
        Grant {
            access: Access::Read,
            execute: false,
        },
    );
    policy.categories.insert(
        Category::Extensions,
        Grant {
            access: Access::ReadWrite,
            execute: true,
        },
    );
    let count = 10_000;
    let mut samples = Vec::with_capacity(count);
    for _ in 0..count {
        let start = Instant::now();
        let operation = catalog.get(black_box("fixture_999")).unwrap();
        black_box(policy.authorize(black_box(operation))).unwrap();
        samples.push(start.elapsed().as_nanos());
    }
    let policy_p50 = percentile(&mut samples, 50);
    let policy_p95 = percentile(&mut samples, 95);
    let audit = AuditWriter::new(AuditConfig {
        enabled: false,
        ..Default::default()
    })
    .unwrap();
    let dispatcher = Dispatcher::new(
        catalog,
        policy,
        Arc::new(Fixture),
        Arc::new(audit),
        Limits::default(),
    )
    .unwrap();
    samples.clear();
    for _ in 0..count {
        let start = Instant::now();
        black_box(dispatcher.invoke("system_info", json!({})).await).unwrap();
        samples.push(start.elapsed().as_nanos());
    }
    println!(
        "{}",
        json!({"kind":"host_fixture_microbenchmark", "catalog_operations":1005, "iterations":count, "input":"empty object", "backend":"in-memory fixture", "audit":"disabled", "policy_lookup_p50_ns":policy_p50, "policy_lookup_p95_ns":policy_p95, "dispatcher_p50_ns":percentile(&mut samples,50), "dispatcher_p95_ns":percentile(&mut samples,95)})
    );
}
