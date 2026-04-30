use dicom::{
    core::{Tag, value::Value},
    object::InMemDicomObject,
};
use dicom_dictionary_std::tags::{
    AGE_CORRECTED_SENSITIVITY_DEVIATION_PROBABILITY_VALUE,
    AGE_CORRECTED_SENSITIVITY_DEVIATION_VALUE,
    BACKGROUND_LUMINANCE,
    BLIND_SPOT_X_COORDINATE, BLIND_SPOT_Y_COORDINATE,
    CODE_MEANING, CODE_VALUE, CODING_SCHEME_DESIGNATOR,
    CORRECTED_LOCALIZED_DEVIATION_FROM_NORMAL,
    EXCESSIVE_FALSE_NEGATIVES, EXCESSIVE_FALSE_POSITIVES, EXCESSIVE_FIXATION_LOSSES,
    FALSE_NEGATIVES_ESTIMATE, FALSE_NEGATIVES_QUANTITY,
    FALSE_POSITIVES_ESTIMATE, FALSE_POSITIVES_QUANTITY,
    FIXATION_CHECKED_QUANTITY, FIXATION_MONITORING_CODE_SEQUENCE,
    FIXATION_SEQUENCE, FOVEAL_SENSITIVITY,
    GENERALIZED_DEFECT_CORRECTED_SENSITIVITY_DEVIATION_PROBABILITY_VALUE,
    GENERALIZED_DEFECT_CORRECTED_SENSITIVITY_DEVIATION_VALUE,
    GLOBAL_DEVIATION_FROM_NORMAL, IMAGE_LATERALITY, LATERALITY,
    LOCALIZED_DEVIATION_FROM_NORMAL, MANUFACTURER, MANUFACTURER_MODEL_NAME,
    MEASUREMENT_LATERALITY,
    NEGATIVE_CATCH_TRIALS_QUANTITY, NUMBER_OF_VISUAL_STIMULI,
    PATIENT_AGE, PATIENT_BIRTH_DATE, PATIENT_NAME, PATIENT_NOT_PROPERLY_FIXATED_QUANTITY,
    PATIENT_SEX,
    PERFORMED_PROTOCOL_CODE_SEQUENCE, POSITIVE_CATCH_TRIALS_QUANTITY,
    SENSITIVITY_VALUE, SERIES_DESCRIPTION,
    STIMULI_RETESTING_QUANTITY, STIMULUS_AREA, STIMULUS_RESULTS, STUDY_DATE,
    VISUAL_FIELD_HORIZONTAL_EXTENT, VISUAL_FIELD_MEAN_SENSITIVITY,
    VISUAL_FIELD_TEST_DURATION,
    VISUAL_FIELD_TEST_POINT_NORMALS_SEQUENCE,
    VISUAL_FIELD_TEST_POINT_SEQUENCE, VISUAL_FIELD_TEST_POINT_X_COORDINATE,
    VISUAL_FIELD_TEST_POINT_Y_COORDINATE,
};

#[derive(Clone, Copy, PartialEq)]
pub enum TestStrategy {
    Threshold,
    TwoZone,
    ThreeZone,
    QuantifyDefects,
}

impl TestStrategy {
    pub fn is_suprathreshold(self) -> bool {
        matches!(self, Self::TwoZone | Self::ThreeZone | Self::QuantifyDefects)
    }
}

pub struct VfPoint {
    pub x: f32,
    pub y: f32,
    /// Sensitivity in dB. None when "NOT SEEN" or suprathreshold test.
    pub sensitivity: Option<f32>,
    pub seen: bool,
    /// Raw DICOM StimulusResults value (e.g. "SEEN", "NOT SEEN", "NORMAL", "RELATIVE DEFECT").
    /// Kept for potential diagnostic display.
    #[expect(dead_code)]
    pub stimulus_result: String,
    pub is_blind_spot: bool,
    /// Total deviation (age-corrected sensitivity deviation).
    pub td: Option<f32>,
    /// Pattern deviation (generalized-defect-corrected).
    pub pd: Option<f32>,
    /// TD probability p-value (0..100, DICOM percentage range).
    pub td_p: Option<f32>,
    /// PD probability p-value (0..100, DICOM percentage range).
    pub pd_p: Option<f32>,
}

