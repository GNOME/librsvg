enum State {
    Whitespace,
    IntegralPart,
    FractionalPart,
    ExponentSign,
    Exponent
}

pub fn strtod (string: &str) -> (f64, &str) {
    let mut state = State::Whitespace;
    let mut value: f64 = 0.0;
    let mut sign: f64 = 1.0;
    let mut fraction: f64 = 1.0;
    let mut exponent_sign: f64 = 1.0;
    let mut exponent: f64 = 0.0;
    let mut last_pos: usize = 0;

    for (pos, c) in string.chars ().enumerate () {
        last_pos = pos;

        match state {
            State::Whitespace => {
                if c.is_whitespace () {
                    continue;
                } else if c == '+' || c == '-' {
                    if c == '-' {
                        sign = -1.0;
                    }

                    state = State::IntegralPart;
                } else if c.is_digit (10) {
                    state = State::IntegralPart;
                    value = (c as i32 - '0' as i32) as f64;
                } else if c == '.' {
                    state = State::FractionalPart;
                } else {
                    break;
                }
            },

            State::IntegralPart => {
                if c.is_digit (10) {
                    value = value * 10.0 + (c as i32 - '0' as i32) as f64;
                } else if c == '.' {
                    state = State::FractionalPart;
                } else if c == 'e' || c == 'E' {
                    state = State::ExponentSign;
                } else {
                    break;
                }
            },

            State::FractionalPart => {
                if c.is_digit (10) {
                    fraction *= 0.1;
                    value += fraction * (c as i32 - '0' as i32) as f64;
                } else if c == 'e' || c == 'E' {
                    state = State::ExponentSign;
                } else {
                    break;
                }
            },

            State::ExponentSign => {
                if c == '+' || c == '-' {
                    if c == '-' {
                        exponent_sign = -1.0;
                    }

                    state = State::Exponent;
                } else if c.is_digit (10) {
                    exponent = (c as i32 - '0' as i32) as f64;
                } else {
                    break;
                }
            },

            State::Exponent => {
                if c.is_digit (10) {
                    exponent = exponent * 10.0 + (c as i32 - '0' as i32) as f64;
                } else {
                    break;
                }
            }
        }

        last_pos += 1;
    }

    // return tuple with the value and the non-matching slice

    (sign * value * 10.0f64.powf (exponent * exponent_sign),
     &string[last_pos .. ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_empty_string () {
        let str = "";
        assert_eq! (strtod (str), (0.0f64, &str[0..]));
    }

    #[test]
    fn handles_integer () {
        let str = "-123foo";
        assert_eq! (strtod (str), (-123.0f64, &str[4..]));

        let str = "12345";
        assert_eq! (strtod (str), (12345.0f64, &str[5..]));
    }

    #[test]
    fn handles_float () {
        let str = "-123.25";
        assert_eq! (strtod (str), (-123.25f64, &str[7..]));

        let str = "123.25bar";
        assert_eq! (strtod (str), (123.25f64, &str[6..]));
    }

    #[test]
    fn handles_dot_numbers () {
        let str = "-.25foo";
        assert_eq! (strtod (str), (-0.25f64, &str[4..]));

        let str = ".25";
        assert_eq! (strtod (str), (0.25f64, &str[3..]));
    }

    #[test]
    fn handles_dot () {
        let str = "-.";
        assert_eq! (strtod (str), (-0.0, &str[2..]));

        let str = ".bar";
        assert_eq! (strtod (str), (0.0, &str[1..]));
    }

    #[test]
    fn handles_exponent () {
        let str = "-123.45e2foo";
        assert_eq! (strtod (str), (-12345.0, &str[9..]));

        let str = "123.45E2";
        assert_eq! (strtod (str), (12345.0, &str[8..]));
    }

    #[test]
    fn handles_negative_exponent () {
        let str = "-123.25e-2";
        assert_eq! (strtod (str), (-1.2325, &str[10..]));

        let str = "123.25E-2bar";
        assert_eq! (strtod (str), (1.2325, &str[9..]));
    }
}
