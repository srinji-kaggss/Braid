use braid_capability::Capability;
use braid_ir::braid::{Braid, Strand};
use braid_ir::term::{EffectClass, Exposure, TermRegistry, TermSpec, TypeTag};
use braid_ir::{Capsule, ConfirmPolicy, Value, IR_VERSION};
use braid_run::{execute, ExecutionError, Host};
use braid_verify::verify;

fn setup_test_registry() -> TermRegistry {
    let mut reg = TermRegistry::new(1);

    // 1. Literal Int generator (Pure)
    reg.insert(TermSpec {
        id: "math.add".into(),
        inputs: vec![TypeTag::Int, TypeTag::Int],
        output: TypeTag::Int,
        capability: None,
        effect: EffectClass::Pure,
        source_exposure: Exposure::Public,
        egress_ceiling: None,
        cost: 1,
    })
    .unwrap();

    // 2. Math Mul (Pure)
    reg.insert(TermSpec {
        id: "math.mul".into(),
        inputs: vec![TypeTag::Int, TypeTag::Int],
        output: TypeTag::Int,
        capability: None,
        effect: EffectClass::Pure,
        source_exposure: Exposure::Public,
        egress_ceiling: None,
        cost: 2,
    })
    .unwrap();

    // 3. Math Lit (Pure, cost 1)
    reg.insert(TermSpec {
        id: "math.lit".into(),
        inputs: vec![],
        output: TypeTag::Int,
        capability: None,
        effect: EffectClass::Pure,
        source_exposure: Exposure::Public,
        egress_ceiling: None,
        cost: 1,
    })
    .unwrap();

    // 4. File Write (Stateful, requires capability)
    reg.insert(TermSpec {
        id: "fs.write".into(),
        inputs: vec![TypeTag::Int],
        output: TypeTag::Bool,
        capability: Some(Capability::new("fs.write")),
        effect: EffectClass::Irreversible,
        source_exposure: Exposure::Public,
        egress_ceiling: Some(Exposure::Public),
        cost: 5,
    })
    .unwrap();

    reg
}

struct CustomHost {
    pub writes: Vec<i64>,
}

impl Host for CustomHost {
    fn call(
        &mut self,
        term_id: &str,
        inputs: &[Value],
        _spec: &TermSpec,
    ) -> Result<Value, ExecutionError> {
        match term_id {
            "math.lit" => Ok(Value::Int(10)),
            "math.add" => {
                if let (Some(Value::Int(a)), Some(Value::Int(b))) = (inputs.first(), inputs.get(1)) {
                    Ok(Value::Int(a + b))
                } else {
                    Err(ExecutionError::HostError("expected two Ints".into()))
                }
            }
            "math.mul" => {
                if let (Some(Value::Int(a)), Some(Value::Int(b))) = (inputs.first(), inputs.get(1)) {
                    Ok(Value::Int(a * b))
                } else {
                    Err(ExecutionError::HostError("expected two Ints".into()))
                }
            }
            "fs.write" => {
                if let Some(Value::Int(v)) = inputs.first() {
                    self.writes.push(*v);
                    Ok(Value::Bool(true))
                } else {
                    Err(ExecutionError::HostError("expected Int to write".into()))
                }
            }
            _ => Err(ExecutionError::UnknownTerm(term_id.into())),
        }
    }
}

#[test]
fn pure_dag_execution_and_journal() {
    let reg = setup_test_registry();

    // Capsule computes: (lit (10) + lit (10)) * lit (10) = 20 * 10 = 200
    let capsule = Capsule {
        ir_version: IR_VERSION,
        vocab_version: reg.vocab_version,
        registry_cid: reg.cid(),
        intent: "compute math in DAG order".into(),
        grants: vec![],
        braid: Braid {
            strands: vec![
                Strand {
                    term: "math.lit".into(),
                    inputs: vec![],
                }, // strand 0 = 10
                Strand {
                    term: "math.lit".into(),
                    inputs: vec![],
                }, // strand 1 = 10
                Strand {
                    term: "math.add".into(),
                    inputs: vec![0, 1],
                }, // strand 2 = 20
                Strand {
                    term: "math.mul".into(),
                    inputs: vec![2, 0],
                }, // strand 3 = 20 * 10 = 200
            ],
            outputs: vec![3],
        },
        budget: 10,
        confirm: ConfirmPolicy::None,
        evidence: vec![],
    };

    // 1. Verify admission.
    let bytes = capsule.to_bytes();
    let verdict = verify(&bytes, &reg, &[]);
    assert!(matches!(verdict, braid_verify::Verdict::Admit { .. }));

    // 2. Execute.
    let mut host = CustomHost { writes: vec![] };
    let journal = execute(&capsule, &reg, &mut host).expect("execution succeeds");

    // 3. Inspect outputs & journal.
    assert_eq!(journal.outputs, vec![Value::Int(200)]);
    assert_eq!(journal.total_cost, 1 + 1 + 1 + 2); // 5 units
    assert_eq!(journal.entries.len(), 4);
    assert_eq!(journal.entries[3].output, Value::Int(200));
}