pub struct DicomVfData {
    pub patient_name: String,
    pub study_date: String,
    pub laterality: String,
    pub horizontal_extent: f32,
    pub foveal_sensitivity: Option<f32>,
    pub test_duration_s: Option<f32>,
    pub md: Option<f32>,
    pub psd: Option<f32>,
    pub false_neg: Option<f32>,
    pub false_pos: Option<f32>,
    pub fixation_loss: Option<bool>,
    pub fp_flag: Option<bool>,
    pub fn_flag: Option<bool>,
    pub fixation_method: Option<String>,
    pub test_strategy: TestStrategy,
    pub points: Vec<VfPoint>,
    /// False positives: caught / total catch trials
    pub fp_quantity: Option<u16>,
    pub fp_catch_trials: Option<u16>,
    /// False negatives: caught / total catch trials
    pub fn_quantity: Option<u16>,
    pub fn_catch_trials: Option<u16>,
    /// Fixation losses: lost / checked
    pub fixation_lost: Option<u16>,
    pub fixation_checked: Option<u16>,
    /// Total stimuli presented
    pub stimuli_count: Option<u16>,
    /// Stimuli retested
    pub stimuli_retested: Option<u16>,
    /// Patient age at time of study (e.g. "051Y")
    pub patient_age: Option<String>,
    /// Patient sex (M/F/O)
    pub patient_sex: Option<String>,
    /// Patient birth date (formatted)
    pub patient_birth_date: Option<String>,
    /// Device manufacturer (e.g. "Frey")
    pub manufacturer: Option<String>,
    /// Device model (e.g. "AP-600")
    pub model_name: Option<String>,
    /// Series description (e.g. "OD: 30-2, Standard")
    pub series_description: Option<String>,
    /// Goldmann stimulus size (I-V) derived from StimulusArea
    pub stimulus_size: Option<String>,
    /// Background luminance in cd/m²
    pub background_luminance: Option<f32>,
    /// Visual field mean sensitivity (MS) in dB
    pub mean_sensitivity: Option<f32>,
}

fn read_fl(obj: &InMemDicomObject, tag: Tag) -> Option<f32> {
    obj.get(tag)
        .and_then(|e| e.value().to_str().ok().map(|s| s.into_owned()))
        .and_then(|s| s.trim().parse::<f32>().ok())
}

fn read_us(obj: &InMemDicomObject, tag: Tag) -> Option<u16> {
    obj.get(tag)
        .and_then(|e| e.value().to_str().ok().map(|s| s.into_owned()))
        .and_then(|s| s.trim().parse::<u16>().ok())
}

fn read_str(obj: &InMemDicomObject, tag: Tag) -> Option<String> {
    obj.get(tag)
        .and_then(|e| e.value().to_str().ok().map(|s| s.trim().to_owned()))
        .filter(|s| !s.is_empty())
}

/// Read the first item of a nested sequence and call `f` on it.
fn first_seq_item<T>(
    obj: &InMemDicomObject,
    seq_tag: Tag,
    f: impl Fn(&InMemDicomObject) -> T,
) -> Option<T> {
    let elem = obj.get(seq_tag)?;
    let Value::Sequence(seq) = elem.value() else { return None; };
    let item = seq.items().first()?;
    Some(f(item))
}

