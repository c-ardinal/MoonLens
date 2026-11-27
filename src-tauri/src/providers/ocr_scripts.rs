//! Recognition models for writing systems the standard model does not
//! read: Korean, Cyrillic, Arabic, Thai, Devanagari, Tamil and Telugu.
//!
//! The standard recognition model of [`super::ocr_ppocr`] reads
//! Japanese, Chinese and Latin script. A line in another script comes
//! out of it empty, or as the digits and punctuation it happens to
//! contain. Each script here has a small recognition model of its own
//! (PP-OCRv5 mobile, Apache-2.0, published by PaddlePaddle), fetched on
//! request and checked against a pinned SHA-256. Detection is shared:
//! the detector finds lines of every script.
//!
//! Reading every line once per installed model would multiply the
//! reading time, so a script model sees a line only when there is a
//! reason to doubt the standard reading ([`doubtful`]), or when its
//! script is already known to be in the picture. Its reading replaces
//! the standard one only where it is the better of the two
//! ([`reads_better`]). The rules were set on rendered pages of every
//! script: none of the Japanese, Chinese and English lines was taken
//! over by a script model, with all seven installed.

use std::path::Path;

use super::ocr_onnx::Remote;

/// A writing system with a recognition model of its own.
#[derive(Debug)]
pub struct Script {
    /// Name in the settings, the log and the IPC.
    pub id: &'static str,
    pub model: Remote,
    /// Download size.
    pub bytes: u64,
    /// One character per line, in the model's class order.
    dict: &'static str,
    /// Code point ranges of the script's own letters.
    ranges: &'static [(u32, u32)],
    /// Written right to left: the model reads such a line in the order
    /// the characters appear from left to right.
    rtl: bool,
}

macro_rules! script {
    ($id:literal, $repo:literal, $commit:literal, $sha256:literal, $bytes:literal, $ranges:expr, $rtl:literal) => {
        Script {
            id: $id,
            model: Remote {
                url: concat!(
                    "https://huggingface.co/PaddlePaddle/",
                    $repo,
                    "_PP-OCRv5_mobile_rec_onnx/resolve/",
                    $commit,
                    "/inference.onnx"
                ),
                sha256: $sha256,
                file: concat!("ppocrv5-rec-", $id, ".onnx"),
            },
            bytes: $bytes,
            dict: include_str!(concat!("ocr_dicts/", $id, ".txt")),
            ranges: $ranges,
            rtl: $rtl,
        }
    };
}

/// Every script on offer, each model pinned to a commit of the
/// publisher's repository.
pub static SCRIPTS: [Script; 7] = [
    script!(
        "korean",
        "korean",
        "5c6f574b8e2230adf4287b33e736d71b9fabd28e",
        "92f0b7785e64fc9090106a241cf4c1eb97472824558272751b88a2a4476d3a08",
        13_418_787,
        &[(0xAC00, 0xD7AF), (0x1100, 0x11FF), (0x3130, 0x318F)],
        false
    ),
    script!(
        "cyrillic",
        "cyrillic",
        "2cef88145434beb8afa9dd82d77d799eb1ad7b29",
        "5371ee1ddaa7983cc62d0818d99e982b6804638c85e4f960d59a574094e172e5",
        8_048_799,
        &[(0x0400, 0x052F)],
        false
    ),
    script!(
        "arabic",
        "arabic",
        "14aaedcd75825982689ecf5cd64ab33ee083215a",
        "799113ebf267fbe742deb99eb36e8d42c9ddc5291ceacf92add41b4d52a59110",
        7_998_947,
        &[
            (0x0600, 0x06FF),
            (0x0750, 0x077F),
            (0xFB50, 0xFDFF),
            (0xFE70, 0xFEFF)
        ],
        true
    ),
    script!(
        "thai",
        "th",
        "1d4adbbafb1034a2fd6618498575b81ea7b69f69",
        "27618be66018f8598ac0a526a593f9f1cebf794e7eded93428e8fb016e537f5f",
        7_891_015,
        &[(0x0E00, 0x0E7F)],
        false
    ),
    script!(
        "devanagari",
        "devanagari",
        "251aec19e36739540d35e2cc943f6aa7503b98e5",
        "cb789212ce96c69d3e74728ae4309d179281d68cb3945d0616b67cafab41c986",
        7_912_311,
        &[(0x0900, 0x097F)],
        false
    ),
    script!(
        "tamil",
        "ta",
        "0db89798af0d5218fad9ea7ace0557526e9077f1",
        "c6d2b682d2a0ea4cb1fccdba295976f93fd439964d16cdc666cadef531accbee",
        7_885_691,
        &[(0x0B80, 0x0BFF)],
        false
    ),
    script!(
        "telugu",
        "te",
        "f1197c83430241c58c64b0952b3a80366d04eeba",
        "8238bfc46d4cffe720ed6706e3842802467343497428693ff2bfb4e6b3caa36b",
        7_898_759,
        &[(0x0C00, 0x0C7F)],
        false
    ),
];

