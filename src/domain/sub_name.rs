// An extension trait to provide the `graphemes` method
// on `String` and `&str`
use unicode_segmentation::UnicodeSegmentation;

#[derive(Debug)]
pub struct SubscriberName(String);

//if an instance of SubscriberName exists somewhere in the code, it is guaranteed to contain a valid name
// 1 you can only create one by going through the below pub fn parse (coz String inside of it is private)
// 2 that function only returns an instance if validation checks pass
//this means we don't need to re-do the checks anywhere else in our code!
//in essence we're using Rust's type system to enforce the checks we want on name - it is physically impossible to have a name instance floating around the app that doesn't comply anymore
//= "type driven development" = when you use the language's type system to model a domain
//this means we've moved the error from the land of responsibility of engineers to the land of responsibility of the compiler
impl SubscriberName {
    pub fn parse(name: String) -> Result<Self, String> {
        if !is_valid_name(&name) {
            Err(String::from("bad name"))
        } else {
            Ok(Self(name))
        }
    }
}

impl AsRef<str> for SubscriberName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

fn is_valid_name(name: &str) -> bool {
    //check not empty
    let is_empty = name.trim().is_empty();
    //check < 256 chars
    let is_too_long = name.graphemes(true).count() > 256;
    //check doesn't contain bad chars
    let forbidden_chars = ['/', '(', ')', '"', '<', '>', '\\', '{', '}'];
    let has_bad_chars = name.chars().any(|c| forbidden_chars.contains(&c));
    !(is_empty || is_too_long || has_bad_chars)
}

//these are unit tests
//while in tests folder we have the integration tests
#[cfg(test)]
mod tests {
    use crate::domain::SubscriberName;
    use claim::{assert_err, assert_ok};

    #[test]
    fn a_256_grapheme_name_is_valid() {
        let name = "a".repeat(256);
        assert_ok!(SubscriberName::parse(name));
    }

    #[test]
    fn a_name_longer_than_256_is_rejected() {
        let name = "a".repeat(257);
        assert_err!(SubscriberName::parse(name));
    }

    #[test]
    fn whitespace_only_names_are_rejected() {
        let name = String::from(" ");
        assert_err!(SubscriberName::parse(name));
    }

    #[test]
    fn empty_string_is_rejected() {
        let name = String::new();
        assert_err!(SubscriberName::parse(name));
    }

    #[test]
    fn names_w_bad_char_rejected() {
        for name in &['/', '(', ')', '"', '<', '>', '\\', '{', '}'] {
            let name = name.to_string();
            assert_err!(SubscriberName::parse(name));
        }
    }
}
