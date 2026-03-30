use serde::{Deserialize, Serialize};

pub const DEFAULT_SEARCH_ENGINE: &str = "bing";

pub const SEARCH_ENGINES: &[&str] = &["bing", "baidu", "google", "duckduckgo"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultItem {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

pub fn get_extraction_js(engine: &str) -> &'static str {
    match engine {
        "google" => GOOGLE_JS,
        "bing" => BING_JS,
        "baidu" => BAIDU_JS,
        "duckduckgo" => DUCKDUCKGO_JS,
        _ => GOOGLE_JS,
    }
}

pub fn parse_search_results(value: &serde_json::Value) -> Vec<SearchResultItem> {
    let results = value
        .get("result")
        .and_then(|r| r.get("value"));

    let arr = match results {
        Some(serde_json::Value::Array(arr)) => arr,
        _ => return Vec::new(),
    };

    let mut items = Vec::with_capacity(arr.len());
    for item in arr {
        let title = item.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let url = item.get("url").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let snippet = item.get("snippet").and_then(|v| v.as_str()).unwrap_or("").to_string();
        if !title.is_empty() && !url.is_empty() {
            items.push(SearchResultItem { title, url, snippet });
        }
    }
    items
}

pub fn format_search_results(query: &str, engine: &str, results: &[SearchResultItem]) -> String {
    if results.is_empty() {
        return format!("No search results found for \"{}\" via {}.", query, engine);
    }

    let mut output = format!(
        "Found {} search results for \"{}\" via {}:\n",
        results.len(),
        query,
        engine
    );

    for (i, item) in results.iter().enumerate() {
        output.push_str(&format!(
            "{}. {}\n   URL: {}\n   {}\n",
            i + 1,
            item.title,
            item.url,
            item.snippet
        ));
    }

    output
}

pub fn extract_links(results: &[SearchResultItem]) -> Vec<String> {
    results.iter().map(|r| r.url.clone()).collect()
}

const GOOGLE_JS: &str = r#"
(async function() {
  await new Promise(function(resolve) {
    var attempts = 0;
    var timer = setInterval(function() {
      var els = document.querySelectorAll('#search .MjjYud');
      if (els.length > 0 || attempts > 40) { clearInterval(timer); resolve(); }
      attempts++;
    }, 200);
  });
  var results = [];
  var containers = document.querySelectorAll('#search .MjjYud');
  for (var i = 0; i < containers.length; i++) {
    var el = containers[i];
    var h3 = el.querySelector('h3');
    var title = h3 ? h3.textContent.trim() : '';
    var url = '';
    if (h3) {
      var a = h3.closest('a') || h3.querySelector('a');
      if (a) {
        var href = a.getAttribute('href') || '';
        if (href.indexOf('/url?q=') === 0) {
          var match = href.match(/[?&]q=([^&]*)/);
          url = match ? decodeURIComponent(match[1]) : href;
        } else {
          url = href;
        }
      }
    }
    var snippet = '';
    var snippetSelectors = ['.VwiC3b', '.yXK7lf', '.s3v9rd'];
    for (var s = 0; s < snippetSelectors.length; s++) {
      var sn = el.querySelector(snippetSelectors[s]);
      if (sn && sn.textContent.trim()) {
        snippet = sn.textContent.trim();
        break;
      }
    }
    if (title && url) {
      results.push({ title: title, url: url, snippet: snippet });
    }
  }
  return results;
})()
"#;

const BING_JS: &str = r#"
(async function() {
  await new Promise(function(resolve) {
    var attempts = 0;
    var timer = setInterval(function() {
      var els = document.querySelectorAll('#b_results .b_algo');
      if (els.length > 0 || attempts > 40) { clearInterval(timer); resolve(); }
      attempts++;
    }, 200);
  });
  var results = [];
  var containers = document.querySelectorAll('#b_results .b_algo');
  for (var i = 0; i < containers.length; i++) {
    var el = containers[i];
    var h2a = el.querySelector('h2 a');
    var title = h2a ? h2a.textContent.trim() : '';
    var url = '';
    if (h2a) {
      var href = h2a.getAttribute('href') || '';
      if (href.indexOf('/ck/a?u=') !== -1) {
        var match = href.match(/[?&]u=([^&]*)/);
        if (match) {
          var raw = decodeURIComponent(match[1]);
          if (raw.indexOf('a1') === 0) {
            raw = raw.substring(2);
          }
          try { url = atob(raw); } catch(e) { url = href; }
        } else {
          url = href;
        }
      } else {
        url = href;
      }
    }
    var snippet = '';
    var snippetSelectors = ['.b_caption p', '.b_snippet', '.lisn_content'];
    for (var s = 0; s < snippetSelectors.length; s++) {
      var sn = el.querySelector(snippetSelectors[s]);
      if (sn && sn.textContent.trim()) {
        snippet = sn.textContent.trim();
        break;
      }
    }
    if (title && url) {
      results.push({ title: title, url: url, snippet: snippet });
    }
  }
  return results;
})()
"#;

const BAIDU_JS: &str = r#"
(async function() {
  await new Promise(function(resolve) {
    var attempts = 0;
    var timer = setInterval(function() {
      var els = document.querySelectorAll('#content_left .result, #content_left .result-op');
      if (els.length > 0 || attempts > 40) { clearInterval(timer); resolve(); }
      attempts++;
    }, 200);
  });
  var results = [];
  var containers = document.querySelectorAll('#content_left .result, #content_left .result-op');
  for (var i = 0; i < containers.length; i++) {
    var el = containers[i];
    var h3a = el.querySelector('h3 a');
    var title = h3a ? h3a.textContent.trim() : '';
    var url = '';
    if (h3a) {
      try {
        url = new URL(h3a.getAttribute('href'), window.location.origin).href;
      } catch(e) {
        url = h3a.getAttribute('href') || '';
      }
    }
    var snippet = '';
    var snippetSelectors = ['.c-abstract', '.content-right_8Zs40', '.c-span-last', '.c-color-text'];
    for (var s = 0; s < snippetSelectors.length; s++) {
      var sn = el.querySelector(snippetSelectors[s]);
      if (sn && sn.textContent.trim()) {
        snippet = sn.textContent.trim();
        break;
      }
    }
    if (title && url) {
      results.push({ title: title, url: url, snippet: snippet });
    }
  }
  return results;
})()
"#;

const DUCKDUCKGO_JS: &str = r#"
(async function() {
  await new Promise(function(resolve) {
    var attempts = 0;
    var timer = setInterval(function() {
      var els = document.querySelectorAll('[data-result]');
      if (els.length === 0) { els = document.querySelectorAll('article[data-testid="result"]'); }
      if (els.length > 0 || attempts > 40) { clearInterval(timer); resolve(); }
      attempts++;
    }, 200);
  });
  var results = [];
  var containers = document.querySelectorAll('[data-result]');
  if (containers.length === 0) {
    containers = document.querySelectorAll('article[data-testid="result"]');
  }
  for (var i = 0; i < containers.length; i++) {
    var el = containers[i];
    var titleLink = el.querySelector('a[data-testid="result-title-a"]') || el.querySelector('h2 a');
    var title = titleLink ? titleLink.textContent.trim() : '';
    var url = titleLink ? (titleLink.getAttribute('href') || '') : '';
    var snippet = '';
    var sn1 = el.querySelector('[data-result="snippet"]');
    if (sn1 && sn1.textContent.trim()) {
      snippet = sn1.textContent.trim();
    } else {
      var sn2 = el.querySelector('.result__snippet');
      if (sn2 && sn2.textContent.trim()) {
        snippet = sn2.textContent.trim();
      }
    }
    if (title && url) {
      results.push({ title: title, url: url, snippet: snippet });
    }
  }
  return results;
})()
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_search_results_with_items() {
        let results = vec![
            SearchResultItem {
                title: "Example Title".to_string(),
                url: "https://example.com".to_string(),
                snippet: "An example snippet".to_string(),
            },
            SearchResultItem {
                title: "Second Result".to_string(),
                url: "https://second.com".to_string(),
                snippet: "Another snippet".to_string(),
            },
        ];
        let output = format_search_results("test query", "google", &results);
        assert!(output.starts_with("Found 2 search results for \"test query\" via google:"));
        assert!(output.contains("1. Example Title"));
        assert!(output.contains("   URL: https://example.com"));
        assert!(output.contains("   An example snippet"));
        assert!(output.contains("2. Second Result"));
        assert!(output.contains("   URL: https://second.com"));
    }

    #[test]
    fn test_format_search_results_empty() {
        let results: Vec<SearchResultItem> = vec![];
        let output = format_search_results("test query", "google", &results);
        assert_eq!(output, "No search results found for \"test query\" via google.");
    }

    #[test]
    fn test_parse_search_results_valid() {
        let value = serde_json::json!({
            "result": {
                "value": [
                    { "title": "Title1", "url": "https://example.com", "snippet": "Snippet1" },
                    { "title": "Title2", "url": "https://example2.com", "snippet": "" }
                ]
            }
        });
        let results = parse_search_results(&value);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].title, "Title1");
        assert_eq!(results[0].url, "https://example.com");
        assert_eq!(results[0].snippet, "Snippet1");
        assert_eq!(results[1].title, "Title2");
        assert_eq!(results[1].snippet, "");
    }

    #[test]
    fn test_parse_search_results_empty() {
        let value = serde_json::json!({ "result": { "value": [] } });
        let results = parse_search_results(&value);
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_search_results_missing_path() {
        let value = serde_json::json!({ "foo": "bar" });
        let results = parse_search_results(&value);
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_search_results_filters_invalid() {
        let value = serde_json::json!({
            "result": {
                "value": [
                    { "title": "", "url": "https://example.com", "snippet": "s" },
                    { "title": "Title", "url": "", "snippet": "s" },
                    { "title": "Valid", "url": "https://valid.com", "snippet": "s" }
                ]
            }
        });
        let results = parse_search_results(&value);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Valid");
    }

    #[test]
    fn test_extract_links() {
        let results = vec![
            SearchResultItem { title: "T1".to_string(), url: "https://a.com".to_string(), snippet: "s1".to_string() },
            SearchResultItem { title: "T2".to_string(), url: "https://b.com".to_string(), snippet: "s2".to_string() },
        ];
        let links = extract_links(&results);
        assert_eq!(links, vec!["https://a.com", "https://b.com"]);
    }

    #[test]
    fn test_get_extraction_js_returns_different_scripts() {
        let google_js = get_extraction_js("google");
        let bing_js = get_extraction_js("bing");
        let baidu_js = get_extraction_js("baidu");
        let ddg_js = get_extraction_js("duckduckgo");
        assert_ne!(google_js, bing_js);
        assert_ne!(google_js, baidu_js);
        assert_ne!(google_js, ddg_js);
    }

    #[test]
    fn test_get_extraction_js_defaults_to_google() {
        let default_js = get_extraction_js("unknown");
        let google_js = get_extraction_js("google");
        assert_eq!(default_js, google_js);
    }
}
