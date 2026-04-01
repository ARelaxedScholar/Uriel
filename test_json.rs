fn main() {
    let s = serde_json::Value::Array(vec![serde_json::Value::String("a".to_string())]);
    if let Some(arr) = s.as_array() {
        println!("{:?}", arr);
    }
}
