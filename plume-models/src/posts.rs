use activitypub::{
    activity::{Create, Delete, Update},
    link,
    object::{Article, Image, Tombstone},
    CustomObject,
};
use chrono::{NaiveDateTime, TimeZone, Utc};
use diesel::{self, BelongingToDsl, ExpressionMethods, QueryDsl, RunQueryDsl, SaveChangesDsl};
use heck::{CamelCase, KebabCase};
use serde_json;
use std::collections::HashSet;

use blogs::Blog;
use instance::Instance;
use medias::Media;
use mentions::Mention;
use plume_common::{
    activity_pub::{
        inbox::{AsObject, FromId},
        Hashtag, Id, IntoId, Licensed, Source, PUBLIC_VISIBILITY,
    },
    utils::md_to_html,
};
use post_authors::*;
use safe_string::SafeString;
use schema::posts;
use search::Searcher;
use tags::*;
use users::User;
use {ap_url, Connection, Error, Result};

pub type LicensedArticle = CustomObject<Licensed, Article>;

#[derive(Queryable, Identifiable, Clone, AsChangeset)]
#[changeset_options(treat_none_as_null = "true")]
pub struct Post {
    pub id: i32,
    pub blog_id: i32,
    pub slug: String,
    pub title: String,
    pub content: SafeString,
    pub published: bool,
    pub license: String,
    pub creation_date: NaiveDateTime,
    pub ap_url: String,
    pub subtitle: String,
    pub source: String,
    pub cover_id: Option<i32>,
}

#[derive(Insertable)]
#[table_name = "posts"]
pub struct NewPost {
    pub blog_id: i32,
    pub slug: String,
    pub title: String,
    pub content: SafeString,
    pub published: bool,
    pub license: String,
    pub creation_date: Option<NaiveDateTime>,
    pub ap_url: String,
    pub subtitle: String,
    pub source: String,
    pub cover_id: Option<i32>,
}

impl Post {
    get!(posts);
    find_by!(posts, find_by_slug, slug as &str, blog_id as i32);
    find_by!(posts, find_by_ap_url, ap_url as &str);

    pub fn update(&self, conn: &Connection, searcher: &Searcher) -> Result<Self> {
        diesel::update(self).set(self).execute(conn)?;
        let post = Self::get(conn, self.id)?;
        searcher.update_document(conn, &post)?;
        Ok(post)
    }

    pub fn delete(&self, conn: &Connection, searcher: &Searcher) -> Result<()> {
        for m in Mention::list_for_post(&conn, self.id)? {
            m.delete(conn)?;
        }
        diesel::delete(self).execute(conn)?;
        searcher.delete_document(self);
        Ok(())
    }

    pub fn list_by_tag(
        conn: &Connection,
        tag: String,
        (min, max): (i32, i32),
    ) -> Result<Vec<Post>> {
        use schema::tags;

        let ids = tags::table.filter(tags::tag.eq(tag)).select(tags::post_id);
        posts::table
            .filter(posts::id.eq_any(ids))
            .filter(posts::published.eq(true))
            .order(posts::creation_date.desc())
            .offset(min.into())
            .limit((max - min).into())
            .load(conn)
            .map_err(Error::from)
    }

    pub fn count_for_tag(conn: &Connection, tag: String) -> Result<i64> {
        use schema::tags;
        let ids = tags::table.filter(tags::tag.eq(tag)).select(tags::post_id);
        posts::table
            .filter(posts::id.eq_any(ids))
            .filter(posts::published.eq(true))
            .count()
            .load(conn)?
            .iter()
            .next()
            .cloned()
            .ok_or(Error::NotFound)
    }

