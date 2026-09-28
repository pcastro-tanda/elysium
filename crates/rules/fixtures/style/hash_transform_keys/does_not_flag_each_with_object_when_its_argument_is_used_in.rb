x.each_with_object({}) { |(k, v), h| h[h[k.to_sym]] = v }
