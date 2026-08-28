use extendr_api::prelude::*;

pub enum RobjContainer {
    DataFrame { robj: Robj },
    Matrix { robj: Robj },
    List { robj: Robj },
    Vector { robj: Robj },
}

impl RobjContainer {
    pub fn robj(&self) -> &Robj {
        match self {
            RobjContainer::DataFrame { robj }
            | RobjContainer::Matrix { robj }
            | RobjContainer::List { robj }
            | RobjContainer::Vector { robj } => robj,
        }
    }
}

#[derive(Debug)]
pub struct UnsupportedTypeError {
    rtype: String,
}

impl std::fmt::Display for UnsupportedTypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Cannot view object of type `{}`; expected a data.frame, matrix, list, or vector",
            self.rtype
        )
    }
}
impl std::error::Error for UnsupportedTypeError {}

pub fn classify(x: Robj) -> Result<RobjContainer, UnsupportedTypeError> {
    if x.is_frame() {
        Ok(RobjContainer::DataFrame { robj: x })
    } else if x.is_matrix() {
        Ok(RobjContainer::Matrix { robj: x })
    } else if x.get_attrib("dim").is_some() {
        Err(UnsupportedTypeError {
            rtype: r_class_name(&x),
        })
    } else if x.is_list() {
        Ok(RobjContainer::List { robj: x })
    } else if x.is_vector_atomic() {
        Ok(RobjContainer::Vector { robj: x })
    } else {
        Err(UnsupportedTypeError {
            rtype: r_class_name(&x),
        })
    }
}

fn r_class_name(x: &Robj) -> String {
    match call!("class", x) {
        Ok(r) => r
            .as_str_vector()
            .and_then(|v| v.first().map(|s| s.to_string()))
            .unwrap_or_else(|| format!("{:?}", x.rtype())),
        Err(_) => format!("{:?}", x.rtype()),
    }
}
