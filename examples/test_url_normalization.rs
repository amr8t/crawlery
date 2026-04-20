//! Test example to verify URL fragment normalization works correctly.
//!
//! This example demonstrates that URLs with different fragments (e.g., #section1, #section2)
//! are treated as the same URL after normalization.

use crawlery::state::{CrawlConfig, CrawlState};

fn main() {
    println!("Testing URL fragment normalization...\n");

    let config = CrawlConfig {
        start_url: "https://example.com".to_string(),
        max_depth: 2,
        max_pages: Some(100),
        respect_robots_txt: true,
    };

    let mut state = CrawlState::new(config);

    // Test 1: Mark a URL with fragment as visited
    println!("Test 1: Marking visited with fragment");
    state.mark_visited("https://example.com/page#section1".to_string());

    // Check if the same URL with different fragment is considered visited
    let is_visited_diff_fragment = state.is_visited("https://example.com/page#section2");
    let is_visited_no_fragment = state.is_visited("https://example.com/page");

    println!(
        "  - Is 'https://example.com/page#section2' visited? {}",
        is_visited_diff_fragment
    );
    println!(
        "  - Is 'https://example.com/page' visited? {}",
        is_visited_no_fragment
    );

    if is_visited_diff_fragment && is_visited_no_fragment {
        println!("  ✓ Test 1 PASSED: URLs with different fragments treated as same\n");
    } else {
        println!("  ✗ Test 1 FAILED: URLs with different fragments not normalized\n");
    }

    // Test 2: Adding pending URLs with fragments should deduplicate
    println!("Test 2: Adding pending URLs with different fragments");
    let initial_pending = state.pending_count();
    println!("  - Initial pending count: {}", initial_pending);

    state.add_pending(
        vec![
            "https://example.com/page2#intro".to_string(),
            "https://example.com/page2#conclusion".to_string(),
            "https://example.com/page2".to_string(),
        ],
        0,
    );

    let final_pending = state.pending_count();
    println!("  - Final pending count: {}", final_pending);
    println!("  - URLs added: {}", final_pending - initial_pending);

    if final_pending - initial_pending == 1 {
        println!("  ✓ Test 2 PASSED: Duplicate fragments deduplicated correctly\n");
    } else {
        println!(
            "  ✗ Test 2 FAILED: Expected 1 URL added, got {}\n",
            final_pending - initial_pending
        );
    }

    // Test 3: Visited URL should prevent adding pending with different fragment
    println!("Test 3: Visited URL blocks pending with different fragment");
    state.mark_visited("https://example.com/page3".to_string());
    let before_pending = state.pending_count();

    state.add_pending(vec!["https://example.com/page3#faq".to_string()], 0);

    let after_pending = state.pending_count();
    println!("  - Pending count before: {}", before_pending);
    println!("  - Pending count after: {}", after_pending);

    if before_pending == after_pending {
        println!("  ✓ Test 3 PASSED: Visited URL blocks pending with fragment\n");
    } else {
        println!("  ✗ Test 3 FAILED: Visited URL did not block pending\n");
    }

    // Test 4: Pending URL should prevent adding duplicate with different fragment
    println!("Test 4: Pending URL blocks duplicate with different fragment");
    state.add_pending(vec!["https://example.com/page4#top".to_string()], 0);
    let before = state.pending_count();

    state.add_pending(vec!["https://example.com/page4#bottom".to_string()], 0);

    let after = state.pending_count();
    println!("  - Pending count before: {}", before);
    println!("  - Pending count after: {}", after);

    if before == after {
        println!("  ✓ Test 4 PASSED: Pending URL blocks duplicate with fragment\n");
    } else {
        println!("  ✗ Test 4 FAILED: Duplicate was not blocked\n");
    }

    println!("═══════════════════════════════════════════════════");
    println!("URL fragment normalization testing complete!");
    println!("═══════════════════════════════════════════════════");
}
