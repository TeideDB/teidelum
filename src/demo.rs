use std::ffi::CString;
use std::fs;
use std::path::Path;

use anyhow::Result;
use rand::seq::SliceRandom;
use rand::Rng;

/// Generate demo data: markdown documents + teide splayed tables.
pub fn generate(data_dir: &Path) -> Result<()> {
    let docs_dir = data_dir.join("docs");
    let tables_dir = data_dir.join("tables");
    fs::create_dir_all(&docs_dir)?;
    fs::create_dir_all(&tables_dir)?;

    generate_documents(&docs_dir)?;
    generate_tables(&tables_dir)?;

    tracing::info!("demo data generated in {}", data_dir.display());
    Ok(())
}

fn generate_documents(dir: &Path) -> Result<()> {
    let docs = vec![
        (
            "auth-redesign-rfc.md",
            "Authentication Redesign RFC",
            "notion",
            r#"# Authentication Redesign RFC

## Summary
We need to migrate from session-based auth to JWT tokens. The current system doesn't scale
well with our microservices architecture and causes issues with cross-service authentication.

## Motivation
- Session store is a single point of failure
- Cross-service auth requires session sharing via Redis
- Mobile clients need stateless authentication
- Token refresh flow is simpler than session renewal

## Proposed Design
1. Issue short-lived JWTs (15 min) with refresh tokens (7 days)
2. Store refresh tokens in a dedicated service
3. Use RSA-256 for token signing
4. Implement token rotation on refresh

## Migration Plan
- Phase 1: Add JWT support alongside sessions (2 weeks)
- Phase 2: Migrate internal services (3 weeks)
- Phase 3: Migrate client apps (2 weeks)
- Phase 4: Remove session support (1 week)

## Open Questions
- Should we use asymmetric or symmetric signing?
- How do we handle token revocation?
- What's the fallback if the auth service is down?
"#,
        ),
        (
            "deployment-runbook-v3.md",
            "Deployment Runbook v3",
            "notion",
            r#"# Deployment Runbook v3

## Pre-deployment Checklist
- [ ] All tests passing on CI
- [ ] Database migrations reviewed
- [ ] Feature flags configured for gradual rollout
- [ ] Monitoring dashboards updated
- [ ] On-call engineer notified

## Deployment Steps
1. Tag release in git: `git tag v3.x.x`
2. Push tag to trigger CI/CD pipeline
3. Monitor canary deployment (10% traffic) for 15 minutes
4. Check error rates, latency p99, and memory usage
5. Promote to 50% traffic
6. Wait 10 minutes, check metrics
7. Promote to 100%

## Rollback Procedure
If error rate exceeds 1% or p99 latency exceeds 500ms:
1. Trigger rollback via ArgoCD
2. Notify #ops channel
3. Create incident ticket
4. Run post-mortem within 48 hours

## Recent Changes
- Added blue-green deployment support
- Integrated with PagerDuty for automatic alerts
- Added database migration dry-run step
"#,
        ),
        (
            "rate-limiting-design.md",
            "Rate Limiting Design",
            "notion",
            r#"# Rate Limiting Design

## Overview
Implement distributed rate limiting across all API endpoints to prevent abuse
and ensure fair usage.

## Algorithm
We'll use a sliding window rate limiter with Redis as the backing store.

### Limits
| Tier      | Requests/min | Burst |
|-----------|-------------|-------|
| Free      | 60          | 10    |
| Pro       | 600         | 50    |
| Enterprise| 6000        | 200   |

## Implementation
- Token bucket algorithm with sliding window
- Redis MULTI/EXEC for atomic operations
- X-RateLimit-Remaining header in responses
- 429 Too Many Requests with Retry-After header

## Edge Cases
- What happens during Redis failover? Default to allowing requests.
- How do we handle distributed clock skew? Use Redis server time.
- WebSocket connections: rate limit on message frequency, not connection count.
"#,
        ),
        (
            "q3-okrs.md",
            "Q3 2026 OKRs",
            "notion",
            r#"# Q3 2026 OKRs

## Objective 1: Improve Platform Reliability
- KR1: Reduce p99 latency from 450ms to 200ms
- KR2: Achieve 99.95% uptime (currently 99.9%)
- KR3: Zero critical incidents lasting > 30 minutes

## Objective 2: Scale Data Infrastructure
- KR1: Migrate analytics pipeline to streaming (Kafka)
- KR2: Reduce data warehouse query time by 50%
- KR3: Implement real-time dashboards for top 5 metrics

## Objective 3: Developer Productivity
- KR1: CI pipeline under 10 minutes (currently 18 min)
- KR2: Deploy 3x per day (currently 1x)
- KR3: Onboarding time for new engineers < 1 week

## Team Allocation
- Platform team: O1 (80%), O3 (20%)
- Data team: O2 (100%)
- Product engineering: O3 (60%), feature work (40%)
"#,
        ),
        (
            "incident-2026-02-15.md",
            "Incident Report: Database Failover",
            "zulip",
            r#"# Incident Report: Database Failover (2026-02-15)

## Timeline
- 14:23 UTC: Primary database CPU spikes to 100%
- 14:25 UTC: Automated failover triggers, replica promoted
- 14:27 UTC: Connection pool exhaustion in auth service
- 14:30 UTC: Manual restart of auth service pods
- 14:32 UTC: All services recovered

## Impact
- 9 minutes of degraded service
- ~2,400 failed API requests
- Auth service was the bottleneck due to connection pool config

## Root Cause
A long-running analytics query held table locks, causing cascading delays.
The connection pool in auth service was configured with max_idle=5 (too low).

## Action Items
- [x] Increase auth service connection pool to max_idle=20
- [x] Add query timeout of 30s for analytics queries
- [ ] Implement query isolation (separate read replicas for analytics)
- [ ] Add connection pool monitoring to dashboards
"#,
        ),
        (
            "api-v2-migration-guide.md",
            "API v2 Migration Guide",
            "notion",
            r#"# API v2 Migration Guide

## Breaking Changes

### Authentication
- Bearer tokens now required (API keys deprecated)
- Token format changed from opaque to JWT
- New endpoint: POST /v2/auth/token

### Pagination
- Offset-based pagination replaced with cursor-based
- Response format: `{ data: [], next_cursor: "..." }`
- Page size parameter renamed from `limit` to `page_size`

### Error Format
```json
{
  "error": {
    "code": "VALIDATION_ERROR",
    "message": "Human readable message",
    "details": [{ "field": "email", "issue": "invalid format" }]
  }
}
```

### Removed Endpoints
- GET /v1/users/search (use GET /v2/users?q=...)
- POST /v1/bulk-create (use batch API instead)

## Migration Steps
1. Update authentication to use bearer tokens
2. Switch to cursor-based pagination
3. Update error handling for new format
4. Replace deprecated endpoints
5. Test with v2 sandbox environment
"#,
        ),
        (
            "zulip-standup-2026-02-20.md",
            "Engineering Standup 2026-02-20",
            "zulip",
            r#"# Engineering Standup — February 20, 2026

## Alice (Backend)
- Yesterday: Finished JWT token rotation implementation
- Today: Writing integration tests for auth service
- Blocker: Need review on PR #847

## Bob (Frontend)
- Yesterday: Updated dashboard components for new API
- Today: Implementing cursor-based pagination in list views
- Blocker: None

## Carol (Data)
- Yesterday: Kafka consumer for analytics events is working
- Today: Setting up schema registry and data validation
- Blocker: Need access to production Kafka cluster

## Dave (Platform)
- Yesterday: Reduced CI pipeline time from 18 to 14 minutes
- Today: Investigating Docker layer caching improvements
- Blocker: None

## Discussion
- Deployment freeze next week for Q3 planning
- New on-call rotation starts Monday
- Carol needs Kafka access — Dave to set up by EOD
"#,
        ),
        (
            "onboarding-checklist.md",
            "New Engineer Onboarding",
            "notion",
            r#"# New Engineer Onboarding Checklist

## Day 1
- [ ] Laptop setup (IT ticket auto-created)
- [ ] GitHub access + 2FA enabled
- [ ] Slack channels: #engineering, #standup, #incidents
- [ ] Read deployment runbook
- [ ] Clone main repositories

## Week 1
- [ ] Pair with buddy on first PR
- [ ] Complete security training
- [ ] Set up local development environment
- [ ] Attend architecture overview session
- [ ] Ship first small change to production

## Week 2
- [ ] Shadow on-call engineer for one day
- [ ] Review recent incident reports
- [ ] Complete API documentation walkthrough
- [ ] Join a design review meeting

## Key Resources
- Architecture docs: /docs/architecture
- API reference: /docs/api
- Runbooks: /docs/runbooks
- Team directory: /people
"#,
        ),
    ];

    for (filename, _title, _source, content) in &docs {
        fs::write(dir.join(filename), content)?;
    }

    tracing::info!("generated {} markdown documents", docs.len());
    Ok(())
}

