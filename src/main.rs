use diesel_async::scoped_futures::ScopedFutureExt;
use futures::task::noop_waker;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    env_logger::init();

    let mut config = diesel_async::pooled_connection::ManagerConfig::default();
    config.recycling_method =
        diesel_async::pooled_connection::RecyclingMethod::CustomQuery(RECYCLING_QUERY.into());

    let config = diesel_async::pooled_connection::AsyncDieselConnectionManager::<
        diesel_async::AsyncPgConnection,
    >::new_with_config(std::env::var("DATABASE_URL").unwrap(), config);

    let pool = diesel_async::pooled_connection::deadpool::Pool::builder(config)
        .wait_timeout(Some(std::time::Duration::from_secs(1)))
        .runtime(deadpool::Runtime::Tokio1)
        .max_size(1)
        .build()
        .unwrap();

    {
        let fut = async {
            let mut conn = pool.get().await.unwrap();

            conn.build_transaction()
                .run(|_conn| async { Ok::<_, diesel::result::Error>(()) }.scope_boxed())
                .await
        };

        let waker = noop_waker();
        let mut ctx = std::task::Context::from_waker(&waker);
        let mut fut = std::pin::pin!(fut);

        for _ in 0..9 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            println!("{:?}", std::future::Future::poll(fut.as_mut(), &mut ctx));
        }
    }

    let mut conn = pool.get().await.unwrap();

    conn.build_transaction()
        .deferrable()
        .run(|_conn| async { Ok::<_, diesel::result::Error>(()) }.scope_boxed())
        .await
        .unwrap();
}

const RECYCLING_QUERY: &str = "DO $$ DECLARE clean bool; BEGIN
    SELECT statement_timestamp() = transaction_timestamp() INTO clean;
    ASSERT clean, 'dirty-conn';
END $$;";
