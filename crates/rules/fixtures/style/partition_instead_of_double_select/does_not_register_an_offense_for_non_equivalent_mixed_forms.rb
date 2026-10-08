positives = arr.select(&:positive?)
negatives = arr.reject { |x| x.negative? }
