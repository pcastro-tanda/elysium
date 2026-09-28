[1, 2, 3].each.with_index.each_with_object({}) { |(k, v), h| h[k] = foo(v) }
