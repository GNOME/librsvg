use rsvg::test_svg_reference;

test_svg_reference!(
    ellipse_auto_rx_ry,
    "tests/fixtures/reftests/svg2-reftests/ellipse-auto-rx-ry.svg",
    "tests/fixtures/reftests/svg2-reftests/ellipse-auto-rx-ry-ref.svg"
);

test_svg_reference!(
    ellipse_single_auto_rx_ry,
    "tests/fixtures/reftests/svg2-reftests/ellipse-single-auto-rx-ry.svg",
    "tests/fixtures/reftests/svg2-reftests/ellipse-single-auto-rx-ry-ref.svg"
);
