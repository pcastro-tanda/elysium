arr.select(&:even?).reject(&:nil?)[arr.select(&:even?).count - 2]
