use activitypub::{
    activity::{Create, Delete},
    link,
    object::{Note, Tombstone},
};
use chrono::{self, NaiveDateTime};
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl, SaveChangesDsl};
use serde_json;

use std::collections::HashSet;

use comment_seers::{CommentSeers, NewCommentSeers};
use instance::Instance;
use medias::Media;
use mentions::Mention;
use notifications::*;
use plume_common::activity_pub::{
    inbox::{AsActor, AsObject, FromId},
    Id, IntoId, PUBLIC_VISIBILITY,
};
use plume_common::utils;
use posts::Post;
use safe_string::SafeString;
use schema::comments;
use users::User;
use {Connection, Error, Result};

#[derive(Queryable, Identifiable, Clone, AsChangeset)]
pub struct Comment {
    pub id: i32,
    pub content: SafeString,
    pub in_response_to_id: Option<i32>,
    pub post_id: i32,
    pub author_id: i32,
    pub creation_date: NaiveDateTime,
    pub ap_url: Option<String>,
    pub sensitive: bool,
    pub spoiler_text: String,
    pub public_visibility: bool,
}

#[derive(Insertable, Default)]
#[table_name = "comments"]
pub struct NewComment {
    pub content: SafeString,
    pub in_response_to_id: Option<i32>,
    pub post_id: i32,
    pub author_id: i32,
    pub ap_url: Option<String>,
    pub sensitive: bool,
    pub spoiler_text: String,
    pub public_visibility: bool,
}

impl Comment {
    insert!(comments, NewComment, |inserted, conn| {
        if inserted.ap_url.is_none() {
            inserted.ap_url = Some(format!(
                "{}comment/{}",
                inserted.get_post(conn)?.ap_url,
                inserted.id
            ));
            let _: Comment = inserted.save_changes(conn)?;
        }
        Ok(inserted)
    });
    get!(comments);
    list_by!(comments, list_by_post, post_id as i32);
    find_by!(comments, find_by_ap_url, ap_url as &str);

    pub fn get_author(&self, conn: &Connection) -> Result<User> {
        User::get(conn, self.author_id)
    }

    pub fn get_post(&self, conn: &Connection) -> Result<Post> {
        Post::get(conn, self.post_id)
    }

    pub fn count_local(conn: &Connection) -> Result<i64> {
        use schema::users;
        let local_authors = users::table
            .filter(users::instance_id.eq(Instance::get_local()?.id))
            .select(users::id);
        comments::table
            .filter(comments::author_id.eq_any(local_authors))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn get_responses(&self, conn: &Connection) -> Result<Vec<Comment>> {
        comments::table
            .filter(comments::in_response_to_id.eq(self.id))
            .load::<Comment>(conn)
            .map_err(Error::from)
    }

    pub fn can_see(&self, conn: &Connection, user: Option<&User>) -> bool {
        self.public_visibility
            || user
                .as_ref()
                .map(|u| CommentSeers::can_see(conn, self, u).unwrap_or(false))
                .unwrap_or(false)
    }

    pub fn build_delete(&self, conn: &Connection) -> Result<Delete> {
        let mut act = Delete::default();
        act.delete_props
            .set_actor_link(self.get_author(conn)?.into_id())?;

        let mut tombstone = Tombstone::default();
        tombstone.object_props.set_id_string(self.ap_url.clone()?)?;
        act.delete_props.set_object_object(tombstone)?;

        act.object_props
            .set_id_string(format!("{}#delete", self.ap_url.clone().unwrap()))?;
        act.object_props
            .set_to_link_vec(vec![Id::new(PUBLIC_VISIBILITY)])?;

        Ok(act)
    }
}

pub struct CommentTree {
    pub comment: Comment,
    pub responses: Vec<CommentTree>,
}

impl CommentTree {
    pub fn from_post(conn: &Connection, p: &Post, user: Option<&User>) -> Result<Vec<Self>> {
        Ok(Comment::list_by_post(conn, p.id)?
            .into_iter()
            .filter(|c| c.in_response_to_id.is_none())
            .filter(|c| c.can_see(conn, user))
            .filter_map(|c| Self::from_comment(conn, c, user).ok())
            .collect())
    }

    pub fn from_comment(conn: &Connection, comment: Comment, user: Option<&User>) -> Result<Self> {
        let responses = comment
            .get_responses(conn)?
            .into_iter()
            .filter(|c| c.can_see(conn, user))
            .filter_map(|c| Self::from_comment(conn, c, user).ok())
            .collect();
        Ok(CommentTree { comment, responses })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbox::{inbox, tests::fill_database, InboxResult};
    use crate::safe_string::SafeString;
    use diesel::Connection;

}
