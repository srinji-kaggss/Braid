//! Issue #77 execution edge: real DSL source reaches the proof-gated
//! `braid-run` reference interpreter, never a raw-capsule shortcut.

use std::collections::BTreeMap;

use braid_elaborate_dsl::elaborate;
use braid_flow_ir::Predicate;
use braid_flow_plan::{CompletionMap, FlowSnapshot};
use braid_ir::{TypeTag, Value};
use braid_run::{ExecutionError, Host, JustificationProof, RunnableInvocation, execute_runnable};
use braid_vocab_cms::registry_v0;

const EDIT: &str = include_str!("fixtures/edit-home-hero.brd");
const PUBLISH: &str = include_str!("fixtures/publish-services.brd");
const LISTING: &str = include_str!("fixtures/render-work-listing.brd");

#[derive(Default)]
struct ReferenceCmsHost {
    calls: Vec<String>,
}

fn placeholder(output: &TypeTag) -> Value {
    match output {
        TypeTag::Bool => Value::Bool(false),
        TypeTag::Int => Value::Int(0),
        TypeTag::Bytes | TypeTag::Opaque(_, _) => Value::Bytes(Vec::new()),
        TypeTag::Text => Value::Text("reference-value".into()),
        TypeTag::Cid => Value::Bytes(vec![0; 32]),
        TypeTag::List(_) => Value::List(Vec::new()),
    }
}

impl Host for ReferenceCmsHost {
    fn call(
        &mut self,
        term_id: &str,
        _inputs: &[Value],
        spec: &braid_ir::TermSpec,
    ) -> Result<Value, ExecutionError> {
        self.calls.push(term_id.to_owned());
        Ok(placeholder(&spec.output))
    }
}

fn execute_source(source: &str) -> (braid_run::Journal, ReferenceCmsHost) {
    let elaboration = elaborate(source).expect("DSL source admits");
    let registry = registry_v0();
    let snapshot = FlowSnapshot::new(BTreeMap::new());
    let justification = JustificationProof::prove(
        &elaboration.capsule,
        &Predicate::Const(true),
        &snapshot,
        &CompletionMap::new(),
    )
    .expect("constant-true justification produces proof");
    let admission = braid_verify::admit(&elaboration.bytes, &registry, &elaboration.capsule.grants)
        .expect("independent admission produces proof");
    let invocation = RunnableInvocation::prepare(
        elaboration.capsule.clone(),
        &registry,
        admission,
        justification,
        &elaboration.capsule.grants,
        &snapshot,
    )
    .expect("proofs bind to exact capsule, authority, registry, and snapshot");
    let mut host = ReferenceCmsHost::default();
    let journal = execute_runnable(
        &invocation,
        &registry,
        &elaboration.capsule.grants,
        &snapshot,
        &mut host,
    )
    .expect("reference execution succeeds");
    (journal, host)
}

#[test]
fn three_dsl_demo_actions_reach_proof_gated_execution() {
    for (source, expected_calls) in [
        (
            EDIT,
            vec!["lit.entity", "lit.text", "cms.edit_section", "view.section"],
        ),
        (
            PUBLISH,
            vec!["lit.entity", "lit.text", "cms.edit_section", "cms.publish"],
        ),
        (LISTING, vec!["lit.entity", "proj.listing"]),
    ] {
        let (journal, host) = execute_source(source);
        assert_eq!(host.calls, expected_calls);
        assert_eq!(journal.entries.len(), expected_calls.len());
        assert!(!journal.outputs.is_empty());
    }
}

#[test]
fn execution_refuses_authority_transplant_after_dsl_admission() {
    let elaboration = elaborate(EDIT).unwrap();
    let registry = registry_v0();
    let snapshot = FlowSnapshot::new(BTreeMap::new());
    let justification = JustificationProof::prove(
        &elaboration.capsule,
        &Predicate::Const(true),
        &snapshot,
        &CompletionMap::new(),
    )
    .unwrap();
    let admission =
        braid_verify::admit(&elaboration.bytes, &registry, &elaboration.capsule.grants).unwrap();
    let invocation = RunnableInvocation::prepare(
        elaboration.capsule.clone(),
        &registry,
        admission,
        justification,
        &elaboration.capsule.grants,
        &snapshot,
    )
    .unwrap();
    let mut host = ReferenceCmsHost::default();
    let error = execute_runnable(&invocation, &registry, &[], &snapshot, &mut host)
        .expect_err("narrower ambient authority must not execute");
    assert!(matches!(error, ExecutionError::InvalidRunnableProof { .. }));
    assert!(host.calls.is_empty());
}
