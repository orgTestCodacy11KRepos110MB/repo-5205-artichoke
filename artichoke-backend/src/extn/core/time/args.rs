use crate::convert::to_int;
use crate::extn::prelude::*;

#[derive(Debug)]
pub struct TimeArgs {
    year: i64,
    month: i64,
    day: i64,
    hour: i64,
    minute: i64,
    second: i64,
    micros: i64,
}

impl Default for TimeArgs {
    fn default() -> TimeArgs {
        TimeArgs {
            year: 0,
            month: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
            micros: 0,
        }
    }
}

impl TimeArgs {
    pub fn year(&self) -> Result<i32, Error> {
        Ok(0)
    }

    pub fn month(&self) -> Result<u8, Error> {
        match self.month {
            1..=12 => Ok(self.month as u8),
            _ => Err(ArgumentError::with_message("mon out of range").into())
        }
    }

    pub fn day(&self) -> Result<u8, Error> {
        match self.day {
            1..=31 => Ok(self.month as u8),
            _ => Err(ArgumentError::with_message("mday out of range").into())
        }
    }

    pub fn hour(&self) -> Result<u8, Error> {
        match self.hour {
            0..=23 => Ok(self.month as u8),
            _ => Err(ArgumentError::with_message("hour out of range").into())
        }
    }

    pub fn minute(&self) -> Result<u8, Error> {
        match self.minute {
            0..=59 => Ok(self.minute as u8),
            _ => Err(ArgumentError::with_message("min out of range").into())
        }
    }

    pub fn second(&self) -> Result<u8, Error> {
        match self.second {
            0..=60 => Ok(self.second as u8),
            _ => Err(ArgumentError::with_message("sec out of range").into())
        }
    }

    pub fn nanoseconds(&self) -> Result<u32, Error> {
        // TimeArgs take a micros parameter, not a nanos value. The below
        // multiplication and casting is gauranteed to be inside a `u32`.
        match self.micros {
            0..=999_999 => Ok((self.micros * 1000) as u32),
            _ => Err(ArgumentError::with_message("sec out of range").into())
        }
    }
}

pub fn as_time_args(interp: &mut Artichoke, args: &[Value]) -> Result<TimeArgs, Error> {
    // TimeArgs are in order of year, month, day, hour, minute, second, micros.
    // This is unless there are 10 arguments provided (`Time#to_a` format), at
    // which points it is second, minute, hour, day, month, year. The number of
    // expected parameters doesn't give this hint though.

    match args.len() {
        0 | 9 | 11.. => {
            let mut message = br#"wrong number of arguments (given "#.to_vec();
            message.extend(args.len().to_string().bytes());
            message.extend_from_slice(b", expected 1..8)");
            Err(ArgumentError::from(message).into())
        }
        1..=8 => {
            // For 0..=7 params, we need to validate to_int
            let mut result = TimeArgs::default();
            for (i, arg) in args.iter().enumerate() {
                let arg = to_int(interp, *arg)?;
                // unwrap is safe since to_int gaurnatees a non nil Ruby::Integer
                let arg: i64 = arg.try_convert_into::<Option<i64>>(interp)?.unwrap();

                match i {
                    0 => result.year = arg,
                    1 => result.month = arg,
                    2 => result.day = arg,
                    3 => result.hour = arg,
                    4 => result.minute = arg,
                    5 => result.second = arg,
                    6 => result.micros = arg,
                    7 => {}
                    _ => unreachable!(),
                }
            }
            Ok(result)
        }
        10 => todo!(),
        _ => unreachable!(),
    }
}

#[cfg(test)]
mod tests {
    use super::as_time_args;
    use crate::test::prelude::*;
    use bstr::ByteSlice;

    #[test]
    fn requires_at_least_one_param() {
        let mut interp = interpreter();

        let raw_args = [];

        let err = as_time_args(&mut interp, &raw_args).unwrap_err();

        assert_eq!(err.name(), "ArgumentError");
        assert_eq!(
            err.message().as_bstr(),
            b"wrong number of arguments (given 0, expected 1..8)"
                .as_slice()
                .as_bstr()
        );
    }

    #[test]
    fn one_to_eight_params() {
        // TODO: Table test 1..8 params
        let mut interp = interpreter();

        let raw_args = [interp.eval(b"2022").unwrap()];

        let args = as_time_args(&mut interp, &raw_args).unwrap();
        let result = args.year().unwrap();

        assert_eq!(2022, result)
    }

    #[test]
    fn subsec_is_micros_not_nanos() {}

    #[test]
    fn fractional_seconds_return_nanos() {}

    #[test]
    fn eighth_param_is_ignored() {}

    #[test]
    fn nine_args_not_supported() {
        let mut interp = interpreter();

        let raw_args = [
            interp.eval(b"2022").unwrap(),
            interp.eval(b"12").unwrap(),
            interp.eval(b"25").unwrap(),
            interp.eval(b"12").unwrap(),
            interp.eval(b"34").unwrap(),
            interp.eval(b"56").unwrap(),
            interp.eval(b"0").unwrap(),
            interp.eval(b"nil").unwrap(),
            interp.eval(b"0").unwrap(),
        ];

        let result = as_time_args(&mut interp, &raw_args);
        let error = result.unwrap_err();

        assert_eq!(
            error.message().as_bstr(),
            b"wrong number of arguments (given 9, expected 1..8)"
                .as_slice()
                .as_bstr()
        );
        assert_eq!(error.name(), "ArgumentError");
    }

    #[test]
    fn ten_args_changes_unit_order() {}

    #[test]
    fn ten_args_removes_micros() {}

    #[test]
    fn eleven_args_too_many() {
        let mut interp = interpreter();

        let raw_args = [
            interp.eval(b"2022").unwrap(),
            interp.eval(b"12").unwrap(),
            interp.eval(b"25").unwrap(),
            interp.eval(b"12").unwrap(),
            interp.eval(b"34").unwrap(),
            interp.eval(b"56").unwrap(),
            interp.eval(b"0").unwrap(),
            interp.eval(b"nil").unwrap(),
            interp.eval(b"0").unwrap(),
            interp.eval(b"0").unwrap(),
            interp.eval(b"0").unwrap(),
        ];

        let result = as_time_args(&mut interp, &raw_args);
        let error = result.unwrap_err();

        assert_eq!(
            error.message().as_bstr(),
            b"wrong number of arguments (given 11, expected 1..8)"
                .as_slice()
                .as_bstr()
        );
        assert_eq!(error.name(), "ArgumentError");
    }
}
