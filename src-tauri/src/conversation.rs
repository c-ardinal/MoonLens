//! Face-to-face conversation: two people share one microphone and one
//! screen, each speaking their own language. Every utterance is
//! translated into the language of the other person.
//!
//! This module decides the direction of one utterance. Capture and the
//! recognition loop are those of the live subtitles (`subtitle`).

use serde::Serialize;

use crate::lang;
use crate::settings::schema::SubtitleSettings;
use crate::subtitle_revise::Languages;

/// Who said an utterance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Speaker {
    /// The user of this PC.
    Own,
    /// The person they talk to.
    Partner,
}

/// The two languages of a conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pair {
    /// The user's language: the target of the live subtitles
    /// (`subtitle.target_language`), which may be a way of writing one
    /// (easy Japanese).
    pub own: String,
    /// The language of the person they talk to.
    pub partner: String,
}

/// One utterance: who spoke, and what it is translated from and into.
#[derive(Debug, Clone, PartialEq)]
pub struct Turn {
    pub speaker: Speaker,
    pub languages: Languages,
}

impl Pair {
    /// The pair the settings describe. A partner language equal to the
    /// user's own leaves nothing to translate, so the conversation then
    /// runs against English (Japanese for an English-speaking user).
    pub fn from_settings(s: &SubtitleSettings) -> Self {
        let own = s.target_language.clone();
        let own_spoken = lang::primary(&own);
        let partner = s.conversation_language.trim();
        let partner = if partner.is_empty() || lang::primary(partner) == own_spoken {
            if own_spoken == "en" { "ja" } else { "en" }.to_string()
        } else {
            partner.to_string()
        };
        Self { own, partner }
    }

    /// The languages speech recognition chooses between, as primary
    /// tags (`zh-Hans` → `zh`): the user's first.
    pub fn candidates(&self) -> Vec<String> {
        vec![lang::primary(&self.own), lang::primary(&self.partner)]
    }

    /// Who said `text`. `reported`: the language speech recognition
    /// decoded it in, when the engine tells. Without it the writing
    /// system of `text` decides, which cannot tell two languages of one
    /// script apart: such speech counts as the partner's.
    pub fn turn(&self, reported: Option<&str>, text: &str) -> Turn {
        let own_spoken = lang::primary(&self.own);
        let spoken = reported
            .map(lang::primary)
            .filter(|l| !l.is_empty())
            .or_else(|| lang::detect(text).map(str::to_string));
        if spoken.as_deref() == Some(own_spoken.as_str()) {
            Turn {
                speaker: Speaker::Own,
                languages: Languages {
                    source: own_spoken,
                    target: self.partner.clone(),
                },
            }
        } else {
            Turn {
                speaker: Speaker::Partner,
                languages: Languages {
                    source: self.partner.clone(),
                    target: self.own.clone(),
                },
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(own: &str, partner: &str) -> Pair {
        let s = SubtitleSettings {
            target_language: own.into(),
            conversation_language: partner.into(),
            ..Default::default()
        };
        Pair::from_settings(&s)
    }

    fn languages(source: &str, target: &str) -> Languages {
        Languages {
            source: source.into(),
            target: target.into(),
        }
    }

    #[test]
    fn each_side_is_translated_into_the_language_of_the_other() {
        let p = pair("ja", "en");
        let own = p.turn(Some("ja"), "駅はどこですか。");
        assert_eq!(own.speaker, Speaker::Own);
        assert_eq!(own.languages, languages("ja", "en"));
        let partner = p.turn(Some("en"), "It is over there.");
        assert_eq!(partner.speaker, Speaker::Partner);
        assert_eq!(partner.languages, languages("en", "ja"));
    }

    #[test]
    fn regional_tags_are_compared_by_their_language() {
        let p = pair("ja", "zh-Hans");
        assert_eq!(p.candidates(), ["ja", "zh"]);
        let partner = p.turn(Some("zh"), "车站在哪里？");
        assert_eq!(partner.speaker, Speaker::Partner);
        // The translator is told the full tag.
        assert_eq!(partner.languages, languages("zh-Hans", "ja"));
    }

    #[test]
    fn easy_japanese_is_what_the_user_reads_not_what_they_speak() {
        let p = pair("ja-easy", "en");
        assert_eq!(p.candidates(), ["ja", "en"]);
        assert_eq!(
            p.turn(Some("en"), "Hello.").languages,
            languages("en", "ja-easy")
        );
        assert_eq!(
            p.turn(Some("ja"), "こんにちは。").languages,
            languages("ja", "en")
        );
    }

    #[test]
    fn without_a_reported_language_the_writing_system_decides() {
        let p = pair("ja", "en");
        assert_eq!(p.turn(None, "駅はどこですか。").speaker, Speaker::Own);
        assert_eq!(
            p.turn(None, "Where is the station?").speaker,
            Speaker::Partner
        );
        // Too short to tell: translated for the user.
        assert_eq!(p.turn(None, "OK").speaker, Speaker::Partner);
    }

    #[test]
    fn a_partner_language_equal_to_the_users_own_falls_back() {
        assert_eq!(pair("ja", "ja").partner, "en");
        assert_eq!(pair("ja-easy", "ja").partner, "en");
        assert_eq!(pair("en", "en").partner, "ja");
        assert_eq!(pair("ko", "").partner, "en");
        assert_eq!(pair("ko", "fr").partner, "fr");
    }
}
