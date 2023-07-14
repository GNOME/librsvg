use rsvg::test_svg_reference;

test_svg_reference!(
    bug_996_malicious_url,
    "tests/fixtures/loading/disallowed-996.svg",
    "tests/fixtures/loading/disallowed-996-ref.svg"
);
