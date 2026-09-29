foo.sort_by { |k, v| v }.each { |unused_key, v| do_something(v) }
