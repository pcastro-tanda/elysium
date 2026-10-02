arr.map(&:to_s)[arr.select(&:even?).count - 2]
