from scripts.demo_flow import run_demo


def test_demo_flow_passes_and_writes_required_events():
    result = run_demo()
    assert result["status"] == "passed"
    assert result["proof_valid_before_revoke"] is True
    assert result["revoke_proof_valid"] is True
    events = [event["event_type"] for event in result["audit"]]
    for expected in [
        "identity.created",
        "consent.granted",
        "proof.generated",
        "proof.verified",
        "consent.revoked",
    ]:
        assert expected in events
