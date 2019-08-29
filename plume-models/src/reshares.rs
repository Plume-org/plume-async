use activitypub::activity::{Announce, Undo};
use chrono::NaiveDateTime;
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl};

use notifications::*;
use plume_common::activity_pub::{
    inbox::{AsActor, AsObject, FromId},
    Id, IntoId, PUBLIC_VISIBILITY,
};
use posts::Post;
use schema::reshares;
use users::User;
use {Connection, Error, Result};

#[derive(Clone, Queryable, Identifiable)]
pub struct Reshare {
    pub id: i32,
    pub user_id: i32,
    pub post_id: i32,
    pub ap_url: String,
    pub creation_date: NaiveDateTime,
}

#[derive(Insertable)]
#[table_name = "reshares"]
pub struct NewReshare {
    pub user_id: i32,
    pub post_id: i32,
    pub ap_url: String,
}

impl Reshare {
    insert!(reshares, NewReshare);
    get!(reshares);
    find_by!(reshares, find_by_ap_url, ap_url as &str);
    find_by!(
        reshares,
        find_by_user_on_post,
        user_id as i32,
        post_id as i32
    );

    pub fn get_recents_for_author(
        conn: &Connection,
        user: &User,
        limit: i64,
    ) -> Result<Vec<Reshare>> {
        reshares::table
            .filter(reshares::user_id.eq(user.id))
            .order(reshares::creation_date.desc())
            .limit(limit)
            .load::<Reshare>(conn)
            .map_err(Error::from)
    }

    pub fn get_post(&self, conn: &Connection) -> Result<Post> {
        Post::get(conn, self.post_id)
    }

    pub fn get_user(&self, conn: &Connection) -> Result<User> {
        User::get(conn, self.user_id)
    }

    pub fn to_activity(&self, conn: &Connection) -> Result<Announce> {
        let mut act = Announce::default();
        act.announce_props
            .set_actor_link(User::get(conn, self.user_id)?.into_id())?;
        act.announce_props
            .set_object_link(Post::get(conn, self.post_id)?.into_id())?;
        act.object_props.set_id_string(self.ap_url.clone())?;
        act.object_props
            .set_to_link_vec(vec![Id::new(PUBLIC_VISIBILITY.to_string())])?;
        act.object_props
            .set_cc_link_vec(vec![Id::new(self.get_user(conn)?.followers_endpoint)])?;

        Ok(act)
    }

    pub fn build_undo(&self, conn: &Connection) -> Result<Undo> {
        let mut act = Undo::default();
        act.undo_props
            .set_actor_link(User::get(conn, self.user_id)?.into_id())?;
        act.undo_props.set_object_object(self.to_activity(conn)?)?;
        act.object_props
            .set_id_string(format!("{}#delete", self.ap_url))?;
        act.object_props
            .set_to_link_vec(vec![Id::new(PUBLIC_VISIBILITY.to_string())])?;
        act.object_props
            .set_cc_link_vec(vec![Id::new(self.get_user(conn)?.followers_endpoint)])?;

        Ok(act)
    }
}

impl NewReshare {
    pub fn new(p: &Post, u: &User) -> Self {
        let ap_url = format!("{}/reshare/{}", u.ap_url, p.ap_url);
        NewReshare {
            post_id: p.id,
            user_id: u.id,
            ap_url,
        }
    }
}