    pub fn count_local(conn: &Connection) -> Result<i64> {
        use schema::post_authors;
        use schema::users;
        let local_authors = users::table
            .filter(users::instance_id.eq(Instance::get_local()?.id))
            .select(users::id);
        let local_posts_id = post_authors::table
            .filter(post_authors::author_id.eq_any(local_authors))
            .select(post_authors::post_id);
        posts::table
            .filter(posts::id.eq_any(local_posts_id))
            .filter(posts::published.eq(true))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn count(conn: &Connection) -> Result<i64> {
        posts::table
            .filter(posts::published.eq(true))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn list_filtered(
        conn: &Connection,
        title: Option<String>,
        subtitle: Option<String>,
        content: Option<String>,
    ) -> Result<Vec<Post>> {
        let mut query = posts::table.into_boxed();
        if let Some(title) = title {
            query = query.filter(posts::title.eq(title));
        }
        if let Some(subtitle) = subtitle {
            query = query.filter(posts::subtitle.eq(subtitle));
        }
        if let Some(content) = content {
            query = query.filter(posts::content.eq(content));
        }

        query.get_results::<Post>(conn).map_err(Error::from)
    }

    pub fn get_recents(conn: &Connection, limit: i64) -> Result<Vec<Post>> {
        posts::table
            .order(posts::creation_date.desc())
            .filter(posts::published.eq(true))
            .limit(limit)
            .load::<Post>(conn)
            .map_err(Error::from)
    }

    pub fn get_recents_for_author(
        conn: &Connection,
        author: &User,
        limit: i64,
    ) -> Result<Vec<Post>> {
        use schema::post_authors;

        let posts = PostAuthor::belonging_to(author).select(post_authors::post_id);
        posts::table
            .filter(posts::id.eq_any(posts))
            .filter(posts::published.eq(true))
            .order(posts::creation_date.desc())
            .limit(limit)
            .load::<Post>(conn)
            .map_err(Error::from)
    }

    pub fn get_recents_for_blog(conn: &Connection, blog: &Blog, limit: i64) -> Result<Vec<Post>> {
        posts::table
            .filter(posts::blog_id.eq(blog.id))
            .filter(posts::published.eq(true))
            .order(posts::creation_date.desc())
            .limit(limit)
            .load::<Post>(conn)
            .map_err(Error::from)
    }

    pub fn get_for_blog(conn: &Connection, blog: &Blog) -> Result<Vec<Post>> {
        posts::table
            .filter(posts::blog_id.eq(blog.id))
            .filter(posts::published.eq(true))
            .load::<Post>(conn)
            .map_err(Error::from)
    }

    pub fn count_for_blog(conn: &Connection, blog: &Blog) -> Result<i64> {
        posts::table
            .filter(posts::blog_id.eq(blog.id))
            .filter(posts::published.eq(true))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn blog_page(conn: &Connection, blog: &Blog, (min, max): (i32, i32)) -> Result<Vec<Post>> {
        posts::table
            .filter(posts::blog_id.eq(blog.id))
            .filter(posts::published.eq(true))
            .order(posts::creation_date.desc())
            .offset(min.into())
            .limit((max - min).into())
            .load::<Post>(conn)
            .map_err(Error::from)
    }

    /// Give a page of all the recent posts known to this instance (= federated timeline)
    pub fn get_recents_page(conn: &Connection, (min, max): (i32, i32)) -> Result<Vec<Post>> {
        posts::table
            .order(posts::creation_date.desc())
            .filter(posts::published.eq(true))
            .offset(min.into())
            .limit((max - min).into())
            .load::<Post>(conn)
            .map_err(Error::from)
    }

    /// Give a page of posts from a specific instance
    pub fn get_instance_page(
        conn: &Connection,
        instance_id: i32,
        (min, max): (i32, i32),
    ) -> Result<Vec<Post>> {
        use schema::blogs;

        let blog_ids = blogs::table
            .filter(blogs::instance_id.eq(instance_id))
            .select(blogs::id);

        posts::table
            .order(posts::creation_date.desc())
            .filter(posts::published.eq(true))
            .filter(posts::blog_id.eq_any(blog_ids))
            .offset(min.into())
            .limit((max - min).into())
            .load::<Post>(conn)
            .map_err(Error::from)
    }

    /// Give a page of customized user feed, based on a list of followed users
    pub fn user_feed_page(
        conn: &Connection,
        followed: Vec<i32>,
        (min, max): (i32, i32),
    ) -> Result<Vec<Post>> {
        use schema::post_authors;
        let post_ids = post_authors::table
            .filter(post_authors::author_id.eq_any(followed))
            .select(post_authors::post_id);

        posts::table
            .order(posts::creation_date.desc())
            .filter(posts::published.eq(true))
            .filter(posts::id.eq_any(post_ids))
            .offset(min.into())
            .limit((max - min).into())
            .load::<Post>(conn)
            .map_err(Error::from)
    }

    pub fn drafts_by_author(conn: &Connection, author: &User) -> Result<Vec<Post>> {
        use schema::post_authors;

        let posts = PostAuthor::belonging_to(author).select(post_authors::post_id);
        posts::table
            .order(posts::creation_date.desc())
            .filter(posts::published.eq(false))
            .filter(posts::id.eq_any(posts))
            .load::<Post>(conn)
            .map_err(Error::from)
    }

    pub fn get_authors(&self, conn: &Connection) -> Result<Vec<User>> {
        use schema::post_authors;
        use schema::users;
        let author_list = PostAuthor::belonging_to(self).select(post_authors::author_id);
        users::table
            .filter(users::id.eq_any(author_list))
            .load::<User>(conn)
            .map_err(Error::from)
    }

    pub fn is_author(&self, conn: &Connection, author_id: i32) -> Result<bool> {
        use schema::post_authors;
        Ok(PostAuthor::belonging_to(self)
            .filter(post_authors::author_id.eq(author_id))
            .count()
            .get_result::<i64>(conn)?
            > 0)
    }

    pub fn get_blog(&self, conn: &Connection) -> Result<Blog> {
        use schema::blogs;
        blogs::table
            .filter(blogs::id.eq(self.blog_id))
            .limit(1)
            .load::<Blog>(conn)?
            .into_iter()
            .nth(0)
            .ok_or(Error::NotFound)
    }

    pub fn count_likes(&self, conn: &Connection) -> Result<i64> {
        use schema::likes;
        likes::table
            .filter(likes::post_id.eq(self.id))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn count_reshares(&self, conn: &Connection) -> Result<i64> {
        use schema::reshares;
        reshares::table
            .filter(reshares::post_id.eq(self.id))
            .count()
            .get_result(conn)
            .map_err(Error::from)
    }

    pub fn get_receivers_urls(&self, conn: &Connection) -> Result<Vec<String>> {
        let followers = self
            .get_authors(conn)?
            .into_iter()
            .filter_map(|a| a.get_followers(conn).ok())
            .collect::<Vec<Vec<User>>>();
        Ok(followers.into_iter().fold(vec![], |mut acc, f| {
            for x in f {
                acc.push(x.ap_url);
            }
            acc
        }))
    }

    pub fn to_activity(&self, conn: &Connection) -> Result<LicensedArticle> {
        let cc = self.get_receivers_urls(conn)?;
        let to = vec![PUBLIC_VISIBILITY.to_string()];

        let mut mentions_json = Mention::list_for_post(conn, self.id)?
            .into_iter()
            .map(|m| json!(m.to_activity(conn).ok()))
            .collect::<Vec<serde_json::Value>>();
        let mut tags_json = Tag::for_post(conn, self.id)?
            .into_iter()
            .map(|t| json!(t.to_activity().ok()))
            .collect::<Vec<serde_json::Value>>();
        mentions_json.append(&mut tags_json);

        let mut article = Article::default();
        article.object_props.set_name_string(self.title.clone())?;
        article.object_props.set_id_string(self.ap_url.clone())?;

        let mut authors = self
            .get_authors(conn)?
            .into_iter()
            .map(|x| Id::new(x.ap_url))
            .collect::<Vec<Id>>();
        authors.push(self.get_blog(conn)?.into_id()); // add the blog URL here too
        article
            .object_props
            .set_attributed_to_link_vec::<Id>(authors)?;
        article
            .object_props
            .set_content_string(self.content.get().clone())?;
        article.ap_object_props.set_source_object(Source {
            content: self.source.clone(),
            media_type: String::from("text/markdown"),
        })?;
        article
            .object_props
            .set_published_utctime(Utc.from_utc_datetime(&self.creation_date))?;
        article
            .object_props
            .set_summary_string(self.subtitle.clone())?;
        article.object_props.tag = Some(json!(mentions_json));

        if let Some(media_id) = self.cover_id {
            let media = Media::get(conn, media_id)?;
            let mut cover = Image::default();
            cover.object_props.set_url_string(media.url()?)?;
            if media.sensitive {
                cover
                    .object_props
                    .set_summary_string(media.content_warning.unwrap_or_default())?;
            }
            cover.object_props.set_content_string(media.alt_text)?;
            cover
                .object_props
                .set_attributed_to_link_vec(vec![User::get(conn, media.owner_id)?.into_id()])?;
            article.object_props.set_icon_object(cover)?;
        }

        article.object_props.set_url_string(self.ap_url.clone())?;
        article
            .object_props
            .set_to_link_vec::<Id>(to.into_iter().map(Id::new).collect())?;
        article
            .object_props
            .set_cc_link_vec::<Id>(cc.into_iter().map(Id::new).collect())?;
        let mut license = Licensed::default();
        license.set_license_string(self.license.clone())?;
        Ok(LicensedArticle::new(article, license))
    }

    pub fn create_activity(&self, conn: &Connection) -> Result<Create> {
        let article = self.to_activity(conn)?;
        let mut act = Create::default();
        act.object_props
            .set_id_string(format!("{}activity", self.ap_url))?;
        act.object_props
            .set_to_link_vec::<Id>(article.object.object_props.to_link_vec()?)?;
        act.object_props
            .set_cc_link_vec::<Id>(article.object.object_props.cc_link_vec()?)?;
        act.create_props
            .set_actor_link(Id::new(self.get_authors(conn)?[0].clone().ap_url))?;
        act.create_props.set_object_object(article)?;
        Ok(act)
    }

    pub fn update_activity(&self, conn: &Connection) -> Result<Update> {
        let article = self.to_activity(conn)?;
        let mut act = Update::default();
        act.object_props.set_id_string(format!(
            "{}/update-{}",
            self.ap_url,
            Utc::now().timestamp()
        ))?;
        act.object_props
            .set_to_link_vec::<Id>(article.object.object_props.to_link_vec()?)?;
        act.object_props
            .set_cc_link_vec::<Id>(article.object.object_props.cc_link_vec()?)?;
        act.update_props
            .set_actor_link(Id::new(self.get_authors(conn)?[0].clone().ap_url))?;
        act.update_props.set_object_object(article)?;
        Ok(act)
    }

    pub fn update_tags(&self, conn: &Connection, tags: Vec<Hashtag>) -> Result<()> {
        let tags_name = tags
            .iter()
            .filter_map(|t| t.name_string().ok())
            .collect::<HashSet<_>>();

        let old_tags = Tag::for_post(&*conn, self.id)?;
        let old_tags_name = old_tags
            .iter()
            .filter_map(|tag| {
                if !tag.is_hashtag {
                    Some(tag.tag.clone())
                } else {
                    None
                }
            })
            .collect::<HashSet<_>>();

        for t in tags {
            if !t
                .name_string()
                .map(|n| old_tags_name.contains(&n))
                .unwrap_or(true)
            {
                Tag::from_activity(conn, &t, self.id, false)?;
            }
        }

        for ot in old_tags.iter().filter(|t| !t.is_hashtag) {
            if !tags_name.contains(&ot.tag) {
                ot.delete(conn)?;
            }
        }
        Ok(())
    }

    pub fn update_hashtags(&self, conn: &Connection, tags: Vec<Hashtag>) -> Result<()> {
        let tags_name = tags
            .iter()
            .filter_map(|t| t.name_string().ok())
            .collect::<HashSet<_>>();

        let old_tags = Tag::for_post(&*conn, self.id)?;
        let old_tags_name = old_tags
            .iter()
            .filter_map(|tag| {
                if tag.is_hashtag {
                    Some(tag.tag.clone())
                } else {
                    None
                }
            })
            .collect::<HashSet<_>>();

        for t in tags {
            if !t
                .name_string()
                .map(|n| old_tags_name.contains(&n))
                .unwrap_or(true)
            {
                Tag::from_activity(conn, &t, self.id, true)?;
            }
        }

        for ot in old_tags.into_iter().filter(|t| t.is_hashtag) {
            if !tags_name.contains(&ot.tag) {
                ot.delete(conn)?;
            }
        }
        Ok(())
    }

    pub fn url(&self, conn: &Connection) -> Result<String> {
        let blog = self.get_blog(conn)?;
        Ok(format!("/~/{}/{}", blog.fqn, self.slug))
    }

    pub fn cover_url(&self, conn: &Connection) -> Option<String> {
        self.cover_id
            .and_then(|i| Media::get(conn, i).ok())
            .and_then(|c| c.url().ok())
    }

    pub fn build_delete(&self, conn: &Connection) -> Result<Delete> {
        let mut act = Delete::default();
        act.delete_props
            .set_actor_link(self.get_authors(conn)?[0].clone().into_id())?;

        let mut tombstone = Tombstone::default();
        tombstone.object_props.set_id_string(self.ap_url.clone())?;
        act.delete_props.set_object_object(tombstone)?;

        act.object_props
            .set_id_string(format!("{}#delete", self.ap_url))?;
        act.object_props
            .set_to_link_vec(vec![Id::new(PUBLIC_VISIBILITY)])?;
        Ok(act)
    }
}

pub struct PostUpdate {
    pub ap_url: String,
    pub title: Option<String>,
    pub subtitle: Option<String>,
    pub content: Option<String>,
    pub cover: Option<i32>,
    pub source: Option<String>,
    pub license: Option<String>,
    pub tags: Option<serde_json::Value>,
}

impl IntoId for Post {
    fn into_id(self) -> Id {
        Id::new(self.ap_url.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inbox::{inbox, tests::fill_database, InboxResult};
    use crate::safe_string::SafeString;
    use diesel::Connection;

    #[test]
    fn licensed_article_serde() {
        let mut article = Article::default();
        article.object_props.set_id_string("Yo".into()).unwrap();
        let mut license = Licensed::default();
        license.set_license_string("WTFPL".into()).unwrap();
        let full_article = LicensedArticle::new(article, license);

        let json = serde_json::to_value(full_article).unwrap();
        let article_from_json: LicensedArticle = serde_json::from_value(json).unwrap();
        assert_eq!(
            "Yo",
            &article_from_json.object.object_props.id_string().unwrap()
        );
        assert_eq!(
            "WTFPL",
            &article_from_json.custom_props.license_string().unwrap()
        );
    }

    #[test]
    fn licensed_article_deserialization() {
        let json = json!({
            "type": "Article",
            "id": "https://plu.me/~/Blog/my-article",
            "attributedTo": ["https://plu.me/@/Admin", "https://plu.me/~/Blog"],
            "content": "Hello.",
            "name": "My Article",
            "summary": "Bye.",
            "source": {
                "content": "Hello.",
                "mediaType": "text/markdown"
            },
            "published": "2014-12-12T12:12:12Z",
            "to": [plume_common::activity_pub::PUBLIC_VISIBILITY]
        });
        let article: LicensedArticle = serde_json::from_value(json).unwrap();
        assert_eq!(
            "https://plu.me/~/Blog/my-article",
            &article.object.object_props.id_string().unwrap()
        );
    }
}