/// Detect test strategy from PerformedProtocolCodeSequence.
/// Looks for DCM-coded items with known suprathreshold codes.
fn extract_test_strategy(obj: &InMemDicomObject) -> TestStrategy {
    let Some(seq_elem) = obj.get(PERFORMED_PROTOCOL_CODE_SEQUENCE) else {
        return TestStrategy::Threshold;
    };
    let Value::Sequence(seq) = seq_elem.value() else {
        return TestStrategy::Threshold;
    };
    for item in seq.items() {
        let scheme = read_str(item, CODING_SCHEME_DESIGNATOR).unwrap_or_default();
        if scheme != "DCM" {
            continue;
        }
        match read_str(item, CODE_VALUE).as_deref() {
            Some("111822") => return TestStrategy::TwoZone,
            Some("111823") => return TestStrategy::ThreeZone,
            Some("111824") => return TestStrategy::QuantifyDefects,
            _ => {}
        }
    }
    TestStrategy::Threshold
}

fn extract_vf_points(obj: &InMemDicomObject) -> Vec<VfPoint> {
    let Some(seq_elem) = obj.get(VISUAL_FIELD_TEST_POINT_SEQUENCE) else {
        return vec![];
    };
    let Value::Sequence(seq) = seq_elem.value() else {
        return vec![];
    };

    seq.items()
        .iter()
        .filter_map(|item| {
            let x = read_fl(item, VISUAL_FIELD_TEST_POINT_X_COORDINATE)?;
            let y = read_fl(item, VISUAL_FIELD_TEST_POINT_Y_COORDINATE)?;
            let sensitivity = read_fl(item, SENSITIVITY_VALUE);
            let stimulus_result = item
                .get(STIMULUS_RESULTS)
                .and_then(|e| e.value().to_str().ok().map(|s| s.trim().to_owned()))
                .unwrap_or_default();
            let seen = stimulus_result.is_empty() || stimulus_result != "NOT SEEN";

            let normals_item_td = first_seq_item(item, VISUAL_FIELD_TEST_POINT_NORMALS_SEQUENCE, |ni| {
                let td = read_fl(ni, AGE_CORRECTED_SENSITIVITY_DEVIATION_VALUE);
                let pd = read_fl(ni, GENERALIZED_DEFECT_CORRECTED_SENSITIVITY_DEVIATION_VALUE);
                let td_p = read_fl(ni, AGE_CORRECTED_SENSITIVITY_DEVIATION_PROBABILITY_VALUE);
                let pd_p = read_fl(ni, GENERALIZED_DEFECT_CORRECTED_SENSITIVITY_DEVIATION_PROBABILITY_VALUE);
                (td, pd, td_p, pd_p)
            });

            let td = normals_item_td
                .and_then(|(v, _, _, _)| v)
                .or_else(|| read_fl(item, AGE_CORRECTED_SENSITIVITY_DEVIATION_VALUE))
                .or_else(|| read_fl(item, CORRECTED_LOCALIZED_DEVIATION_FROM_NORMAL));
            let pd = normals_item_td
                .and_then(|(_, v, _, _)| v)
                .or_else(|| read_fl(item, GENERALIZED_DEFECT_CORRECTED_SENSITIVITY_DEVIATION_VALUE));
            let td_p = normals_item_td
                .and_then(|(_, _, v, _)| v)
                .or_else(|| read_fl(item, AGE_CORRECTED_SENSITIVITY_DEVIATION_PROBABILITY_VALUE));
            let pd_p = normals_item_td
                .and_then(|(_, _, _, v)| v)
                .or_else(|| read_fl(item, GENERALIZED_DEFECT_CORRECTED_SENSITIVITY_DEVIATION_PROBABILITY_VALUE));

            Some(VfPoint { x, y, sensitivity, seen, stimulus_result, is_blind_spot: false, td, pd, td_p, pd_p })
        })
        .collect()
}

fn parse_date(raw: &str) -> String {
    if raw.len() == 8 && raw.chars().all(|c| c.is_ascii_digit()) {
        format!("{}-{}-{}", &raw[..4], &raw[4..6], &raw[6..8])
    } else {
        raw.to_owned()
    }
}