fn generate_tables(dir: &Path) -> Result<()> {
    let mut rng = rand::thread_rng();
    let sym_path = dir.join("sym");

    // Use a single session so all tables share the same symbol table.
    // The sym file is saved once at the end with all column name symbols.
    let mut session = teide::Session::new()?;

    generate_team_members(&mut session, dir)?;
    generate_project_tasks(&mut session, &mut rng, dir)?;
    generate_incidents(&mut session, &mut rng, dir)?;

    // Save the shared symbol table once, covering all column names
    save_sym(&sym_path)?;

    tracing::info!("generated 3 splayed tables");
    Ok(())
}

fn load_csv_table(session: &mut teide::Session, name: &str, csv_path: &Path) -> Result<()> {
    session.execute(&format!(
        "CREATE TABLE {name} AS SELECT * FROM read_csv('{}')",
        csv_path.display()
    ))?;
    Ok(())
}

fn splay_table(session: &mut teide::Session, name: &str, table_dir: &Path) -> Result<()> {
    fs::create_dir_all(table_dir)?;
    let result = session.execute(&format!("SELECT * FROM {name}"))?;
    if let teide::ExecResult::Query(q) = result {
        save_splayed(&q.table, table_dir)?;
    }
    Ok(())
}

fn generate_team_members(session: &mut teide::Session, dir: &Path) -> Result<()> {
    let csv_path = dir.join("_tmp_team_members.csv");
    let csv_content = "\
id,name,role,department,start_date
1,Alice Chen,Senior Backend Engineer,Engineering,2024-03-15
2,Bob Martinez,Frontend Engineer,Engineering,2024-06-01
3,Carol Wu,Data Engineer,Data,2025-01-10
4,Dave Johnson,Platform Engineer,Platform,2023-11-20
5,Eve Park,Engineering Manager,Engineering,2023-08-05
6,Frank Liu,DevOps Engineer,Platform,2024-09-12
7,Grace Kim,ML Engineer,Data,2025-04-01
8,Hank Wilson,QA Engineer,Engineering,2024-07-18
9,Iris Patel,Security Engineer,Platform,2025-02-01
10,Jack Brown,Product Manager,Product,2024-01-08";

    fs::write(&csv_path, csv_content)?;
    load_csv_table(session, "team_members", &csv_path)?;
    splay_table(session, "team_members", &dir.join("team_members"))?;
    fs::remove_file(&csv_path)?;
    Ok(())
}