pub fn by_id(id: &str) -> Option<&'static Script> {
    SCRIPTS.iter().find(|s| s.id == id)
}

/// The scripts whose model is in `models_dir`.
pub fn installed(models_dir: &Path) -> Vec<&'static Script> {
    SCRIPTS
        .iter()
        .filter(|s| s.model.present(models_dir))
        .collect()
}

/// Delete the model of `script` from `models_dir`. A model that is not
/// there is not an error.
pub fn remove(models_dir: &Path, script: &Script) -> std::io::Result<()> {
    match std::fs::remove_file(models_dir.join(script.model.file)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
        _ => Ok(()),
    }
}

/// The lines of a dictionary file. Nothing is trimmed but the line
/// end: a line can hold a character that is blank space.
pub fn dict_lines(text: &str) -> Vec<String> {
    text.lines()
        .map(|l| l.trim_end_matches('\r').to_string())
        .collect()
}

impl Script {
    pub fn dict(&self) -> Vec<String> {
        dict_lines(self.dict)
    }

    fn has(&self, c: char) -> bool {
        let u = c as u32;
        self.ranges.iter().any(|&(a, b)| (a..=b).contains(&u))
    }

    /// Whether `text` is written in this script: it has letters of the
    /// script, and no more letters of other scripts than of this one.
    /// Every script model also knows Latin letters and reads a line of
    /// another script as a jumble of them; such a reading is not its
    /// own.
    pub fn owns(&self, text: &str) -> bool {
        let own = text.chars().filter(|&c| self.has(c)).count();
        let other = text
            .chars()
            .filter(|&c| c.is_alphabetic() && !self.has(c))
            .count();
        own > 0 && own >= other
    }

    /// `text` as the model read it, in the order it is written.
    ///
    /// The model of a right-to-left script reads the characters from
    /// the left of the picture: the line's last character first.
    /// Numbers and Latin words inside the line run left to right and
    /// keep their order (the rule PaddleOCR applies to Arabic, with
    /// the comma added so that `1,024` stays one number).
    pub fn in_writing_order(&self, text: String) -> String {
        if !self.rtl {
            return text;
        }
        let ltr = |c: char| c.is_ascii_alphanumeric() || " :*.,/%+-".contains(c);
        let mut runs: Vec<String> = Vec::new();
        let mut run = String::new();
        for c in text.chars() {
            if ltr(c) {
                run.push(c);
            } else {
                if !run.is_empty() {
                    runs.push(std::mem::take(&mut run));
                }
                runs.push(c.to_string());
            }
        }
        if !run.is_empty() {
            runs.push(run);
        }
        runs.into_iter()
            .rev()
            .collect::<String>()
            .trim()
            .to_string()
    }
}

/// Below this confidence the standard reading of a line is in doubt.
/// On the measured pages the standard model read its own scripts at
/// 0.89 or more.
const DOUBT_BELOW: f32 = 0.85;

/// A script model's reading counts from this confidence on. A model
/// reading its own script stayed above 0.87, one reading another
/// script's line as its own below 0.6.
const ACCEPT_FROM: f32 = 0.8;

fn written(text: &str) -> usize {
    text.chars().filter(|c| !c.is_whitespace()).count()
}

/// Whether the standard reading (`text`, `confidence`) of a line
/// picture `width` × `height` leaves room for the line being in
/// another script: it is unsure, or too short for the line's length.
/// Even the widest characters fill a line with one per line height;
/// far fewer means most of the line was not read.
pub fn doubtful(text: &str, confidence: f32, width: u32, height: u32) -> bool {
    confidence < DOUBT_BELOW || written(text) * 2 * (height.max(1) as usize) < width as usize
}

