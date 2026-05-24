use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct PointNm {
    pub x_nm: i64,
    pub y_nm: i64,
}

pub fn parse_point_nm(value: &str) -> Result<PointNm, String> {
    let (x, y) = value
        .split_once(',')
        .ok_or_else(|| "expected point as `<x>,<y>`, for example `10mm,20mm`".to_string())?;
    Ok(PointNm {
        x_nm: parse_distance_nm(x.trim())?,
        y_nm: parse_distance_nm(y.trim())?,
    })
}

pub fn parse_distance_nm(value: &str) -> Result<i64, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err("distance cannot be empty".to_string());
    }

    let split_at = trimmed
        .find(|ch: char| !(ch.is_ascii_digit() || ch == '.' || ch == '-' || ch == '+'))
        .unwrap_or(trimmed.len());
    let (number, unit) = trimmed.split_at(split_at);
    if number.is_empty() || number == "+" || number == "-" {
        return Err(format!("missing numeric distance in `{value}`"));
    }

    let number = number
        .parse::<f64>()
        .map_err(|err| format!("invalid distance `{value}`: {err}"))?;
    let scale = match unit.trim().to_ascii_lowercase().as_str() {
        "" | "nm" => 1.0,
        "um" | "µm" => 1_000.0,
        "mm" => 1_000_000.0,
        "cm" => 10_000_000.0,
        "m" => 1_000_000_000.0,
        "mil" | "mils" => 25_400.0,
        "in" | "inch" | "inches" => 25_400_000.0,
        other => {
            return Err(format!(
                "unsupported distance unit `{other}` in `{value}`; expected nm, um, mm, cm, m, mil, or in"
            ))
        }
    };
    let nm = number * scale;
    if !nm.is_finite() || nm < i64::MIN as f64 || nm > i64::MAX as f64 {
        return Err(format!("distance `{value}` is outside the supported range"));
    }
    Ok(nm.round() as i64)
}

pub fn nm_to_mm(nm: i64) -> f64 {
    nm as f64 / 1_000_000.0
}

#[cfg(test)]
mod tests {
    use super::{parse_distance_nm, parse_point_nm};

    #[test]
    fn parses_common_distance_units() {
        assert_eq!(parse_distance_nm("42nm"), Ok(42));
        assert_eq!(parse_distance_nm("1um"), Ok(1_000));
        assert_eq!(parse_distance_nm("2.54mm"), Ok(2_540_000));
        assert_eq!(parse_distance_nm("100mil"), Ok(2_540_000));
        assert_eq!(parse_distance_nm("1in"), Ok(25_400_000));
    }

    #[test]
    fn parses_points() {
        let point = parse_point_nm("1mm, 2.5mm").expect("point should parse");
        assert_eq!(point.x_nm, 1_000_000);
        assert_eq!(point.y_nm, 2_500_000);
    }

    #[test]
    fn rejects_unknown_units() {
        let err = parse_distance_nm("1parsec").expect_err("unit should fail");
        assert!(err.contains("unsupported distance unit"));
    }
}
