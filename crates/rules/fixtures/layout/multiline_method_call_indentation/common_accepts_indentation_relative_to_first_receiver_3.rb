node
  .children.map { |n| string_source(n) }.compact
  .any? { |s| preferred.any? { |d| s.include?(d) } }
