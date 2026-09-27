#![forbid(unsafe_code)]

//! The contract route technology — a technology of `xmip-core-route`.
//!
//! A Subscription's filter names properties, and each property is read from
//! one source. This source names the contract the content is bound to and the
//! type it announces: `contract:name` is the contract the first section is
//! bound to; `contract:section:<n>` the contract of section `<n>`, counted
//! from zero as the content selector counts `orders[0]`; and `contract:type`
//! the type the content announced when it took its shape, read from the
//! context key `MessageType` — the property routing has filtered on since the
//! capability's first example, and the key a promotion is to write the type
//! under; nothing in the estate writes it yet, and this sentence is the record
//! of that convention. A section bound to no contract, a section number the
//! Message does not reach, and a type nothing announced all promote nothing,
//! so a filter over them declines with its reason. A name outside the three,
//! or a section number that is not one, is an error naming the three.
//! ADR-0046.
//!
//! A route technology does not decide anything: it reads.

use context::ContextValue;
use message::Message;
use path::Content;
use route::{Reading, Source};

/// The manifest leaf and the prefix a property carries.
pub const TECHNOLOGY: &str = "contract";

/// The context key the announced type is promoted under.
pub const TYPE_KEY: &str = "MessageType";

/// The names this technology reads, in the order the crate documentation
/// gives them.
pub const NAMES: [&str; 3] = ["name", "section:<n>", "type"];

/// Reads `contract:name`, `contract:section:<n>` and `contract:type`.
pub struct ContractSource;

impl Source for ContractSource {
    fn technology(&self) -> &'static str {
        TECHNOLOGY
    }

    fn compile(&self, name: &str) -> Result<Box<dyn Reading>, String> {
        let binding = match name.split_once(':') {
            None if name == "name" => Binding::Section(0),
            None if name == "type" => Binding::Type,
            Some(("section", number)) => Binding::Section(number.parse().map_err(|_| {
                format!("{number} is not a section number, which counts from zero")
            })?),
            _ => {
                return Err(format!(
                    "not a thing a contract binding says; the names are {}",
                    NAMES.join(", ")
                ));
            }
        };
        Ok(Box::new(binding))
    }
}

/// What a name asks of the binding, decided when compiled.
enum Binding {
    /// The contract section `n` is bound to.
    Section(usize),
    /// The type the content announced.
    Type,
}

impl Reading for Binding {
    fn read(&self, message: &Message, _: Option<&Content<'_>>) -> Result<Option<String>, String> {
        match self {
            Self::Section(index) => Ok(message
                .sections()
                .get(*index)
                .and_then(|section| section.contract.clone())),
            Self::Type => match message.context().get(TYPE_KEY) {
                None | Some(ContextValue::Null) => Ok(None),
                Some(ContextValue::Text(text)) => Ok(Some(text.clone())),
                Some(other) => Err(format!("{TYPE_KEY} is {other:?}, and a type is text")),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use context::MessageContext;
    use message::{MessageSection, MessageTreatment};
    use route::{Gathering, Promoted, SourceError};
    use stream::Stream;
    use xcore::{MessageId, SectionId, StreamId};

    fn section(id: u128, contract: Option<&str>) -> MessageSection {
        MessageSection {
            section_id: SectionId::new(id),
            name: None,
            stream: Stream::new(StreamId::new(id), b"<order/>".to_vec(), None),
            contract: contract.map(str::to_string),
        }
    }

    fn message() -> Message {
        Message::received(
            MessageId::new(1),
            vec![section(10, Some("Order.v2")), section(11, None)],
            MessageContext::new().with_value(TYPE_KEY, ContextValue::Text("Order".into())),
            MessageTreatment::default(),
        )
    }

    fn promote(message: &Message, properties: &[&str]) -> Result<Promoted, SourceError> {
        Gathering::new(&[&ContractSource], properties).promote(message)
    }

    fn read_from(message: &Message, name: &str) -> Result<Option<String>, SourceError> {
        let property = format!("contract:{name}");
        Ok(promote(message, &[property.as_str()])?
            .get(&property)
            .map(str::to_string))
    }

    fn read(name: &str) -> Result<Option<String>, SourceError> {
        read_from(&message(), name)
    }

    #[test]
    fn the_name_is_the_first_sections_contract_and_a_section_is_counted_from_zero() {
        assert_eq!(read("name").expect("bound"), Some("Order.v2".into()));
        assert_eq!(read("section:0").expect("bound"), Some("Order.v2".into()));
        assert_eq!(read("section:1").expect("unbound"), None);
        assert_eq!(read("section:7").expect("no such section"), None);
    }

    #[test]
    fn the_type_is_what_the_content_announced_and_nothing_announced_is_nothing() {
        assert_eq!(read("type").expect("announced"), Some("Order".into()));

        let silent = Message::received(
            MessageId::new(2),
            Vec::new(),
            MessageContext::new(),
            MessageTreatment::default(),
        );
        assert_eq!(read_from(&silent, "type").expect("silent"), None);
        assert_eq!(read_from(&silent, "name").expect("no section"), None);
    }

    #[test]
    fn a_name_outside_the_three_and_a_section_that_is_not_a_number_are_refused() {
        let refused = read("version").expect_err("refused");
        assert_eq!(refused.technology, "contract");
        assert_eq!(refused.property, "version");
        assert!(refused.reason.contains("name, section:<n>, type"));

        let number = read("section:first").expect_err("not a number");
        assert!(number.reason.contains("first is not a section number"));

        let typed = Message::received(
            MessageId::new(3),
            Vec::new(),
            MessageContext::new().with_value(TYPE_KEY, ContextValue::Integer(4)),
            MessageTreatment::default(),
        );
        let not_text = read_from(&typed, "type").expect_err("not text");
        assert!(not_text.reason.contains("a type is text"));
    }

    #[test]
    fn the_technology_is_contract_and_promote_reads_the_prefixed_property() {
        assert_eq!(ContractSource.technology(), "contract");

        let promoted = promote(
            &message(),
            &["contract:name", "contract:section:1", "contract:type"],
        )
        .expect("readable");

        assert_eq!(promoted.get("contract:name"), Some("Order.v2"));
        assert_eq!(promoted.get("contract:section:1"), None);
        assert!(
            path::expression::Expression::parse(
                "contract:type = 'Order' and contract:name like 'Order.%'"
            )
            .expect("compiles")
            .evaluate(&promoted)
            .holds()
        );
    }
}
