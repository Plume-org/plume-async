use diesel::r2d2::{
    ConnectionManager, CustomizeConnection, Error as ConnError, Pool, PooledConnection,
};
#[cfg(feature = "sqlite")]
use diesel::{dsl::sql_query, ConnectionError, RunQueryDsl};
use std::ops::Deref;

use Connection;

pub type DbPool = Pool<ConnectionManager<Connection>>;

// From rocket documentation /// XXX: Will need adaptation for actix ;)

// Connection request guard type: a wrapper around an r2d2 pooled connection.
pub struct DbConn(pub PooledConnection<ConnectionManager<Connection>>);

// For the convenience of using an &DbConn as an &Connection.
impl Deref for DbConn {
    type Target = Connection;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

// Execute a pragma for every new sqlite connection
#[derive(Debug)]
pub struct PragmaForeignKey;
impl CustomizeConnection<Connection, ConnError> for PragmaForeignKey {
    #[cfg(feature = "sqlite")] // will default to an empty function for postgres
    fn on_acquire(&self, conn: &mut Connection) -> Result<(), ConnError> {
        sql_query("PRAGMA foreign_keys = on;")
            .execute(conn)
            .map(|_| ())
            .map_err(|_| {
                ConnError::ConnectionError(ConnectionError::BadConnection(String::from(
                    "PRAGMA foreign_keys = on failed",
                )))
            })
    }
}
