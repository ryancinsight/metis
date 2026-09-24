use super::{
    Launch, MAX_FORWARDED_ARGUMENTS, claim_or_forward, decode_arguments, encode_arguments,
};
use std::io::ErrorKind;

#[test]
fn arguments_round_trip_including_empty_ones() {
    let arguments = ["viewer", "", "org.example.viewer://open/study%201"];
    let message = encode_arguments(arguments).expect("encode");
    assert_eq!(decode_arguments(&message).expect("decode"), arguments);
    assert!(decode_arguments(&[]).expect("empty").is_empty());
}

#[test]
fn argument_bounds_and_malformed_messages_are_refused() {
    assert!(encode_arguments(["a\0b"]).is_err());
    let many = vec!["x"; MAX_FORWARDED_ARGUMENTS + 1];
    assert!(encode_arguments(&many).is_err());
    let huge = "x".repeat(moirai_pal::instance::MAX_INSTANCE_MESSAGE_BYTES);
    assert!(encode_arguments([huge]).is_err());
    for malformed in [
        &b"no-terminator"[..],
        &[0xff, 0],
        &[0; MAX_FORWARDED_ARGUMENTS + 1],
    ] {
        assert_eq!(
            decode_arguments(malformed).expect_err("malformed").kind(),
            ErrorKind::InvalidData
        );
    }
}

#[test]
fn later_launches_forward_their_arguments() {
    let name = format!("org.metis.single-instance-test-{}", std::process::id());
    let Launch::Primary(mut primary) = claim_or_forward(&name, ["first"]).expect("first") else {
        panic!("the first launch must be primary");
    };
    assert_eq!(primary.try_receive().expect("idle"), None);
    let second = claim_or_forward(&name, ["viewer", "org.metis.test://open/7"]).expect("second");
    assert!(matches!(second, Launch::Forwarded));
    let received = (0..1_000)
        .find_map(|_| primary.try_receive().transpose())
        .expect("forwarded arguments")
        .expect("decoded");
    assert_eq!(received, ["viewer", "org.metis.test://open/7"]);
    assert!(claim_or_forward("Not Valid", ["x"]).is_err());
}