/// Extract fixation monitoring method from FixationSequence → FixationMonitoringCodeSequence → CodeMeaning.
fn extract_fixation_method(obj: &InMemDicomObject) -> Option<String> {
    let fix_seq_elem = obj.get(FIXATION_SEQUENCE)?;
    let Value::Sequence(fix_seq) = fix_seq_elem.value() else { return None; };
    let fix_item = fix_seq.items().first()?;
    let mon_seq_elem = fix_item.get(FIXATION_MONITORING_CODE_SEQUENCE)?;
    let Value::Sequence(mon_seq) = mon_seq_elem.value() else { return None; };
    let mon_item = mon_seq.items().first()?;
    read_str(mon_item, CODE_MEANING)
}

/// Convert StimulusArea (mm²) to Goldmann size label.
fn goldmann_size(area_mm2: f32) -> &'static str {
    // Standard Goldmann sizes (diameter → area):
    // I: 0.25mm → 0.049 mm²,  II: 0.5mm → 0.196 mm²,  III: 0.43° ≈ 0.146 mm² (most common)
    // IV: 2mm → 3.14 mm²,     V: 4mm → 12.57 mm²
    if area_mm2 < 0.1 { "I" }
    else if area_mm2 < 0.18 { "III" }
    else if area_mm2 < 1.0 { "II" }  // some devices encode II differently
    else if area_mm2 < 8.0 { "IV" }
    else { "V" }
}

/// Parse DICOM age string (e.g. "051Y") to a displayable format (e.g. "51").
fn parse_age(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.len() >= 2 && trimmed.ends_with('Y') {
        trimmed[..trimmed.len()-1].trim_start_matches('0').to_string()
    } else {
        trimmed.to_string()
    }
}

/// Fallback blind spot detection using known anatomical location.
fn is_blind_spot_anatomical(x: f32, y: f32, laterality: &str) -> bool {
    let expected_x: f32 = match laterality.trim().to_uppercase().as_str() {
        "R" | "RIGHT" | "OD" => 15.0,
        "L" | "LEFT" | "OS" => -15.0,
        _ => return false,
    };
    (x - expected_x).abs() < 2.5 && (y + 2.0).abs() < 2.5
}