#[test]
fn capability_gated_execution_fails_if_grant_missing() {
    let reg = setup_test_registry();

    // Capsule uses fs.write without declaring grant
    let capsule = Capsule {
        ir_version: IR_VERSION,
        vocab_version: reg.vocab_version,
        registry_cid: reg.cid(),
        intent: "unauthorized write".into(),
        grants: vec![], // Missing fs.write capability!
        braid: Braid {
            strands: vec![
                Strand {
                    term: "math.lit".into(),
                    inputs: vec![],
                },
                Strand {
                    term: "fs.write".into(),
                    inputs: vec![0],
                },
            ],
            outputs: vec![1],
        },
        budget: 10,
        confirm: ConfirmPolicy::HumanConfirm,
        evidence: vec![],
    };

    let mut host = CustomHost { writes: vec![] };
    let err = execute(&capsule, &reg, &mut host).unwrap_err();
    assert_eq!(
        err,
        ExecutionError::MissingCapability(Capability::new("fs.write"))
    );
}

#[test]
fn budget_exhaustion_terminates_execution() {
    let reg = setup_test_registry();

    let capsule = Capsule {
        ir_version: IR_VERSION,
        vocab_version: reg.vocab_version,
        registry_cid: reg.cid(),
        intent: "expensive computation".into(),
        grants: vec![],
        braid: Braid {
            strands: vec![
                Strand {
                    term: "math.lit".into(),
                    inputs: vec![],
                }, // cost 1
                Strand {
                    term: "math.mul".into(),
                    inputs: vec![0, 0],
                }, // cost 2
                Strand {
                    term: "math.mul".into(),
                    inputs: vec![1, 0],
                }, // cost 2 -> total 5
            ],
            outputs: vec![2],
        },
        budget: 4, // Budget is 4, but total cost is 5!
        confirm: ConfirmPolicy::None,
        evidence: vec![],
    };

    let mut host = CustomHost { writes: vec![] };
    let err = execute(&capsule, &reg, &mut host).unwrap_err();
    assert_eq!(
        err,
        ExecutionError::BudgetExhausted {
            budget: 4,
            consumed: 5,
        }
    );
}

#[test]
fn confirmation_policy_enforcement() {
    let reg = setup_test_registry();

    let capsule = Capsule {
        ir_version: IR_VERSION,
        vocab_version: reg.vocab_version,
        registry_cid: reg.cid(),
        intent: "unconfirmed irreversible effect".into(),
        grants: vec![Capability::new("fs.write")],
        braid: Braid {
            strands: vec![
                Strand {
                    term: "math.lit".into(),
                    inputs: vec![],
                },
                Strand {
                    term: "fs.write".into(),
                    inputs: vec![0],
                },
            ],
            outputs: vec![1],
        },
        budget: 10,
        confirm: ConfirmPolicy::None, // Should be HumanConfirm!
        evidence: vec![],
    };

    let mut host = CustomHost { writes: vec![] };
    let err = execute(&capsule, &reg, &mut host).unwrap_err();
    assert!(matches!(err, ExecutionError::UnconfirmedEffect { .. }));
}

#[test]
fn list_type_confusion_rejected() {
    use braid_run::validate_type_tag;

    // List of Text when List of Int was expected
    let val = Value::List(vec![Value::Text("not an int".into())]);
    let expected = TypeTag::List(Box::new(TypeTag::Int));
    assert!(matches!(
        validate_type_tag(&val, &expected),
        Err(ExecutionError::TypeMismatch { .. })
    ));
}

#[test]
fn cid_length_validated() {
    use braid_run::validate_type_tag;

    // Valid 32-byte CID
    let valid_cid = Value::Bytes(vec![0u8; 32]);
    assert!(validate_type_tag(&valid_cid, &TypeTag::Cid).is_ok());

    // Invalid 16-byte CID
    let invalid_cid = Value::Bytes(vec![0u8; 16]);
    assert!(matches!(
        validate_type_tag(&invalid_cid, &TypeTag::Cid),
        Err(ExecutionError::TypeMismatch { .. })
    ));
}

#[test]
fn header_mismatch_rejected() {
    let reg = setup_test_registry();

    let mut capsule = Capsule {
        ir_version: IR_VERSION,
        vocab_version: reg.vocab_version,
        registry_cid: reg.cid(),
        intent: "tampered header".into(),
        grants: vec![],
        braid: Braid {
            strands: vec![Strand {
                term: "math.lit".into(),
                inputs: vec![],
            }],
            outputs: vec![0],
        },
        budget: 10,
        confirm: ConfirmPolicy::None,
        evidence: vec![],
    };

    // Tamper ir_version
    capsule.ir_version = 999;
    let mut host = CustomHost { writes: vec![] };
    assert!(matches!(
        execute(&capsule, &reg, &mut host),
        Err(ExecutionError::InvalidCapsuleHeader(_))
    ));
}
