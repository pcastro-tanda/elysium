arr.select(&:even?)[arr.reject(&:even?).size - 2]