pub fn extract_data(obj: &InMemDicomObject) -> DicomVfData {
    let patient_name = read_str(obj, PATIENT_NAME)
        .unwrap_or_else(|| "Unknown".to_string())
        .replace('^', " ");
    let study_date = read_str(obj, STUDY_DATE)
        .map(|d| parse_date(&d))
        .unwrap_or_default();
    let laterality = read_str(obj, MEASUREMENT_LATERALITY)
        .or_else(|| read_str(obj, LATERALITY))
        .or_else(|| read_str(obj, IMAGE_LATERALITY))
        .unwrap_or_default();
    let horizontal_extent = read_fl(obj, VISUAL_FIELD_HORIZONTAL_EXTENT).unwrap_or(30.0);
    let foveal_sensitivity = read_fl(obj, FOVEAL_SENSITIVITY);
    let test_duration_s = read_fl(obj, VISUAL_FIELD_TEST_DURATION).map(|s| s / 1000.0);
    let md = read_fl(obj, GLOBAL_DEVIATION_FROM_NORMAL);
    let psd = read_fl(obj, LOCALIZED_DEVIATION_FROM_NORMAL);
    let false_neg = read_fl(obj, FALSE_NEGATIVES_ESTIMATE);
    let false_pos = read_fl(obj, FALSE_POSITIVES_ESTIMATE);
    let fixation_loss = obj
        .get(EXCESSIVE_FIXATION_LOSSES)
        .and_then(|e| e.value().to_str().ok().map(|s| s.into_owned()))
        .map(|s| s.trim() == "YES");
    let fp_flag = obj
        .get(EXCESSIVE_FALSE_POSITIVES)
        .and_then(|e| e.value().to_str().ok().map(|s| s.into_owned()))
        .map(|s| s.trim() == "YES");
    let fn_flag = obj
        .get(EXCESSIVE_FALSE_NEGATIVES)
        .and_then(|e| e.value().to_str().ok().map(|s| s.into_owned()))
        .map(|s| s.trim() == "YES");
    let fixation_method = extract_fixation_method(obj);

    let fp_quantity = read_us(obj, FALSE_POSITIVES_QUANTITY);
    let fp_catch_trials = read_us(obj, POSITIVE_CATCH_TRIALS_QUANTITY);
    let fn_quantity = read_us(obj, FALSE_NEGATIVES_QUANTITY);
    let fn_catch_trials = read_us(obj, NEGATIVE_CATCH_TRIALS_QUANTITY);
    let fixation_lost = read_us(obj, PATIENT_NOT_PROPERLY_FIXATED_QUANTITY);
    let fixation_checked = read_us(obj, FIXATION_CHECKED_QUANTITY);
    let stimuli_count = read_us(obj, NUMBER_OF_VISUAL_STIMULI);
    let stimuli_retested = read_us(obj, STIMULI_RETESTING_QUANTITY);

    let patient_age = read_str(obj, PATIENT_AGE).map(|a| parse_age(&a));
    let patient_sex = read_str(obj, PATIENT_SEX);
    let patient_birth_date = read_str(obj, PATIENT_BIRTH_DATE).map(|d| parse_date(&d));
    let manufacturer = read_str(obj, MANUFACTURER);
    let model_name = read_str(obj, MANUFACTURER_MODEL_NAME);
    let series_description = read_str(obj, SERIES_DESCRIPTION);
    let stimulus_size = read_fl(obj, STIMULUS_AREA).map(|a| goldmann_size(a).to_string());
    let background_luminance = read_fl(obj, BACKGROUND_LUMINANCE);
    let mean_sensitivity = read_fl(obj, VISUAL_FIELD_MEAN_SENSITIVITY);

    let test_strategy = extract_test_strategy(obj);
    let mut points = extract_vf_points(obj);

    // Mark blind spot using DICOM tags if present, otherwise fall back to anatomy.
    let bs_x = read_fl(obj, BLIND_SPOT_X_COORDINATE);
    let bs_y = read_fl(obj, BLIND_SPOT_Y_COORDINATE);
    for pt in &mut points {
        pt.is_blind_spot = match (bs_x, bs_y) {
            (Some(bx), Some(by)) => (pt.x - bx).abs() < 2.5 && (pt.y - by).abs() < 2.5,
            _ => is_blind_spot_anatomical(pt.x, pt.y, &laterality),
        };
    }

    // Fall back to heuristic if strategy not found in protocol sequence.
    let test_strategy = if test_strategy == TestStrategy::Threshold
        && !points.is_empty()
        && points.iter().all(|p| p.td.is_none() && p.pd.is_none())
    {
        // No TD/PD data present — treat as suprathreshold (unknown subtype).
        TestStrategy::TwoZone
    } else {
        test_strategy
    };

    DicomVfData {
        patient_name,
        study_date,
        laterality,
        horizontal_extent,
        foveal_sensitivity,
        test_duration_s,
        md,
        psd,
        false_neg,
        false_pos,
        fixation_loss,
        fp_flag,
        fn_flag,
        fixation_method,
        test_strategy,
        points,
        fp_quantity,
        fp_catch_trials,
        fn_quantity,
        fn_catch_trials,
        fixation_lost,
        fixation_checked,
        stimuli_count,
        stimuli_retested,
        patient_age,
        patient_sex,
        patient_birth_date,
        manufacturer,
        model_name,
        series_description,
        stimulus_size,
        background_luminance,
        mean_sensitivity,
    }
}
