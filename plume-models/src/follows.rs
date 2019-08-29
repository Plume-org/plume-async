use activitypub::activity::{Accept, Follow as FollowAct, Undo};
use diesel::{self, ExpressionMethods, QueryDsl, RunQueryDsl, SaveChangesDsl};

use notifications::*;
use plume_common::activity_pub::{
    broadcast,
    inbox::{AsActor, AsObject, FromId},
    sign::Signer,
    Id, IntoId, PUBLIC_VISIBILITY,
};
use schema::follows;
use users::User;
use {ap_url, Connection, Error, Result};

#[derive(Clone, Queryable, Identifiable, Associations, AsChangeset)]
#[belongs_to(User, foreign_key = "following_id")]
pub struct Follow {
    pub id: i32,
    pub follower_id: i32,
    pub following_id: i32,
    pub ap_url: String,
}

#[derive(Insertable)]
#[table_name = "follows"]
pub struct NewFollow {
    pub follower_id: i32,
    pub following_id: i32,
    pub ap_url: String,
}

impl Follow {
    get!(follows);
    find_by!(follows, find_by_ap_url, ap_url as &str);

    pub fn find(conn: &Connection, from: i32, to: i32) -> Result<Follow> {
        follows::table
            .filter(follows::follower_id.eq(from))
            .filter(follows::following_id.eq(to))
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn to_activity(&self, conn: &Connection) -> Result<FollowAct> {
        let user = User::get(conn, self.follower_id)?;
        let target = User::get(conn, self.following_id)?;

        let mut act = FollowAct::default();
        act.follow_props
            .set_actor_link::<Id>(user.clone().into_id())?;
        act.follow_props
            .set_object_link::<Id>(target.clone().into_id())?;
        act.object_props.set_id_string(self.ap_url.clone())?;
        act.object_props.set_to_link_vec(vec![target.into_id()])?;
        act.object_props
            .set_cc_link_vec(vec![Id::new(PUBLIC_VISIBILITY.to_string())])?;
        Ok(act)
    }

    pub fn build_undo(&self, conn: &Connection) -> Result<Undo> {
        let mut undo = Undo::default();
        undo.undo_props
            .set_actor_link(User::get(conn, self.follower_id)?.into_id())?;
        undo.object_props
            .set_id_string(format!("{}/undo", self.ap_url))?;
        undo.undo_props
            .set_object_link::<Id>(self.clone().into_id())?;
        undo.object_props
            .set_to_link_vec(vec![User::get(conn, self.following_id)?.into_id()])?;
        undo.object_props
            .set_cc_link_vec(vec![Id::new(PUBLIC_VISIBILITY.to_string())])?;
        Ok(undo)
    }
}

impl IntoId for Follow {
    fn into_id(self) -> Id {
        Id::new(self.ap_url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use diesel::Connection;
    use tests::db;
    use users::tests as user_tests;
}
