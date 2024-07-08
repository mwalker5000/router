use std::ops::Deref;

use apollo_compiler::ast::Value;
use apollo_compiler::schema::Directive;
use apollo_compiler::Name;
use apollo_compiler::Node;

use crate::error::FederationError;
use crate::error::SingleFederationError;
use crate::link::graphql_definition::BooleanOrVariable;

pub(crate) fn directive_optional_enum_argument(
    application: &Node<Directive>,
    name: &Name,
) -> Result<Option<Name>, FederationError> {
    match application.argument_by_name(name) {
        Some(value) => match value.deref() {
            Value::Enum(name) => Ok(Some(name.clone())),
            Value::Null => Ok(None),
            _ => Err(SingleFederationError::Internal {
                message: format!(
                    "Argument \"{}\" of directive \"@{}\" must be an enum value.",
                    name, application.name
                ),
            }
            .into()),
        },
        None => Ok(None),
    }
}

pub(crate) fn directive_required_enum_argument(
    application: &Node<Directive>,
    name: &Name,
) -> Result<Name, FederationError> {
    directive_optional_enum_argument(application, name)?.ok_or_else(|| {
        SingleFederationError::Internal {
            message: format!(
                "Required argument \"{}\" of directive \"@{}\" was not present.",
                name, application.name
            ),
        }
        .into()
    })
}

pub(crate) fn directive_optional_string_argument<'doc>(
    application: &'doc Node<Directive>,
    name: &Name,
) -> Result<Option<&'doc str>, FederationError> {
    match application.argument_by_name(name) {
        Some(value) => match value.deref() {
            Value::String(name) => Ok(Some(name)),
            Value::Null => Ok(None),
            _ => Err(SingleFederationError::Internal {
                message: format!(
                    "Argument \"{}\" of directive \"@{}\" must be a string.",
                    name, application.name
                ),
            }
            .into()),
        },
        None => Ok(None),
    }
}

pub(crate) fn directive_required_string_argument<'doc>(
    application: &'doc Node<Directive>,
    name: &Name,
) -> Result<&'doc str, FederationError> {
    directive_optional_string_argument(application, name)?.ok_or_else(|| {
        SingleFederationError::Internal {
            message: format!(
                "Required argument \"{}\" of directive \"@{}\" was not present.",
                name, application.name
            ),
        }
        .into()
    })
}

pub(crate) fn directive_optional_boolean_argument(
    application: &Node<Directive>,
    name: &Name,
) -> Result<Option<bool>, FederationError> {
    match application.argument_by_name(name) {
        Some(value) => match value.deref() {
            Value::Boolean(value) => Ok(Some(*value)),
            Value::Null => Ok(None),
            _ => Err(SingleFederationError::Internal {
                message: format!(
                    "Argument \"{}\" of directive \"@{}\" must be a boolean.",
                    name, application.name
                ),
            }
            .into()),
        },
        None => Ok(None),
    }
}

#[allow(dead_code)]
pub(crate) fn directive_required_boolean_argument(
    application: &Node<Directive>,
    name: &Name,
) -> Result<bool, FederationError> {
    directive_optional_boolean_argument(application, name)?.ok_or_else(|| {
        SingleFederationError::Internal {
            message: format!(
                "Required argument \"{}\" of directive \"@{}\" was not present.",
                name, application.name
            ),
        }
        .into()
    })
}

pub(crate) fn directive_optional_variable_boolean_argument(
    application: &Node<Directive>,
    name: &Name,
) -> Result<Option<BooleanOrVariable>, FederationError> {
    match application.argument_by_name(name) {
        Some(value) => match value.deref() {
            Value::Variable(name) => Ok(Some(BooleanOrVariable::Variable(name.clone()))),
            Value::Boolean(value) => Ok(Some(BooleanOrVariable::Boolean(*value))),
            Value::Null => Ok(None),
            _ => Err(FederationError::internal(format!(
                "Argument \"{}\" of directive \"@{}\" must be a boolean.",
                name, application.name
            ))),
        },
        None => Ok(None),
    }
}

pub(crate) fn directive_optional_list_argument<'doc>(
    application: &'doc Node<Directive>,
    name: &Name,
) -> Result<Option<&'doc Vec<Node<Value>>>, FederationError> {
    match application.argument_by_name(name) {
        Some(value) => match value.deref() {
            Value::List(items) => Ok(Some(items)),
            _ => Err(FederationError::internal(format!(
                "Argument \"{}\" of directive \"@{}\" must be an object.",
                name, application.name
            ))),
        },
        None => Ok(None),
    }
}

pub(crate) fn directive_required_list_argument<'doc>(
    application: &'doc Node<Directive>,
    name: &Name,
) -> Result<&'doc Vec<Node<Value>>, FederationError> {
    directive_optional_list_argument(application, name)?.ok_or_else(|| {
        SingleFederationError::Internal {
            message: format!(
                "Required argument \"{}\" of directive \"@{}\" was not present.",
                name, application.name
            ),
        }
        .into()
    })
}

pub(crate) fn directive_optional_object_argument<'doc>(
    application: &'doc Node<Directive>,
    name: &Name,
) -> Result<Option<&'doc Vec<(Name, Node<Value>)>>, FederationError> {
    match application.argument_by_name(name) {
        Some(value) => match value.deref() {
            Value::Object(fields) => Ok(Some(fields)),
            _ => Err(FederationError::internal(format!(
                "Argument \"{}\" of directive \"@{}\" must be an object.",
                name, application.name
            ))),
        },
        None => Ok(None),
    }
}

pub(crate) fn directive_required_object_argument<'doc>(
    application: &'doc Node<Directive>,
    name: &Name,
) -> Result<&'doc Vec<(Name, Node<Value>)>, FederationError> {
    directive_optional_object_argument(application, name)?.ok_or_else(|| {
        SingleFederationError::Internal {
            message: format!(
                "Required argument \"{}\" of directive \"@{}\" was not present.",
                name, application.name
            ),
        }
        .into()
    })
}
