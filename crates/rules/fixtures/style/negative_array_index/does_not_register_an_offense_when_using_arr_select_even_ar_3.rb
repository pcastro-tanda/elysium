arr.select(&:even?)[arr.reject(&:even?).count - 2]
