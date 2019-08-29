use activitypub::activity::*;
use serde_json;

use crate::{
    comments::Comment,
    follows, likes,
    posts::{Post, PostUpdate},
    reshares::Reshare,
    users::User,
    Error,
};
use plume_common::activity_pub::inbox::Inbox;

macro_rules! impl_into_inbox_result {
    ( $( $t:ty => $variant:ident ),+ ) => {
        $(
            impl From<$t> for InboxResult {
                fn from(x: $t) -> InboxResult {
                    InboxResult::$variant(x)
                }
            }
        )+
    }
}

pub enum InboxResult {
    Commented(Comment),
    Followed(follows::Follow),
    Liked(likes::Like),
    Other,
    Post(Post),
    Reshared(Reshare),
}

impl From<()> for InboxResult {
    fn from(_: ()) -> InboxResult {
        InboxResult::Other
    }
}

impl_into_inbox_result! {
    Comment => Commented,
    follows::Follow => Followed,
    likes::Like => Liked,
    Post => Post,
    Reshare => Reshared
}

#[cfg(test)]
pub(crate) mod tests {
    use super::InboxResult;
    use crate::blogs::tests::fill_database as blog_fill_db;
    use crate::safe_string::SafeString;
    use diesel::Connection;

}
