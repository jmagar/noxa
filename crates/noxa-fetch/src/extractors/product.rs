use regex::Regex;
use serde_json::{Value, json};

pub fn parse_product_page(url: &str, html: &str, source: &str) -> Value {
    let product = json_ld_values(html)
        .into_iter()
        .flat_map(flatten_graph)
        .find(is_product)
        .unwrap_or_else(|| json!({}));
    let offers = product.get("offers").and_then(first_or_self);
    let rating = product.get("aggregateRating");

    json!({
        "url": url,
        "source": source,
        "title": string_field(&product, "name").or_else(|| og(html, "title")),
        "description": string_field(&product, "description"),
        "sku": string_field(&product, "sku"),
        "brand": product.get("brand").and_then(|brand| {
            string_field(brand, "name").or_else(|| brand.as_str().map(ToString::to_string))
        }),
        "image": product.get("image").cloned(),
        "price": offers.and_then(|offers| string_field(offers, "price")).or_else(|| meta_property(html, "product:price:amount")),
        "currency": offers.and_then(|offers| string_field(offers, "priceCurrency")).or_else(|| meta_property(html, "product:price:currency")),
        "availability": offers
            .and_then(|offers| string_field(offers, "availability"))
            .map(|availability| availability.rsplit('/').next().unwrap_or(&availability).to_string()),
        "offer_url": offers.and_then(|offers| string_field(offers, "url")),
        "rating": rating.and_then(|rating| string_field(rating, "ratingValue")),
        "review_count": rating.and_then(|rating| string_field(rating, "reviewCount")),
    })
}

pub fn parse_trustpilot_page(url: &str, html: &str) -> Value {
    let business = json_ld_values(html)
        .into_iter()
        .flat_map(flatten_graph)
        .find(|value| {
            value.get("@type").is_some_and(|kind| {
                kind == "LocalBusiness" || kind == "Organization" || kind == "Corporation"
            })
        })
        .unwrap_or_else(|| json!({}));
    let reviews: Vec<_> = business
        .get("review")
        .and_then(first_or_array)
        .into_iter()
        .flatten()
        .map(|review| {
            json!({
                "author": review.pointer("/author/name").and_then(Value::as_str),
                "rating": review.pointer("/reviewRating/ratingValue").cloned(),
                "body": review.get("reviewBody").cloned(),
                "date": review.get("datePublished").cloned(),
            })
        })
        .collect();

    json!({
        "url": url,
        "business": string_field(&business, "name"),
        "rating": business.pointer("/aggregateRating/ratingValue").cloned(),
        "review_count": business.pointer("/aggregateRating/reviewCount").cloned(),
        "reviews": reviews,
    })
}

fn json_ld_values(html: &str) -> Vec<Value> {
    let Ok(re) = Regex::new(
        r#"(?is)<script[^>]+type=["']application/ld\+json["'][^>]*>(.*?)</script>"#,
    ) else {
        return Vec::new();
    };
    re.captures_iter(html)
        .filter_map(|captures| captures.get(1))
        .filter_map(|body| serde_json::from_str::<Value>(body.as_str().trim()).ok())
        .collect()
}

fn flatten_graph(value: Value) -> Vec<Value> {
    if let Some(values) = value.as_array() {
        return values.clone();
    }
    if let Some(graph) = value.get("@graph").and_then(Value::as_array) {
        return graph.clone();
    }
    vec![value]
}

fn is_product(value: &Value) -> bool {
    match value.get("@type") {
        Some(Value::String(kind)) => kind == "Product",
        Some(Value::Array(kinds)) => kinds.iter().any(|kind| kind == "Product"),
        _ => false,
    }
}

fn first_or_self(value: &Value) -> Option<&Value> {
    value.as_array().and_then(|values| values.first()).or(Some(value))
}

fn first_or_array(value: &Value) -> Option<Vec<&Value>> {
    value
        .as_array()
        .map(|values| values.iter().collect())
        .or_else(|| Some(vec![value]))
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(|field| {
        field
            .as_str()
            .map(ToString::to_string)
            .or_else(|| field.as_i64().map(|number| number.to_string()))
            .or_else(|| field.as_f64().map(|number| number.to_string()))
    })
}

fn og(html: &str, prop: &str) -> Option<String> {
    meta_property(html, &format!("og:{prop}"))
}

fn meta_property(html: &str, property: &str) -> Option<String> {
    let pattern = format!(
        r#"(?is)<meta[^>]+property=["']{}["'][^>]+content=["']([^"']+)["']"#,
        regex::escape(property)
    );
    Regex::new(&pattern)
        .ok()?
        .captures(html)
        .and_then(|captures| captures.get(1))
        .map(|value| value.as_str().to_string())
}