/// Whether `other`, the reading of a line by the model of `script`, is
/// to replace `standard`, the reading by the standard model. Both are
/// `(text, confidence)`.
///
/// The script model must be sure and the text its own. Then it wins
/// where the standard model was unsure, or read clearly less of the
/// line: a line of Korean with a number in it comes out of the
/// standard model as the number alone, read with full confidence.
pub fn reads_better(script: &Script, standard: (&str, f32), other: (&str, f32)) -> bool {
    other.1 >= ACCEPT_FROM
        && script.owns(other.0)
        && (standard.1 < DOUBT_BELOW || written(standard.0) * 20 < written(other.0) * 17)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(id: &str) -> &'static Script {
        by_id(id).unwrap()
    }

    #[test]
    fn every_dictionary_is_whole() {
        // Classes of each model minus the blank and the space.
        let sizes = [11945, 850, 747, 524, 568, 513, 540];
        for (s, n) in SCRIPTS.iter().zip(sizes) {
            let dict = s.dict();
            assert_eq!(dict.len(), n, "{}", s.id);
            assert!(
                dict.iter().all(|l| l.chars().count() == 1),
                "{}: a line is not one character",
                s.id
            );
            assert!(dict.iter().any(|l| s.has(l.chars().next().unwrap())));
        }
    }

    #[test]
    fn ids_and_files_are_distinct() {
        for (i, a) in SCRIPTS.iter().enumerate() {
            assert_eq!(by_id(a.id).unwrap().id, a.id);
            assert_eq!(a.model.sha256.len(), 64);
            for b in &SCRIPTS[i + 1..] {
                assert_ne!(a.id, b.id);
                assert_ne!(a.model.file, b.model.file);
                assert_ne!(a.model.sha256, b.model.sha256);
            }
        }
        assert!(by_id("latin").is_none());
    }

    #[test]
    fn a_script_owns_text_mostly_in_its_letters() {
        let ko = script("korean");
        assert!(ko.owns("다운로드 완료: 1,024 MB / 2.4 GB"));
        assert!(ko.owns("안녕하세요"));
        // What the Korean model makes of a Russian line.
        assert!(!ko.owns("HacTpoükn coxpaHeHbl"));
        assert!(!ko.owns("2,450 310"));
        assert!(!ko.owns(""));
        let ru = script("cyrillic");
        assert!(ru.owns("Настройки сохранены."));
        assert!(!ru.owns("Settings saved."));
        assert!(script("thai").owns("บันทึกการตั้งค่าแล้ว"));
        assert!(script("devanagari").owns("डाउनलोड पूरा हुआ"));
    }

    #[test]
    fn right_to_left_lines_are_put_in_writing_order() {
        let ar = script("arabic");
        // As read from the left of the picture.
        assert_eq!(ar.in_writing_order("ابحرم".into()), "مرحبا");
        // A number inside the line keeps its digits in order.
        assert_eq!(ar.in_writing_order("ج 1,024 ب".into()), "ب 1,024 ج");
        let ko = script("korean");
        assert_eq!(ko.in_writing_order("안녕 12".into()), "안녕 12");
    }

    #[test]
    fn an_unsure_or_sparse_reading_is_doubtful() {
        // A line 40 line heights long read as two marks, with confidence.
        assert!(doubtful(", .", 0.95, 1920, 48));
        assert!(doubtful("", 0.0, 400, 48));
        assert!(doubtful("設定", 0.6, 96, 48));
        assert!(!doubtful("設定を保存しました", 0.99, 420, 48));
        assert!(!doubtful("OK", 0.99, 60, 48));
    }

    #[test]
    fn a_script_reading_wins_only_where_the_standard_one_falls_short() {
        let ko = script("korean");
        // The standard model read the number alone, sure of itself.
        assert!(reads_better(
            ko,
            (": 1,024 MB / 2.4 GB", 0.97),
            ("다운로드 완료:1,024MB/2.4 GB", 0.98)
        ));
        assert!(reads_better(ko, ("", 0.0), ("안녕하세요", 0.95)));
        // Not sure enough.
        assert!(!reads_better(ko, ("", 0.0), ("안녕하세요", 0.7)));
        // Not its own script.
        assert!(!reads_better(ko, ("", 0.0), ("OAHax_bl CeBepHbiú", 0.87)));
        // The standard model read the line in full: it stands, even
        // where a look-alike reading exists.
        let ru = script("cyrillic");
        assert!(!reads_better(ru, ("CHEF", 0.99), ("СНЕГ", 0.95)));
        assert!(reads_better(ru, ("« »", 0.79), ("«Корона»", 0.99)));
    }

    #[test]
    fn installed_scripts_are_those_with_a_model_file() {
        let dir = tempfile::tempdir().unwrap();
        assert!(installed(dir.path()).is_empty());
        let th = script("thai");
        std::fs::write(dir.path().join(th.model.file), b"model").unwrap();
        let ids: Vec<_> = installed(dir.path()).iter().map(|s| s.id).collect();
        assert_eq!(ids, ["thai"]);
        remove(dir.path(), th).unwrap();
        remove(dir.path(), th).unwrap();
        assert!(installed(dir.path()).is_empty());
    }
}
