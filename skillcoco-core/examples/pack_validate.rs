//! Demonstrate topic-pack JSON-schema validation.
//!
//! Validates a minimal in-memory pack JSON against the bundled
//! Draft 2020-12 schema, then validates a deliberately-broken pack to
//! show the error-list shape.
//!
//! This example reads NO file from disk — the pack payload is constructed
//! inline as `serde_json::Value` to keep the example purely illustrative.
//!
//! Run with:
//!
//! ```bash
//! cargo run -p skillcoco-core --example pack_validate
//! ```

use skillcoco_core::packs::schema;
use serde_json::json;

fn main() {
    // 1. A minimal VALID pack.
    let valid = json!({
        "id": "demo-pack",
        "title": "Demo Pack",
        "description": "A minimal pack that satisfies the SkillCoco schema.",
        "domain_module": "programming",
        "modules": [
            {
                "id": "intro",
                "title": "Introduction",
                "description": "Welcome module.",
                "objectives": ["Understand the example"]
            }
        ]
    });

    let errors_valid = schema::validate(&valid);
    println!("Validating minimal valid pack...");
    println!("  errors: {} (expected: 0)", errors_valid.len());
    for e in &errors_valid {
        println!("    - {e}");
    }
    assert!(
        errors_valid.is_empty(),
        "minimal valid pack must produce zero errors"
    );

    // 2. A pack with multiple violations:
    //    - missing required `description`
    //    - wrong `domain_module` enum
    //    - empty `modules` array
    let broken = json!({
        "id": "demo-pack",
        "title": "Broken pack",
        "domain_module": "not-a-valid-domain",
        "modules": []
    });

    let errors_broken = schema::validate(&broken);
    println!();
    println!("Validating deliberately broken pack...");
    println!("  errors: {} (expected: >= 1)", errors_broken.len());
    for e in &errors_broken {
        println!("    - {e}");
    }
    assert!(
        !errors_broken.is_empty(),
        "broken pack must produce at least one error"
    );

    println!();
    println!("All assertions passed.");
}
