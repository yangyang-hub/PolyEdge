#[cfg(test)]
mod orderbook_cache_tests {
    use super::*;

    fn level(price: i64, size: i64) -> polyedge_application::CachedBookLevel {
        polyedge_application::CachedBookLevel {
            price: rust_decimal::Decimal::new(price, 2),
            size: rust_decimal::Decimal::new(size, 0),
        }
    }

    #[tokio::test]
    async fn bounded_book_keeps_best_levels_when_input_unsorted() {
        let cache = InMemoryOrderbookCache::new(60_000, 2);
        let book = polyedge_application::CachedOrderBook {
            token_id: "tok1".to_string(),
            // Unsorted bids; the two best (highest) are 0.60 then 0.55.
            bids: vec![level(50, 10), level(60, 10), level(55, 10), level(40, 10)],
            // Unsorted asks; the two best (lowest) are 0.62 then 0.65.
            asks: vec![level(70, 10), level(62, 10), level(80, 10), level(65, 10)],
            observed_at: 0,
            source: polyedge_application::BookSource::Poll,
        };
        cache.set_book(&book).await.expect("set book");
        let got = cache
            .get_book("tok1")
            .await
            .expect("get book")
            .expect("book present");

        // Depth trimmed to the 2 BEST levels per side, correctly ordered.
        assert_eq!(
            got.bids.iter().map(|l| l.price).collect::<Vec<_>>(),
            vec![
                rust_decimal::Decimal::new(60, 2),
                rust_decimal::Decimal::new(55, 2)
            ]
        );
        assert_eq!(
            got.asks.iter().map(|l| l.price).collect::<Vec<_>>(),
            vec![
                rust_decimal::Decimal::new(62, 2),
                rust_decimal::Decimal::new(65, 2)
            ]
        );
    }

    #[tokio::test]
    async fn cache_rejects_older_snapshot_overwrite() {
        let cache = InMemoryOrderbookCache::new(60_000, 10);
        let newer = polyedge_application::CachedOrderBook {
            token_id: "tok1".to_string(),
            bids: vec![level(60, 10)],
            asks: vec![level(62, 10)],
            observed_at: 200,
            source: polyedge_application::BookSource::Ws,
        };
        let older = polyedge_application::CachedOrderBook {
            token_id: "tok1".to_string(),
            bids: vec![level(40, 10)],
            asks: vec![level(80, 10)],
            observed_at: 100,
            source: polyedge_application::BookSource::Poll,
        };

        cache.set_book(&newer).await.expect("set newer");
        cache.set_book(&older).await.expect("ignore older");
        let got = cache
            .get_book("tok1")
            .await
            .expect("get book")
            .expect("book present");

        assert_eq!(got.observed_at, 200);
        assert_eq!(got.bids[0].price, rust_decimal::Decimal::new(60, 2));
    }

    #[tokio::test]
    async fn batch_cache_rejects_older_snapshot_overwrite() {
        let cache = InMemoryOrderbookCache::new(60_000, 10);
        let newer = polyedge_application::CachedOrderBook {
            token_id: "tok1".to_string(),
            bids: vec![level(60, 10)],
            asks: vec![],
            observed_at: 200,
            source: polyedge_application::BookSource::Ws,
        };
        let older = polyedge_application::CachedOrderBook {
            observed_at: 100,
            ..newer.clone()
        };

        cache.set_book(&newer).await.expect("set newer");
        cache.set_books(&[older]).await.expect("ignore older batch");

        assert_eq!(
            cache
                .get_book("tok1")
                .await
                .expect("get book")
                .expect("book present")
                .observed_at,
            200
        );
    }
}
