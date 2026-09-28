{a: 1, b: 2}.each_with_object({}) {|(k, v), h| h[k.to_sym] = foo(v)}
