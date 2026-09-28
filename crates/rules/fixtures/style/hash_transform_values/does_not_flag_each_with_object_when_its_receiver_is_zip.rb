%i[a b c].zip([1, 2, 3]).each_with_object({}) { |(k, v), h| h[k] = foo(v) }
