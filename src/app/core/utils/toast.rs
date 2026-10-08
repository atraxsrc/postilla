// SPDX-License-Identifier: GPL-3.0

use cosmic::widget::Toast;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct PostillaToast {
    pub message: String,
}

impl PostillaToast {
    pub fn new<T>(message: T) -> Self
    where
        T: ToString,
    {
        Self {
            message: message.to_string(),
        }
    }
}

impl From<PostillaToast> for Toast<crate::app::Message> {
    fn from(toast: PostillaToast) -> Self {
        Toast::new(toast.message).duration(Duration::from_secs(5))
    }
}