fn generate_project_tasks(
    session: &mut teide::Session,
    rng: &mut impl Rng,
    dir: &Path,
) -> Result<()> {
    let assignees = [
        "Alice Chen",
        "Bob Martinez",
        "Carol Wu",
        "Dave Johnson",
        "Eve Park",
        "Frank Liu",
        "Grace Kim",
        "Hank Wilson",
    ];
    let statuses = ["todo", "in_progress", "review", "done"];
    let priorities = ["low", "medium", "high", "critical"];
    let projects = [
        "auth-redesign",
        "api-v2",
        "analytics-pipeline",
        "ci-speedup",
        "rate-limiting",
    ];
    let task_titles = [
        "Implement JWT token rotation",
        "Add cursor-based pagination",
        "Set up Kafka consumer",
        "Optimize Docker layer caching",
        "Write rate limiter middleware",
        "Update API error format",
        "Add connection pool monitoring",
        "Create onboarding automation",
        "Implement token revocation",
        "Add streaming analytics",
        "Fix database failover handling",
        "Update deployment runbook",
        "Add integration tests for auth",
        "Set up schema registry",
        "Implement blue-green deploys",
        "Add API v2 sandbox",
        "Create migration guide",
        "Set up PagerDuty alerts",
        "Reduce CI pipeline time",
        "Add feature flag service",
    ];

    let mut csv = String::from("id,title,assignee,status,priority,project,created_at\n");
    for (i, title) in task_titles.iter().enumerate() {
        let assignee = assignees.choose(rng).unwrap();
        let status = statuses.choose(rng).unwrap();
        let priority = priorities.choose(rng).unwrap();
        let project = projects.choose(rng).unwrap();
        let day = rng.gen_range(1..=28);
        let month = rng.gen_range(1..=2);
        csv.push_str(&format!(
            "{},{},{},{},{},{},2026-{:02}-{:02}\n",
            i + 1,
            title,
            assignee,
            status,
            priority,
            project,
            month,
            day
        ));
    }

    let csv_path = dir.join("_tmp_project_tasks.csv");
    fs::write(&csv_path, &csv)?;
    load_csv_table(session, "project_tasks", &csv_path)?;
    splay_table(session, "project_tasks", &dir.join("project_tasks"))?;
    fs::remove_file(&csv_path)?;
    Ok(())
}

