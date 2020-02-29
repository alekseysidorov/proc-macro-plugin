use serde::{Deserialize, Serialize};
use text_message_derive::TextMessage;

#[derive(Debug, Serialize, Deserialize, PartialEq, TextMessage)]
#[text_message(codec = "serde_json", params(pretty))]
struct FooMessage {
    name: String,
    description: String,
    value: u64,
}

fn main() {
    let msg = FooMessage {
        name: "Linus Torvalds".to_owned(),
        description: "The Linux founder.".to_owned(),
        value: 1,
    };

    let text = msg.to_string();

    println!("{}", text);

    let msg2 = text.parse().unwrap();

    assert_eq!(msg, msg2);
}
