x.to_enum(:foreach, path).select { |entry| entry.file? }.each_with_object({}) {|(k, v), h| h[foo(k)] = v}