fn generate_incidents(session: &mut teide::Session, rng: &mut impl Rng, dir: &Path) -> Result<()> {
    let severities = ["sev1", "sev2", "sev3", "sev3", "sev3", "sev2"];
    let reporters = [
        "Alice Chen",
        "Dave Johnson",
        "Frank Liu",
        "Eve Park",
        "Iris Patel",
    ];
    let titles = [
        "Database failover during peak traffic",
        "Auth service connection pool exhaustion",
        "CI pipeline stuck for 2 hours",
        "Deployment rollback due to memory leak",
        "Kafka consumer lag exceeded threshold",
        "Rate limiter misconfiguration in prod",
        "SSL certificate expiry warning",
        "Search index corruption after reindex",
    ];

    let mut csv = String::from("id,title,severity,reporter,resolved,duration_min,created_at\n");
    for (i, title) in titles.iter().enumerate() {
        let severity = severities.choose(rng).unwrap();
        let reporter = reporters.choose(rng).unwrap();
        let resolved: bool = rng.gen_bool(0.75);
        let duration: u32 = rng.gen_range(5..120);
        let day = rng.gen_range(1..=28);
        let month = rng.gen_range(1..=2);
        csv.push_str(&format!(
            "{},{},{},{},{},{},2026-{:02}-{:02}\n",
            i + 1,
            title,
            severity,
            reporter,
            resolved,
            duration,
            month,
            day
        ));
    }

    let csv_path = dir.join("_tmp_incidents.csv");
    fs::write(&csv_path, &csv)?;
    load_csv_table(session, "incidents", &csv_path)?;
    splay_table(session, "incidents", &dir.join("incidents"))?;
    fs::remove_file(&csv_path)?;
    Ok(())
}

fn save_splayed(table: &teide::Table, dir: &Path) -> Result<()> {
    let c_dir = CString::new(dir.to_str().unwrap())?;

    let err =
        unsafe { teide::ffi::td_splay_save(table.as_raw(), c_dir.as_ptr(), std::ptr::null()) };

    if err != teide::ffi::td_err_t::TD_OK {
        anyhow::bail!("td_splay_save failed with error code {err:?}");
    }

    Ok(())
}

