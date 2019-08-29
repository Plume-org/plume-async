use activitypub::activity;
use chrono::NaiveDateTime;
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};

use notifications::*;
use plume_common::activity_pub::{
    inbox::{AsActor, AsObject, FromId},
    Id, IntoId, PUBLIC_VISIBILITY,
};
use posts::Post;
use schema::likes;
use users::User;
use {Connection, Error, Result};

#[derive(Clone, Queryable, Identifiable)]
pub struct Like {
    pub id: i32,
    pub user_id: i32,
    pub post_id: i32,
    pub creation_date: NaiveDateTime,
    pub ap_url: String,
}

#[derive(Default, Insertable)]
#[table_name = "likes"]
pub struct NewLike {
    pub user_id: i32,
    pub post_id: i32,
    pub ap_url: String,
}

impl Like {
    insert!(likes, NewLike);
    get!(likes);
    find_by!(likes, find_by_ap_url, ap_url as &str);
    find_by!(likes, find_by_user_on_post, user_id as i32, post_id as i32);

    pub fn to_activity(&self, conn: &Connection) -> Result<activity::Like> {
        let mut act = activity::Like::default();
        act.like_props
            .set_actor_link(User::get(conn, self.user_id)?.into_id())?;
        act.like_props
            .set_object_link(Post::get(conn, self.post_id)?.into_id())?;
        act.object_props
            .set_to_link_vec(vec![Id::new(PUBLIC_VISIBILITY.to_string())])?;
        act.object_props.set_cc_link_vec(vec![Id::new(
            User::get(conn, self.user_id)?.followers_endpoint,
        )])?;
        act.object_props.set_id_string(self.ap_url.clone())?;

        Ok(act)
    }

    pub fn build_undo(&self, conn: &Connection) -> Result<activity::Undo> {
        let mut act = activity::Undo::default();
        act.undo_props
            .set_actor_link(User::get(conn, self.user_id)?.into_id())?;
        act.undo_props.set_object_object(self.to_activity(conn)?)?;
        act.object_props
            .set_id_string(format!("{}#delete", self.ap_url))?;
        act.object_props
            .set_to_link_vec(vec![Id::new(PUBLIC_VISIBILITY.to_string())])?;
        act.object_props.set_cc_link_vec(vec![Id::new(
            User::get(conn, self.user_id)?.followers_endpoint,
        )])?;

        Ok(act)
    }
}

impl NewLike {
    pub fn new(p: &Post, u: &User) -> Self {
        // TODO: this URL is not valid
        let ap_url = format!("{}/like/{}", u.ap_url, p.ap_url);
        NewLike {
            post_id: p.id,
            user_id: u.id,
            ap_url,
        }
    }
}
