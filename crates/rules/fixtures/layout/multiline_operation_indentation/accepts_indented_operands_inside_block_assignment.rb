a = b.map do |c|
  c +
    d
end

requires_interpolation = node.children.any? do |s|
  s.type == :dstr ||
    s.source_range.source =~ REGEXP
end