fn save_sym(sym_path: &Path) -> Result<()> {
    let c_path = CString::new(sym_path.to_str().unwrap())?;

    let err = unsafe { teide::ffi::td_sym_save(c_path.as_ptr()) };

    if err != teide::ffi::td_err_t::TD_OK {
        anyhow::bail!("td_sym_save failed with error code {err:?}");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_demo_data() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path();

        generate(data_dir).unwrap();

        // --- Documents ---
        let docs_dir = data_dir.join("docs");
        assert!(docs_dir.exists(), "docs directory should exist");

        let md_files: Vec<_> = fs::read_dir(&docs_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map_or(false, |ext| ext == "md"))
            .collect();
        assert_eq!(md_files.len(), 8, "should generate 8 markdown documents");

        // Verify a known document has expected content
        let rfc = fs::read_to_string(docs_dir.join("auth-redesign-rfc.md")).unwrap();
        assert!(rfc.contains("Authentication Redesign RFC"));
        assert!(rfc.contains("JWT"));

        // --- Splayed tables ---
        let tables_dir = data_dir.join("tables");
        assert!(tables_dir.exists(), "tables directory should exist");

        // Sym file must exist for cross-session column name resolution
        assert!(tables_dir.join("sym").exists(), "sym file should exist");

        // Each table directory must have a .d schema file
        for table_name in &["team_members", "project_tasks", "incidents"] {
            let table_dir = tables_dir.join(table_name);
            assert!(table_dir.exists(), "{table_name} directory should exist");
            assert!(
                table_dir.join(".d").exists(),
                "{table_name}/.d schema file should exist"
            );
        }

        // team_members: 5 columns
        let tm_dir = tables_dir.join("team_members");
        for col in &["id", "name", "role", "department", "start_date"] {
            assert!(tm_dir.join(col).exists(), "team_members/{col} should exist");
        }

        // project_tasks: 7 columns
        let pt_dir = tables_dir.join("project_tasks");
        for col in &[
            "id",
            "title",
            "assignee",
            "status",
            "priority",
            "project",
            "created_at",
        ] {
            assert!(
                pt_dir.join(col).exists(),
                "project_tasks/{col} should exist"
            );
        }

        // incidents: 7 columns
        let inc_dir = tables_dir.join("incidents");
        for col in &[
            "id",
            "title",
            "severity",
            "reporter",
            "resolved",
            "duration_min",
            "created_at",
        ] {
            assert!(inc_dir.join(col).exists(), "incidents/{col} should exist");
        }
    }

    #[test]
    fn test_splayed_tables_loadable() {
        let tmp = tempfile::tempdir().unwrap();
        let data_dir = tmp.path();

        generate(data_dir).unwrap();

        // Load tables in a fresh session to verify sym file works across sessions
        let tables_dir = data_dir.join("tables");
        let sym_path = tables_dir.join("sym");
        let mut session = teide::Session::new().unwrap();

        for table_name in &["team_members", "project_tasks", "incidents"] {
            let table_dir = tables_dir.join(table_name);
            let sql = format!(
                "CREATE TABLE {table_name} AS SELECT * FROM read_splayed('{}', '{}')",
                table_dir.display(),
                sym_path.display(),
            );
            session.execute(&sql).unwrap();

            let (nrows, ncols) = session
                .table_info(table_name)
                .expect(&format!("{table_name} should be registered"));
            assert!(nrows > 0, "{table_name} should have rows");
            assert!(ncols > 0, "{table_name} should have columns");
        }

        // Query to verify data integrity
        let result = session
            .execute("SELECT name, role FROM team_members LIMIT 1")
            .unwrap();
        if let teide::ExecResult::Query(q) = result {
            assert_eq!(q.columns.len(), 2);
            assert_eq!(q.columns[0], "name");
            assert_eq!(q.columns[1], "role");
            assert_eq!(q.table.nrows(), 1);
        } else {
            panic!("expected query result");
        }
    }
}
