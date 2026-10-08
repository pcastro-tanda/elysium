arr.select(&:even?)[arr.reject(&:even?).length - 2]
