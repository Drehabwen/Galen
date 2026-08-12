use medical_core::MedicalCore;

/// Live-network reproduction test for the PubMed search flow used by the
/// `search_pubmed` tool. Ignored by default; run with:
///   cargo test -p medical-core --test pubmed_live -- --ignored --nocapture
#[tokio::test]
#[ignore]
async fn live_search_pubmed_returns_papers() {
    let core = MedicalCore::new(None);
    let papers = core
        .search_pubmed("scoliosis AND exercise", 5)
        .await
        .expect("search_pubmed should succeed");
    assert!(!papers.is_empty(), "expected at least one paper");
    println!("got {} papers, first: {:?}", papers.len(), papers.first());
    for p in papers.iter().take(3) {
        assert!(!p.pmid.is_empty());
    }
}
